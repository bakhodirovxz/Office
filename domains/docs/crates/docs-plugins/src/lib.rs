//! The Docs adapter of the plugin system (Phase 7 WP3; docs/phase7/PLAN.md sections 4, 5, 6 and 9).
//!
//! - [`textmap`]: the walker shared with `docs-translit`, and the plugin text map read and apply.
//! - [`exec`]: the action executor. One invocation is one undo step, any error restores the document, a stale revision is refused.
//! - this file: the loaded plugins, the 64 slot commands (a `CommandSpec` holds fn pointers, so a plugin command is one of 64
//!   fixed functions indexing a table, the pattern of `docs-office::hooks`), and the contributions (ribbon group, context menu,
//!   palette through the registry, side panel).
//! - [`panel`]: the egui side panel and the context items, drawn through the carried hook `wordcraft/0147`.
//!
//! The guest is a trait ([`Guest`]): wasmtime natively (WP2), a bridge to a Worker on the web (WP4/5), a closure in tests.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use plugin_api::action::Widget;
use plugin_api::manifest::{DocAccess, Localized, Manifest};
use plugin_api::wire::Message;
use serde_json::{Map, Value};
use wordcraft_engine::{CmdResult, CommandSpec, Registry, Session};

pub mod bridge;
pub mod exec;
pub mod panel;
pub mod textmap;

pub use panel::{context_items, panel as show_panel};

/// Slot commands: the most plugin commands registered at once (PLAN section 4.1: 16 commands per plugin).
pub const SLOTS: usize = 64;
const PARAMS: &str = r#"{"scope"?: "auto"|"selection"|"document", ...the plugin's own params}"#;

/// One invocation's conversation with the plugin. Natively this is `plugin_host::Session`; `round` is its method of that name.
pub trait GuestSession {
    fn round(&mut self, call: &Message) -> Result<Message, String>;
    /// Shorten the time every later round may take (an `event` invocation). A guest without a clock of its own ignores it.
    fn limit_budget(&mut self, _d: std::time::Duration) {}
}

/// A loaded plugin module. `start` begins one invocation (a fresh instance, or a reset one).
pub trait Guest: Send + Sync {
    fn start(&self) -> Result<Box<dyn GuestSession>, String>;
    /// The plugin runs elsewhere and answers later (the web page's Worker): the invocation goes through [`bridge`].
    fn remote(&self) -> bool {
        false
    }
}

/// The host side of the `fetch` action, after the grant check passed. Returns the response as JSON.
pub trait Fetch: Send + Sync {
    fn fetch(&self, url: &str, method: &str, headers: &BTreeMap<String, String>, body: Option<&str>) -> Result<Value, String>;
}

/// What a panel shows and the text a user typed into its fields.
#[derive(Default)]
pub struct UiState {
    pub panels: BTreeMap<String, Vec<Widget>>,
    pub fields: BTreeMap<String, String>,
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
        Plugin { manifest, settings, guest, fetcher: None, origins: Vec::new(), enabled: AtomicBool::new(true), timeouts: AtomicU32::new(0), ui: Mutex::new(UiState::default()) }
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    /// Count one invocation that ran out of time; returns the count so far.
    fn timed_out(&self) -> u32 {
        self.timeouts.fetch_add(1, Ordering::Relaxed) + 1
    }
    /// A command mutates the document when the plugin may write it.
    fn mutates(&self) -> bool {
        self.manifest.permissions.document == DocAccess::Write
    }
}

#[derive(Clone)]
struct Slot {
    plugin: Arc<Plugin>,
    command: usize,
}

#[derive(Default)]
struct State {
    plugins: Vec<Arc<Plugin>>,
    slots: Vec<Option<Slot>>,
    /// Commands that did not fit the 64 slots, for the settings page.
    overflow: Vec<String>,
}

static STATE: RwLock<State> = RwLock::new(State { plugins: Vec::new(), slots: Vec::new(), overflow: Vec::new() });
/// Bumped by every change of the plugin set; the ribbon preset and the registry compare it to know they are stale.
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn state() -> std::sync::RwLockReadGuard<'static, State> {
    STATE.read().unwrap_or_else(|e| e.into_inner())
}

fn state_mut() -> std::sync::RwLockWriteGuard<'static, State> {
    STATE.write().unwrap_or_else(|e| e.into_inner())
}

pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Fill the slots: first come, first served, in plugin order. Disabled plugins hold none.
fn assign(st: &mut State) {
    st.slots = vec![None; SLOTS];
    st.overflow.clear();
    let mut next = 0;
    for p in st.plugins.iter().filter(|p| p.enabled()).cloned().collect::<Vec<_>>() {
        for (i, c) in p.manifest.commands.iter().enumerate() {
            if next < SLOTS {
                st.slots[next] = Some(Slot { plugin: p.clone(), command: i });
                next += 1;
            } else {
                st.overflow.push(plugin_api::manifest::registry_id(&p.manifest.id, &c.id));
            }
        }
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Add a plugin (replacing one with the same id), enabled. Call [`register`] (or let the next frame do it) afterwards.
pub fn add(p: Plugin) -> Result<(), String> {
    plugin_api::manifest::validate(&p.manifest).map_err(|e| e.to_string())?;
    if !plugin_api::supports(p.manifest.api) {
        return Err(format!("plugin api {} is not supported", p.manifest.api));
    }
    if !p.manifest.apps.iter().any(|a| a == "docs") {
        return Err("the plugin is not for Docs".into());
    }
    let mut st = state_mut();
    st.plugins.retain(|q| q.manifest.id != p.manifest.id);
    st.plugins.push(Arc::new(p));
    assign(&mut st);
    Ok(())
}

pub fn remove(id: &str) {
    let mut st = state_mut();
    st.plugins.retain(|q| q.manifest.id != id);
    assign(&mut st);
}

/// Enable or disable a plugin: its commands, buttons and menu items appear or disappear at the next [`register`] or frame.
pub fn set_enabled(id: &str, on: bool) {
    let mut st = state_mut();
    if let Some(p) = st.plugins.iter().find(|p| p.manifest.id == id) {
        p.enabled.store(on, Ordering::Relaxed);
        if on {
            p.timeouts.store(0, Ordering::Relaxed);
        }
    }
    assign(&mut st);
}

/// Forget every plugin (tests, and a host that reloads its list).
pub fn clear() {
    let mut st = state_mut();
    st.plugins.clear();
    assign(&mut st);
}

pub fn plugins() -> Vec<Arc<Plugin>> {
    state().plugins.clone()
}

/// Commands that found no free slot.
pub fn overflow() -> Vec<String> {
    state().overflow.clone()
}

/// The text of `l` in the interface language `lang` (`uz-latn`, `uz-Latn`, `ru`, ...); English when missing.
pub fn localized(l: &Localized, lang: &str) -> String {
    l.iter().find(|(k, _)| k.eq_ignore_ascii_case(lang)).or_else(|| l.get_key_value("en")).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// Registry ids and labels live for the whole run (`CommandSpec` holds `&'static str`); the same text is leaked once.
fn intern(s: &str) -> &'static str {
    static POOL: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(x) = pool.get(s) {
        return x;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    pool.insert(leaked);
    leaked
}

fn slot(n: usize) -> Option<Slot> {
    state().slots.get(n).cloned().flatten()
}

fn slot_enabled<const N: usize>(_: &Session) -> Option<&'static str> {
    // Before the engine's protection gate (which would say "protected"): the bridge's own lock is why a pending call refuses another.
    if bridge::locked() {
        return Some(bridge::BUSY);
    }
    match slot(N) {
        Some(s) if s.plugin.enabled() => None,
        _ => Some("This plugin is not enabled"),
    }
}

fn slot_run<const N: usize>(s: &mut Session, v: &Value) -> CmdResult {
    let slot = slot(N).ok_or_else(|| wordcraft_engine::CmdError::Disabled("This plugin is not enabled".into()))?;
    exec::run_command(s, &slot.plugin, slot.command, v)
}

macro_rules! slot_fns {
    ($($n:literal),*) => { [$((slot_enabled::<$n> as fn(&Session) -> Option<&'static str>, slot_run::<$n> as fn(&mut Session, &Value) -> CmdResult)),*] };
}

fn slot_table() -> [(fn(&Session) -> Option<&'static str>, fn(&mut Session, &Value) -> CmdResult); SLOTS] {
    slot_fns!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
        37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63
    )
}

/// The specs of the commands the enabled plugins contribute, labelled in `lang`.
pub fn specs(lang: &str) -> Vec<CommandSpec> {
    let table = slot_table();
    let st = state();
    st.slots
        .iter()
        .enumerate()
        .filter_map(|(n, s)| {
            let s = s.as_ref()?;
            let c = s.plugin.manifest.commands.get(s.command)?;
            let (enabled, run) = table[n];
            Some(CommandSpec {
                id: intern(&plugin_api::manifest::registry_id(&s.plugin.manifest.id, &c.id)),
                label: intern(&localized(&c.label, lang)),
                location: "Plugins",
                shortcut: "",
                params: PARAMS,
                mutates: s.plugin.mutates(),
                enabled,
                run,
            })
        })
        .collect()
}

/// Put the plugin commands in `s.registry` (dropping the ones registered before), labelled in `lang`. Safe to call again.
pub fn register(s: &mut Session, lang: &str) {
    let ours = specs(lang);
    if ours.is_empty() && !s.registry.all().iter().any(|c| c.id.starts_with("plugin.")) {
        return;
    }
    let mut all: Vec<CommandSpec> = s.registry.all().iter().filter(|c| !c.id.starts_with("plugin.")).cloned().collect();
    all.extend(ours);
    s.registry = Arc::new(Registry::new(all));
}

/// Start-up hook: registers what is loaded now (English labels until the first frame knows the language).
pub fn install(s: &mut Session) {
    register(s, "en");
}

/// The ribbon preset's "Plugins" group: its name (the first toolbar item's `group` in `lang`, else `default_name`) and the command
/// ids of the enabled plugins' toolbar buttons and `tools` menu items. `None` when there are none.
pub fn ribbon_group(lang: &str, default_name: &str) -> Option<(String, Vec<&'static str>)> {
    let st = state();
    let mut name: Option<String> = None;
    let mut ids: Vec<&'static str> = Vec::new();
    for p in st.plugins.iter().filter(|p| p.enabled()) {
        let m = &p.manifest;
        let items = m.toolbar.iter().map(|t| (&t.command, t.group.as_ref())).chain(
            m.menu.iter().filter(|x| x.place == plugin_api::manifest::MenuPlace::Tools).map(|x| (&x.command, x.group.as_ref())),
        );
        for (cmd, group) in items {
            let id = intern(&plugin_api::manifest::registry_id(&m.id, cmd));
            if !ids.contains(&id) && st.slots.iter().flatten().any(|s| Arc::ptr_eq(&s.plugin, p) && m.commands.get(s.command).is_some_and(|c| &c.id == cmd)) {
                ids.push(id);
                if name.is_none() {
                    name = group.map(|g| localized(g, lang));
                }
            }
        }
    }
    (!ids.is_empty()).then(|| (name.unwrap_or_else(|| default_name.to_string()), ids))
}

/// The registry ids of the `context.text` menu items, with their labels in `lang`.
pub fn context_commands(lang: &str) -> Vec<(&'static str, String)> {
    let st = state();
    let mut out = Vec::new();
    for p in st.plugins.iter().filter(|p| p.enabled()) {
        for x in p.manifest.menu.iter().filter(|x| x.place == plugin_api::manifest::MenuPlace::ContextText) {
            if let Some(c) = p.manifest.commands.iter().find(|c| c.id == x.command) {
                out.push((intern(&plugin_api::manifest::registry_id(&p.manifest.id, &c.id)), localized(&c.label, lang)));
            }
        }
    }
    out
}

thread_local! {
    /// (generation, language, registry the commands were put in) of the last `frame`.
    static DONE: std::cell::RefCell<Option<(u64, String, usize)>> = const { std::cell::RefCell::new(None) };
    static LANG: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The interface language of the last frame (the panel painter has no catalog of its own).
pub fn lang() -> String {
    LANG.with(|l| l.borrow().clone())
}

/// Called once per frame by the ribbon hook: re-registers the commands when the plugin set, the language or the session's registry
/// changed, and fires events. Cheap when nothing is loaded.
pub fn frame(s: &mut Session, lang: &str) {
    LANG.with(|l| {
        let mut l = l.borrow_mut();
        if *l != lang {
            *l = lang.to_string();
        }
    });
    let key = (generation(), lang.to_string(), Arc::as_ptr(&s.registry) as usize);
    let stale = DONE.with(|d| d.borrow().as_ref().is_none_or(|k| *k != key));
    if stale {
        register(s, lang);
        let key = (key.0, key.1, Arc::as_ptr(&s.registry) as usize);
        DONE.with(|d| *d.borrow_mut() = Some(key));
    }
    exec::events_frame(s, lang);
}

#[cfg(test)]
mod tests;
