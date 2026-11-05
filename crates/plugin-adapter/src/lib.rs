//! The app-neutral half of the Sheets and Slides plugin adapters (Phase 7 WP13; docs/phase7/PLAN.md sections 4, 5 and 11, and
//! docs/plugins.md). Docs has its own adapter (`docs-plugins`, the model this one follows).
//!
//! - this file: the loaded plugins and their 64 slots. The engines' registries are closed (`OnceLock`) and only grow through
//!   `register_command`, which takes fn pointers and has no remove: so a registry id is bound to one of 64 fixed slot functions for
//!   the life of the process, and the slot looks its plugin command up by that id on every run.
//! - [`exec`]: the action executor over an [`Editor`]. One invocation is one undo step, any error restores the document, a stale
//!   revision is refused, a protected document is refused, and the allowlist of the app (`plugin_api::allow::list`) applies.
//! - [`bridge`]: the web side. The plugin runs in a Worker of the page; the editor keeps the pending invocation, the input lock and
//!   the timeout. [`web`] is its JS end (`<prefix>:plugin*` messages).
//! - `native` (feature `native`): the wasmtime guest and the loader of the user's plugin store.
//!
//! What Sheets and Slides do not have in v1 (docs/plugins.md section 6): events are never sent, and a panel is drawn only on the web
//! (by the page host); natively a `ui.panel` action is accepted and kept but not drawn.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use plugin_api::action::Widget;
use plugin_api::manifest::{DocAccess, Localized, Manifest};
use plugin_api::textmap::{Apply, TextMap};
use plugin_api::wire::Message;
use serde_json::{Map, Value};

pub mod bridge;
pub mod exec;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use plugin_api;

/// Slot commands: the most distinct plugin commands one process registers (PLAN section 4.1: 16 commands per plugin).
pub const SLOTS: usize = 64;
/// The params text of every slot command (what `describe` shows).
pub const PARAMS: &str = r#"{"scope"?: "auto"|"selection"|"document", ...the plugin's own params}"#;

/// One editor session as the executor sees it. Implemented by the engine crates over their `Session`.
pub trait Editor {
    /// What [`Editor::restore`] puts back: the document, the selection, the undo and redo stacks.
    type Snap: 'static;
    /// What a text map read keeps for its apply (the cells or paragraphs it walked).
    type Map: 'static;
    /// The app id of the manifest (`sheets`, `slides`).
    fn app(&self) -> &'static str;
    /// The open document, `None` without one.
    fn doc_id(&self) -> Option<u64>;
    /// Moves whenever the document's content changes (never for a selection change).
    fn rev(&self) -> u64;
    fn snapshot(&self) -> Option<Self::Snap>;
    fn restore(&mut self, snap: Self::Snap);
    /// After a successful invocation: everything changed since `snap` becomes one undo step labelled `label`. `in_command`: the
    /// slot command's own `Session::execute` frames the call, so an engine that already makes that one step does nothing here.
    fn commit(&mut self, snap: &Self::Snap, label: &str, in_command: bool);
    /// Run a registry command (already checked against the allowlist).
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String>;
    /// Why a plugin that may write must not start on this document now (a protected workbook), or `None`.
    fn protected(&self) -> Option<&'static str>;
    /// The text map of `scope` (`auto`, `selection`, `document`, or the app's own names: `sheet`, `workbook`, `slide`, `deck`).
    fn read_map(&mut self, scope: &str) -> Result<(TextMap, Self::Map), String>;
    /// Write the changed segments of `ap` (already validated against `map`); every target is checked against the live document
    /// first. Returns the paragraphs (cells) written.
    fn apply_map(&mut self, map: &TextMap, at: &Self::Map, ap: &Apply) -> Result<usize, String>;
    /// The selected text, paragraphs (cells) joined by a line feed.
    fn selection_text(&self) -> String;
}

/// One invocation's conversation with the plugin. Natively this is `plugin_host::Session`.
pub trait GuestSession {
    fn round(&mut self, call: &Message) -> Result<Message, String>;
    /// Shorten the time every later round may take. A guest without a clock of its own ignores it.
    fn limit_budget(&mut self, _d: std::time::Duration) {}
}

/// A loaded plugin module. `start` begins one invocation.
pub trait Guest: Send + Sync {
    fn start(&self) -> Result<Box<dyn GuestSession>, String>;
    /// The plugin runs in the page (a Worker) and answers later: the invocation goes through [`bridge`].
    fn remote(&self) -> bool {
        false
    }
}

