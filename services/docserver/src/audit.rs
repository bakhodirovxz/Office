//! The audit log and the rate limits (PLAN phase 5, WP8, 4 "Audit log" and "Rate limits"). One keyed hash chain
//! (`convert::audit::AuditLog::open_keyed`, the converter's log, not a fork), and the converter's fixed-window `Limiter` on
//! session creation and on failed authentication, counted per client address.
//!
//! What goes in: ids as a short hash (`sid_ref`, `key_ref`: the session id is a bearer secret of the file route's URL and the document
//! key is the provider's), hosts and paths without query, sizes, digests, counts. Never a token, a query string, document bytes, and
//! the file name only without `redact` (the chain drops the field `file` itself when it was opened with `redact`).
//!
//! A write that fails is said on stderr and does not fail the request (availability over a lost audit line; the operator sees it).
use crate::Docserver;
use crate::sessions::sha256_hex;
use convert::audit::AuditLog;
use convert_api::app::Limiter;
use serde_json::{Value, json};

pub struct Watch {
    log: Option<AuditLog>,
    pub create_rl: Limiter,
    pub auth_rl: Limiter,
    /// lets one `limit.hit` line per client and kind through per minute, so a flood fills the log with one line, not one per request
    once: Limiter,
}

impl Watch {
    pub fn new(log: Option<AuditLog>, create_per_min: u32, auth_fail_per_min: u32) -> Watch {
        Watch {
            log,
            create_rl: Limiter::new(create_per_min),
            auth_rl: Limiter::new(auth_fail_per_min),
            once: Limiter::new(1),
        }
    }

    /// No log, limits out of the way: for unit tests that build a `Docserver` by hand.
    pub fn off() -> Watch {
        Watch::new(None, u32::MAX, u32::MAX)
    }
}

/// How a session appears in the audit log and in `callback`/`save` lines: 16 hex characters of its id's digest.
pub fn sid_ref(sid: &str) -> String {
    sha256_hex(sid.as_bytes())[..16].to_string()
}

impl Docserver {
    /// One audit line. `fields` is a JSON object.
    pub fn audit(&self, event: &str, fields: Value) {
        if let Some(l) = &self.watch.log
            && let Err(e) = l.append(event, fields)
        {
            eprintln!("docserver: audit log: {e}");
        }
    }

    /// An audit line that `key` may write once a minute (the rest are dropped): for lines a client can provoke at will.
    pub fn audit_once(&self, key: &str, event: &str, fields: Value) {
        if self.watch.once.hit(key).is_ok() {
            self.audit(event, fields);
        }
    }

    /// A refused request, counted against the client. `Err(secs)` when the client is over its limit (the caller answers 429 instead
    /// of 401). Under the limit each refusal is an `auth.fail` line; over it only the first of a minute is written, as `limit.hit`, so a
    /// flood cannot fill the log or make every request wait for a disk sync.
    pub fn auth_failed(&self, route: &str, client: &str, key: &str, why: &str) -> Result<(), u64> {
        let r = self.watch.auth_rl.hit(key);
        match r {
            Ok(()) => {
                eprintln!("docserver: auth.fail {route}: {why} (client {client})");
                self.audit(
                    "auth.fail",
                    json!({"route": route, "reason": why, "client": client}),
                );
            }
            Err(secs) => self.limit_hit("auth", client, key, secs),
        }
        r
    }

    /// A session create attempt, counted. `Err(secs)` over the limit.
    pub fn create_allowed(&self, client: &str, key: &str) -> Result<(), u64> {
        let r = self.watch.create_rl.hit(key);
        if let Err(secs) = r {
            self.limit_hit("create", client, key, secs);
        }
        r
    }

    fn limit_hit(&self, kind: &str, client: &str, key: &str, retry_s: u64) {
        if self.watch.once.hit(&format!("{kind}/{key}")).is_ok() {
            self.audit(
                "limit.hit",
                json!({"kind": kind, "client": client, "retry_s": retry_s}),
            );
        }
    }
}
