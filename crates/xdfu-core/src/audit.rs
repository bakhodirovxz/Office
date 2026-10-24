//! The XDFU audit log (PLAN 10, WP15): one JSON object per line, append-only, hash-chained.
//!
//! Every line carries `seq` (1, 2, 3, ...), `ts` (UTC), `prev` (the `hash` of the line before it, 64 zeros for the first), the
//! event's fields, and `hash`: SHA-256 over the line's JSON text without the `hash` field (keys in sorted order, so the text is
//! the same everywhere). Changing, deleting or inserting a line breaks the chain from there on and `verify` names the line.
//!
//! An unkeyed chain only stops a naive edit: whoever can write the file can rewrite lines k..n and recompute every hash (gate review M5).
//! With an operator key (`open_keyed`, a file kept outside the log) the `hash` is HMAC-SHA256 under that key and every line carries
//! `alg: "hmac-sha256"`, so a rewrite without the key fails `verify_keyed`. A keyed log is never accepted without its key, and a
//! line without `alg` is refused under a key (no downgrade).
//!
//! Limits, said plainly. A chain proves that the lines it holds were not changed or reordered. It cannot prove that lines were not
//! cut off at the END of the file, so the operator should record `verify`'s `last_hash` and count somewhere else from time to time.
//! "Append-only" is a property of how this code writes the file; making the file itself append-only is the operator's job
//! (an ACL that grants append but not write on Windows, `chattr +a` on Linux).
//!
//! What is logged: who (`user`), when, from where (`client`, `host`), which job and file (`job`, `file`) and which copy number.
//! Never any document content. With `redact` the file name is left out.
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// (year, month, day) of a day count since 1970-01-01 (Howard Hinnant's civil-from-days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// `2026-10-21 10:15:03 UTC` for the Unix time `secs` (the watermark's date and time).
pub fn utc_text(secs: u64) -> String {
    let (y, m, d) = civil((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        t / 3600,
        t % 3600 / 60,
        t % 60
    )
}

/// `2026-10-21T10:15:03Z`, the audit timestamp.
pub fn utc_iso(secs: u64) -> String {
    utc_text(secs)
        .replace(' ', "T")
        .replace("TUTC", "Z")
        .trim_end_matches("UTC")
        .trim_end_matches('T')
        .to_string()
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

const ALG: &str = "hmac-sha256";

/// HMAC-SHA256 (RFC 2104), by hand: the one primitive, so no crate for it.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |b: u8| k.map(|x| x ^ b);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(msg)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 of the line's JSON text, or HMAC-SHA256 under `key`.
fn hash_of(body: &Map<String, Value>, key: Option<&[u8]>) -> String {
    let text = serde_json::to_string(body).expect("a JSON object serialises");
    match key {
        Some(k) => hex(&hmac_sha256(k, text.as_bytes())),
        None => hex(&Sha256::digest(text.as_bytes())),
    }
}

/// The operator's key: a file of at least 32 bytes (a trailing line break is not part of it).
pub fn load_key(path: &Path) -> Result<Vec<u8>, String> {
    let mut b = fs::read(path)
        .map_err(|e| format!("cannot read the audit key file {}: {e}", path.display()))?;
    while b.last().is_some_and(|c| matches!(c, b'\r' | b'\n')) {
        b.pop();
    }
    if b.len() < 32 {
        return Err(format!(
            "the audit key file {} holds {} bytes; it needs at least 32 random bytes",
            path.display(),
            b.len()
        ));
    }
    Ok(b)
}

/// Said once by the CLI and the API when no key is configured.
pub const UNKEYED_WARNING: &str = "the XDFU audit log has no key (xdfu_audit_key_file / --xdfu-audit-key): its hash chain only stops a naive edit, because anyone who can write the file can rewrite it and recompute the hashes";

#[derive(Debug)]
pub struct Verified {
    pub lines: u64,
    pub last_hash: String,
}

/// Check the whole chain of `path`, unkeyed. A keyed log is refused here (it needs `verify_keyed`).
pub fn verify(path: &Path) -> Result<Verified, String> {
    verify_keyed(path, None)
}

/// Check the whole chain of `path` under `key`. `Err` names the first line that does not fit ("line 7: the hash does not match its content").
pub fn verify_keyed(path: &Path, key: Option<&[u8]>) -> Result<Verified, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| "the log is not valid UTF-8".to_string())?;
    let (mut prev, mut seq) = (GENESIS.to_string(), 0u64);
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(line) else {
            return Err(format!("line {n}: not a JSON object"));
        };
        let Some(Value::String(hash)) = obj.remove("hash") else {
            return Err(format!("line {n}: no hash"));
        };
        if obj.get("prev").and_then(Value::as_str) != Some(prev.as_str()) {
            return Err(format!(
                "line {n}: it does not follow the line before it (a line was changed, deleted or inserted)"
            ));
        }
        if obj.get("seq").and_then(Value::as_u64) != Some(seq + 1) {
            return Err(format!("line {n}: the sequence number is not {}", seq + 1));
        }
        let has_alg = obj.get("alg").and_then(Value::as_str) == Some(ALG);
        if key.is_none() && has_alg {
            return Err(format!(
                "line {n}: the log is keyed (HMAC-SHA256); give the key file"
            ));
        }
        if key.is_some() && !has_alg {
            return Err(format!(
                "line {n}: the line is not keyed (HMAC), so the log was rewritten or started without the key"
            ));
        }
        if hash_of(&obj, key) != hash {
            return Err(format!(
                "line {n}: the hash does not match its content (the line was changed{})",
                if key.is_some() {
                    ", or the key is not the one that wrote the log"
                } else {
                    ""
                }
            ));
        }
        (prev, seq) = (hash, seq + 1);
    }
    Ok(Verified {
        lines: seq,
        last_hash: prev,
    })
}

