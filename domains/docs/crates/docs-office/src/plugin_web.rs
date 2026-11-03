//! The browser end of the plugin bridge (Phase 7 WP4; docs/plugins.md "The web bridge"): the `wordcraft:plugin*` messages of the host
//! page, turned into calls of `docs_plugins::bridge`. The carried patch `wordcraft/0148` only forwards the messages here and posts what
//! comes back, so the protocol lives in our crate and not in the upstream file.
//!
//! Page to editor: `wordcraft:plugins` (the whole list of plugins, see [`docs_plugins::bridge::set_plugins`]) and
//! `wordcraft:pluginActions` (the plugin's answer or why there is none). Editor to page: `wordcraft:pluginsSet`,
//! `wordcraft:pluginCall` (a transferred `ArrayBuffer` per blob) and `wordcraft:pluginDone`.
use std::cell::RefCell;

use docs_plugins::bridge::{self, Out, Remote};
use js_sys::{Array, ArrayBuffer, JSON, Object, Reflect, Uint8Array};
use plugin_api::wire::{MAX_BLOBS, MAX_HEADER, MAX_MESSAGE, Message};
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

fn number(data: &JsValue, key: &str) -> Option<u64> {
    get(data, key).as_f64().filter(|n| *n >= 0.0 && n.fract() == 0.0).map(|n| n as u64)
}

fn blobs(data: &JsValue) -> Option<Vec<Vec<u8>>> {
    let list = get(data, "blobs");
    if list.is_undefined() || list.is_null() {
        return Some(Vec::new());
    }
    let list: Array = list.dyn_into().ok()?;
    if list.length() as usize > MAX_BLOBS {
        return None;
    }
    list.iter()
        .map(|b| {
            if let Some(a) = b.dyn_ref::<Uint8Array>() {
                Some(a.to_vec())
            } else if b.is_instance_of::<ArrayBuffer>() {
                Some(Uint8Array::new(&b).to_vec())
            } else {
                None
            }
        })
        .collect()
}

/// The plugin's answer from the page: `{call, rev?, header: {actions, next?}, blobs?}`, or `{call, error}`.
fn answer(s: &mut Session, data: &JsValue) {
    let Some(call) = number(data, "call") else { return };
    let rev = number(data, "rev");
    if let Some(e) = get(data, "error").as_string() {
        return bridge::answer(s, call, rev, Err(e));
    }
    let msg = (|| {
        let header: String = JSON::stringify(&get(data, "header")).ok()?.into();
        let blobs = blobs(data)?;
        let size = header.len() + blobs.iter().map(Vec::len).sum::<usize>();
        (header.len() <= MAX_HEADER && size <= MAX_MESSAGE).then_some(Message { header, blobs })
    })();
    bridge::answer(s, call, rev, msg.ok_or_else(|| "the page sent an answer that is not a message".to_string()));
}

fn set(data: &JsValue, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let v: Value = JSON::stringify(data).ok().and_then(|t| serde_json::from_str(&String::from(t)).ok()).unwrap_or(Value::Null);
    let list = v["plugins"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| Remote {
                    manifest: p["manifest"].as_str().unwrap_or_default().to_string(),
                    settings: p["settings"].as_object().cloned(),
                    origins: p["origins"].as_array().map(|o| o.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    enabled: p["enabled"].as_bool().unwrap_or(true),
                })
                .collect()
        })
        .unwrap_or_default();
    let (ids, errors) = bridge::set_plugins(list, v["timeoutMs"].as_f64());
    let strings = |l: Vec<String>| l.iter().map(|x| JsValue::from_str(x)).collect::<Array>();
    let m = Object::new();
    for (k, val) in [("type", JsValue::from_str("wordcraft:pluginsSet")), ("ids", strings(ids).into()), ("errors", strings(errors).into())] {
        let _ = Reflect::set(&m, &JsValue::from_str(k), &val);
    }
    post(&m, &Array::new());
}

fn outgoing(o: Out, post: &dyn Fn(&JsValue, &Array) -> bool) {
    let m = Object::new();
    let transfer = Array::new();
    let mut put = |k: &str, v: JsValue| {
        let _ = Reflect::set(&m, &JsValue::from_str(k), &v);
    };
    match o {
        Out::Call { call, plugin, rev, round, header, blobs } => {
            put("type", "wordcraft:pluginCall".into());
            put("call", (call as f64).into());
            put("plugin", plugin.as_str().into());
            put("rev", (rev as f64).into());
            put("round", round.into());
            put("header", JSON::parse(&header).unwrap_or(JsValue::NULL));
            let list = Array::new();
            for b in blobs {
                let buf = Uint8Array::from(b.as_slice()).buffer();
                transfer.push(&buf);
                list.push(&buf);
            }
            put("blobs", list.into());
        }
        Out::Done { call, plugin, error } => {
            put("type", "wordcraft:pluginDone".into());
            put("call", (call as f64).into());
            put("plugin", plugin.as_str().into());
            put("ok", error.is_none().into());
            put("error", error.as_deref().unwrap_or("").into());
        }
    }
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
