//! The callback client (PLAN phase 5, 3.3, F14, S23): what the docserver tells the provider, and what it does when the provider
//! does not answer `{"error":0}`. A save is acknowledged only by that answer; until then the version is dirty (`SessionRecord::acked`).
//!
//! Statuses: 1 on the first dirty event, 6 on every explicit save (`forcesavetype` 1), 2 on a close after a save, 4 on a close with
//! nothing saved. 2 is retried from a queue on disk (`<data_dir>/queue/<sid>.json`, written BEFORE the first try, so a crash during
//! it loses nothing); 6, 4 and 1 are not (the user still has the dirty document; the next save or the next dirty event tries again).
//!
//! The body is the DocsAPI callback body, signed the way the HR backend verifies it: `token` in the body is a JWT of the body, and
//! `Authorization: Bearer` is a JWT of `{"payload": body}`, both with the provider's secret. Logs carry host and path only.
use crate::Docserver;
use crate::audit::sid_ref;
use crate::outbound::{OutErr, host_path};
use crate::sessions::{SessionRecord, VersionRec, url_allowed, valid_sid};
use convert_api::jwt::{now, sign};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// The retry schedule of a failed status 2: delays after the 1st, 2nd, 3rd and 4th failure, then hourly; it stops 24 h after the close.
const BACKOFF_S: [u64; 4] = [60, 300, 900, 3600];
const GIVE_UP_AFTER_S: u64 = 24 * 3600;
const TOKEN_TTL_S: u64 = 600;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Cb {
    /// the provider answered `{"error":0}`
    Ok,
    /// the provider answered otherwise, or could not be reached
    Failed,
    /// no answer in time: the provider may have acted
    Unknown,
}

