//! Spike 3 (A2/A3): IME kill-test probe. One multiline `TextEdit` plus a raw-event log.
//!
//! Run 1 ("stock"): shim OFF. Only what winit 0.30.13 hands to egui reaches the `TextEdit`.
//! Run 2 ("shim"): shim ON. `raw_input_hook` polls GameActivity's `TextInputState` and turns
//! its changes into `egui::ImeEvent::{Preedit, Commit, DeleteSurrounding}` / `Key::Enter`.
//!
//! The mode and the renderer are chosen by files in the app's internal data dir so the same APK
//! serves every run (`run-as com.office.probe sh -c 'echo on > files/shim'`).
#![cfg(target_os = "android")]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use android_activity::input::{TextInputState, TextSpan};
use android_activity::AndroidApp;
use eframe::egui::{self, Event, ImeEvent, Key, Modifiers};
use eframe::{NativeOptions, Renderer};

const MAX_LOG: usize = 400;

// ---------------------------------------------------------------------------------------------
// The shim (everything between the BEGIN/END markers is the A3 deliverable; its size is counted).
// ---------------------------------------------------------------------------------------------
// BEGIN SHIM

/// GameTextInput state with every index converted from Java UTF-16 code units to `char` indices.
#[derive(Clone, PartialEq, Debug, Default)]
struct Mirror {
    text: String,
    sel: (usize, usize),
    comp: Option<(usize, usize)>,
}

fn u16_to_char(text: &str, idx16: usize) -> usize {
    let (mut units, mut chars) = (0, 0);
    for c in text.chars() {
        if units >= idx16 {
            break;
        }
        units += c.len_utf16();
        chars += 1;
    }
    chars
}

fn char_to_u16(text: &str, idx: usize) -> usize {
    text.chars().take(idx).map(char::len_utf16).sum()
}

fn mirror_from(s: &TextInputState) -> Mirror {
    // android-activity hands over Java's UTF-16 indices unchanged, but `text` is a Rust String.
    Mirror {
        text: s.text.clone(),
        sel: (u16_to_char(&s.text, s.selection.start), u16_to_char(&s.text, s.selection.end)),
        comp: s
            .compose_region
            .map(|c| (u16_to_char(&s.text, c.start), u16_to_char(&s.text, c.end)))
            .filter(|c| c.0 < c.1), // GameTextInput reports an empty region (0,0) when nothing is composing
    }
}

fn mirror_to(m: &Mirror) -> TextInputState {
    let t = &m.text;
    TextInputState {
        text: t.clone(),
        selection: TextSpan { start: char_to_u16(t, m.sel.0), end: char_to_u16(t, m.sel.1) },
        compose_region: m.comp.map(|c| TextSpan { start: char_to_u16(t, c.0), end: char_to_u16(t, c.1) }),
    }
}

fn slice(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}

/// (committed text without the composing region, cursor in that text, preedit)
fn split(m: &Mirror) -> (String, usize, String) {
    let n = m.text.chars().count();
    match m.comp {
        Some((a, b)) if a < b && b <= n => {
            (format!("{}{}", slice(&m.text, 0, a), slice(&m.text, b, n)), a, slice(&m.text, a, b))
        }
        _ => (m.text.clone(), m.sel.1.min(n), String::new()),
    }
}

