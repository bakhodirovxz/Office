//! The retention sweep (PLAN phase 5, section 4 "Retention", Q10, S18): what the server stores is deleted when it is no longer needed.
//!
//! A session folder `sessions/<sid>/` goes
//! - `keep_closed_hours` (168, 7 days) after a good close: closed, and the provider answered `{"error":0}` to the close's callback (`Closed::acked`);
//! - `keep_abandoned_days` (7) after its last activity otherwise: a view session never closed, or a close never acknowledged (a status 2 the retry
//!   queue gave up on falls here, so the operator has the rest of the 7 days to recover it from the folder);
//! - never while an edit session is open: the idle close (`callback::close_idle`) closes it first and sends its callback;
//! - never while a status 2 is still in the retry queue (`queue/<sid>.json` without `gave_up`; an unreadable entry counts as queued).
//!
//! With it goes the session's history document (`history::doc_id`, one per provider and `document.key`) and the bytes no other
//! document uses, unless another session of the same key is still on disk, or the provider says `history = "keep"`. A provider that
//! sends a new key on every open has nothing to find in that history once the session is gone, and keeping it would keep
//! every document ever opened. Each decision and deletion runs under the session lock, so it cannot race a request on that session.
//!
//! One `retention.sweep` audit line per sweep that deleted something: counts only, never an id (S17).
use crate::Docserver;
use crate::history::doc_id;
use crate::sessions::{SessionRecord, lock, valid_sid};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

/// How often the sweep runs after the first one at start.
const SWEEP_EVERY_S: u64 = 3600;

impl Docserver {
    /// Whether the session is due for deletion at `now`.
    fn expired(&self, rec: &SessionRecord, now: u64) -> bool {
        // a status 2 not sent yet (a crash before its callback): `recover_unnotified` sends it at start, so never sweep it first
        if crate::callback::owes_status_2(rec) {
            return false;
        }
        // an open edit session ends by the idle close (`close_idle`), which sends its callback: never sweep it before that ran
        if rec.perms.edit && rec.closed.is_none() {
            return false;
        }
        if self.queue_path(&rec.sid).is_some_and(|p| p.exists())
            && !self.queued(&rec.sid).is_some_and(|q| q.gave_up)
        {
            return false;
        }
        match &rec.closed {
            Some(c) if c.notified && c.acked => now >= c.at.saturating_add(self.keep_closed_s),
            _ => now >= rec.last_activity.saturating_add(self.keep_abandoned_s),
        }
    }

