//! XDFU code shared by the converter (`services/convert`) and the editors (PLAN phase 4, WP11): the grif finder, the stamp line
//! (Resolution 766 cl. 12) and the keyed audit chain. Moved here unchanged from `services/convert/src/xdfu.rs`.
#![forbid(unsafe_code)]

pub mod audit;

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Watermark,
    Refuse,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s {
            "off" => Ok(Mode::Off),
            "watermark" => Ok(Mode::Watermark),
            "refuse" => Ok(Mode::Refuse),
            other => Err(format!(
                "xdfu = \"{other}\": expected \"off\", \"watermark\" or \"refuse\""
            )),
        }
    }
}

// ---- finding the grif ----------------------------------------------------------------------------------------------------

/// Lowercase alphanumeric words of `text`: everything else separates them.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// Latin letters that look like Cyrillic ones, folded to the Cyrillic letter (a grif typed with a Latin "x" or "c" is still a grif).
fn fold_cyrillic(w: &str) -> String {
    w.chars()
        .map(|c| match c {
            'a' => 'а',
            'c' => 'с',
            'e' => 'е',
            'k' => 'к',
            'm' => 'м',
            'o' => 'о',
            'p' => 'р',
            't' => 'т',
            'x' => 'х',
            'y' => 'у',
            c => c,
        })
        .collect()
}

const PHRASES: [(&str, &str); 3] = [
    ("для служебного пользования", "Для служебного пользования"),
    (
        "xizmat doirasida foydalanish uchun",
        "Xizmat doirasida foydalanish uchun",
    ),
    (
        "хизмат доирасида фойдаланиш учун",
        "Хизмат доирасида фойдаланиш учун",
    ),
];

/// The grif marker found in `text`, if any. Case and spacing do not matter: "xdfu", "X D F U", "Х.Д.Ф.У." and "ДСП" all match, as
/// do the phrase "для служебного пользования" and its Uzbek forms. A code must stand alone as a word (or as spaced single letters),
/// so "ДСПЕ" or "xdfux" do not match.
pub fn find_grif(text: &str) -> Option<&'static str> {
    let w = words(text);
    let codes = |s: &str| -> Option<&'static str> {
        match fold_cyrillic(s).as_str() {
            "хдфу" => Some("ХДФУ"),
            "дсп" => Some("ДСП"),
            _ if s == "xdfu" => Some("XDFU"),
            _ => None,
        }
    };
    // whole words
    if let Some(m) = w.iter().find_map(|x| codes(x)) {
        return Some(m);
    }
    // single letters in a row ("Х.Д.Ф.У.", "D S P"), also after another one-letter word ("a x d f u")
    let mut run: Vec<char> = vec![];
    for x in w.iter().map(Some).chain([None]) {
        match x {
            Some(x) if x.chars().count() == 1 => run.extend(x.chars()),
            _ => {
                for len in [3, 4] {
                    if let Some(m) = run
                        .windows(len)
                        .find_map(|win| codes(&win.iter().collect::<String>()))
                    {
                        return Some(m);
                    }
                }
                run.clear();
            }
        }
    }
    let joined = format!(" {} ", w.join(" "));
    PHRASES
        .iter()
        .find(|(p, _)| joined.contains(&format!(" {p} ")))
        .map(|(_, shown)| *shown)
}

pub struct Stamp {
    pub copy: u64,
    pub client: String,
    pub user: String,
    pub when: String,
}

fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .take(64)
        .collect()
}

impl Stamp {
    /// `XDFU copy 17 | 10.0.0.5 | alice | 2026-10-21 10:15:03 UTC`
    pub fn line(&self) -> String {
        format!(
            "XDFU copy {} | {} | {} | {}",
            self.copy,
            one_line(&self.client),
            one_line(&self.user),
            self.when
        )
    }
    /// The same for outputs whose fonts cover Western European text only (pdf, png): anything else becomes `_`, and a user name
    /// that had to change gets a short digest so that two such names stay apart.
    pub fn ascii_line(&self) -> String {
        let a = |s: &str| -> String {
            one_line(s)
                .chars()
                .map(|c| {
                    if c.is_ascii_graphic() && !matches!(c, '<' | '>') || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let user = a(&self.user);
        let user = if user == one_line(&self.user) {
            user
        } else {
            format!("{user}~{}", &digest(&self.user)[..6])
        };
        format!(
            "XDFU copy {} | {} | {} | {}",
            self.copy,
            a(&self.client),
            user,
            self.when
        )
    }
}

fn digest(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Where the CLI keeps the audit log when `--xdfu-audit` is not given: the user's data directory.
pub fn default_audit_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        });
    base.map_or_else(
        || PathBuf::from("xdfu-audit.jsonl"),
        |b| b.join("convert").join("xdfu-audit.jsonl"),
    )
}

pub fn os_user() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

pub fn machine_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

/// The address this computer reaches the network with: the source address a UDP socket would use towards a public address (nothing
/// is sent). Falls back to the loopback address on a computer with no route.
pub fn machine_addr() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").and_then(|_| s.local_addr()))
        .map_or_else(|_| "127.0.0.1".into(), |a| a.ip().to_string())
}

