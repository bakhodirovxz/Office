//! The browser end of the Sheets and Slides plugin bridge (Phase 7 WP13; `docs-office::plugin_web` with the app's prefix): the
//! `<prefix>:plugin*` messages of the host page (`web/plugins/host.js`) turned into calls of [`crate::bridge`]. The carried patches of
//! the web apps (gridcraft-web, deckcraft-web) only forward the messages here and post what comes back.
//!
//! Page to editor: `plugins` (the plugins the page hosts, whole list) and `pluginActions`. Editor to page: `pluginsSet`, `pluginCall`
//! (the command's reads are transferred `ArrayBuffer`s), `pluginResults`. Sheets and Slides send no `pluginEvent` in v1.
use std::cell::RefCell;

use js_sys::{Array, ArrayBuffer, JSON, Object, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};

use crate::Editor;
use crate::bridge::{self, Answer, Out, Registration};

thread_local! {
    static INBOX: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
}

/// Is `data` a plugin message of the app with this prefix (`gridcraft`, `deckcraft`)?
pub fn is_plugin_message(prefix: &str, data: &JsValue) -> bool {
    get(data, "type").as_string().is_some_and(|t| {
        t.strip_prefix(prefix)
            .is_some_and(|r| r.starts_with(":plugin"))
    })
}

/// A plugin message arrived (queued until the next frame has a session to run it against).
pub fn incoming(data: &JsValue) {
    INBOX.with(|i| i.borrow_mut().push(data.clone()));
}

fn get(data: &JsValue, key: &str) -> JsValue {
    Reflect::get(data, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn json(v: &JsValue) -> Value {
    if v.is_undefined() {
        return Value::Null;
    }
    JSON::stringify(v)
        .ok()
        .and_then(|t| serde_json::from_str(&String::from(t)).ok())
        .unwrap_or(Value::Null)
}

fn js(v: &Value) -> JsValue {
    JSON::parse(&v.to_string()).unwrap_or(JsValue::NULL)
}

fn buffers(list: &JsValue) -> Vec<Vec<u8>> {
    if !list.is_instance_of::<Array>() {
        return Vec::new();
    }
    Array::from(list)
        .iter()
        .map(|b| match b.dyn_ref::<Uint8Array>() {
            Some(a) => a.to_vec(),
            None => Uint8Array::new(&b).to_vec(),
        })
        .collect()
}

fn object(fields: Vec<(&str, JsValue)>) -> Object {
    let m = Object::new();
    for (k, v) in fields {
        let _ = Reflect::set(&m, &JsValue::from_str(k), &v);
    }
    m
}

fn buffer(b: &[u8]) -> ArrayBuffer {
    Uint8Array::from(b).buffer()
}

fn set(prefix: &str, data: &JsValue, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let v = json(data);
    let list = v["plugins"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| Registration {
                    manifest: p["manifest"].clone(),
                    settings: p["settings"].as_object().cloned(),
                    enabled: p["enabled"].as_bool().unwrap_or(true),
                })
                .collect()
        })
        .unwrap_or_default();
    let (ids, errors) = bridge::set_plugins(list, v["timeoutMs"].as_f64());
    let strings = |l: Vec<String>| l.iter().map(|x| JsValue::from_str(x)).collect::<Array>();
    let m = object(vec![
        ("type", format!("{prefix}:pluginsSet").into()),
        ("ids", strings(ids).into()),
        ("errors", strings(errors).into()),
    ]);
    post(&m, &Array::new());
}

fn answer<E: Editor>(ed: &mut E, data: &JsValue) {
    let call = json(&get(data, "call"));
    if !(call.is_number() || call.is_string()) {
        return;
    }
    bridge::answer(
        ed,
        Answer {
            call,
            plugin: get(data, "plugin").as_string(),
            actions: json(&get(data, "actions")),
            blobs: buffers(&get(data, "blobs")),
            reads: json(&get(data, "reads")),
            more: get(data, "more").as_bool().unwrap_or(false),
            abort: get(data, "abort").as_bool().unwrap_or(false),
        },
    );
}

fn outgoing(prefix: &str, o: Out, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let transfer = Array::new();
    let m = match o {
        Out::Call {
            call,
            plugin,
            command,
            params,
            reads,
        } => {
            let r = Object::new();
            for (name, bytes) in reads {
                let b = buffer(&bytes);
                transfer.push(&b);
                let _ = Reflect::set(&r, &JsValue::from_str(name), &b);
            }
            object(vec![
                ("type", format!("{prefix}:pluginCall").into()),
                ("call", js(&call)),
                ("plugin", plugin.as_str().into()),
                ("command", command.as_str().into()),
                ("params", js(&Value::Object(params))),
                ("reads", r.into()),
            ])
        }
        Out::Results {
            call,
            results,
            reads,
            error,
        } => {
            let list = Array::new();
            for bytes in reads {
                let b = buffer(&bytes);
                transfer.push(&b);
                list.push(&b);
            }
            let mut f = vec![
                ("type", format!("{prefix}:pluginResults").into()),
                ("call", js(&call)),
                ("results", js(&Value::Array(results))),
                ("reads", list.into()),
            ];
            if let Some(e) = error {
                f.push(("error", e.as_str().into()));
            }
            object(f)
        }
    };
    post(&m, &transfer);
}

/// After each frame: run the queued messages, then send what the bridge has for the page. `post(message, transfer)` posts to the
/// host page. Returns (active, status lines for the editor's UI); active while an invocation is pending or something just
/// happened, so the caller asks for another frame. The caller binds and registers new slots afterwards (the plugin set may change).
pub fn sync<E: Editor>(
    ed: &mut E,
    prefix: &str,
    post: &dyn Fn(&JsValue, &Array) -> bool,
) -> (bool, Vec<String>) {
    let inbox = INBOX.with(|i| std::mem::take(&mut *i.borrow_mut()));
    let mut active = !inbox.is_empty();
    for data in inbox {
        let kind = get(&data, "type").as_string().unwrap_or_default();
        match kind.strip_prefix(prefix).unwrap_or_default() {
            ":plugins" => set(prefix, &data, post),
            ":pluginActions" => answer(ed, &data),
            _ => {}
        }
    }
    active |= bridge::tick(ed);
    for o in bridge::take_out() {
        active = true;
        outgoing(prefix, o, post);
    }
    (active, bridge::take_status())
}
