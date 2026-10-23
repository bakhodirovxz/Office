// Added by the office toolbar patch queue. MIT OR Apache-2.0.
//! The simplified Slides toolbar (PLAN phase 4 WP3, docs/phase4/toolbar-design.md section 5): one row of the controls people
//! use most, drawn instead of `ribbon::show`. The full ribbon stays one click away (desktop); the web build always shows
//! this row. Icons are Fluent UI System Icons (MIT, `assets/icons/fluent`), rasterised at build time into alpha masks.
//!
//! `show` returns whether it drew the row; the call site draws the upstream ribbon otherwise. In the full ribbon mode it
//! only floats the "Simple ribbon" pill over the right end of the tab row.

use std::sync::OnceLock;

use egui::{Align2, Color32, Context, CornerRadius, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use serde_json::{Value, json};

use crate::SlideApp;
use crate::office_overlay;
use crate::ribbon;
use crate::theme::{self, Tokens};
use crate::widgets;

/// The registry and UI command ids the row runs. `office_toolbar_ids_exist` checks them against the live registry at every
/// sync, and that this file still names each one.
pub const IDS: [&str; 20] = [
    "slide.new",
    "slide.layout",
    "format.font",
    "format.size",
    "format.bold",
    "format.italic",
    "format.underline",
    "format.color",
    "format.bullets",
    "format.numbering",
    "format.alignLeft",
    "format.alignCenter",
    "format.alignRight",
    "insert.textBox",
    "app.insertPictureDialog",
    "shape.insert",
    "insert.table",
    "tools.translitCyrl",
    "tools.translitLatn",
    "tools.translitAuto",
];

/// The ids that are ours (not in the engine registry): the history button and the two toggle labels.
pub const OURS: [&str; 3] = ["office.history", "office.ribbonFull", "office.ribbonSimple"];

/// Below this width the row drops its text labels and keeps the icons and tooltips (the 1366 px laptop case).
const LABELS_MIN: f32 = 1480.0;
const ROW_H: f32 = 40.0;
const BTN_H: f32 = 28.0;
const ICON: f32 = 18.0;

/// The Fluent icons this row draws, in the order of `assets/office_icons.bin` (`scripts/gen-slides-icon-bitmaps.sh`; a test checks
/// the names). The SVG loader does not fit the Slides wasm budget (docs/phase4/icons-slides.md), so the icons are rasterised at
/// build time into 8-bit alpha masks at the sizes in `SIZES`, and drawn as tinted textures.
#[cfg(test)]
pub(crate) const ICONS: [&str; 19] = [
    "slide_add",
    "slide_layout",
    "text_bold",
    "text_italic",
    "text_underline",
    "text_color",
    "text_bullet_list",
    "text_number_list_ltr",
    "text_align_left",
    "text_align_center",
    "text_align_right",
    "textbox",
    "image",
    "shapes",
    "table",
    "translate",
    "history",
    "window_header_horizontal",
    "window_header_horizontal_off",
];
pub(crate) const SIZES: [usize; 3] = [20, 28, 40];
pub(crate) static BIN: &[u8] = include_bytes!("../assets/office_icons.bin");

/// `office_icons.bin`: per icon `name\0`, then the masks of every size in `SIZES`, row by row.
pub(crate) fn masks(name: &str) -> Option<[&'static [u8]; SIZES.len()]> {
    let mut rest = BIN;
    while let Some(z) = rest.iter().position(|b| *b == 0) {
        let found = &rest[..z] == name.as_bytes();
        rest = &rest[z + 1..];
        let mut out: [&[u8]; SIZES.len()] = [&[]; SIZES.len()];
        for (m, s) in out.iter_mut().zip(SIZES) {
            let (a, b) = rest.split_at_checked(s * s)?;
            *m = a;
            rest = b;
        }
        if found {
            return Some(out);
        }
    }
    None
}

/// Draw icon `name` centred in `rect` in colour `tint`, from the smallest mask that covers the on-screen pixels.
fn paint_icon(ui: &Ui, rect: Rect, name: &str, tint: Color32) {
    let px = (ICON * ui.ctx().pixels_per_point()).ceil() as usize;
    let at = SIZES.iter().position(|s| *s >= px).unwrap_or(SIZES.len() - 1);
    let (size, id) = (SIZES[at], egui::Id::new(("office_toolbar_icon", name, SIZES[at])));
    let texture = match ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        Some(t) => t,
        None => {
            let Some(mask) = masks(name).map(|m| m[at]) else { return };
            let rgba: Vec<u8> = mask.iter().flat_map(|a| [255, 255, 255, *a]).collect();
            let image = egui::ColorImage::from_rgba_unmultiplied([size, size], &rgba);
            let t = ui.ctx().load_texture(format!("office-toolbar/{name}/{size}"), image, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };
    let target = Rect::from_center_size(rect.center(), vec2(ICON, ICON));
    ui.painter().image(texture.id(), target, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
}

// ---- the simple/full choice ----

fn mode_id() -> egui::Id {
    egui::Id::new("office_toolbar_simple")
}

/// Is the simple row showing? Per window; starts from `office_overlay::toolbar_default`.
pub fn simple(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(mode_id())).unwrap_or_else(office_overlay::toolbar_default)
}

pub fn set_simple(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(mode_id(), on));
    ctx.request_repaint();
}

/// What the History button does (version history, PLAN WP10). Until a handler is set the button is disabled.
static HISTORY: OnceLock<fn(&mut SlideApp)> = OnceLock::new();

pub fn set_history_handler(f: fn(&mut SlideApp)) {
    let _ = HISTORY.set(f);
}

// ---- drawing ----

struct Btn<'a> {
    icon: &'a str,
    label: &'a str,
    tip: String,
    enabled: bool,
    selected: bool,
    toggle: bool,
    split: bool,
    /// A menu arrow inside the button (the whole button opens the menu).
    menu: bool,
    /// A colour bar under the glyph (font colour).
    bar: Option<Color32>,
}

