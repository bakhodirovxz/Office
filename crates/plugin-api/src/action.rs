//! Calls, answers, actions and widgets (PLAN phase 7, section 4.3) and the permission check of an action list.
//!
//! The host's call is read leniently by a plugin (unknown fields are ignored, section 4.7). The plugin's answer is read
//! strictly by the host: an unknown action or field refuses the whole invocation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::allow;
use crate::manifest::{DocAccess, check_origin};

pub const MAX_ACTIONS: usize = 256;
pub const MAX_WIDGETS: usize = 64;
const MAX_UI_TEXT: usize = 1024;
const MAX_STATUS: usize = 256;
const MAX_HEADERS: usize = 16;
const MAX_BODY: usize = 1 << 20;
const MAX_URL: usize = 2048;
const MAX_ITEMS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CallKind {
    Command,
    Ui,
    Event,
    Resume,
}

/// Host to plugin (the JSON header of a call message).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub kind: CallKind,
    pub api: u32,
    pub api_minor: u32,
    pub app: String,
    pub lang: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// `ui`: the widget id that was clicked or changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    /// `event`: the event name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub settings: Map<String, Value>,
    /// `resume`: one result per action of the previous round, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<Value>,
    /// What was read, to the index of the blob that holds it (`textmap`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reads: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadWhat {
    Textmap,
    Selection,
    Paragraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Auto,
    Selection,
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadReq {
    pub what: ReadWhat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    /// `paragraph`: the paragraph index of the text map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// The fixed widget list of a panel (section 4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Widget {
    Heading {
        text: String,
    },
    Text {
        text: String,
    },
    List {
        items: Vec<ListItem>,
    },
    Button {
        id: String,
        label: String,
    },
    TextField {
        id: String,
        #[serde(default)]
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },
    Select {
        id: String,
        options: Vec<SelectOption>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Action {
    #[serde(rename = "run")]
    Run {
        id: String,
        #[serde(default)]
        params: Value,
    },
    #[serde(rename = "textmap.apply")]
    TextmapApply { blob: u32 },
    #[serde(rename = "read")]
    Read(ReadReq),
    #[serde(rename = "fetch")]
    Fetch {
        url: String,
        #[serde(default = "get")]
        method: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
        #[serde(default)]
        body: Option<String>,
    },
    #[serde(rename = "ui.panel")]
    UiPanel { panel: String, widgets: Vec<Widget> },
    #[serde(rename = "ui.status")]
    UiStatus { text: String },
}

fn get() -> String {
    "GET".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Next {
    pub reads: Vec<ReadReq>,
}

/// Plugin to host (the JSON header of an answer message).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Next>,
}

/// What the host has granted this plugin for this invocation.
pub struct Grants<'a> {
    pub document: DocAccess,
    /// Granted network origins (the manifest's, minus what nobody granted).
    pub origins: &'a [String],
    /// Panel ids the manifest declares.
    pub panels: &'a [String],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denied {
    TooManyActions,
    /// An `event` invocation may only run `ui.*` actions (decision D8).
    EventOnlyUi,
    NeedsDocument {
        need: DocAccess,
    },
    Command {
        id: String,
        why: &'static str,
    },
    Network(String),
    UnknownPanel(String),
    Invalid(&'static str),
}

fn spoof(c: char, allow_newline: bool) -> bool {
    (c.is_control() && !(allow_newline && c == '\n'))
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

fn ui_text(s: &str, max: usize, newline: bool) -> Result<(), Denied> {
    if s.chars().count() > max || s.chars().any(|c| spoof(c, newline)) {
        return Err(Denied::Invalid(
            "widget text too long or holds control/bidi characters",
        ));
    }
    Ok(())
}

fn ui_id(s: &str) -> Result<(), Denied> {
    if s.is_empty()
        || s.len() > 48
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Denied::Invalid(
            "widget id is empty, too long or has other characters than letters, digits, _ and -",
        ));
    }
    Ok(())
}

/// Check the widget list of one `ui.panel`.
pub fn check_widgets(ws: &[Widget]) -> Result<(), Denied> {
    if ws.len() > MAX_WIDGETS {
        return Err(Denied::Invalid("more than 64 widgets"));
    }
    for w in ws {
        match w {
            Widget::Heading { text } => ui_text(text, MAX_UI_TEXT, false)?,
            Widget::Text { text } => ui_text(text, MAX_UI_TEXT, true)?,
            Widget::List { items } => {
                if items.len() > MAX_ITEMS {
                    return Err(Denied::Invalid("more than 256 list items"));
                }
                for i in items {
                    ui_text(&i.text, MAX_UI_TEXT, false)?;
                    if let Some(id) = &i.id {
                        ui_id(id)?;
                    }
                }
            }
            Widget::Button { id, label } => {
                ui_id(id)?;
                ui_text(label, MAX_UI_TEXT, false)?;
            }
            Widget::TextField {
                id,
                value,
                placeholder,
            } => {
                ui_id(id)?;
                ui_text(value, MAX_UI_TEXT, false)?;
                if let Some(p) = placeholder {
                    ui_text(p, MAX_UI_TEXT, false)?;
                }
            }
            Widget::Select { id, options, value } => {
                ui_id(id)?;
                if options.len() > MAX_ITEMS {
                    return Err(Denied::Invalid("more than 256 select options"));
                }
                for o in options {
                    ui_text(&o.value, MAX_UI_TEXT, false)?;
                    ui_text(&o.label, MAX_UI_TEXT, false)?;
                }
                if let Some(v) = value {
                    ui_text(v, MAX_UI_TEXT, false)?;
                }
            }
        }
    }
    Ok(())
}

/// The origin (`https://host[:port]`) of a URL, or `None` for anything the browser and an HTTP client could read
/// differently: userinfo, backslashes, whitespace or control characters, a URL that is not `https`.
pub fn url_origin(url: &str) -> Option<&str> {
    if url.len() > MAX_URL
        || url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let origin = &url[..8 + end];
    if rest[..end].contains('@') || !check_origin(origin) {
        return None;
    }
    Some(origin)
}

const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];
const FORBIDDEN_HEADERS: &[&str] = &[
    "host",
    "cookie",
    "content-length",
    "connection",
    "transfer-encoding",
    "origin",
    "referer",
    "upgrade",
    "te",
    "expect",
];