/// The host side of the `fetch` action, after the grant check passed. Returns the response as JSON.
pub trait Fetch: Send + Sync {
    fn fetch(
        &self,
        url: &str,
        method: &str,
        headers: &BTreeMap<String, String>,
        body: Option<&str>,
    ) -> Result<Value, String>;
}

/// What a panel shows (kept for a later version that draws it natively) and the last error.
#[derive(Default)]
pub struct UiState {
    pub panels: BTreeMap<String, Vec<Widget>>,
    pub last_error: Option<String>,
}

pub struct Plugin {
    pub manifest: Manifest,
    pub settings: Map<String, Value>,
    pub guest: Arc<dyn Guest>,
    pub fetcher: Option<Arc<dyn Fetch>>,
    /// Network origins that were granted (the manifest's list minus what nobody granted).
    pub origins: Vec<String>,
    enabled: AtomicBool,
    /// Invocations that ran out of time in this session; at `DISABLE_AFTER_TIMEOUTS` the plugin is switched off.
    timeouts: AtomicU32,
    pub ui: Mutex<UiState>,
}

impl Plugin {
    pub fn new(manifest: Manifest, guest: Arc<dyn Guest>) -> Plugin {
        let settings = manifest.settings.clone();
        Plugin {
            manifest,
            settings,
            guest,
            fetcher: None,
            origins: Vec::new(),
            enabled: AtomicBool::new(true),
            timeouts: AtomicU32::new(0),
            ui: Mutex::new(UiState::default()),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    fn timed_out(&self) -> u32 {
        self.timeouts.fetch_add(1, Ordering::Relaxed) + 1
    }
    /// A command may change the document when the plugin may write it.
    pub fn mutates(&self) -> bool {
        self.manifest.permissions.document == DocAccess::Write
    }
    fn fail(&self, why: &str) {
        self.ui.lock().unwrap_or_else(|e| e.into_inner()).last_error = Some(why.to_string());
    }
}

#[derive(Default)]
struct State {
    plugins: Vec<Arc<Plugin>>,
    /// Slot n runs the plugin command whose registry id is `bound[n]`, for the whole process (the engines cannot unregister).
    bound: Vec<&'static str>,
    /// Commands that found no free slot.
    overflow: Vec<String>,
}

static STATE: RwLock<State> = RwLock::new(State {
    plugins: Vec::new(),
    bound: Vec::new(),
    overflow: Vec::new(),
});
static GENERATION: AtomicU64 = AtomicU64::new(0);
static APP: OnceLock<&'static str> = OnceLock::new();

fn state() -> std::sync::RwLockReadGuard<'static, State> {
    STATE.read().unwrap_or_else(|e| e.into_inner())
}

fn state_mut() -> std::sync::RwLockWriteGuard<'static, State> {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    STATE.write().unwrap_or_else(|e| e.into_inner())
}

/// The app this process hosts (`sheets`, `slides`); set once by the engine crate before the first [`add`].
pub fn set_app(app: &'static str) {
    let _ = APP.set(app);
}

pub fn app() -> &'static str {
    APP.get().copied().unwrap_or("")
}

/// Bumped by every change of the plugin set (the web host's sync re-binds the slots when it moved).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Add a plugin (replacing one with the same id), enabled. The engine crate then binds and registers its commands.
pub fn add(p: Plugin) -> Result<(), String> {
    plugin_api::manifest::validate(&p.manifest).map_err(|e| e.to_string())?;
    if !plugin_api::supports(p.manifest.api) {
        return Err(format!("plugin api {} is not supported", p.manifest.api));
    }
    let app = app();
    if app.is_empty() || !p.manifest.apps.iter().any(|a| a == app) {
        return Err(format!(
            "the plugin is not for {}",
            if app.is_empty() { "this app" } else { app }
        ));
    }
    let mut st = state_mut();
    st.plugins.retain(|q| q.manifest.id != p.manifest.id);
    st.plugins.push(Arc::new(p));
    Ok(())
}

pub fn remove(id: &str) {
    state_mut().plugins.retain(|q| q.manifest.id != id);
}

