//! XDFU on the server (PLAN phase 5, WP14; Resolution 766 cl. 12). Off by default: with `xdfu = "off"` nothing here is built, nothing is
//! registered and no byte is changed.
//!
//! On: a copy that leaves the system (a history version downloaded by a user) is checked for the grif (convert's
//! `grif_in_ooxml`, the same places and rules as the converter). The provider's working copy (the file a callback hands over) is never
//! stamped (owner, 2026-10-26: the stamps would pile up). A document without the grif is served as it is and nothing is logged. A
//! document with it gets the stamp line (copy number, network address, user, time) through the converter's docx/xlsx/pptx stamping, and one line in `<data_dir>/xdfu-audit.jsonl`, a keyed chain whose `seq` is the copy number. The log line is
//! written only when the stamped copy was produced. A copy that cannot be checked (a pdf, a damaged package) or stamped, or a log that
//! cannot be written, is refused: no unmarked copy of a document that may be marked leaves the server.
//!
//! `shortcut: pdf documents cannot be checked here (no pdf tool in this process), so with xdfu on their copies are refused; upgrade when
//! the docserver calls pdf-cli.`
use crate::Docserver;
use crate::sessions::{SessErr, SessionRecord};
use convert::audit::{self, AuditLog};
use convert::xdfu::{Stamp, grif_in_ooxml, stamp_ooxml};
use serde_json::json;
use std::path::Path;

pub struct Xdfu {
    log: AuditLog,
}

impl Xdfu {
    /// Opens (and verifies) the copy log under `data_dir`, keyed with the audit key.
    pub fn open(data_dir: &Path, key_file: Option<&Path>) -> Result<Xdfu, String> {
        AuditLog::open_keyed(&data_dir.join("xdfu-audit.jsonl"), false, key_file)
            .map(|log| Xdfu { log })
            .map_err(|e| format!("docserver: xdfu: {e}"))
    }
}

impl Docserver {
    /// `bytes` as a copy for `client` (a network address) of the session `rec`: unchanged unless xdfu is on and the document has the
    /// grif. `kind` is `download` (the provider's working copy from a callback is never stamped, owner 2026-10-26).
    pub(crate) fn issue_copy(
        &self,
        rec: &SessionRecord,
        bytes: Vec<u8>,
        client: &str,
        kind: &str,
    ) -> Result<Vec<u8>, SessErr> {
        let Some(x) = &self.xdfu else {
            return Ok(bytes);
        };
        let ext = rec.file_type.as_str();
        let refuse = |m: String| {
            eprintln!("docserver: xdfu: no {kind} copy served: {m}");
            SessErr::BadDocument(format!("xdfu: the {kind} copy is refused"))
        };
        let Some(marker) = grif_in_ooxml(&bytes, ext).map_err(&refuse)? else {
            return Ok(bytes);
        };
        let fields = json!({"user": rec.user_id, "client": client, "session": crate::audit::sid_ref(&rec.sid),
            "provider": rec.provider, "kind": kind, "file": rec.title, "marker": marker, "mode": "watermark"});
        let (_, out) = x
            .log
            .append_after("copy", fields, |copy| {
                let st = Stamp {
                    copy,
                    client: client.to_string(),
                    user: rec.user_name.clone(),
                    when: audit::utc_text(audit::now_secs()),
                };
                stamp_ooxml(&bytes, ext, &st.line())
            })
            .map_err(|e| match e {
                audit::Appended::Produce(m) => refuse(m),
                audit::Appended::Log(m) => SessErr::Io(format!("xdfu copy log: {m}")),
            })?;
        Ok(out)
    }
}