    /// One sweep at `now` (the caller's clock; tests inject it). Returns the number of session folders deleted.
    pub fn sweep(&self, now: u64) -> usize {
        let Ok(dir) = std::fs::read_dir(self.data_dir.join("sessions")) else {
            return 0;
        };
        let (mut seen, mut kept, mut gone) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        let mut n = 0;
        for e in dir.flatten() {
            let sid = e.file_name().to_string_lossy().into_owned();
            if !valid_sid(&sid) {
                continue;
            }
            seen.insert(sid.clone());
            let _g = lock();
            let Some(rec) = self.load_session(&sid) else {
                // a folder without a readable record (a crash right after create, a damaged file): its own age decides
                let old = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_some_and(|m| now >= m.as_secs().saturating_add(self.keep_abandoned_s));
                if old && std::fs::remove_dir_all(e.path()).is_ok() {
                    n += 1;
                }
                continue;
            };
            if !self.expired(&rec, now) {
                kept.insert(doc_id(&rec));
                continue;
            }
            if let Err(e) = std::fs::remove_dir_all(e.path()) {
                eprintln!("docserver: retention: a session folder could not be deleted: {e}");
                kept.insert(doc_id(&rec));
                continue;
            }
            // a given-up status 2 has nothing left to retry
            if let Some(q) = self.queue_path(&sid) {
                let _ = std::fs::remove_file(q);
            }
            n += 1;
            if !self.provider(&rec.provider).is_some_and(|p| p.keep_history) {
                gone.insert(doc_id(&rec));
            }
        }
        let mut forgotten = 0;
        if !gone.is_empty() {
            // under the lock: a session made during the sweep (its first version snapshots into its history under this lock) keeps its key's history
            let _g = lock();
            for e in std::fs::read_dir(self.data_dir.join("sessions"))
                .into_iter()
                .flatten()
                .flatten()
            {
                let sid = e.file_name().to_string_lossy().into_owned();
                if !seen.contains(&sid)
                    && let Some(rec) = self.load_session(&sid)
                {
                    kept.insert(doc_id(&rec));
                }
            }
            let backend = self.version_backend();
            for id in gone.difference(&kept) {
                match backend.forget(id) {
                    Ok(true) => forgotten += 1,
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!("docserver: retention: a history could not be deleted: {e:?}")
                    }
                }
            }
        }
        if n > 0 || forgotten > 0 {
            eprintln!(
                "docserver: retention: {n} session(s) and {forgotten} history document(s) deleted"
            );
            self.audit(
                "retention.sweep",
                json!({"sessions": n, "histories": forgotten}),
            );
        }
        n
    }

    /// A thread that sweeps once at start, then every hour, for the life of the process.
    pub fn spawn_sweep_loop(self: &Arc<Self>) {
        let ds = self.clone();
        std::thread::spawn(move || {
            loop {
                ds.sweep(convert_api::jwt::now());
                std::thread::sleep(std::time::Duration::from_secs(SWEEP_EVERY_S));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callback::Queued;
    use convert_api::jwt::sign;
    use std::path::{Path, PathBuf};

    const T0: u64 = 1_900_000_000;
    const H: u64 = 3600;
    const D: u64 = 86_400;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // keep_closed_hours = 24 (not the 7-day default) so the tests tell a good close from an abandoned session
    fn server(name: &str, history: &str) -> (Docserver, Dir) {
        let d = std::env::temp_dir().join(format!("docserver-ret-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("server.key"), "s".repeat(40)).unwrap();
        std::fs::write(d.join("prov.key"), "p".repeat(40)).unwrap();
        let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
        let t: toml::Table = format!(
            r#"server_secret_file = "{}"
data_dir = "{}"
keep_closed_hours = 24
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = ["http://api:8000"]
callback_allow = ["http://api:8000"]
frame_origins = ["https://app.test"]
history = "{history}"
"#,
            p("server.key"),
            p("data"),
            p("prov.key")
        )
        .parse()
        .unwrap();
        (Docserver::from_table(t, None).unwrap(), Dir(d))
    }

    /// A session of `key` opened at `at` with `bytes` as version 1 (which also goes to the key's history).
    fn open(ds: &Docserver, key: &str, at: u64, bytes: &[u8]) -> String {
        let cfg = json!({"document": {"fileType": "docx", "key": key, "url": "http://api:8000/f"},
            "editorConfig": {"callbackUrl": "http://api:8000/cb", "user": {"id": "u1"}}, "exp": at + 3600});
        let rec = ds
            .create_session(&sign(&[b'p'; 40], &cfg), "https://app.test", at)
            .unwrap();
        ds.put_version(&rec.sid, bytes, "one", false, at).unwrap();
        rec.sid
    }

    /// Closes at `at`; `acked`: the provider answered the close's callback.
    fn close(ds: &Docserver, sid: &str, at: u64, acked: bool) {
        ds.close_session(sid, at).unwrap();
        ds.update_session(sid, |r| {
            let c = r.closed.as_mut().unwrap();
            c.notified = true;
            c.acked = acked;
        });
    }

    fn queue(ds: &Docserver, sid: &str, gave_up: bool) {
        let q = Queued {
            sid: sid.into(),
            attempts: 3,
            first: T0 - 30 * D,
            next: T0,
            gave_up,
        };
        let p = ds.queue_path(sid).unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, serde_json::to_vec(&q).unwrap()).unwrap();
    }

    fn there(ds: &Docserver, sid: &str) -> bool {
        ds.session_dir(sid).unwrap().is_dir()
    }

    fn object(ds: &Docserver, bytes: &[u8]) -> bool {
        ds.data_dir
            .join("history/objects")
            .join(crate::sessions::sha256_hex(bytes))
            .is_file()
    }

    fn history_of(ds: &Docserver, sid: &str) -> Vec<suite_core::history::Version> {
        let rec = ds.load_session(sid).unwrap();
        ds.version_backend().list(&doc_id(&rec)).unwrap()
    }

    fn audit_lines(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("data/audit.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains("retention.sweep"))
            .map(str::to_string)
            .collect()
    }

    /// S18: the clock is the argument of `sweep`, so nothing sleeps.
    #[test]
    fn sweep_deletes_closed_and_abandoned_sessions_with_their_history() {
        let (ds, dir) = server("sweep", "session");
        let now = T0 + 25 * H;
        // closed and acknowledged 25 h ago: gone, with its history and the bytes only it used
        let closed_old = open(&ds, "k-old", T0 - H, b"only-old");
        ds.put_version(&closed_old, b"shared", "u1", true, T0)
            .unwrap();
        close(&ds, &closed_old, T0, true);
        let old_rec = ds.load_session(&closed_old).unwrap();
        // closed and acknowledged 23 h ago: kept; it holds the same bytes as the one above
        let closed_young = open(&ds, "k-young", T0 + 2 * H, b"shared");
        close(&ds, &closed_young, T0 + 2 * H, true);
        // closed by the idle close, its status 4 never acknowledged, no activity for 7 days: gone
        let abandoned = open(&ds, "k-aband", now - 7 * D - 1, b"aband");
        close(&ds, &abandoned, now - 7 * D - 1, false);
        // never closed, active 6 days ago: kept
        let idle = open(&ds, "k-idle", now - 6 * D, b"idle");
        // an edit session never closed, 30 days idle: kept until the idle close has run (it sends the callback)
        let open_edit = open(&ds, "k-open", now - 30 * D, b"open");
        // closed long ago, its close never acknowledged, but a status 2 still queued: kept however old
        let queued = open(&ds, "k-queued", T0 - 60 * D, b"queued");
        close(&ds, &queued, T0 - 60 * D, false);
        queue(&ds, &queued, false);
        // a status 2 the queue gave up on: the abandoned rule (7 days), and its queue entry goes too
        let gave_up = open(&ds, "k-gaveup", T0 - 8 * D, b"gaveup");
        close(&ds, &gave_up, T0 - 8 * D, false);
        queue(&ds, &gave_up, true);
        // closed without an acknowledgement 2 days ago: not a good close, so it waits for the 7 days
        let unacked = open(&ds, "k-unacked", now - 2 * D, b"unacked");
        close(&ds, &unacked, now - 2 * D, false);
        assert_eq!(history_of(&ds, &closed_old).len(), 2);

        assert_eq!(ds.sweep(now), 3);
        for (sid, kept) in [
            (&closed_old, false),
            (&closed_young, true),
            (&abandoned, false),
            (&idle, true),
            (&queued, true),
            (&gave_up, false),
            (&unacked, true),
            (&open_edit, true),
        ] {
            assert_eq!(there(&ds, sid), kept, "session kept: {kept}");
        }
        assert!(ds.queued(&queued).is_some());
        assert!(ds.queue_path(&gave_up).is_some_and(|p| !p.exists()));
        // the history of a swept session and its own bytes are gone; bytes another document uses stay
        assert!(
            ds.version_backend()
                .list(&doc_id(&old_rec))
                .unwrap()
                .is_empty()
        );
        assert!(!object(&ds, b"only-old") && !object(&ds, b"aband") && !object(&ds, b"gaveup"));
        assert!(
            object(&ds, b"shared")
                && object(&ds, b"idle")
                && object(&ds, b"queued")
                && object(&ds, b"open")
        );
        assert_eq!(history_of(&ds, &closed_young).len(), 1);
        // one audit line with counts only, no session reference
        let lines = audit_lines(&dir.0);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("\"sessions\":3") && lines[0].contains("\"histories\":3"));
        for sid in [&closed_old, &abandoned, &gave_up] {
            assert!(!lines[0].contains(sid.as_str()));
            assert!(!lines[0].contains(&crate::audit::sid_ref(sid)));
        }
        // nothing more to do half an hour later: no deletion, no line; the young one goes when it turns 24 h
        assert_eq!(ds.sweep(now + H / 2), 0);
        assert_eq!(audit_lines(&dir.0).len(), 1);
        assert_eq!(ds.sweep(T0 + 26 * H), 1);
        assert!(!there(&ds, &closed_young) && !object(&ds, b"shared"));
    }

    #[test]
    fn a_status_2_never_sent_is_not_swept() {
        let (ds, _dir) = server("owed", "session");
        let sid = open(&ds, "k-owed", T0 - 30 * D, b"owed");
        ds.put_version(&sid, b"owed-2", "u1", true, T0 - 30 * D)
            .unwrap();
        ds.close_session(&sid, T0 - 30 * D).unwrap();
        assert!(crate::callback::owes_status_2(
            &ds.load_session(&sid).unwrap()
        ));
        assert_eq!(ds.sweep(T0), 0);
        assert!(there(&ds, &sid));
    }

    #[test]
    fn a_key_still_open_in_another_session_keeps_its_history() {
        let (ds, _dir) = server("shared", "session");
        let a = open(&ds, "same-key", T0, b"v1");
        close(&ds, &a, T0, true);
        let b = open(&ds, "same-key", T0 + 24 * H, b"v2");
        assert_eq!(ds.sweep(T0 + 25 * H), 1);
        assert!(!there(&ds, &a) && there(&ds, &b));
        assert_eq!(
            history_of(&ds, &b).len(),
            2,
            "the open session still sees the key's history"
        );
    }

    #[test]
    fn history_keep_leaves_the_history_alone() {
        let (ds, _dir) = server("keep", "keep");
        let a = open(&ds, "stable", T0, b"kept-bytes");
        let rec = ds.load_session(&a).unwrap();
        close(&ds, &a, T0, true);
        assert_eq!(ds.sweep(T0 + 25 * H), 1);
        assert!(!there(&ds, &a));
        assert_eq!(ds.version_backend().list(&doc_id(&rec)).unwrap().len(), 1);
        assert!(object(&ds, b"kept-bytes"));
    }

    #[test]
    fn a_folder_without_a_readable_record_goes_by_its_own_age() {
        let (ds, _dir) = server("norec", "session");
        let sid = "0123456789abcdef0123456789abcdef";
        let f = ds.session_dir(sid).unwrap();
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("v1.bin"), b"x").unwrap();
        let wall = convert_api::jwt::now();
        assert_eq!(ds.sweep(wall), 0, "a fresh folder stays");
        assert_eq!(ds.sweep(wall + 8 * D), 1);
        assert!(!f.exists());
    }
}
