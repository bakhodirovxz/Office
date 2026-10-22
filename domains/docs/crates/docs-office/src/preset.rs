//! The simple Docs ribbon as a Customize Ribbon layout (toolbar-design section 3): Home and Insert unchanged, our "Document" tab
//! (Hujjat), the other ten built-in tabs hidden, and Show/Hide marks plus the simple/full toggle on the Quick Access Toolbar.
//!
//! Custom tab and group names are literal text in upstream's ribbon, so they are looked up in the interface language's catalog
//! (`names`) and written again when the language changes (`sync`). A saved layout counts as "ours" when its shape (tab order,
//! hidden flags, group commands) equals the preset's, whatever language its names are in; anything else is the user's own and is
//! left alone.

use std::cell::RefCell;

use serde_json::{Value, json};
use wordcraft_engine::{Registry, RibbonLayout, Session};

use crate::mode::{self, Mode};

pub const FULL: &str = "office.ribbonFull";
pub const SIMPLE: &str = "office.ribbonSimple";
const MARKS: &str = "view.marks";
const TAB_KEY: &str = "Document";

/// Built-in tabs the simple ribbon hides (they stay in the full ribbon and in the command search).
const HIDDEN: [&str; 10] = ["Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Developer", "Zotero", "Help"];

/// The custom tab's groups: (catalog key of the group's name, its commands). Every command runs with `{}` (design 3.4).
const GROUPS: [(&str, &[&str]); 5] = [
    ("File", &["file.print", "office.exportPdf"]),
    ("Page", &["layout.orientation", "layout.pageSetup"]),
    ("Spelling", &["review.spelling"]),
    ("Transliterate", &["tools.translitCyrl", "tools.translitLatn", "tools.translitAuto"]),
    ("History", &["office.history"]),
];

/// Every command id the preset and the Quick Access Toolbar name (the `office_toolbar_ids_exist` test checks them).
pub fn ids() -> Vec<&'static str> {
    let mut v: Vec<&str> = GROUPS.iter().flat_map(|(_, c)| c.iter().copied()).collect();
    v.extend(["file.save", "edit.undo", "edit.redo", MARKS, FULL, SIMPLE]);
    v
}

/// `t` turns a catalog key (English) into the interface language's text.
fn layout(reg: &Registry, t: &dyn Fn(&str) -> String) -> RibbonLayout {
    let groups: Vec<Value> = GROUPS.iter().map(|(key, cmds)| json!({"name": t(key), "commands": cmds})).collect();
    let mut tabs = vec![json!({"name": "Home"}), json!({"name": "Insert"}), json!({"name": t(TAB_KEY), "custom": true, "groups": groups})];
    tabs.extend(HIDDEN.iter().map(|n| json!({"name": n, "hidden": true})));
    let mut l = RibbonLayout::from_value(&json!({ "tabs": tabs }));
    l.retain_known(reg);
    l
}

/// The layout with names and group titles left out: what makes it "ours".
fn shape(l: &RibbonLayout) -> Vec<(String, bool, bool, Vec<Vec<String>>)> {
    l.tabs()
        .iter()
        .map(|t| (if t.custom { String::new() } else { t.name.clone() }, t.custom, t.hidden, t.groups.iter().map(|g| g.commands.clone()).collect()))
        .collect()
}

pub fn is_preset(l: &RibbonLayout, reg: &Registry) -> bool {
    !l.ribbon_is_default() && shape(l) == shape(&layout(reg, &|k| k.to_string()))
}

/// Put Show/Hide marks and the toggle that names what a click does on the Quick Access Toolbar (the toggle on desktop only).
fn set_qat(s: &mut Session, mode: Mode) {
    let reg = s.registry.clone();
    let (here, other) = if mode == Mode::Simple { (FULL, SIMPLE) } else { (SIMPLE, FULL) };
    s.ribbon.qat_remove(other);
    let _ = s.ribbon.qat_add(&reg, MARKS, None);
    if mode::toggle_available() {
        let _ = s.ribbon.qat_add(&reg, here, None);
    } else {
        s.ribbon.qat_remove(here);
    }
}

#[derive(Default)]
struct State {
    /// The (language, mode) the layout was last brought in line with.
    done: Option<(String, Mode)>,
    force: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Ask the next `sync` to apply the preset whatever the layout is (the "Sodda lenta" button).
pub fn force_next_sync() {
    STATE.with(|s| *s.borrow_mut() = State { done: None, force: true });
}

/// Bring the ribbon in line with the mode and the language. Cheap when nothing changed: the apps call it every frame.
/// Simple: a default or preset layout becomes the preset in this language. Full: a preset layout is reset. Anything else is left alone,
/// unless `force_next_sync` asked for the preset.
pub fn sync(s: &mut Session, lang: &str, t: &dyn Fn(&str) -> String) {
    crate::levels::set_lang(&lang.to_ascii_lowercase());
    let mode = mode::mode();
    let force = STATE.with(|st| {
        let st = st.borrow();
        if st.done.as_ref().is_some_and(|(l, m)| l == lang && *m == mode) {
            return None;
        }
        Some(st.force)
    });
    let Some(force) = force else { return };
    let reg = s.registry.clone();
    match mode {
        Mode::Simple if force || s.ribbon.ribbon_is_default() || is_preset(&s.ribbon, &reg) => {
            let want = layout(&reg, t);
            s.ribbon.tabs = want.tabs;
        }
        Mode::Full if is_preset(&s.ribbon, &reg) => s.ribbon.reset_ribbon(),
        _ => {}
    }
    set_qat(s, mode);
    STATE.with(|st| *st.borrow_mut() = State { done: Some((lang.to_string(), mode)), force: false });
}

/// `office.ribbonFull`: the full ribbon. The user's own ribbon customization is discarded (design 2.1); the QAT survives.
pub fn to_full(s: &mut Session) {
    mode::set_mode(Mode::Full);
    s.ribbon.reset_ribbon();
    set_qat(s, Mode::Full);
    STATE.with(|st| *st.borrow_mut() = State::default());
}

/// `office.ribbonSimple`: the preset is written by the next frame's `sync`, which knows the language.
pub fn to_simple(s: &mut Session) {
    mode::set_mode(Mode::Simple);
    set_qat(s, Mode::Simple);
    force_next_sync();
}