impl<'a> Btn<'a> {
    fn new(icon: &'a str, label: &'a str) -> Self {
        Btn { icon, label, tip: label.to_string(), enabled: true, selected: false, toggle: false, split: false, menu: false, bar: None }
    }
    fn tip(mut self, tip: String) -> Self {
        self.tip = tip;
        self
    }
    fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    fn toggle(mut self, on: bool) -> Self {
        self.toggle = true;
        self.selected = on;
        self
    }
    fn split(mut self) -> Self {
        self.split = true;
        self
    }
    fn menu(mut self) -> Self {
        self.menu = true;
        self
    }
    fn bar(mut self, c: Color32) -> Self {
        self.bar = Some(c);
        self
    }
}

/// One button: the glyph, the label when `labels`, a menu arrow when `split`. Returns the main click area and the arrow's.
fn button(ui: &mut Ui, b: Btn, labels: bool) -> (Response, Option<Response>) {
    let t = Tokens::get(ui.ctx());
    let font = theme::font(12.0);
    let show_label = labels && !b.label.is_empty();
    let text_w = if show_label { ui.fonts_mut(|f| f.layout_no_wrap(b.label.to_string(), font.clone(), t.text).size().x) + 4.0 } else { 0.0 };
    let arrow_w = if b.split { 14.0 } else { 0.0 };
    let chev_w = if b.menu { 12.0 } else { 0.0 };
    let main_w = 6.0 + ICON + text_w + chev_w + 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(main_w + arrow_w, BTN_H), Sense::hover());
    let sense = if b.enabled { Sense::click() } else { Sense::hover() };
    let main_rect = Rect::from_min_size(rect.min, vec2(main_w, BTN_H));
    let arrow_rect = Rect::from_min_max(pos2(rect.min.x + main_w, rect.min.y), rect.max);
    let main = ui.interact(main_rect, resp.id.with("m"), sense);
    let arrow = b.split.then(|| ui.interact(arrow_rect, resp.id.with("a"), sense));
    let wash = |r: &Response, selected: bool| {
        if !b.enabled {
            None
        } else if r.is_pointer_button_down_on() {
            Some(t.pressed)
        } else if r.hovered() {
            Some(t.hover)
        } else if selected {
            Some(t.selected)
        } else {
            None
        }
    };
    let painter = ui.painter_at(rect);
    if let Some(c) = wash(&main, b.selected) {
        painter.rect_filled(main_rect, CornerRadius::same(4), c);
    }
    if let Some(a) = &arrow
        && let Some(c) = wash(a, false)
    {
        painter.rect_filled(arrow_rect, CornerRadius::same(4), c);
    }
    let ink = if b.enabled { t.text } else { t.text_faint };
    let glyph = Rect::from_min_size(pos2(main_rect.min.x + 6.0, main_rect.min.y), vec2(ICON, BTN_H));
    paint_icon(ui, if b.bar.is_some() { glyph.translate(vec2(0.0, -2.0)) } else { glyph }, b.icon, ink);
    if let Some(c) = b.bar {
        painter.rect_filled(
            Rect::from_min_size(pos2(glyph.min.x, main_rect.max.y - 6.0), vec2(ICON, 3.0)),
            CornerRadius::ZERO,
            if b.enabled { c } else { t.text_faint },
        );
    }
    if show_label {
        painter.text(pos2(glyph.max.x + 4.0, rect.center().y), Align2::LEFT_CENTER, b.label, font, ink);
    }
    if b.split {
        widgets::chevron(&painter, pos2(arrow_rect.center().x, rect.center().y), 2.8, t.text_dim);
    }
    if b.menu {
        widgets::chevron(&painter, pos2(main_rect.max.x - 11.0, rect.center().y), 2.8, t.text_dim);
    }
    let name = b.label.to_string();
    if b.toggle {
        main.widget_info(|| WidgetInfo::selected(WidgetType::Button, b.enabled, b.selected, &name));
    } else {
        main.widget_info(|| WidgetInfo::labeled(WidgetType::Button, b.enabled, &name));
    }
    // The split arrow is its own control: a screen reader needs a name for it too.
    if let Some(a) = &arrow {
        a.widget_info(|| WidgetInfo::labeled(WidgetType::Button, b.enabled, format!("{name} \u{25be}")));
    }
    (main.on_hover_text(b.tip), arrow)
}