fn enter() -> Vec<Event> {
    [true, false]
        .iter()
        .map(|&pressed| Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

/// Translate the transition `old -> new` of the IME's editable into egui events.
/// `None` means the edit is not a plain "change before/after the cursor" and the caller must resync.
fn translate(old: &Mirror, new: &Mirror) -> Option<Vec<Event>> {
    let (co, cur_o, pre_o) = split(old);
    let (cn, cur_n, pre_n) = split(new);
    let (bo, ao) = (slice(&co, 0, cur_o), slice(&co, cur_o, usize::MAX));
    let (bn, an) = (slice(&cn, 0, cur_n), slice(&cn, cur_n, usize::MAX));
    let mut ev = Vec::new();
    let preedit = |t: String| Event::Ime(ImeEvent::Preedit { text: t, active_range_chars: None });
    if bo == bn && ao == an {
        if pre_o != pre_n {
            ev.push(preedit(pre_n)); // composition grew, shrank or ended without commit
        }
        return Some(ev);
    }
    if ao != an {
        return None;
    }
    if !pre_o.is_empty() {
        ev.push(preedit(String::new())); // drop egui's inline preedit before editing around it
    }
    let p = bo.chars().zip(bn.chars()).take_while(|(x, y)| x == y).count();
    let deleted = bo.chars().count() - p;
    if deleted > 0 {
        ev.push(Event::Ime(ImeEvent::DeleteSurrounding { before_chars: deleted, after_chars: 0 }));
    }
    let ins = slice(&bn, p, usize::MAX);
    if ins == "\n" {
        ev.extend(enter()); // egui's TextEdit ignores Commit("\n"); it wants Key::Enter
    } else if !ins.is_empty() {
        ev.push(Event::Ime(ImeEvent::Commit(ins)));
    }
    if !pre_n.is_empty() {
        ev.push(preedit(pre_n));
    }
    Some(ev)
}
// END SHIM

// ---------------------------------------------------------------------------------------------
// Probe app
// ---------------------------------------------------------------------------------------------

struct Probe {
    app: AndroidApp,
    started: Instant,
    text: String,
    log: VecDeque<String>,
    shim: bool,
    renderer: &'static str,
    te_id: egui::Id,
    /// What we believe GameTextInput currently holds and egui agrees with (shim only).
    known: Option<Mirror>,
    last_gti: Option<Mirror>,
    had_focus: bool,
    cfg_dir: std::path::PathBuf,
}

impl Probe {
    fn note(&mut self, s: impl Into<String>) {
        let line = format!("{:>7.2} {}", self.started.elapsed().as_secs_f32(), s.into());
        log::info!(target: "probe", "{line}");
        if self.log.len() >= MAX_LOG {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    fn fmt_event(e: &Event) -> Option<String> {
        Some(match e {
            Event::Text(t) => format!("Text {t:?}"),
            Event::Ime(i) => format!("Ime {i:?}"),
            Event::Key { key, pressed, repeat, .. } => format!("Key {key:?} {} rep={repeat}", if *pressed { "down" } else { "up" }),
            Event::Paste(t) => format!("Paste {t:?}"),
            Event::Copy | Event::Cut => "Copy/Cut".to_string(),
            _ => return None,
        })
    }

    fn egui_cursor(&self, ctx: &egui::Context) -> Option<usize> {
        let st = egui::TextEdit::load_state(ctx, self.te_id)?;
        st.cursor.char_range().map(|r| r.primary.index.0)
    }

    fn set_egui_cursor(&self, ctx: &egui::Context, idx: usize) {
        if let Some(mut st) = egui::TextEdit::load_state(ctx, self.te_id) {
            let c = egui::text::CCursor::new(idx);
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(c)));
            st.store(ctx, self.te_id);
        }
    }

    fn shim_step(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let focused = ctx.memory(|m| m.has_focus(self.te_id));
        if !focused {
            self.had_focus = false;
            return;
        }
        let cur = self.egui_cursor(ctx).unwrap_or_else(|| self.text.chars().count());
        let egui_side = Mirror { text: self.text.clone(), sel: (cur, cur), comp: None };
        let push = |this: &mut Self, why: &str| {
            this.app.set_text_input_state(mirror_to(&egui_side));
            this.note(format!("SHIM push egui->IME ({why}) text={:?} cur={cur}", egui_side.text));
            this.known = Some(egui_side.clone());
        };
        if !self.had_focus {
            self.had_focus = true;
            push(self, "focus");
            return;
        }
        let known = self.known.clone().unwrap_or_default();
        let composing = known.comp.is_some();
        // egui changed the text/cursor by itself (tap, hardware key, winit Key event): IME must follow.
        if self.text != known.text || (!composing && cur != known.sel.1) {
            push(self, "egui changed");
            return;
        }
        let now = mirror_from(&self.app.text_input_state());
        if now == known {
            return;
        }
        match translate(&known, &now) {
            Some(ev) => {
                for e in &ev {
                    if let Some(s) = Self::fmt_event(e) {
                        self.note(format!("shim> {s}"));
                    }
                }
                raw.events.extend(ev);
                self.known = Some(now.clone());
                if known.text == now.text && known.comp == now.comp && now.comp.is_none() {
                    self.set_egui_cursor(ctx, now.sel.1); // pure cursor move by the IME
                }
            }
            None => {
                self.note("SHIM unhandled diff; resync IME from egui");
                push(self, "resync");
            }
        }
    }
}

impl eframe::App for Probe {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // Evidence: what winit gave egui (before the shim adds anything).
        let lines: Vec<String> = raw.events.iter().filter_map(Self::fmt_event).collect();
        for l in lines {
            self.note(format!("winit> {l}"));
        }
        // Evidence: what GameTextInput itself holds (visible in both modes, delivered to egui only with the shim).
        let gti = mirror_from(&self.app.text_input_state());
        if self.last_gti.as_ref() != Some(&gti) {
            self.note(format!("GTI text={:?} sel={:?} comp={:?}", gti.text, gti.sel, gti.comp));
            self.last_gti = Some(gti);
        }
        if self.shim {
            self.shim_step(ctx, raw);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(33)); // polling is how the shim sees GTI
        ui.add_space(44.0); // below the status bar (the surface is edge to edge)
        egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let mut s = self.shim;
            if ui.checkbox(&mut s, "shim").changed() {
                self.shim = s;
                self.known = None;
                self.had_focus = false;
                let _ = std::fs::write(self.cfg_dir.join("shim"), if s { "on" } else { "off" });
                self.note(format!("shim -> {s}"));
            }
            ui.label(format!("renderer: {}", self.renderer));
            if ui.button("clear log").clicked() {
                self.log.clear();
            }
            if ui.button("clear text").clicked() {
                self.text.clear();
            }
        });
        let te = egui::TextEdit::multiline(&mut self.text)
            .id(self.te_id)
            .desired_width(f32::INFINITY)
            .desired_rows(4)
            .lock_focus(true)
            .hint_text("tap here");
        ui.add(te);
        ui.label(egui::RichText::new(format!("text = {:?}", self.text)).monospace().size(11.0));
        egui::ScrollArea::vertical().stick_to_bottom(true).max_height(ui.available_height().min(380.0)).show(ui, |ui| {
            for l in &self.log {
                ui.label(egui::RichText::new(l).monospace().size(10.0));
            }
        });
        });
    }
}

