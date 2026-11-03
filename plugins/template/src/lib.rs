//! SDK template (docs/plugins.md section 11): the command `hello` opens the panel `main`; its button shows a status line.
//! Copy the folder, rename it and replace the handler. Every call is stateless: a native call starts a fresh instance.
use plugin_sdk::{Action, Call, CallKind, Reply, Widget};

/// `key, en, ru, uz-Latn, uz-Cyrl, status` per line; the first line is the header. Shipped in the package as `strings.tsv`.
const STRINGS: &str = include_str!("../strings.tsv");

/// The text of `key` in the host's language (`call.lang`); English when the language or the key is missing.
fn tr(lang: &str, key: &str) -> String {
    let col = match lang.to_ascii_lowercase().as_str() {
        "ru" => 2,
        "uz-latn" => 3,
        "uz-cyrl" => 4,
        _ => 1,
    };
    STRINGS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|c| c[0] == key)
        .map_or_else(|| key.to_string(), |c| c[col].to_string())
}

pub fn handle(call: Call, _blobs: Vec<Vec<u8>>) -> Result<Reply, String> {
    let lang = &call.lang;
    match (call.kind, call.command.as_deref(), call.widget.as_deref()) {
        (CallKind::Command, Some("hello"), _) => Ok(Reply {
            actions: vec![Action::UiPanel {
                panel: "main".into(),
                widgets: vec![
                    Widget::Heading {
                        text: tr(lang, "title"),
                    },
                    Widget::Button {
                        id: "again".into(),
                        label: tr(lang, "again"),
                    },
                ],
            }],
            ..Default::default()
        }),
        (CallKind::Ui, _, Some("again")) => Ok(Reply::status(tr(lang, "hello"))),
        // An error becomes a status line and changes nothing.
        _ => Err(format!("unexpected call {:?}", call.kind)),
    }
}

plugin_sdk::export!(handle);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(v: serde_json::Value) -> Call {
        let mut base = json!({"api": 1, "api_minor": 0, "app": "docs", "lang": "en"});
        base.as_object_mut()
            .unwrap()
            .extend(v.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn the_command_opens_the_panel_and_the_button_says_hello() {
        let r = handle(call(json!({"kind": "command", "command": "hello"})), vec![]).unwrap();
        assert!(matches!(&r.actions[..], [Action::UiPanel { panel, .. }] if panel == "main"));
        let r = handle(call(json!({"kind": "ui", "widget": "again"})), vec![]).unwrap();
        assert!(matches!(&r.actions[..], [Action::UiStatus { text }] if text == "Hello"));
    }

    #[test]
    fn a_missing_language_falls_back_to_english() {
        assert_eq!(tr("de", "hello"), "Hello");
        assert_eq!(tr("uz-Cyrl", "hello"), "Салом");
    }
}
