//! Multilevel List Library presets in the interface language. Upstream writes English level text ("Article %1.", "Section %1.%2",
//! "Chapter %1") into the document; HRM orders use these lists, so under Uzbek and Russian the text must read "I modda.",
//! "1.01-boʻlim", "1-bob". The number sits where the language puts it. The three patterns per language are the
//! reviewed gallery rows (`i18n/wordcraft.tsv`: Article I., Section 1.01, Chapter 1) with the number as a placeholder.
//! `list.multilevelPreset` is re-registered (F2): the original runs, then the new list's level text is replaced, inside the same undo step.

use std::cell::RefCell;
use std::sync::OnceLock;

use serde_json::Value;
use wordcraft_engine::{CmdResult, CommandSpec, Registry, Session};

const ID: &str = "list.multilevelPreset";

/// (article, section, chapter) level text per language code; other languages keep upstream's text.
fn texts(lang: &str) -> Option<[&'static str; 3]> {
    Some(match lang {
        "uz-latn" => ["%1 modda.", "%1.%2-boʻlim", "%1-bob"],
        "uz-cyrl" => ["%1 модда.", "%1.%2-бўлим", "%1-боб"],
        "ru" => ["Статья %1.", "Раздел %1.%2", "Глава %1"],
        _ => return None,
    })
}

thread_local! {
    static LANG: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The interface language (lowercase code); `ribbon_frame` keeps it current.
pub fn set_lang(lang: &str) {
    LANG.with(|l| {
        if *l.borrow() != lang {
            *l.borrow_mut() = lang.to_string();
        }
    });
}

static ORIGINAL: OnceLock<fn(&mut Session, &Value) -> CmdResult> = OnceLock::new();

fn run(s: &mut Session, v: &Value) -> CmdResult {
    let Some(orig) = ORIGINAL.get() else { return Err(wordcraft_engine::CmdError::Unknown(ID.into())) };
    let r = orig(s, v)?;
    let Some(t) = LANG.with(|l| texts(&l.borrow())) else { return Ok(r) };
    let abstract_id = r["num"].as_u64().and_then(|n| s.doc.numbering.nums.iter().find(|x| u64::from(x.id) == n)).map(|n| n.abstract_id);
    let Some(a) = abstract_id.and_then(|id| s.doc.numbering.abstracts.iter_mut().find(|a| a.id == id)) else { return Ok(r) };
    for l in &mut a.levels {
        match l.text.as_str() {
            "Article %1." => l.text = t[0].into(),
            "Section %1.%2" => l.text = t[1].into(),
            "Chapter %1" => l.text = t[2].into(),
            _ => {}
        }
    }
    Ok(r)
}

pub fn wrapped(reg: &Registry) -> Vec<CommandSpec> {
    let Some(c) = reg.get(ID) else { return Vec::new() };
    ORIGINAL.get_or_init(|| c.run);
    vec![CommandSpec { run, ..c.clone() }]
}