/// The stamp line put at the top of a text output (`csv`, `md`, `html`, anything else is a plain first line). `old` is the file's bytes.
pub fn stamp_text(old: &[u8], ext: &str, line: &str) -> Result<Vec<u8>, String> {
    const BOM: &[u8] = b"\xEF\xBB\xBF";
    let bom: &[u8] = if old.starts_with(BOM) { BOM } else { b"" };
    let body = &old[bom.len()..];
    let mut new = bom.to_vec();
    match ext {
        "csv" => new.extend_from_slice(format!("\"{}\"\r\n", line.replace('"', "\"\"")).as_bytes()),
        "md" => new.extend_from_slice(format!("{line}\n\n").as_bytes()),
        "html" => {
            let text = String::from_utf8_lossy(body).into_owned();
            let lower = text.to_ascii_lowercase();
            let at = lower
                .find("<body")
                .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
                .ok_or("the html output has no <body>: refusing to leave it unmarked")?;
            let mark = format!(
                "<p style=\"color:#b00;font-weight:bold\">{}</p>",
                html_escape(line)
            );
            new.extend_from_slice(format!("{}{}{}", &text[..at], mark, &text[at..]).as_bytes());
            return Ok(new);
        }
        _ => new.extend_from_slice(format!("{line}\n").as_bytes()),
    }
    new.extend_from_slice(body);
    Ok(new)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---- the editors: one audit line per issued copy -------------------------------------------------------------------------

/// What an editor needs to stamp a copy it prints, exports or saves: the audit chain whose `seq` is the copy number (cl. 12).
pub struct Issuer {
    log: audit::AuditLog,
    addr: String,
}

impl Issuer {
    /// Open (and create) the log at `path`. A `key_file` that exists keys the chain (`audit::load_key`); with none the chain is the plain one.
    pub fn open(
        path: &std::path::Path,
        key_file: Option<&std::path::Path>,
    ) -> Result<Issuer, String> {
        let key = key_file.filter(|k| k.is_file());
        Ok(Issuer {
            log: audit::AuditLog::open_keyed(path, false, key)?,
            addr: machine_addr(),
        })
    }

    pub fn log(&self) -> &audit::AuditLog {
        &self.log
    }

    /// Reserve the next copy number and run `produce` with the stamp it belongs to (the user, the address, the time). The `event` line
    /// (`print`, `export`, `save_as`, ...) is written only when `produce` succeeded; a failure writes `<event>_failed`.
    pub fn issue<T, E: std::fmt::Display>(
        &self,
        event: &str,
        file: &str,
        produce: impl FnOnce(&Stamp) -> Result<T, E>,
    ) -> Result<(u64, T), audit::Appended<E>> {
        let (user, client) = (os_user(), self.addr.clone());
        let fields = serde_json::json!({"user": user, "client": client, "host": machine_name(), "mode": "watermark", "file": file});
        self.log.append_after(event, fields, |copy| {
            let when = audit::utc_text(audit::now_secs());
            produce(&Stamp {
                copy,
                client,
                user,
                when,
            })
        })
    }
}

/// An [`Issuer`] opened on first use, for an editor that never refuses a copy (the owner's watermark-only decision): when the audit log is
/// unusable the copy is still made, with copy number 0, and the reason goes to stderr.
pub struct LazyIssuer {
    log: PathBuf,
    key: PathBuf,
    issuer: std::sync::Mutex<Option<Issuer>>,
}

impl LazyIssuer {
    pub fn new(log: PathBuf, key: PathBuf) -> LazyIssuer {
        LazyIssuer {
            log,
            key,
            issuer: std::sync::Mutex::new(None),
        }
    }

    /// `produce` runs once with the stamp; its result is returned whatever happens to the log. The audit line is written only when it succeeded.
    pub fn issue<T, E: std::fmt::Display>(
        &self,
        event: &str,
        file: &str,
        produce: impl FnOnce(&Stamp) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut guard = self.issuer.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            match Issuer::open(&self.log, Some(&self.key)) {
                Ok(i) => *guard = Some(i),
                Err(e) => eprintln!(
                    "office: XDFU audit log {} unusable: {e}",
                    self.log.display()
                ),
            }
        }
        let mut produce = Some(produce);
        let mut slot: Option<Result<T, E>> = None;
        if let Some(i) = guard.as_ref() {
            let r = i.issue(event, file, |st| {
                let out = (produce.take().expect("runs once"))(st);
                let ok = out.is_ok();
                slot = Some(out);
                if ok {
                    Ok(())
                } else {
                    Err("the copy was not made")
                }
            });
            if let Err(audit::Appended::Log(e)) = r {
                eprintln!("office: XDFU audit write failed: {e}");
            }
        }
        match slot {
            Some(r) => r,
            None => {
                let st = Stamp {
                    copy: 0,
                    client: machine_addr(),
                    user: os_user(),
                    when: audit::utc_text(audit::now_secs()),
                };
                (produce.take().expect("did not run"))(&st)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_numbers_copies_from_the_chain() {
        let d = std::env::temp_dir().join(format!("xdfu-core-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let log = d.join("a.jsonl");
        let i = Issuer::open(&log, None).unwrap();
        let (n1, l1) = i
            .issue("print", "a.docx", |s| Ok::<_, String>(s.line()))
            .unwrap();
        assert!(
            i.issue("export", "a.docx", |_| Err::<(), _>("boom"))
                .is_err()
        );
        assert_eq!(n1, 1);
        assert!(l1.starts_with("XDFU copy 1 | ") && l1.contains(&os_user()));
        assert_eq!(audit::verify(&log).unwrap().lines, 2); // the failed one is logged as export_failed
        let j = Issuer::open(&log, None).unwrap(); // reopening verifies and continues the count
        assert_eq!(j.issue("print", "b", |_| Ok::<_, String>(())).unwrap().0, 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_grif_finder_and_the_text_stamp() {
        assert_eq!(find_grif("Х.Д.Ф.У."), Some("ХДФУ"));
        assert_eq!(find_grif("ordinary text"), None);
        assert_eq!(stamp_text(b"a,b\n", "csv", "L").unwrap(), b"\"L\"\r\na,b\n");
    }
}
