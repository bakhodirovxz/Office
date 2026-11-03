//! The native end of the plugin system (Phase 7; docs/phase7/PLAN.md WP3 and docs/plugins.md): the wasmtime host as a
//! [`docs_plugins::Guest`], the host-performed `fetch`, and the loader that registers the enabled, verified plugins of the
//! suite-core store. Desktop and CLI only; the web build runs plugins in a Worker (`plugin_web`) and never links wasmtime.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use docs_plugins::{Fetch, Guest, GuestSession, Plugin};
use plugin_api::wire::Message;
use plugin_host::net::Net;
use serde_json::{Value, json};
use suite_core::plugins::Store;

/// The wall-clock budget of one round (the epoch deadline); the manifest sets only the memory.
const ROUND: Duration = Duration::from_secs(5);

struct Wasm(plugin_host::Plugin);
struct Sess(plugin_host::Session);

impl Guest for Wasm {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        self.0.start().map(|s| Box::new(Sess(s)) as Box<dyn GuestSession>).map_err(|e| e.to_string())
    }
}

impl GuestSession for Sess {
    fn round(&mut self, call: &Message) -> Result<Message, String> {
        self.0.round(call).map_err(|e| e.to_string())
    }
    fn limit_budget(&mut self, d: Duration) {
        self.0.limit_round_budget(d);
    }
}

/// `fetch` for one plugin: the granted origins, the checked address, the answer as `{status, body}`.
struct Granted(Net);

impl Fetch for Granted {
    fn fetch(&self, url: &str, method: &str, headers: &BTreeMap<String, String>, body: Option<&str>) -> Result<Value, String> {
        self.0.fetch(url, method, headers, body).map(|r| json!({"status": r.status, "body": r.body}))
    }
}

fn host() -> Result<&'static plugin_host::Host, String> {
    static HOST: OnceLock<Result<plugin_host::Host, String>> = OnceLock::new();
    HOST.get_or_init(plugin_host::Host::new).as_ref().map_err(Clone::clone)
}

/// Load every enabled, verified plugin of `store` and register it with `docs_plugins`, replacing what was registered before.
/// A package the store or the host refuses is said on stderr and skipped. Returns the ids that were registered.
/// Compiling takes about 150 ms a plugin, so the apps call [`init`], which runs this on a thread.
pub fn load(store: &Store) -> Vec<String> {
    let mut ids = Vec::new();
    let Ok(host) = host().inspect_err(|e| eprintln!("plugins: host did not start: {e}")) else { return ids };
    // `enabled` verifies each package (digest, record) and says why it refuses one
    let list = store.enabled();
    let keep = |id: &str| list.iter().any(|e| e.package.manifest.id == id);
    for p in docs_plugins::plugins() {
        if !keep(&p.manifest.id) {
            docs_plugins::remove(&p.manifest.id);
        }
    }
    for e in list {
        let m = e.package.manifest.clone();
        let id = m.id.clone();
        let r = suite_core::plugins::read_wasm(&e.package)
            .and_then(|w| host.load(&w, m.permissions.memory_mb, ROUND).map_err(|e| e.to_string()))
            .and_then(|g| {
                let mut p = Plugin::new(m, Arc::new(Wasm(g)));
                p.fetcher = Some(Arc::new(Granted(Net::new(e.grant.clone()))));
                p.origins = e.grant;
                docs_plugins::add(p)
            });
        match r {
            Ok(()) => ids.push(id),
            Err(why) => eprintln!("plugins: {id}: {why}"),
        }
    }
    ids
}

/// Once per process: load the user's plugins on a thread (the commands appear on a later frame). Not in an isolated run
/// (`WORDCRAFT_NO_PREFS`, the converter's sandboxed CLI) and not in this crate's own tests.
pub fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if std::env::var_os("WORDCRAFT_NO_PREFS").is_some() {
            return;
        }
        let Some(store) = Store::open() else { return };
        let _ = std::thread::Builder::new().name("plugin-load".into()).spawn(move || {
            load(&store);
        });
    });
}
