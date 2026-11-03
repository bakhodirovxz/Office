//! The browser end of the plugin bridge (Phase 7 WP4; docs/plugins.md sections 9 and 9a): the `wordcraft:plugin*` messages of the host
//! page (`web/plugins/host.js`), turned into calls of `docs_plugins::bridge`. The carried patch `wordcraft/0148` only forwards the
//! messages here and posts what comes back, so the protocol lives in our crate and not in the upstream file.
//!
//! Page to editor: `plugins` (the plugins the page hosts, whole list) and `pluginActions` (the editor actions of one answer).
//! Editor to page: `pluginsSet`, `pluginCall` (the command's reads are transferred `ArrayBuffer`s), `pluginResults`, `pluginEvent`.
use std::cell::RefCell;

use docs_plugins::bridge::{self, Answer, Out, Registration};
use js_sys::{Array, ArrayBuffer, JSON, Object, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wordcraft_engine::Session;

thread_local! {
    static INBOX: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
}

/// A `wordcraft:plugin*` message arrived (queued until the next frame has a session to run it against).
pub fn incoming(data: &JsValue) {
    INBOX.with(|i| i.borrow_mut().push(data.clone()));
}

fn get(data: &JsValue, key: &str) -> JsValue {
    Reflect::get(data, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

/// A JS value as JSON (`Null` for undefined).
fn json(v: &JsValue) -> Value {
    if v.is_undefined() {
        return Value::Null;
    }
    JSON::stringify(v).ok().and_then(|t| serde_json::from_str(&String::from(t)).ok()).unwrap_or(Value::Null)
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

fn set(data: &JsValue, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let v = json(data);
    let list = v["plugins"]
        .as_array()
        .map(|a| a.iter().map(|p| Registration { manifest: p["manifest"].clone(), settings: p["settings"].as_object().cloned(), enabled: p["enabled"].as_bool().unwrap_or(true) }).collect())
        .unwrap_or_default();
    let (ids, errors) = bridge::set_plugins(list, v["timeoutMs"].as_f64());
    let strings = |l: Vec<String>| l.iter().map(|x| JsValue::from_str(x)).collect::<Array>();
    let m = object(vec![("type", "wordcraft:pluginsSet".into()), ("ids", strings(ids).into()), ("errors", strings(errors).into())]);
    post(&m, &Array::new());
}

fn answer(s: &mut Session, data: &JsValue) {
    let call = json(&get(data, "call"));
    if !(call.is_number() || call.is_string()) {
        return;
    }
    bridge::answer(
        s,
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

fn outgoing(o: Out, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let transfer = Array::new();
    let m = match o {
        Out::Call { call, plugin, command, params, reads } => {
            let r = Object::new();
            for (name, bytes) in reads {
                let b = buffer(&bytes);
                transfer.push(&b);
                let _ = Reflect::set(&r, &JsValue::from_str(name), &b);
            }
            object(vec![
                ("type", "wordcraft:pluginCall".into()),
                ("call", js(&call)),
                ("plugin", plugin.as_str().into()),
                ("command", command.as_str().into()),
                ("params", js(&Value::Object(params))),
                ("reads", r.into()),
            ])
        }
        Out::Results { call, results, reads, error } => {
            let list = Array::new();
            for bytes in reads {
                let b = buffer(&bytes);
                transfer.push(&b);
                list.push(&b);
            }
            let mut f = vec![("type", "wordcraft:pluginResults".into()), ("call", js(&call)), ("results", js(&Value::Array(results))), ("reads", list.into())];
            if let Some(e) = error {
                f.push(("error", e.as_str().into()));
            }
            object(f)
        }
        Out::Event { event } => object(vec![("type", "wordcraft:pluginEvent".into()), ("event", event.as_str().into())]),
    };
    post(&m, &transfer);
}

/// After each frame: run the queued messages, then send what the bridge has for the page. `post(message, transfer)` posts to the
/// host page. True while an invocation is pending or something just happened, so the caller asks for another frame: the document
/// the actions changed gets drawn, and a timeout fires without input.
pub fn sync(s: &mut Session, post: &dyn Fn(&JsValue, &Array) -> bool) -> bool {
    let inbox = INBOX.with(|i| std::mem::take(&mut *i.borrow_mut()));
    let mut active = !inbox.is_empty();
    for data in inbox {
        match get(&data, "type").as_string().as_deref() {
            Some("wordcraft:plugins") => set(&data, post),
            Some("wordcraft:pluginActions") => answer(s, &data),
            _ => {}
        }
    }
    active |= bridge::tick(s);
    for o in bridge::take_out() {
        active = true;
        outgoing(o, post);
    }
    active
}