/// A lock file next to the log, so two processes (the CLI run twice, the CLI beside the API) cannot both extend the same chain.
struct FileLock(PathBuf);

impl FileLock {
    fn take(log: &Path) -> Result<FileLock, String> {
        // `<log>.lock`: a log that is itself named `*.lock` must not be its own lock (gate review m12)
        let p = PathBuf::from(format!("{}.lock", log.display()));
        for _ in 0..200 {
            match OpenOptions::new().write(true).create_new(true).open(&p) {
                Ok(_) => return Ok(FileLock(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // a lock whose owner died: older than 30 s
                    if fs::metadata(&p)
                        .and_then(|m| m.modified())
                        .is_ok_and(|t| t.elapsed().is_ok_and(|e| e > Duration::from_secs(30)))
                    {
                        // renamed, not deleted: of two processes that both saw the stale lock only one rename succeeds
                        let gone =
                            PathBuf::from(format!("{}.stale-{}", p.display(), std::process::id()));
                        if fs::rename(&p, &gone).is_ok() {
                            let _ = fs::remove_file(&gone);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(format!("cannot lock the audit log: {e}")),
            }
        }
        Err("the audit log is locked by another process".into())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub struct AuditLog {
    path: PathBuf,
    redact: bool,
    key: Option<Vec<u8>>,
    mu: Mutex<()>,
}

impl AuditLog {
    /// Open (and create) the log. An existing log must verify: a damaged chain stops XDFU handling instead of being extended.
    pub fn open(path: &Path, redact: bool) -> Result<AuditLog, String> {
        AuditLog::open_keyed(path, redact, None)
    }

    /// `key_file`: the operator's key (`load_key`); `None` keeps the plain SHA-256 chain, which is weaker (`UNKEYED_WARNING`).
    pub fn open_keyed(
        path: &Path,
        redact: bool,
        key_file: Option<&Path>,
    ) -> Result<AuditLog, String> {
        let key = key_file.map(load_key).transpose()?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        if path.exists() {
            verify_keyed(path, key.as_deref())
                .map_err(|e| format!("the audit log {} is damaged: {e}", path.display()))?;
        }
        Ok(AuditLog {
            path: path.to_path_buf(),
            redact,
            key,
            mu: Mutex::new(()),
        })
    }

    /// The chain is an HMAC under an operator key.
    pub fn is_keyed(&self) -> bool {
        self.key.is_some()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The last line's `(seq, hash)`, read from the end of the file.
    fn tail(&self) -> Result<(u64, String), String> {
        let Ok(mut f) = fs::File::open(&self.path) else {
            return Ok((0, GENESIS.to_string()));
        };
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        let from = len.saturating_sub(64 * 1024);
        f.seek(SeekFrom::Start(from)).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&buf);
        let Some(line) = text.lines().rev().find(|l| !l.trim().is_empty()) else {
            return Ok((0, GENESIS.to_string()));
        };
        let v: Value = serde_json::from_str(line)
            .map_err(|_| "the last line of the audit log is not JSON".to_string())?;
        match (v["seq"].as_u64(), v["hash"].as_str()) {
            (Some(s), Some(h)) => Ok((s, h.to_string())),
            _ => Err("the last line of the audit log has no seq or hash".into()),
        }
    }

    /// Append one event. `fields` is a JSON object; `file` is dropped when the log was opened with `redact`. Returns the line's `seq`,
    /// which the converter also uses as the copy number. The line is on disk when this returns.
    pub fn append(&self, event: &str, fields: Value) -> Result<u64, String> {
        match self.append_after(event, fields, |_| Ok::<(), String>(())) {
            Ok((seq, ())) => Ok(seq),
            Err(Appended::Log(m)) => Err(m),
            Err(Appended::Produce(m)) => Err(m),
        }
    }

    /// Reserve the next `seq`, run `produce(seq)` (the work the line vouches for, for example the watermark with that copy number),
    /// and write the `event` line only when it succeeded. When it failed, a `<event>_failed` line with the reason is written instead, so
    /// the log never records something that was not produced (gate review m12). The log stays locked meanwhile: `produce` should be quick.
    pub fn append_after<T, E: std::fmt::Display>(
        &self,
        event: &str,
        fields: Value,
        produce: impl FnOnce(u64) -> Result<T, E>,
    ) -> Result<(u64, T), Appended<E>> {
        let _g = self.mu.lock().unwrap_or_else(|e| e.into_inner());
        let _lock = FileLock::take(&self.path).map_err(Appended::Log)?;
        let (seq, prev) = self.tail().map_err(Appended::Log)?;
        let n = seq + 1;
        let mut fields = fields;
        let (event, made) = match produce(n) {
            Ok(t) => (event.to_string(), Ok(t)),
            Err(e) => {
                if let Value::Object(m) = &mut fields {
                    m.insert("reason".into(), e.to_string().into());
                }
                (format!("{event}_failed"), Err(e))
            }
        };
        // a second process may have taken over a lock held > 30 s while `produce` ran: chain onto the tail as it is now, not as it was
        let (tail_seq, tail_hash) = self.tail().map_err(Appended::Log)?;
        let (line_seq, prev) = if tail_seq + 1 == n {
            (n, prev)
        } else {
            (tail_seq + 1, tail_hash)
        };
        if line_seq != n
            && let Value::Object(m) = &mut fields
        {
            m.insert("copy".into(), n.into());
        }
        self.write_line(line_seq, &prev, &event, fields)
            .map_err(Appended::Log)?;
        made.map(|t| (n, t)).map_err(Appended::Produce)
    }

    fn write_line(&self, seq: u64, prev: &str, event: &str, fields: Value) -> Result<(), String> {
        let mut body = match fields {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        if self.redact {
            body.remove("file");
        }
        body.insert("seq".into(), seq.into());
        body.insert("ts".into(), utc_iso(now_secs()).into());
        body.insert("prev".into(), prev.into());
        body.insert("event".into(), event.into());
        if self.key.is_some() {
            body.insert("alg".into(), ALG.into());
        }
        let hash = hash_of(&body, self.key.as_deref());
        body.insert("hash".into(), hash.into());
        let mut line = serde_json::to_string(&body).expect("serialises");
        line.push('\n');
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("cannot open the audit log: {e}"))?;
        f.write_all(line.as_bytes())
            .and_then(|_| f.sync_data())
            .map_err(|e| format!("cannot write the audit log: {e}"))
    }
}

/// Why `append_after` failed: the log itself, or the work it was to vouch for.
#[derive(Debug)]
pub enum Appended<E> {
    Log(String),
    Produce(E),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("convert-audit-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("xdfu-audit.jsonl")
    }

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(utc_text(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_text(951_782_400 + 86_399), "2000-02-29 23:59:59 UTC"); // a leap day
        assert_eq!(utc_text(1_792_577_703), "2026-10-21 10:15:03 UTC");
        assert_eq!(utc_iso(1_792_577_703), "2026-10-21T10:15:03Z");
    }

    #[test]
    fn the_chain_verifies_and_every_kind_of_tampering_breaks_it() {
        let p = tmp("chain");
        let log = AuditLog::open(&p, false).unwrap();
        for i in 1..=4u64 {
            let seq = log.append("copy", json!({"user": format!("u{i}"), "client": "10.0.0.5", "file": "a.docx", "copy": i})).unwrap();
            assert_eq!(seq, i);
        }
        let v = verify(&p).unwrap();
        assert_eq!(v.lines, 4);
        assert_eq!(v.last_hash.len(), 64);
        let good = fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = good.lines().collect();
        let check = |text: String, want: &str| {
            fs::write(&p, text).unwrap();
            let e = verify(&p).unwrap_err();
            assert!(e.starts_with(want), "{e}");
        };
        // one field of line 2 changed
        check(good.replacen("\"u2\"", "\"mallory\"", 1), "line 2:");
        // line 2 removed, two lines swapped, a line inserted from another place in the log
        check([lines[0], lines[2], lines[3]].join("\n") + "\n", "line 2:");
        check(
            [lines[0], lines[2], lines[1], lines[3]].join("\n") + "\n",
            "line 2:",
        );
        check(
            [lines[0], lines[1], lines[1], lines[2], lines[3]].join("\n") + "\n",
            "line 3:",
        );
        // the hash itself rewritten to fit a changed line: the next line no longer follows
        let forged = lines[1].replace("\"u2\"", "\"mallory\"");
        let mut obj: Map<String, Value> = serde_json::from_str(&forged).unwrap();
        obj.remove("hash");
        obj.insert("hash".into(), hash_of(&obj, None).into());
        check(
            [
                lines[0].to_string(),
                serde_json::to_string(&obj).unwrap(),
                lines[2].to_string(),
                lines[3].to_string(),
            ]
            .join("\n")
                + "\n",
            "line 3:",
        );
        // garbage
        check(good.clone() + "not json\n", "line 5:");
        // cutting lines off the END is the known blind spot: it still verifies, with fewer lines
        fs::write(&p, [lines[0], lines[1]].join("\n") + "\n").unwrap();
        assert_eq!(verify(&p).unwrap().lines, 2);
        // a damaged log is not extended, and not opened
        fs::write(&p, good.replacen("\"u3\"", "\"x\"", 1)).unwrap();
        assert!(AuditLog::open(&p, false).err().unwrap().contains("damaged"));
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn hmac_matches_rfc_4231_case_2() {
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// M5: a consistent rewrite (changed line, every hash after it recomputed) passes the plain chain and fails the keyed one.
    #[test]
    fn a_consistent_rewrite_without_the_key_fails_the_hmac_chain() {
        let p = tmp("hmac");
        let kf = p.with_file_name("audit.key");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&kf, b"0123456789abcdef0123456789abcdef\n").unwrap();
        let log = AuditLog::open_keyed(&p, false, Some(&kf)).unwrap();
        assert!(log.is_keyed());
        for i in 1..=3u64 {
            log.append("copy", json!({"user": format!("u{i}")}))
                .unwrap();
        }
        let key = load_key(&kf).unwrap();
        assert_eq!(verify_keyed(&p, Some(&key)).unwrap().lines, 3);
        // no key, or another key
        assert!(verify(&p).unwrap_err().contains("keyed"));
        assert!(
            verify_keyed(&p, Some(&[7u8; 32]))
                .unwrap_err()
                .starts_with("line 1:")
        );
        // the attacker rewrites line 2 and recomputes lines 2..3 with the plain SHA-256 chain (alg field kept or dropped): no good under the key
        let text = fs::read_to_string(&p).unwrap();
        let mut prev = serde_json::from_str::<Value>(text.lines().next().unwrap()).unwrap()["hash"]
            .as_str()
            .unwrap()
            .to_string();
        let mut out = vec![text.lines().next().unwrap().to_string()];
        for l in text.lines().skip(1) {
            let mut o: Map<String, Value> = serde_json::from_str(l).unwrap();
            o.remove("hash");
            if o["user"] == "u2" {
                o.insert("user".into(), "mallory".into());
            }
            o.insert("prev".into(), prev.clone().into());
            let h = hash_of(&o, None);
            o.insert("hash".into(), h.clone().into());
            prev = h;
            out.push(serde_json::to_string(&o).unwrap());
        }
        fs::write(&p, out.join("\n") + "\n").unwrap();
        let e = verify_keyed(&p, Some(&key)).unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
        // an unkeyed open of a keyed log, and a keyed open of a rewritten one, are refused
        assert!(
            AuditLog::open_keyed(&p, false, Some(&kf))
                .err()
                .unwrap()
                .contains("damaged")
        );
        assert!(AuditLog::open(&p, false).err().unwrap().contains("damaged"));
        // a short key is refused
        fs::write(&kf, b"short").unwrap();
        assert!(load_key(&kf).is_err());
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn a_lock_taken_over_during_production_keeps_the_chain_intact() {
        let p = tmp("takeover");
        let a = AuditLog::open(&p, false).unwrap();
        let b = AuditLog::open(&p, false).unwrap();
        a.append("export", serde_json::json!({})).unwrap();
        let lock = PathBuf::from(format!("{}.lock", p.display()));
        // the other process took the stale lock and wrote its line while `a` was still producing
        let (n, ()) = a
            .append_after("print", serde_json::json!({}), |_| {
                fs::remove_file(&lock).unwrap();
                b.append("export", serde_json::json!({})).map(|_| ())
            })
            .ok()
            .unwrap();
        assert_eq!(n, 2, "the copy number that was stamped");
        let v = verify(&p).expect("chain verifies");
        assert_eq!(v.lines, 3);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn a_failed_production_leaves_a_failed_line_and_never_a_copy() {
        let p = tmp("after");
        let log = AuditLog::open(&p, false).unwrap();
        let ok = log.append_after("copy", json!({"n": 1}), |seq| Ok::<_, String>(seq * 10));
        assert!(matches!(ok, Ok((1, 10))));
        let bad = log.append_after("copy", json!({"n": 2}), |_| {
            Err::<(), _>("mark failed".to_string())
        });
        assert!(matches!(bad, Err(Appended::Produce(ref m)) if m == "mark failed"));
        let text = fs::read_to_string(&p).unwrap();
        let events: Vec<String> = text
            .lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["event"]
                    .as_str()
                    .unwrap()
                    .into()
            })
            .collect();
        assert_eq!(events, ["copy", "copy_failed"]);
        assert!(text.contains("mark failed"));
        verify(&p).unwrap(); // the chain is unbroken
    }

    #[test]
    fn redact_leaves_the_file_name_out_and_a_reopened_log_continues_the_chain() {
        let p = tmp("redact");
        let log = AuditLog::open(&p, true).unwrap();
        log.append("copy", json!({"file": "secret-name.docx", "user": "u"}))
            .unwrap();
        drop(log);
        let log = AuditLog::open(&p, true).unwrap();
        assert_eq!(
            log.append("copy", json!({"file": "other.docx"})).unwrap(),
            2
        );
        let text = fs::read_to_string(&p).unwrap();
        assert!(
            !text.contains("secret-name")
                && !text.contains("other.docx")
                && !text.contains("\"file\""),
            "{text}"
        );
        assert_eq!(verify(&p).unwrap().lines, 2);
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }
}
