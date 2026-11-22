#![allow(dead_code)]
use kbd_core::{Act, Engine, Key, shown};

pub const BACK: char = '\u{8}';

/// Plays keys against a fake document (what an application would hold). Returns the text and the engine.
/// shortcut: `Key::Ch` only for the keys the TIP maps (s c o g h and set A apostrophes); anything else is `Other`.
pub fn run(keys: &str) -> (String, Engine) {
    let (mut doc, mut e) = (String::new(), Engine::new());
    for k in keys.chars() {
        let key = match k {
            BACK => Key::Back,
            _ if "scogh".contains(k.to_ascii_lowercase()) || uz_translit::is_apostrophe(k) => {
                Key::Ch(k)
            }
            _ => Key::Other,
        };
        let before = e.typed().to_string();
        match e.key(key) {
            Some(Act::Start(_)) => doc += &shown(&before),
            Some(Act::Update(_)) => {}
            Some(Act::Finish(t)) => {
                // the composition's old text is replaced by `t`; nothing of `before` stays
                doc += &t;
            }
            Some(Act::Commit) | None => {
                doc += &shown(&before);
                if k == BACK {
                    doc.pop();
                } else {
                    doc.push(k);
                }
            }
        }
    }
    let out = doc.clone() + &shown(e.typed());
    (out, e)
}

pub fn sim(keys: &str) -> String {
    run(keys).0
}