fn separator(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, BTN_H), Sense::hover());
    ui.painter()
        .line_segment([pos2(rect.center().x, rect.min.y + 4.0), pos2(rect.center().x, rect.max.y - 4.0)], egui::Stroke::new(1.0, t.separator));
}

/// `<label> (<shortcut>)` as the ribbon's tooltips read.
fn tip(id: &str, en: &str) -> String {
    let label = crate::i18n::t(en);
    let shortcut =
        deckcraft_engine::find_command(id).and_then(|c| c.shortcut).or_else(|| crate::UI_COMMANDS.iter().find(|c| c.0 == id).and_then(|c| c.2));
    match shortcut {
        Some(s) => format!("{label} ({})", ribbon::pretty_shortcut(s)),
        None => label.to_string(),
    }
}

fn run(app: &mut SlideApp, id: &str, params: Value) {
    let _ = app.run(id, params);
}

/// A plain command button.
fn command(app: &mut SlideApp, ui: &mut Ui, labels: bool, icon: &str, en: &str, id: &str) {
    let label = crate::i18n::t(en);
    let b = Btn::new(icon, label).tip(tip(id, en)).enabled(ribbon::enabled(app, id));
    if button(ui, b, labels).0.clicked() {
        run(app, id, json!({}));
    }
}

/// A toggle button whose state comes from `format.state`.
fn toggle(app: &mut SlideApp, ui: &mut Ui, icon: &str, en: &str, id: &str, on: bool) {
    let label = crate::i18n::t(en);
    let b = Btn::new(icon, label).tip(tip(id, en)).enabled(ribbon::enabled(app, id)).toggle(on);
    if button(ui, b, false).0.clicked() {
        run(app, id, json!({}));
    }
}

