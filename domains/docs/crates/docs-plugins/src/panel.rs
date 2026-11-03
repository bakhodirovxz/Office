//! The desktop side panel and the text context menu items (PLAN section 4.5), drawn through the carried hook `wordcraft/0147`.
//!
//! The panel is an egui side panel with the fixed widget list of `plugin_api::action::Widget`; every string is drawn as plain text
//! (no markup, no HTML), already limited by `check_widgets`. A click or an edit becomes a `ui` invocation of the plugin.
use plugin_api::action::Widget;
use serde_json::{Value, json};
use wordcraft_engine::Session;

use crate::{Plugin, exec, localized, plugins};

enum Ev {
    Click(String),
    Edit(String, String),
}

fn draw(ui: &mut egui::Ui, plugin: &Plugin, widgets: &[Widget], fields: &mut std::collections::BTreeMap<String, String>) -> Vec<Ev> {
    let mut evs = Vec::new();
    for w in widgets {
        match w {
            Widget::Heading { text } => {
                ui.heading(text);
            }
            Widget::Text { text } => {
                ui.label(text);
            }
            Widget::List { items } => {
                for i in items {
                    ui.label(format!("\u{2022} {}", i.text));
                }
            }
            Widget::Button { id, label } => {
                if ui.button(label).clicked() {
                    evs.push(Ev::Click(id.clone()));
                }
            }
            Widget::TextField { id, value, placeholder } => {
                let buf = fields.entry(format!("{}/{id}", plugin.manifest.id)).or_insert_with(|| value.clone());
                let mut te = egui::TextEdit::singleline(buf);
                if let Some(h) = placeholder {
                    te = te.hint_text(h.as_str());
                }
                let r = ui.add(te);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    evs.push(Ev::Edit(id.clone(), buf.clone()));
                }
            }
            Widget::Select { id, options, value } => {
                let key = format!("{}/{id}", plugin.manifest.id);
                let cur = fields.get(&key).cloned().or_else(|| value.clone()).unwrap_or_default();
                let shown = options.iter().find(|o| o.value == cur).map(|o| o.label.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt(&key).selected_text(shown).show_ui(ui, |ui| {
                    for o in options {
                        if ui.selectable_label(o.value == cur, &o.label).clicked() {
                            fields.insert(key.clone(), o.value.clone());
                            evs.push(Ev::Edit(id.clone(), o.value.clone()));
                        }
                    }
                });
            }
        }
    }
    evs
}

/// Draw the side panels of the enabled plugins that have shown widgets. The hook calls it once per frame inside the pane area.
pub fn panel(s: &mut Session, ui: &mut egui::Ui) {
    let lang = crate::lang();
    for p in plugins().into_iter().filter(|p| p.enabled()) {
        // Copy the state out, so that a `ui` invocation (which locks it again) never runs under the lock.
        let (shown, mut fields): (Vec<(String, Vec<Widget>)>, _) = {
            let st = p.ui.lock().unwrap_or_else(|e| e.into_inner());
            (st.panels.iter().map(|(k, v)| (k.clone(), v.clone())).collect(), st.fields.clone())
        };
        let mut evs = Vec::new();
        for (id, widgets) in &shown {
            let title = p.manifest.panels.iter().find(|x| &x.id == id).map(|x| localized(&x.title, &lang)).unwrap_or_default();
            egui::Panel::right(format!("plugin_panel_{}_{id}", p.manifest.id)).default_size(260.0).resizable(true).show(ui, |ui| {
                ui.heading(&title);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| evs.extend(draw(ui, &p, widgets, &mut fields)));
            });
        }
        p.ui.lock().unwrap_or_else(|e| e.into_inner()).fields = fields;
        for e in evs {
            let _ = match e {
                Ev::Click(id) => exec::ui_event(s, &p, &id, None),
                Ev::Edit(id, v) => exec::ui_event(s, &p, &id, Some(&v)),
            };
        }
    }
}

/// The plugin items of the text context menu. Returns the command the user clicked, for the caller to run (and close the menu).
pub fn context_items(s: &Session, ui: &mut egui::Ui) -> Option<(String, Value)> {
    let items = crate::context_commands(&crate::lang());
    if items.is_empty() {
        return None;
    }
    ui.separator();
    for (id, label) in items {
        let on = s.registry.get(id).is_some_and(|c| (c.enabled)(s).is_none());
        if ui.add_enabled(on, egui::Button::new(label)).clicked() {
            return Some((id.to_string(), json!({})));
        }
    }
    None
}
