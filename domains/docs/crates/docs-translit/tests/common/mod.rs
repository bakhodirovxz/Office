//! Phase 7 WP6 test glue: the real `plugins/uz-translit` module in the native wasmtime host, as a `docs_plugins::Guest`.
//! The module comes from `cargo xt plugin build uz-translit`; `UZ_TRANSLIT_PLUGIN_WASM` names the file (ci.sh sets it).
#![allow(dead_code)]
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::Instant;

use docs_plugins::{Guest, GuestSession};
use plugin_api::wire::Message;

pub const MANIFEST: &str = include_str!("../../../../../../plugins/uz-translit/plugin.toml");

/// Nanoseconds spent in `start` (instantiate) and in `round` (copy in, run, copy out) since the last `take_guest_ns`.
static START_NS: AtomicU64 = AtomicU64::new(0);
static ROUND_NS: AtomicU64 = AtomicU64::new(0);

/// (instantiate ms, rounds ms) accumulated since the last call.
pub fn take_guest_ms() -> (f64, f64) {
    (
        START_NS.swap(0, Ordering::Relaxed) as f64 / 1e6,
        ROUND_NS.swap(0, Ordering::Relaxed) as f64 / 1e6,
    )
}

struct Wasm(plugin_host::Plugin);
struct Sess(plugin_host::Session);

impl Guest for Wasm {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        let t = Instant::now();
        let r = self
            .0
            .start()
            .map(|s| Box::new(Sess(s)) as Box<dyn GuestSession>)
            .map_err(|e| e.to_string());
        START_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
}

impl GuestSession for Sess {
    fn round(&mut self, call: &Message) -> Result<Message, String> {
        let t = Instant::now();
        let r = self.0.round(call).map_err(|e| e.to_string());
        ROUND_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
}

/// The plugin module's bytes, or `None` (with a note on stderr) when no build was named.
pub fn wasm() -> Option<Vec<u8>> {
    let Some(p) = std::env::var_os("UZ_TRANSLIT_PLUGIN_WASM") else {
        eprintln!(
            "UZ_TRANSLIT_PLUGIN_WASM is not set: run `cargo xt plugin build uz-translit` and point it at the .wasm; test skipped"
        );
        return None;
    };
    Some(std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.to_string_lossy())))
}

/// Compile the module once and register it as the plugin `uz-translit` (replacing any other plugin).
pub fn install(wasm: &[u8]) {
    let manifest = plugin_api::manifest::parse(MANIFEST).unwrap();
    let host = plugin_host::Host::new().unwrap();
    let plugin = host
        .load(wasm, manifest.permissions.memory_mb, Duration::from_secs(5))
        .unwrap();
    docs_plugins::clear();
    docs_plugins::add(docs_plugins::Plugin::new(manifest, Arc::new(Wasm(plugin)))).unwrap();
}