/// Draw the toolbar row (`true`), or in the full ribbon mode the pill that goes back to it (`false`: the caller draws the ribbon).
pub fn show(app: &mut SlideApp, ui: &mut Ui) -> bool {
    let ctx = ui.ctx().clone();
    if !simple(&ctx) {
        full_mode_pill(app, &ctx);
        return false;
    }
    let t = Tokens::get(&ctx);
    egui::Panel::top("office_toolbar").exact_size(ROW_H).frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let labels = ui.available_width() >= LABELS_MIN;
        let st = ribbon::fmt_state(app);
        let flag = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
        let align = st.get("align").and_then(Value::as_str).unwrap_or("");
        egui::ScrollArea::horizontal().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(8.0);
                slides_group(app, ui, labels);
                separator(ui);
                font_group(app, ui, &st, &flag);
                separator(ui);
                toggle(app, ui, "text_bullet_list", "Bullets", "format.bullets", flag("bullets"));
                toggle(app, ui, "text_number_list_ltr", "Numbering", "format.numbering", flag("numbering"));
                toggle(app, ui, "text_align_left", "Align Left", "format.alignLeft", align == "l");
                toggle(app, ui, "text_align_center", "Center", "format.alignCenter", align == "ctr");
                toggle(app, ui, "text_align_right", "Align Right", "format.alignRight", align == "r");
                separator(ui);
                insert_group(app, ui, labels);
                separator(ui);
                output_group(app, ui, labels);
                if !cfg!(target_arch = "wasm32") {
                    ui.add_space(6.0);
                    let label = crate::i18n::t("Full ribbon");
                    if button(ui, Btn::new("window_header_horizontal", label).tip(label.to_string()), true).0.clicked() {
                        switch(&ctx, false);
                    }
                }
            });
        });
    });
    true
}

fn switch(ctx: &Context, simple: bool) {
    set_simple(ctx, simple);
    office_overlay::toolbar_changed(simple);
}

fn slides_group(app: &mut SlideApp, ui: &mut Ui, labels: bool) {
    let label = crate::i18n::t("New Slide");
    let b = Btn::new("slide_add", label).tip(tip("slide.new", "New Slide")).enabled(ribbon::enabled(app, "slide.new")).split();
    let (main, arrow) = button(ui, b, labels);
    if main.clicked() {
        run(app, "slide.new", json!({}));
    }
    if let Some(a) = arrow {
        egui::Popup::menu(&a).width(420.0).show(|ui| ribbon::layout_menu(app, ui, "slide.new"));
    }
    let label = crate::i18n::t("Layout");
    let b = Btn::new("slide_layout", label).tip(tip("slide.layout", "Layout")).enabled(ribbon::enabled(app, "slide.layout")).menu();
    let main = button(ui, b, labels).0;
    egui::Popup::menu(&main).width(420.0).show(|ui| ribbon::layout_menu(app, ui, "slide.layout"));
}