/// Enable or disable a plugin: a disabled plugin's slots refuse to run and its menu entries go.
pub fn set_enabled(id: &str, on: bool) {
    let st = state_mut();
    if let Some(p) = st.plugins.iter().find(|p| p.manifest.id == id) {
        p.enabled.store(on, Ordering::Relaxed);
        if on {
            p.timeouts.store(0, Ordering::Relaxed);
        }
    }
}

/// Forget every plugin (tests, and a host that reloads its list). Bound slots stay bound to their ids.
pub fn clear() {
    state_mut().plugins.clear();
}

pub fn plugins() -> Vec<Arc<Plugin>> {
    state().plugins.clone()
}

pub fn overflow() -> Vec<String> {
    state().overflow.clone()
}

/// The text of `l` in the interface language `lang` (`uz-latn`, `uz-Latn`, `ru`, ...); English when missing.
pub fn localized(l: &Localized, lang: &str) -> String {
    l.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(lang))
        .or_else(|| l.get_key_value("en"))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// Registry ids and labels live for the whole run (the engines' `CommandSpec` holds `&'static str`); the same text is leaked once.
pub fn intern(s: &str) -> &'static str {
    static POOL: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(x) = pool.get(s) {
        return x;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    pool.insert(leaked);
    leaked
}

/// Give every enabled plugin command without a slot the next free one. Returns the newly bound `(slot, registry id, English label)`:
/// the engine registers each once with its slot function. A command past the 64th distinct id goes to [`overflow`].
pub fn bind() -> Vec<(usize, &'static str, &'static str)> {
    let mut st = STATE.write().unwrap_or_else(|e| e.into_inner());
    let mut new = Vec::new();
    let mut overflow = Vec::new();
    let mut ids = Vec::new();
    for p in st.plugins.iter().filter(|p| p.enabled()) {
        for c in &p.manifest.commands {
            ids.push((
                plugin_api::manifest::registry_id(&p.manifest.id, &c.id),
                localized(&c.label, "en"),
            ));
        }
    }
    for (id, label) in ids {
        if st.bound.iter().any(|b| *b == id) {
            continue;
        }
        if st.bound.len() < SLOTS {
            let id = intern(&id);
            new.push((st.bound.len(), id, intern(&label)));
            st.bound.push(id);
        } else {
            overflow.push(id);
        }
    }
    st.overflow = overflow;
    new
}

/// The enabled plugin command bound to slot `n`: (plugin, command index).
pub fn slot(n: usize) -> Option<(Arc<Plugin>, usize)> {
    let st = state();
    let id = *st.bound.get(n)?;
    st.plugins.iter().filter(|p| p.enabled()).find_map(|p| {
        let i = p
            .manifest
            .commands
            .iter()
            .position(|c| plugin_api::manifest::registry_id(&p.manifest.id, &c.id) == id)?;
        Some((p.clone(), i))
    })
}

/// The UI's Plugins menu: per enabled plugin its name and its bound commands as (label, registry id), in `lang`. Sheets and Slides
/// have no command palette for plugin commands, so every command is listed, not only the manifest's toolbar and menu entries.
pub fn menu(lang: &str) -> Vec<(String, Vec<(String, &'static str)>)> {
    let st = state();
    let mut out = Vec::new();
    for p in st.plugins.iter().filter(|p| p.enabled()) {
        let items: Vec<(String, &'static str)> = p
            .manifest
            .commands
            .iter()
            .filter_map(|c| {
                let id = plugin_api::manifest::registry_id(&p.manifest.id, &c.id);
                st.bound
                    .iter()
                    .find(|b| **b == id)
                    .map(|b| (localized(&c.label, lang), *b))
            })
            .collect();
        if !items.is_empty() {
            out.push((localized(&p.manifest.name, lang), items));
        }
    }
    out
}

thread_local! {
    static LANG: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The interface language the UI last drew in (calls carry it to the plugin).
pub fn set_lang(lang: &str) {
    LANG.with(|l| {
        if *l.borrow() != lang {
            *l.borrow_mut() = lang.to_string();
        }
    });
}

pub fn lang() -> String {
    LANG.with(|l| {
        let l = l.borrow();
        if l.is_empty() {
            "en".to_string()
        } else {
            l.clone()
        }
    })
}

/// Milliseconds on a clock that also runs in a browser (wasm has no `SystemTime`: reading it traps).
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            * 1e3
    }
}

#[cfg(test)]
mod tests;
