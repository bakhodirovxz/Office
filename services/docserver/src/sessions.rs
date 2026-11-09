//! Sessions on disk (PLAN phase 5, D2) and the opening of one from a provider's signed config (3.2, S3, S5, 8).
//!
//! Layout, under `<data_dir>/sessions/<sid>/` (the `sid` is 32 lower-case hex characters, 128 random bits):
//!   `session.json`  the `SessionRecord`: validated config fields, permissions, the version list, the close state; no token
//!   `v<N>.bin`      version N of the document, N from 1 (1 is the document as opened, then one per save)
//!   `autosave.bin`  the last recovery copy (not a version); a close promotes it to a version when it is newer and differs
//! Every write goes to a `.tmp` file, is synced, then renamed over the target. All changes run under one process-wide lock; there
//! is no state in memory, so a restart loses nothing.
use crate::Docserver;
use crate::config::{Allow, Provider, origin};
use crate::tokens::ct_eq;
use convert_api::jwt::{self, JwtError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The longest a config token may be valid, and the session's absolute life when the token has no `exp` (Q4).
pub const MAX_SESSION_S: u64 = 12 * 3600;

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub enum SessErr {
    /// 401: the credential is missing or bad (one answer for every reason)
    Unauthorized,
    /// 400, naming the field and never its value
    Bad(String),
    /// 404: no such session, or not yours
    NotFound,
    /// 403
    Forbidden,
    /// 413
    TooLarge,
    /// 415: not a document of the session's type, or a zip bomb; the text names the reason, never the content
    BadDocument(String),
    /// 502: the provider could not be reached or refused (the text never holds a URL)
    Upstream(String),
    /// 500; the text goes to stderr only
    Io(String),
}

impl From<std::io::Error> for SessErr {
    fn from(e: std::io::Error) -> Self {
        SessErr::Io(e.to_string())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Perms {
    pub edit: bool,
    pub download: bool,
    pub print: bool,
    pub review: bool,
    pub comment: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionRec {
    pub n: u64,
    pub sha256: String,
    pub size: u64,
    pub at: u64,
    /// the user (`sub`) who saved it; the provider for version 1
    pub by: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Closed {
    pub at: u64,
    /// the callback status the close stands for: 2 (saved since open) or 4 (nothing saved)
    pub status: u8,
    /// the callback for this close has been sent, or queued (status 2) or tried (status 4): a second close sends nothing
    #[serde(default)]
    pub notified: bool,
    /// the provider answered `{"error":0}` to this close's callback (a view session, which sends none, counts): a good close, deleted
    /// `keep_closed_hours` later by the retention sweep
    #[serde(default)]
    pub acked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionRecord {
    /// format version of this file
    pub v: u32,
    pub sid: String,
    pub provider: String,
    pub user_id: String,
    pub user_name: String,
    /// the provider's `document.key`, echoed in the callbacks (WP4)
    pub key: String,
    pub title: String,
    pub file_type: String,
    /// `docs`, `sheets`, `slides` or `pdf`: the editor prefix
    pub editor: String,
    pub mode: String,
    pub perms: Perms,
    /// where WP4 fetches version 1 from; it carries the provider's own token, so it is never logged or returned
    pub document_url: String,
    pub callback_url: Option<String>,
    pub created: u64,
    /// absolute end of the session: the config's `exp`, or 12 h; no session token outlives it
    pub expires: u64,
    pub last_activity: u64,
    pub versions: Vec<VersionRec>,
    pub saved_since_open: bool,
    pub closed: Option<Closed>,
    /// the highest version the provider has acknowledged with `{"error":0}`; a later version is unsaved on the provider's side
    #[serde(default)]
    pub acked: u64,
    /// the status 1 callback went out
    #[serde(default)]
    pub dirty_sent: bool,
    /// the stored `autosave.bin`, when there is one
    #[serde(default)]
    pub autosave: Option<AutosaveRec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AutosaveRec {
    /// the number of versions when it was stored: it is newer than the last version while that number has not changed
    pub after: u64,
    pub sha256: String,
    pub at: u64,
}

impl SessionRecord {
    /// A version exists that the provider has not acknowledged: the document stays dirty.
    pub fn dirty(&self) -> bool {
        self.versions.last().is_some_and(|v| v.n > self.acked)
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha256_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

pub fn valid_sid(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

fn bad(field: &str, why: &str) -> SessErr {
    SessErr::Bad(format!("{field}: {why}"))
}

/// `[0-9A-Za-z._=-]`, 1 to 128 characters (the DocsAPI `document.key` rule).
fn key_ok(k: &str) -> bool {
    (1..=128).contains(&k.len())
        && k.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._=-".contains(&c))
}

fn text(v: Option<&Value>, field: &str, max: usize) -> Result<Option<String>, SessErr> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s))
            if !s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control) =>
        {
            Ok(Some(s.clone()))
        }
        _ => Err(bad(field, "must be a plain text of 1 to a limited length")),
    }
}

fn flag(o: Option<&Value>, field: &str, default: bool) -> Result<bool, SessErr> {
    match o.and_then(|p| p.get(field)) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(bad(
            &format!("document.permissions.{field}"),
            "must be true or false",
        )),
    }
}

/// A fetch or callback URL: exact `scheme://host:port` (the default port when none is written) in `list`, no user info. The DNS and
/// address checks belong to the outbound client (WP4); this one needs no network.
pub(crate) fn url_allowed(u: &str, list: &[Allow], field: &str) -> Result<bool, SessErr> {
    if u.len() > 2048 || !u.bytes().all(|c| c.is_ascii_graphic()) || u.contains('\\') {
        return Err(bad(field, "not a plain URL"));
    }
    let (scheme, rest) = u.split_once("://").ok_or_else(|| bad(field, "not a URL"))?;
    let auth = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let o = origin(&format!("{scheme}://{auth}"), field, false)
        .map_err(|_| bad(field, "not an allowed URL"))?;
    let o = if o
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        o
    } else {
        format!("{o}:{}", if o.starts_with("https") { 443 } else { 80 })
    };
    // the entry's `internal` flag: whether the outbound client may connect to a private address for this URL
    match list.iter().find(|a| a.origin == o) {
        Some(a) => Ok(a.internal),
        None => Err(bad(field, "this address is not on the provider's list")),
    }
}

/// The editor prefix and the DocsAPI `documentType` for a file type, or None when it is not served.
fn editor_for(file_type: &str) -> Option<(&'static str, &'static str)> {
    match file_type {
        "docx" => Some(("docs", "word")),
        "xlsx" => Some(("sheets", "cell")),
        "pptx" => Some(("slides", "slide")),
        "pdf" => Some(("pdf", "pdf")),
        _ => None,
    }
}

fn rand_sid() -> Result<String, SessErr> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).map_err(|e| SessErr::Io(e.to_string()))?;
    Ok(hex(&b))
}

impl Docserver {
    /// `write_atomic` of `bytes` sealed for `path` when encryption at rest is on.
    fn write_stored(&self, path: &Path, bytes: &[u8]) -> Result<(), SessErr> {
        Ok(write_atomic(path, &self.to_store(path, bytes)?)?)
    }

    pub(crate) fn session_dir(&self, sid: &str) -> Option<PathBuf> {
        valid_sid(sid).then(|| self.data_dir.join("sessions").join(sid))
    }

    /// Verifies a config token against the providers' secrets (the unique secret picks the provider) and returns the provider and the
    /// signed payload. Only that payload is used afterwards (S5). One refusal for every reason.
    fn open_config(&self, token: &str, now: u64) -> Result<(&Provider, Value), SessErr> {
        for p in &self.providers {
            match jwt::verify_with(&p.secret, token, p.require_exp) {
                Ok(v) => {
                    // Tokens of some providers carry no `iss`: the secret already names the provider. When one is there it must be ours.
                    if v.get("iss").is_some_and(|i| i.as_str() != Some(&p.issuer)) {
                        return Err(SessErr::Unauthorized);
                    }
                    if v.get("exp")
                        .and_then(Value::as_u64)
                        .is_some_and(|e| e > now + MAX_SESSION_S)
                    {
                        return Err(SessErr::Unauthorized);
                    }
                    return Ok((p, v));
                }
                Err(JwtError::BadSignature) => continue,
                Err(_) => return Err(SessErr::Unauthorized),
            }
        }
        Err(SessErr::Unauthorized)
    }

    /// `POST /ds/session`: opens a session from a provider's signed config; `origin` is the wrapper's `event.origin` of the config message. Writes `session.json`; version 1 comes with the fetch
    /// (WP4, `put_version`).
    pub fn create_session(
        &self,
        token: &str,
        origin: &str,
        now: u64,
    ) -> Result<SessionRecord, SessErr> {
        let (p, c) = self.open_config(token, now)?;
        // cross-provider isolation: the page that handed us the config must be one of THIS provider's frame origins, or opaque with
        // `allow_opaque_origin` (checked after the signature: `null` never opens a session without a valid config token)
        let opaque = self.allow_opaque_origin && origin == "null";
        if !opaque
            && !p
                .frame_origins
                .iter()
                .any(|o| o.eq_ignore_ascii_case(origin))
        {
            eprintln!(
                "docserver: auth.fail: config message origin is not a frame origin of provider \"{}\"",
                p.name
            );
            return Err(SessErr::Forbidden);
        }
        let doc = match c.get("document") {
            Some(d @ Value::Object(_)) => d,
            _ => return Err(bad("document", "is required")),
        };
        let ec = c.get("editorConfig").unwrap_or(&Value::Null);
        let key = text(doc.get("key"), "document.key", 128)?
            .filter(|k| key_ok(k))
            .ok_or_else(|| bad("document.key", "1 to 128 characters of 0-9 A-Z a-z . _ = -"))?;
        let ft = text(doc.get("fileType"), "document.fileType", 8)?
            .unwrap_or_default()
            .to_ascii_lowercase();
        let (editor, dtype) =
            editor_for(&ft).ok_or_else(|| bad("document.fileType", "not a supported type"))?;
        if c.get("documentType")
            .is_some_and(|d| d.as_str() != Some(dtype))
        {
            return Err(bad("documentType", "does not match document.fileType"));
        }
        let url = text(doc.get("url"), "document.url", 2048)?
            .ok_or_else(|| bad("document.url", "is required"))?;
        url_allowed(&url, &p.fetch_allow, "document.url")?;
        let callback_url = text(ec.get("callbackUrl"), "editorConfig.callbackUrl", 2048)?;
        if let Some(u) = &callback_url {
            url_allowed(u, &p.callback_allow, "editorConfig.callbackUrl")?;
        }
        let user = ec.get("user");
        let user_id = match user.and_then(|u| u.get("id")) {
            Some(Value::Number(n)) => Some(n.to_string()),
            v => text(v, "editorConfig.user.id", 128)?,
        }
        .ok_or_else(|| bad("editorConfig.user.id", "is required"))?;
        let user_name = text(
            user.and_then(|u| u.get("name")),
            "editorConfig.user.name",
            128,
        )?
        .unwrap_or_else(|| user_id.clone());
        let title = text(doc.get("title"), "document.title", 255)?
            .unwrap_or_else(|| format!("document.{ft}"));
        let mode = match ec.get("mode").and_then(Value::as_str) {
            None | Some("edit") => "edit",
            Some("view") => "view",
            _ => return Err(bad("editorConfig.mode", "must be edit or view")),
        };
        let pm = doc.get("permissions");
        // edit needs all three: the mode, the permission and a callback to save to (a config without one is a view)
        let edit = mode == "edit" && flag(pm, "edit", true)? && callback_url.is_some();
        // a type the provider does not edit here (`edit_types`, an operator's choice since every bridge has the read-only open and the save
        // acknowledgement, docs/phase5/r6.md) opens as a view
        let edit = edit && p.edit_types.contains(&ft);
        let perms = Perms {
            edit,
            download: flag(pm, "download", true)?,
            print: flag(pm, "print", true)?,
            review: flag(pm, "review", false)?,
            comment: flag(pm, "comment", false)?,
        };
        let expires = c
            .get("exp")
            .and_then(Value::as_u64)
            .map_or(now + MAX_SESSION_S, |e| e.min(now + MAX_SESSION_S));
        let rec = SessionRecord {
            v: 1,
            sid: rand_sid()?,
            provider: p.name.clone(),
            user_id,
            user_name,
            key,
            title,
            file_type: ft,
            editor: editor.into(),
            mode: if edit { "edit" } else { "view" }.into(),
            perms,
            document_url: url,
            callback_url,
            created: now,
            expires,
            last_activity: now,
            versions: vec![],
            saved_since_open: false,
            closed: None,
            acked: 0,
            dirty_sent: false,
            autosave: None,
        };
        let _g = lock();
        let dir = self.session_dir(&rec.sid).ok_or(SessErr::NotFound)?;
        std::fs::create_dir_all(&dir)?; // the sid is fresh: a collision at 128 bits is not a case
        self.store(&rec)?;
        Ok(rec)
    }

    pub fn load_session(&self, sid: &str) -> Option<SessionRecord> {
        let b = self
            .read_stored(&self.session_dir(sid)?.join("session.json"))
            .ok()?;
        serde_json::from_slice(&b).ok()
    }

    fn store(&self, rec: &SessionRecord) -> Result<(), SessErr> {
        let dir = self.session_dir(&rec.sid).ok_or(SessErr::NotFound)?;
        let b = serde_json::to_vec_pretty(rec).map_err(|e| SessErr::Io(e.to_string()))?;
        self.write_stored(&dir.join("session.json"), &b)
    }

    /// An open (not closed) session, for a change. Call under the lock.
    fn open_session(&self, sid: &str) -> Result<SessionRecord, SessErr> {
        self.load_session(sid)
            .filter(|r| r.closed.is_none())
            .ok_or(SessErr::NotFound)
    }

    /// Adds `bytes` as the next version (1 is the opened document). `saved` marks a user save, which makes the close a status 2.
    pub fn put_version(
        &self,
        sid: &str,
        bytes: &[u8],
        by: &str,
        saved: bool,
        now: u64,
    ) -> Result<VersionRec, SessErr> {
        let _g = lock();
        let mut rec = self.open_session(sid)?;
        let v = self.add_version(&mut rec, bytes, by, saved, now)?;
        self.store(&rec)?;
        crate::history::record(self, &rec, bytes, by);
        Ok(v)
    }

    /// `put_version` without the store: the version file and its entry in `rec`. Call under the lock.
    fn add_version(
        &self,
        rec: &mut SessionRecord,
        bytes: &[u8],
        by: &str,
        saved: bool,
        now: u64,
    ) -> Result<VersionRec, SessErr> {
        let n = rec.versions.len() as u64 + 1;
        let dir = self.session_dir(&rec.sid).ok_or(SessErr::NotFound)?;
        self.write_stored(&dir.join(format!("v{n}.bin")), bytes)?;
        if n == 1 {
            rec.acked = 1; // the document as the provider gave it is, by definition, what the provider has
        }
        let v = VersionRec {
            n,
            sha256: sha256_hex(bytes),
            size: bytes.len() as u64,
            at: now,
            by: by.into(),
        };
        rec.versions.push(v.clone());
        rec.saved_since_open |= saved;
        rec.last_activity = now;
        Ok(v)
    }

    /// Version `n` of a session, checked against the digest in the record.
    pub fn read_version(&self, sid: &str, n: u64) -> Option<Vec<u8>> {
        let rec = self.load_session(sid)?;
        let v = rec.versions.iter().find(|v| v.n == n)?;
        let b = self
            .read_stored(&self.session_dir(sid)?.join(format!("v{n}.bin")))
            .ok()?;
        ct_eq(sha256_hex(&b).as_bytes(), v.sha256.as_bytes()).then_some(b)
    }

    pub fn put_autosave(&self, sid: &str, bytes: &[u8], now: u64) -> Result<(), SessErr> {
        let _g = lock();
        let mut rec = self.open_session(sid)?;
        self.set_autosave(&mut rec, bytes, now)?;
        self.store(&rec)
    }

    /// `put_autosave` without the store: `autosave.bin` and its entry in `rec`. Call under the lock.
    fn set_autosave(&self, rec: &mut SessionRecord, bytes: &[u8], now: u64) -> Result<(), SessErr> {
        self.write_stored(
            &self
                .session_dir(&rec.sid)
                .ok_or(SessErr::NotFound)?
                .join("autosave.bin"),
            bytes,
        )?;
        rec.autosave = Some(AutosaveRec {
            after: rec.versions.len() as u64,
            sha256: sha256_hex(bytes),
            at: now,
        });
        rec.last_activity = now;
        Ok(())
    }

    /// `POST .../ping`: the editor page is still there (the idle close counts from the last activity).
    pub fn touch(&self, sid: &str, now: u64) -> Result<(), SessErr> {
        let _g = lock();
        let mut rec = self.open_session(sid)?;
        rec.last_activity = now;
        self.store(&rec)
    }

    /// Closes a session: status 2 when something was saved since open, else 4. Closing twice answers the first status.
    pub fn close_session(&self, sid: &str, now: u64) -> Result<Closed, SessErr> {
        self.close_with(sid, None, now)
    }

    /// `close_session` with the editor's final copy (the body of `POST .../close`, checked by the caller as an autosave body):
    /// in the same locked step it becomes the recovery copy, which the close then makes a version (status 2) unless it equals
    /// the last one. A session already closed ignores it and answers its first status.
    pub fn close_with(&self, sid: &str, last: Option<&[u8]>, now: u64) -> Result<Closed, SessErr> {
        let _g = lock();
        let mut rec = self.load_session(sid).ok_or(SessErr::NotFound)?;
        let from = match last {
            Some(b) if rec.closed.is_none() && rec.perms.edit => {
                self.set_autosave(&mut rec, b, now)?;
                "close"
            }
            _ => "autosave",
        };
        self.close_locked(rec, now, from)
    }

    /// The idle close: an open edit session with no activity for `close_idle_s` (its page went away without a close) is closed like
    /// `close_session`. None when the session is not that (checked under the lock, so a ping in between keeps it open).
    pub fn close_if_idle(&self, sid: &str, now: u64) -> Option<Closed> {
        let _g = lock();
        let rec = self.load_session(sid).filter(|r| {
            r.perms.edit
                && r.closed.is_none()
                && now >= r.last_activity.saturating_add(self.close_idle_s)
        })?;
        self.close_locked(rec, now, "autosave")
            .map_err(|e| eprintln!("docserver: idle close failed: {e:?}"))
            .ok()
    }

    /// Call under the lock. A recovery copy newer than the last version and different from it becomes a version first (the page went
    /// away before its save, ONLYOFFICE's close-time assembly): the version file, the record with its close, then the history.
    /// `from` names where the copy came from in the audit line (`autosave`, or `close`: the close request's own body).
    fn close_locked(
        &self,
        mut rec: SessionRecord,
        now: u64,
        from: &str,
    ) -> Result<Closed, SessErr> {
        if let Some(c) = &rec.closed {
            return Ok(c.clone());
        }
        let promoted = self.autosave_to_promote(&rec);
        let by = rec.user_id.clone();
        let v = match &promoted {
            Some(b) => Some(self.add_version(&mut rec, b, &by, true, now)?),
            None => None,
        };
        let c = Closed {
            at: now,
            status: if rec.saved_since_open { 2 } else { 4 },
            notified: false,
            acked: false,
        };
        rec.closed = Some(c.clone());
        rec.last_activity = now;
        self.store(&rec)?;
        if let (Some(b), Some(v)) = (&promoted, &v) {
            crate::history::record(self, &rec, b, &rec.user_id);
            self.audit(
                "save",
                serde_json::json!({"session": crate::audit::sid_ref(&rec.sid), "version": v.n, "sha256": v.sha256, "size": v.size,
                                   "user": rec.user_id, "from": from}),
            );
        }
        Ok(c)
    }

    /// The bytes of `autosave.bin` when they should become a version at close: an edit session, stored after the last version,
    /// different from it, intact (its digest) and a sound document of the session's type (the checks of a save body).
    fn autosave_to_promote(&self, rec: &SessionRecord) -> Option<Vec<u8>> {
        let a = rec.autosave.as_ref()?;
        let last = rec.versions.last()?;
        if !rec.perms.edit || a.after != rec.versions.len() as u64 || a.sha256 == last.sha256 {
            return None;
        }
        let b = self
            .read_stored(&self.session_dir(&rec.sid)?.join("autosave.bin"))
            .ok()?;
        let sound = ct_eq(sha256_hex(&b).as_bytes(), a.sha256.as_bytes())
            && crate::ooxml::check(&b, &rec.file_type, self.max_document_mb << 20).is_ok();
        if !sound {
            eprintln!("docserver: the recovery copy of a session is damaged and was not used");
            return None;
        }
        Some(b)
    }
}

impl Docserver {
    pub(crate) fn provider(&self, name: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.name == name)
    }

    /// Fetches version 1 from the provider when the session has none yet (WP4, S6-S9). The fetch runs outside the lock; two racing
    /// opens may each fetch and the second adds a version 2 with the same bytes. `shortcut: harmless, and the wrapper opens once.`
    pub fn ensure_document(&self, sid: &str, now: u64) -> Result<(), SessErr> {
        let rec = self.load_session(sid).ok_or(SessErr::NotFound)?;
        if !rec.versions.is_empty() {
            return Ok(());
        }
        let p = self.provider(&rec.provider).ok_or(SessErr::NotFound)?;
        let internal = url_allowed(&rec.document_url, &p.fetch_allow, "document.url")?;
        let cap = self.max_document_mb << 20;
        // the provider's file token is in the URL path (some providers): the log line names the host only
        let host = crate::outbound::host_only(&rec.document_url);
        let sref = crate::audit::sid_ref(sid);
        let bytes = self
            .fetch_bytes(&rec.document_url, internal, cap)
            .map_err(|e| {
                // ureq's text for a transport error may hold the URL with its query: neither log nor browser gets it (F7)
                let (event, why) = match &e {
                    crate::outbound::OutErr::Blocked(w) => ("ssrf.block", w.clone()),
                    crate::outbound::OutErr::Unreachable(_) => {
                        ("document.fetch", "unreachable".to_string())
                    }
                    e => ("document.fetch", e.to_string()),
                };
                eprintln!("docserver: fetch from {host} failed: {why}");
                self.audit(
                    event,
                    serde_json::json!({"session": sref, "phase": "fetch", "host": host, "result": "failed", "reason": why}),
                );
                match e {
                    crate::outbound::OutErr::TooLarge => SessErr::TooLarge,
                    _ => SessErr::Upstream("the document could not be fetched".into()),
                }
            })?;
        let fetched = |r: &str| serde_json::json!({"session": sref, "host": host, "result": r, "size": bytes.len(), "sha256": sha256_hex(&bytes)});
        crate::ooxml::check(&bytes, &rec.file_type, cap).map_err(|e| {
            eprintln!("docserver: the document from {host} was refused");
            self.audit("document.fetch", fetched("refused"));
            Self::doc_err(e)
        })?;
        eprintln!("docserver: document.fetch {host} {} bytes", bytes.len());
        self.audit("document.fetch", fetched("ok"));
        self.put_version(sid, &bytes, &rec.provider, false, now)?;
        Ok(())
    }

    pub(crate) fn doc_err(e: crate::ooxml::DocErr) -> SessErr {
        match e {
            crate::ooxml::DocErr::TooLarge => SessErr::TooLarge,
            crate::ooxml::DocErr::Bad(m) => SessErr::BadDocument(m),
        }
    }

    /// Records the provider's acknowledgement of version `n`.
    pub(crate) fn mark_acked(&self, sid: &str, n: u64) {
        let _g = lock();
        if let Some(mut rec) = self.load_session(sid)
            && rec.acked < n
        {
            rec.acked = n;
            let _ = self.store(&rec);
        }
    }

    /// Edits a stored session under the lock (the callback code's flags).
    pub(crate) fn update_session(&self, sid: &str, f: impl FnOnce(&mut SessionRecord)) {
        let _g = lock();
        if let Some(mut rec) = self.load_session(sid) {
            f(&mut rec);
            let _ = self.store(&rec);
        }
    }
}