impl Cb {
    pub fn as_str(self) -> &'static str {
        match self {
            Cb::Ok => "ok",
            Cb::Failed => "failed",
            Cb::Unknown => "unknown",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Queued {
    pub sid: String,
    pub attempts: u32,
    pub first: u64,
    pub next: u64,
    pub gave_up: bool,
}

impl Queued {
    /// The next try after a failure `n` (1-based) at `now`, or None when the 24 h are over.
    fn after_failure(&self, now: u64) -> Option<u64> {
        let d = BACKOFF_S[(self.attempts as usize)
            .saturating_sub(1)
            .min(BACKOFF_S.len() - 1)];
        (now + d <= self.first + GIVE_UP_AFTER_S).then_some(now + d)
    }
}

impl Docserver {
    fn queue_path(&self, sid: &str) -> Option<PathBuf> {
        valid_sid(sid).then(|| self.data_dir.join("queue").join(format!("{sid}.json")))
    }

    fn queue_write(&self, q: &Queued) -> std::io::Result<()> {
        let p = self
            .queue_path(&q.sid)
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(p.parent().expect("has a parent"))?;
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(q).expect("serialises"))?;
        std::fs::rename(tmp, p)
    }

    pub fn queued(&self, sid: &str) -> Option<Queued> {
        serde_json::from_slice(&std::fs::read(self.queue_path(sid)?).ok()?).ok()
    }

    /// One callback to the provider of `rec`. `ver` is the version the file URL points at (status 2 and 6).
    pub fn send_callback(&self, rec: &SessionRecord, status: u8, ver: Option<&VersionRec>) -> Cb {
        let Some(p) = self.provider(&rec.provider) else {
            return Cb::Failed;
        };
        let Some(cb_url) = &rec.callback_url else {
            return Cb::Failed;
        };
        // the list may have changed since the session was opened (a restart with another config)
        let Ok(internal) = url_allowed(cb_url, &p.callback_allow, "callbackUrl") else {
            eprintln!(
                "docserver: callback to {} refused: no longer on the list",
                host_path(cb_url)
            );
            self.audit(
                "ssrf.block",
                json!({"session": sid_ref(&rec.sid), "phase": "callback", "target": host_path(cb_url), "reason": "not on the callback list"}),
            );
            return Cb::Failed;
        };
        let t = now();
        let mut body = json!({"key": rec.key, "status": status, "users": [rec.user_id]});
        if let Some(v) = ver {
            let Some(base) = &self.internal_url else {
                eprintln!(
                    "docserver: callback needs internal_listen: the provider cannot fetch the file"
                );
                return Cb::Failed;
            };
            let ft = self.mint_file_token(&rec.sid, v.n, &v.sha256, t);
            body["url"] = json!(format!("{base}/ds/files/{}/{}?t={ft}", rec.sid, v.n));
            if status == 6 {
                body["forcesavetype"] = json!(1);
            }
        }
        // `exp` is in the signed payloads only; the provider verifies the token and reads the payload
        let mut signed = body.clone();
        signed["exp"] = json!(t + TOKEN_TTL_S);
        body["token"] = json!(sign(&p.secret, &signed));
        let bearer = sign(
            &p.secret,
            &json!({"payload": signed, "exp": t + TOKEN_TTL_S}),
        );
        let r = self.post_json(cb_url, internal, &bearer, body.to_string().as_bytes());
        if let Err(OutErr::Blocked(w)) = &r {
            self.audit(
                "ssrf.block",
                json!({"session": sid_ref(&rec.sid), "phase": "callback", "target": host_path(cb_url), "reason": w}),
            );
        }
        let (res, why) = match r {
            Ok((200, b)) if serde_json::from_slice::<Value>(&b).is_ok_and(|v| v["error"] == 0) => {
                (Cb::Ok, "ok".to_string())
            }
            Ok((s, _)) => (Cb::Failed, format!("answered {s} or an error")),
            Err(OutErr::Timeout) => (Cb::Unknown, "no answer in time".into()),
            Err(e) => (Cb::Failed, e.to_string()),
        };
        eprintln!(
            "docserver: callback status {status} to {}: {why}",
            host_path(cb_url)
        );
        self.audit(
            "callback",
            json!({"session": sid_ref(&rec.sid), "status": status, "result": res.as_str(), "target": host_path(cb_url), "reason": why}),
        );
        res
    }

    /// Status 6 for the version just saved. Acknowledged on `{"error":0}`.
    pub fn callback_saved(&self, rec: &SessionRecord, v: &VersionRec) -> Cb {
        let r = self.send_callback(rec, 6, Some(v));
        if r == Cb::Ok {
            self.mark_acked(&rec.sid, v.n);
        }
        r
    }

    /// Status 1, once per session: the first dirty event. Not sent again after it succeeded.
    pub fn callback_dirty(&self, sid: &str) -> Option<Cb> {
        let rec = self.load_session(sid).filter(|r| !r.dirty_sent)?;
        let r = self.send_callback(&rec, 1, None);
        if r == Cb::Ok {
            self.update_session(sid, |r| r.dirty_sent = true);
        }
        Some(r)
    }

    /// The callback of a close: status 4 (one try), or status 2 for the last version, queued first and removed when acknowledged.
    /// Does nothing for a session already notified (a second close).
    pub fn callback_closed(&self, sid: &str) -> Option<Cb> {
        let rec = self.load_session(sid)?;
        let c = rec.closed.as_ref().filter(|c| !c.notified)?;
        // a view session has nothing to report (PLAN 5): no callback, and no failure for a config that has no callbackUrl
        let r = if !rec.perms.edit {
            Cb::Ok
        } else if c.status == 4 {
            self.send_callback(&rec, 4, None)
        } else if let Some(v) = rec.versions.last() {
            let q = Queued {
                sid: sid.into(),
                attempts: 0,
                first: now(),
                next: now() + BACKOFF_S[0],
                gave_up: false,
            };
            if let Err(e) = self.queue_write(&q) {
                eprintln!("docserver: cannot write the retry queue: {e}");
            }
            self.try_queued(&rec, v, q, now())
        } else {
            Cb::Ok
        };
        self.update_session(sid, |r| {
            if let Some(c) = &mut r.closed {
                c.notified = true;
            }
        });
        Some(r)
    }

    /// One attempt at a queued status 2; updates or removes its queue entry.
    fn try_queued(&self, rec: &SessionRecord, v: &VersionRec, mut q: Queued, at: u64) -> Cb {
        let r = self.send_callback(rec, 2, Some(v));
        let path = self.queue_path(&q.sid).expect("a valid sid");
        if r == Cb::Ok {
            self.mark_acked(&q.sid, v.n);
            let _ = std::fs::remove_file(path);
            return r;
        }
        q.attempts += 1;
        match q.after_failure(at) {
            Some(t) => q.next = t,
            None => {
                q.gave_up = true;
                eprintln!(
                    "docserver: status 2 callback given up after 24 h; the saved version stays on disk"
                );
            }
        }
        let _ = self.queue_write(&q);
        r
    }

    /// Retries every queued status 2 that is due. Returns how many it tried.
    pub fn retry_due(&self, at: u64) -> usize {
        let Ok(dir) = std::fs::read_dir(self.data_dir.join("queue")) else {
            return 0;
        };
        let mut n = 0;
        for e in dir.flatten() {
            let Some(q) = e
                .file_name()
                .to_str()
                .and_then(|f| f.strip_suffix(".json"))
                .and_then(|sid| self.queued(sid))
                .filter(|q| !q.gave_up && q.next <= at)
            else {
                continue;
            };
            let rec = self.load_session(&q.sid);
            match rec.as_ref().and_then(|r| r.versions.last().map(|v| (r, v))) {
                Some((r, v)) => {
                    n += 1;
                    self.try_queued(r, v, q, at);
                }
                // the session is gone: nothing left to tell
                None => {
                    let _ = std::fs::remove_file(self.queue_path(&q.sid).expect("a valid sid"));
                }
            }
        }
        n
    }

    /// A thread that calls `retry_due` every 15 s for the life of the process.
    pub fn spawn_retry_loop(self: &Arc<Self>) {
        let ds = self.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(15));
                ds.retry_due(now());
            }
        });
    }
}
