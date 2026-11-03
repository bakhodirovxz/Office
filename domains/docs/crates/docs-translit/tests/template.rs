//! Phase 7 WP11 (docs/plugins.md section 11), the done-check of the SDK template on the native host: the real `plugins/template`
//! module in wasmtime registers its command, the command opens the panel, and the panel's button shows the status line "Hello".
//! The module comes from `cargo xt plugin build template`; `TEMPLATE_PLUGIN_WASM` names the file (ci.sh sets it).
//! To load another plugin (the doc test's "hello"), also set `TEMPLATE_PLUGIN_MANIFEST` (its plugin.toml) and
//! `TEMPLATE_PLUGIN_COMMAND` (the command id): the test then only checks that the command runs and shows a status line.
use std::sync::Arc;
use std::time::Duration;

use docs_plugins::{Guest, GuestSession, Plugin, exec};
use plugin_api::wire::Message;
use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_engine::Session;

struct Wasm(plugin_host::Plugin);
struct Sess(plugin_host::Session);

impl Guest for Wasm {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        self.0
            .start()
            .map(|s| Box::new(Sess(s)) as Box<dyn GuestSession>)
            .map_err(|e| e.to_string())
    }
}

impl GuestSession for Sess {
    fn round(&mut self, call: &Message) -> Result<Message, String> {
        self.0.round(call).map_err(|e| e.to_string())
    }
}

#[test]
fn the_template_loads_runs_its_command_and_shows_hello() {
    let Some(path) = std::env::var_os("TEMPLATE_PLUGIN_WASM") else {
        eprintln!(
            "TEMPLATE_PLUGIN_WASM is not set: run `cargo xt plugin build template`; test skipped"
        );
        return;
    };
    let own = std::env::var_os("TEMPLATE_PLUGIN_MANIFEST");
    let text = match &own {
        Some(m) => std::fs::read_to_string(m).unwrap(),
        None => include_str!("../../../../../plugins/template/plugin.toml").to_string(),
    };
    let manifest = plugin_api::manifest::parse(&text).unwrap();
    let id = manifest.id.clone();
    let command = std::env::var("TEMPLATE_PLUGIN_COMMAND").unwrap_or_else(|_| "hello".into());
    let wasm = std::fs::read(path).unwrap();
    let plugin = plugin_host::Host::new()
        .unwrap()
        .load(
            &wasm,
            manifest.permissions.memory_mb,
            Duration::from_secs(5),
        )
        .unwrap();
    docs_plugins::clear();
    docs_plugins::add(Plugin::new(manifest, Arc::new(Wasm(plugin)))).unwrap();
    let mut s = Session::new(Document::from_text("Dear\nEnd"));
    docs_plugins::register(&mut s, "en");
    let p = docs_plugins::plugins()
        .into_iter()
        .find(|p| p.manifest.id == id)
        .unwrap();

    s.run(&format!("plugin.{id}.{command}"), &json!({}))
        .unwrap();
    if own.is_some() {
        assert!(!s.status.is_empty(), "the command showed no status line");
        return;
    }
    assert!(p.ui.lock().unwrap().panels.contains_key("main"));
    exec::ui_event(&mut s, &p, "again", None).unwrap();
    assert_eq!(s.status, "Hello");
}