fn read_cfg(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).map(|s| s.trim().to_string()).unwrap_or_default()
}

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("probe"),
    );
    let cfg_dir = app.internal_data_path().unwrap_or_else(|| "/data/local/tmp".into());
    let shim = read_cfg(&cfg_dir, "shim") == "on";
    let r = read_cfg(&cfg_dir, "renderer"); // "wgpu" (default) or "glow"
    let backend = read_cfg(&cfg_dir, "backend"); // optional WGPU_BACKEND value, e.g. "gl"
    if !backend.is_empty() {
        std::env::set_var("WGPU_BACKEND", &backend);
    }
    let (renderer, rname) = if r == "glow" { (Renderer::Glow, "glow") } else { (Renderer::Wgpu, "wgpu") };
    log::info!(target: "probe", "start shim={shim} renderer={rname} backend={backend:?}");

    if shim {
        // Tell the IME this is a multiline, autocorrecting, sentence-capitalising text field.
        use android_activity::input::{ImeOptions, InputType, TextInputAction};
        app.set_ime_editor_info(
            InputType::TYPE_CLASS_TEXT
                | InputType::TYPE_TEXT_FLAG_MULTI_LINE
                | InputType::TYPE_TEXT_FLAG_AUTO_CORRECT
                | InputType::TYPE_TEXT_FLAG_CAP_SENTENCES,
            TextInputAction::Unspecified,
            ImeOptions::IME_FLAG_NO_FULLSCREEN | ImeOptions::IME_FLAG_NO_ENTER_ACTION,
        );
    }

    let options = NativeOptions { android_app: Some(app.clone()), renderer, ..Default::default() };
    let res = eframe::run_native(
        "ime-probe",
        options,
        Box::new(move |_cc| {
            let mut p = Probe {
                app,
                started: Instant::now(),
                text: String::new(),
                log: VecDeque::new(),
                shim,
                renderer: rname,
                te_id: egui::Id::new("probe-textedit"),
                known: None,
                last_gti: None,
                had_focus: false,
                cfg_dir,
            };
            p.note(format!("start shim={shim} renderer={rname}"));
            Ok(Box::new(p))
        }),
    );
    if let Err(e) = res {
        log::error!(target: "probe", "run_native failed: {e:?}");
    }
}