fn check_fetch(
    url: &str,
    method: &str,
    headers: &BTreeMap<String, String>,
    body: &Option<String>,
    g: &Grants,
) -> Result<(), Denied> {
    let origin = url_origin(url)
        .ok_or_else(|| Denied::Network(format!("`{url}` is not a plain https URL")))?;
    if !g.origins.iter().any(|o| o == origin) {
        return Err(Denied::Network(format!("{origin} is not granted")));
    }
    if !METHODS.contains(&method) {
        return Err(Denied::Invalid(
            "method is not GET, POST, PUT, PATCH or DELETE",
        ));
    }
    if headers.len() > MAX_HEADERS || body.as_ref().is_some_and(|b| b.len() > MAX_BODY) {
        return Err(Denied::Invalid("too many headers or body over 1 MiB"));
    }
    for (k, v) in headers {
        let l = k.to_ascii_lowercase();
        if k.is_empty()
            || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || FORBIDDEN_HEADERS.contains(&l.as_str())
            || l.starts_with("proxy-")
            || l.starts_with("sec-")
        {
            return Err(Denied::Invalid("a request header name is not allowed"));
        }
        if v.len() > 4096 || v.chars().any(|c| c.is_control()) {
            return Err(Denied::Invalid(
                "a request header value is too long or holds control characters",
            ));
        }
    }
    Ok(())
}

fn need(g: &Grants, need: DocAccess) -> Result<(), Denied> {
    if g.document >= need {
        Ok(())
    } else {
        Err(Denied::NeedsDocument { need })
    }
}

/// Check every action of one answer before the host runs the first. `from_event`: the invocation is an event (only `ui.*`).
pub fn check_actions(actions: &[Action], g: &Grants, from_event: bool) -> Result<(), Denied> {
    if actions.len() > MAX_ACTIONS {
        return Err(Denied::TooManyActions);
    }
    for a in actions {
        if from_event && !matches!(a, Action::UiPanel { .. } | Action::UiStatus { .. }) {
            return Err(Denied::EventOnlyUi);
        }
        match a {
            Action::Run { id, params } => {
                let access = allow::check_run(id, params).map_err(|why| Denied::Command {
                    id: id.clone(),
                    why,
                })?;
                need(g, access)?;
            }
            Action::TextmapApply { blob } => {
                need(g, DocAccess::Write)?;
                if *blob as usize >= crate::wire::MAX_BLOBS {
                    return Err(Denied::Invalid("blob index out of range"));
                }
            }
            Action::Read(_) => need(g, DocAccess::Read)?,
            Action::Fetch {
                url,
                method,
                headers,
                body,
            } => check_fetch(url, method, headers, body, g)?,
            Action::UiPanel { panel, widgets } => {
                if !g.panels.iter().any(|p| p == panel) {
                    return Err(Denied::UnknownPanel(panel.clone()));
                }
                check_widgets(widgets)?;
            }
            Action::UiStatus { text } => ui_text(text, MAX_STATUS, false)?,
        }
    }
    Ok(())
}
