//! The Uzbek 2026 keyboard's conversion state machine (docs/keyboard/PLAN.md 2.1, 6). Pure: no Windows, no I/O.
//!
//! The text input processor keeps only the pending tail of a digraph in a composition. It asks [`decide`] on every
//! key what to do with the composition. `typed` is the keys held in the composition ("s", "sh", "Sh", "oʻ", ...).
//! Lifted from the Phase 0 spike (`spikes/uz-keyboard`); the letter tables come from `uz-translit` (rules.md 15).

use uz_translit::{compose_letter, is_apostrophe};

/// A key as the conversion sees it.
#[derive(Clone, Copy, Debug)]
pub enum Key {
    /// s c o g h (either case) or an apostrophe of set A (`'`, `ʻ`, `’`, `` ` ``, ...). Any other char acts as
    /// [`Key::Other`].
    Ch(char),
    Back,
    Other,
}

#[derive(Debug, PartialEq)]
pub enum Act {
    /// Commit any open composition, then start a new one with this text (the keys typed).
    Start(String),
    /// Replace the composition's typed keys.
    Update(String),
    /// Replace the composition's text (may be empty), then commit it.
    Finish(String),
    /// Commit the composition; the key goes on to the application.
    Commit,
}

/// The combining mark that turns `base` + the second key into a 2026 letter, if the pair is a digraph.
fn mark(first: char, second: char) -> Option<char> {
    match (first.to_ascii_lowercase(), second) {
        ('s' | 'c', h) if h.eq_ignore_ascii_case(&'h') => Some('\u{327}'),
        ('o', a) if is_apostrophe(a) => Some('\u{308}'),
        ('g', a) if is_apostrophe(a) => Some('\u{306}'),
        _ => None,
    }
}

/// What the composition shows for the keys typed into it. R10: the first letter decides the case.
pub fn shown(typed: &str) -> String {
    let mut it = typed.chars();
    if let (Some(a), Some(b), None) = (it.next(), it.next(), it.next())
        && let Some(l) = mark(a, b).and_then(|m| compose_letter(a, m))
    {
        return l.to_string();
    }
    typed.into()
}

/// The conversion state machine. `None`: the key is not ours and nothing is pending.
pub fn decide(typed: &str, composing: bool, key: Key) -> Option<Act> {
    let t: Vec<char> = typed.chars().collect();
    let first = t.first().map(|c| c.to_ascii_lowercase());
    match key {
        Key::Ch(c) => {
            let lc = c.to_ascii_lowercase();
            if composing {
                match (first, t.len()) {
                    (Some(f @ ('s' | 'c' | 'o' | 'g')), 1)
                        if mark(f, c).is_some() || (f == 's' && is_apostrophe(c)) =>
                    {
                        return Some(Act::Update(format!("{typed}{c}")));
                    }
                    // s'h: the 1995 separator; 2026 writes plain sh (rules.md 15.2)
                    (Some('s'), 2) if is_apostrophe(t[1]) && lc == 'h' => {
                        return Some(Act::Finish(format!("{}{c}", t[0])));
                    }
                    _ => {}
                }
            }
            if matches!(lc, 's' | 'c' | 'o' | 'g') {
                Some(Act::Start(c.to_string()))
            } else {
                composing.then_some(Act::Commit)
            }
        }
        Key::Back if composing => match t.len() {
            1 => Some(Act::Finish(String::new())),
            _ if first == Some('s') && is_apostrophe(t[1]) => Some(Act::Update(t[0].to_string())),
            _ => Some(Act::Finish(typed.into())),
        },
        Key::Back => None,
        Key::Other => composing.then_some(Act::Commit),
    }
}

/// What the text is without the keyboard: Backspace (`\u{8}`) deletes, everything else is typed.
pub fn plain(typed: &str) -> String {
    let mut s = String::new();
    for c in typed.chars() {
        if c == '\u{8}' {
            s.pop();
        } else {
            s.push(c);
        }
    }
    s
}

/// `typed` plus the update rule, for hosts and tests that do not track it themselves.
#[derive(Default, Debug)]
pub struct Engine {
    typed: String,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }
    /// Keys held in the composition; empty when none is open.
    pub fn typed(&self) -> &str {
        &self.typed
    }
    pub fn composing(&self) -> bool {
        !self.typed.is_empty()
    }
    /// Decides and applies the new `typed`. The caller performs the returned action on the document.
    pub fn key(&mut self, key: Key) -> Option<Act> {
        let act = decide(&self.typed, self.composing(), key);
        match &act {
            Some(Act::Start(t) | Act::Update(t)) => self.typed = t.clone(),
            Some(Act::Finish(_) | Act::Commit) => self.typed.clear(),
            None => {}
        }
        act
    }
}