fn font_group(app: &mut SlideApp, ui: &mut Ui, st: &Value, flag: &dyn Fn(&str) -> bool) {
    let en = ribbon::enabled(app, "format.bold");
    let font = st.get("font").and_then(Value::as_str).unwrap_or("").to_string();
    let families = ribbon::font_families(app);
    widgets::dropdown(ui, "font", &font, 150.0, en, |ui| {
        for family in &families {
            if ui.button(family).clicked() {
                run(app, "format.font", json!({"family": family}));
                ui.close();
            }
        }
    });
    let size = st.get("size").and_then(Value::as_f64).map(|s| widgets::fmt_num(s, "")).unwrap_or_default();
    widgets::dropdown(ui, "size", &size, 52.0, en, |ui| {
        for s in deckcraft_engine::cmd::format::SIZES.iter().chain([72.0, 80.0, 88.0, 96.0].iter()) {
            if ui.button(widgets::fmt_num(*s, "")).clicked() {
                run(app, "format.size", json!({"size": s}));
                ui.close();
            }
        }
    });
    ui.add_space(4.0);
    toggle(app, ui, "text_bold", "Bold", "format.bold", flag("bold"));
    toggle(app, ui, "text_italic", "Italic", "format.italic", flag("italic"));
    toggle(app, ui, "text_underline", "Underline", "format.underline", flag("underline"));
    let label = crate::i18n::t("Font Color");
    let b = Btn::new("text_color", label).tip(tip("format.color", "Font Color")).enabled(en).split().bar(Color32::from_rgb(0xD8, 0x3F, 0x3F));
    let (main, arrow) = button(ui, b, false);
    if main.clicked() {
        run(app, "format.color", json!({"color": "#C00000"}));
    }
    if let Some(a) = arrow {
        let scheme =
            app.session.active().and_then(|d| d.doc.masters.first().map(|m| m.scheme())).unwrap_or_else(|| deckcraft_model::Theme::default().colors);
        widgets::color_popup(&a).show(|ui| {
            if let Some(Some(c)) = widgets::color_grid(ui, &scheme, None) {
                run(app, "format.color", json!({"color": ribbon::cref_param(&c)}));
            }
        });
    }
}

fn insert_group(app: &mut SlideApp, ui: &mut Ui, labels: bool) {
    command(app, ui, labels, "textbox", "Text Box", "insert.textBox");
    command(app, ui, labels, "image", "Picture", "app.insertPictureDialog");
    let label = crate::i18n::t("Shapes");
    let b = Btn::new("shapes", label).tip(label.to_string()).enabled(ribbon::enabled(app, "shape.insert")).menu();
    let main = button(ui, b, labels).0;
    egui::Popup::menu(&main).show(|ui| {
        if let Some(p) = ribbon::shapes_gallery(ui) {
            app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: p.to_string() });
        }
    });
    let label = crate::i18n::t("Table");
    let b = Btn::new("table", label).tip(label.to_string()).enabled(ribbon::enabled(app, "insert.table")).menu();
    let main = button(ui, b, labels).0;
    egui::Popup::menu(&main).show(|ui| {
        if let Some((rows, cols)) = ribbon::table_grid_picker(ui) {
            run(app, "insert.table", json!({"rows": rows, "cols": cols}));
            ui.close();
        }
        ui.separator();
        if ui.button(crate::i18n::t("Insert Table…")).clicked() {
            app.dialog = Some(crate::dialogs::Dialog::new("table"));
            ui.close();
        }
    });
}

fn output_group(app: &mut SlideApp, ui: &mut Ui, labels: bool) {
    let label = crate::i18n::t("Transliterate");
    let b = Btn::new("translate", label).tip(label.to_string()).enabled(ribbon::enabled(app, "tools.translitAuto")).menu();
    let main = button(ui, b, labels).0;
    egui::Popup::menu(&main).show(|ui| ribbon::translit_menu(app, ui));
    let label = crate::i18n::t("Version History");
    let b = Btn::new("history", label).tip(label.to_string()).enabled(HISTORY.get().is_some());
    if button(ui, b, labels).0.clicked()
        && let Some(f) = HISTORY.get()
    {
        f(app);
    }
}

/// The "Simple ribbon" pill, floated at the right end of the tab row while the upstream ribbon shows.
fn full_mode_pill(app: &SlideApp, ctx: &Context) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let title_h = if app.integrated_titlebar { ribbon::MAC_TITLE_BAR } else { ribbon::TITLE_H };
    egui::Area::new(egui::Id::new("office_toolbar_pill")).order(egui::Order::Foreground).anchor(Align2::RIGHT_TOP, vec2(-10.0, title_h + 1.0)).show(
        ctx,
        |ui| {
            let label = crate::i18n::t("Simple ribbon");
            let b = Btn::new("window_header_horizontal_off", label).tip(label.to_string());
            if button(ui, b, true).0.clicked() {
                switch(ctx, true);
            }
        },
    );
}
