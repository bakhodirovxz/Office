// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The simplified Sheets toolbar: one row drawn instead of the ribbon (patch 0140; docs/phase4/toolbar-design.md section 4).
//!
//! - `show` is the one call site in `lib.rs`. With the overlay off, or in full mode, it draws upstream's ribbon.
//! - The row reuses upstream's pickers and menus (`ribbon.rs`), so every command, keytip badge and popup is upstream's own.
//! - Fluent icons (MIT, `assets/icons/fluent`) are 8-bit alpha masks in `office_icons.bin` (`scripts/embed-toolbar-icons.sh`), drawn as
//!   tinted textures. The SVG loader would add about 309 KB brotli to the wasm (docs/phase4/icons-wasm.md, sheets-toolbar-wasm.md).
//! - Width: labels drop when the row does not fit, and then the rightmost groups move into a "..." menu. Fit is measured
//!   from the real text widths, so it holds in every language.
//! - The mode is a process-wide flag, read at start from the shared `settings.toml` (suite-core, key `toolbar`) and written by the toggle.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Align, Color32, Layout, Rect, Response, Sense, Ui, pos2, vec2};
use gridcraft_engine::model::{HAlign, Style, Underline};
use serde_json::{Value, json};

use crate::theme::{self, Tokens};
use crate::widgets::color_palette;
use crate::{SheetApp, office_overlay, ribbon};

/// A menu entry: English label, command id (`dialog:<name>` opens a dialog), params as JSON text.
type Entry = (&'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
enum Kind {
    Font,
    Size,
    Number,
    /// A button that shows its on-state.
    Toggle,
    /// Font colour and fill colour: a button, an arrow, and the colour bar under the glyph.
    Color,
    Borders,
    Menu(&'static [Entry]),
    Button,
    /// The plugin commands (Phase 7 WP13): a menu per plugin, from `office_overlay::plugin_menu`; hidden when there are none.
    Plugins,
}

struct Item {
    kind: Kind,
    /// The command the item runs (for a menu: its first entry), also the keytip target and the tooltip's shortcut.
    id: &'static str,
    icon: &'static str,
    /// English label: the tooltip, the accessible name, and the text next to the icon when the row is wide.
    label: &'static str,
    group: u8,
    /// Shows its label beside the icon while the row is wide.
    text: bool,
}

const fn item(kind: Kind, id: &'static str, icon: &'static str, label: &'static str, group: u8, text: bool) -> Item {
    Item { kind, id, icon, label, group, text }
}

const INSERT: &[Entry] = &[
    ("Insert Sheet Rows", "home.insertRows", "{}"),
    ("Insert Sheet Columns", "home.insertColumns", "{}"),
    ("Insert Sheet", "home.insertSheet", "{}"),
];
const DELETE: &[Entry] = &[("Delete Sheet Rows", "home.deleteRows", "{}"), ("Delete Sheet Columns", "home.deleteColumns", "{}")];
const SORT: &[Entry] = &[("Sort A to Z", "data.sortAscending", "{}"), ("Sort Z to A", "data.sortDescending", "{}")];
const FREEZE: &[Entry] = &[
    ("Freeze Panes", "view.freezePanes", "{}"),
    ("Freeze Top Row", "view.freezeTopRow", "{}"),
    ("Freeze First Column", "view.freezeFirstColumn", "{}"),
    ("Unfreeze Panes", "view.unfreezePanes", "{}"),
];
const TRANSLIT: &[Entry] = &[
    ("Latin to Cyrillic", "tools.translitCyrl", "{}"),
    ("Cyrillic to Latin", "tools.translitLatn", "{}"),
    ("Auto-detect", "tools.translitAuto", "{}"),
];
/// The number picker's short list; "More Number Formats..." opens Format Cells.
const NUMBER_NAMES: &[&str] = &["General", "Number", "Short Date", "Percentage", "Text"];
const BORDER_PRESETS: &[(&str, &str)] = &[
    ("Bottom Border", "bottom"),
    ("Top Border", "top"),
    ("Left Border", "left"),
    ("Right Border", "right"),
    ("No Border", "none"),
    ("All Borders", "all"),
    ("Outside Borders", "outside"),
    ("Thick Outside Borders", "thickOutside"),
    ("Bottom Double Border", "doubleBottom"),
    ("Thick Bottom Border", "thickBottom"),
    ("Top and Bottom Border", "topBottom"),
    ("Top and Thick Bottom Border", "topThickBottom"),
    ("Top and Double Bottom Border", "topDoubleBottom"),
    ("Inside Borders", "inside"),
];

/// Left to right. Ids that are ours (`office.*`) are checked by `tests/office_toolbar.rs` only when the command exists.
const ITEMS: &[Item] = &[
    item(Kind::Font, "home.fontName", "", "Font", 0, false),
    item(Kind::Size, "home.fontSize", "", "Font Size", 0, false),
    item(Kind::Toggle, "home.bold", "text_bold", "Bold", 0, false),
    item(Kind::Toggle, "home.italic", "text_italic", "Italic", 0, false),
    item(Kind::Toggle, "home.underline", "text_underline", "Underline", 0, false),
    item(Kind::Color, "home.fontColor", "text_color", "Font Color", 0, false),
    item(Kind::Color, "home.fillColor", "paint_bucket", "Fill Color", 0, false),
    item(Kind::Borders, "home.borders", "border_all", "Borders", 0, false),
    item(Kind::Toggle, "home.alignLeft", "text_align_left", "Align Left", 1, false),
    item(Kind::Toggle, "home.alignCenter", "text_align_center", "Center", 1, false),
    item(Kind::Toggle, "home.alignRight", "text_align_right", "Align Right", 1, false),
    item(Kind::Toggle, "home.wrapText", "text_wrap", "Wrap Text", 1, false),
    item(Kind::Toggle, "home.mergeCenter", "table_cells_merge", "Merge & Center", 1, false),
    item(Kind::Number, "home.numberFormat", "", "Number Format", 2, false),
    item(Kind::Menu(INSERT), "home.insertRows", "table_insert_row", "Insert", 3, true),
    item(Kind::Menu(DELETE), "home.deleteRows", "table_delete_row", "Delete", 3, true),
    item(Kind::Button, "formulas.autoSum", "autosum", "AutoSum", 4, false),
    item(Kind::Menu(SORT), "data.sortAscending", "arrow_sort_up", "Sort", 4, true),
    item(Kind::Toggle, "data.filter", "filter", "Filter", 4, true),
    item(Kind::Menu(FREEZE), "view.freezePanes", "table_freeze_column_and_row", "Freeze Panes", 5, true),
    item(Kind::Toggle, "formulas.showFormulas", "math_formula", "Show Formulas", 5, false),
    item(Kind::Button, "file.print", "print", "Print", 6, true),
    item(Kind::Menu(TRANSLIT), "tools.translitAuto", "translate", "Transliterate", 6, true),
    // Ours (WP10): hidden until the command is registered.
    item(Kind::Button, "office.history", "history", "Version History", 6, true),
    // Ours (Phase 7 WP13): hidden while no plugin is loaded; no icon of its own, so its label always shows.
    item(Kind::Plugins, "office.plugins", "", "Plugins", 6, true),
];

const H: f32 = 28.0;
const ICON: f32 = 16.0;
const GAP: f32 = 2.0;
const SEP: f32 = 13.0;
const CARD_H: f32 = 40.0;

// ---------------------------------------------------------------------------------------------------------------- mode

/// 1 = simple, anything else = full. Full until [`init`] runs, so an app or test that never calls it keeps upstream's ribbon.
static MODE: AtomicU8 = AtomicU8::new(0);
/// The toggle is offered: the overlay is on, or a host/test has set the mode.
static OFFERED: AtomicU8 = AtomicU8::new(0);

/// Reads the shared setting once, at start-up. Without the overlay it does nothing: upstream's ribbon stays.
pub fn init() {
    if office_overlay::ACTIVE {
        set_simple(read_mode());
    }
}

/// True when the simple toolbar is shown.
pub fn simple() -> bool {
    MODE.load(Ordering::Relaxed) == 1
}

/// Sets the mode for this process only (a host that reads the shared settings itself, and tests).
pub fn set_simple(on: bool) {
    MODE.store(u8::from(on), Ordering::Relaxed);
    OFFERED.store(1, Ordering::Relaxed);
}

fn offered() -> bool {
    office_overlay::ACTIVE || OFFERED.load(Ordering::Relaxed) == 1
}

/// The toggle button: switches at once and remembers the choice for the next start.
fn toggle_mode(on: bool) {
    set_simple(on);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let tag = if on { "simple" } else { "full" };
        let result = match settings_dir() {
            Some(dir) => suite_core::settings::set_in(&dir, "toolbar", tag).map_err(|e| std::io::Error::other(e.to_string())),
            None => suite_core::settings::set_toolbar_mode(if on {
                suite_core::settings::ToolbarMode::Simple
            } else {
                suite_core::settings::ToolbarMode::Full
            }),
        };
        if let Err(e) = result {
            log::warn!("could not write the toolbar mode: {e}");
        }
    }
    // The web keeps the choice in localStorage (suite-core).
    #[cfg(target_arch = "wasm32")]
    if let Err(e) =
        suite_core::settings::set_toolbar_mode(if on { suite_core::settings::ToolbarMode::Simple } else { suite_core::settings::ToolbarMode::Full })
    {
        log::warn!("could not write the toolbar mode: {e}");
    }
}

/// Reads and writes `settings.toml` in `dir` instead of the user's folder (tests, and a host that keeps its own).
#[cfg(not(target_arch = "wasm32"))]
pub fn use_settings_dir(dir: std::path::PathBuf) {
    *SETTINGS_DIR.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

#[cfg(not(target_arch = "wasm32"))]
static SETTINGS_DIR: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

#[cfg(not(target_arch = "wasm32"))]
fn settings_dir() -> Option<std::path::PathBuf> {
    SETTINGS_DIR.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The shared setting (`toolbar` in settings.toml, suite-core). The web: the saved choice in localStorage, then `?toolbar=`, then simple.
fn read_mode() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return match settings_dir() {
        Some(dir) => suite_core::settings::load_in(&dir).settings.toolbar,
        None => suite_core::settings::toolbar_mode(),
    } == suite_core::settings::ToolbarMode::Simple;
    #[cfg(target_arch = "wasm32")]
    return suite_core::settings::toolbar_mode() == suite_core::settings::ToolbarMode::Simple;
}

// -------------------------------------------------------------------------------------------------------------- icons

const BIN: &[u8] = include_bytes!("office_icons.bin");
/// The raster sizes in `office_icons.bin`, in pixels, per icon in this order.
const SIZES: [usize; 3] = [20, 28, 40];

type Masks = [&'static [u8]; SIZES.len()];

/// `office_icons.bin`: per icon `name ` and then the masks of every size in `SIZES`.
fn table() -> &'static [(&'static str, Masks)] {
    static T: OnceLock<Vec<(&'static str, Masks)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = Vec::new();
        let mut rest = BIN;
        while let Some(z) = rest.iter().position(|b| *b == 0) {
            let name = std::str::from_utf8(&rest[..z]).unwrap_or("");
            rest = &rest[z + 1..];
            let mut masks: Masks = [&[]; SIZES.len()];
            for (m, s) in masks.iter_mut().zip(SIZES) {
                let (a, b) = rest.split_at((s * s).min(rest.len()));
                *m = a;
                rest = b;
            }
            out.push((name, masks));
        }
        out
    })
}

/// Every icon name the toolbar draws (a test checks each is embedded).
pub fn icon_names() -> impl Iterator<Item = &'static str> {
    ITEMS.iter().map(|i| i.icon).filter(|n| !n.is_empty()).chain([
        "chevron_down",
        "more_horizontal",
        "window_header_horizontal",
        "window_header_horizontal_off",
    ])
}

pub fn has_icon(name: &str) -> bool {
    table().iter().any(|(n, masks)| *n == name && masks.iter().zip(SIZES).all(|(m, s)| m.len() == s * s) && masks[0].iter().any(|a| *a > 128))
}

/// Every command id the row runs (the registry test reads it).
pub fn command_ids() -> impl Iterator<Item = &'static str> {
    ITEMS.iter().flat_map(|i| match i.kind {
        Kind::Menu(entries) => entries.iter().map(|e| e.1).collect::<Vec<_>>(),
        _ => vec![i.id],
    })
}

/// Draws icon `name` centred on `center`, `size` logical points square, in colour `tint`. A name that is not in the table draws nothing.
fn paint_icon(ui: &Ui, center: egui::Pos2, name: &str, size: f32, tint: Color32) {
    let Some((_, masks)) = table().iter().find(|(n, _)| *n == name) else { return };
    let px = (size * ui.ctx().pixels_per_point()).ceil() as usize;
    let i = SIZES.iter().position(|s| *s >= px).unwrap_or(SIZES.len() - 1);
    let (side, mask) = (SIZES[i], masks[i]);
    let id = egui::Id::new(("office-toolbar-icon", name, side));
    let ctx = ui.ctx();
    let tex = match ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        Some(t) => t,
        None => {
            let rgba: Vec<u8> = mask.iter().flat_map(|a| [255, 255, 255, *a]).collect();
            let image = egui::ColorImage::from_rgba_unmultiplied([side, side], &rgba);
            let t = ctx.load_texture(format!("office-toolbar/{name}/{side}"), image, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };
    let rect = Rect::from_center_size(center, vec2(size, size));
    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
}

// ------------------------------------------------------------------------------------------------------------- widgets

fn text_w(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), theme::ui_font(12.5), Color32::WHITE).size().x
}

fn shortcut(id: &str) -> Option<String> {
    let s = gridcraft_engine::find_command(id)?.shortcut?;
    Some(s.replace("Cmd+", if cfg!(target_os = "macos") { "\u{2318}" } else { "Ctrl+" }))
}

fn tip(app: &SheetApp, label: &str, id: &str) -> String {
    let l = app.ui.language.tr(label);
    shortcut(id).map_or_else(|| l.to_string(), |s| format!("{l} ({s})"))
}

/// Width of an icon button with optional label and chevron.
fn tool_w(label_w: Option<f32>, chevron: bool) -> f32 {
    let w = 6.0 + ICON + label_w.map_or(0.0, |w| 5.0 + w) + if chevron { 4.0 + 10.0 } else { 0.0 } + 6.0;
    w.max(H)
}

fn hover_fill(ui: &Ui, resp: &Response, on: bool, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 4.0, t.pressed);
    } else if on {
        ui.painter().rect_filled(rect, 4.0, t.pressed);
        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, t.separator), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, t.hover);
    }
}

/// An icon button with an optional label and a chevron. `name` is the accessible name; `on` is reported as selected.
fn tool(ui: &mut Ui, icon: &str, label: Option<&str>, chevron: bool, on: Option<bool>, name: &str, tip: String) -> Response {
    let t = Tokens::get(ui.ctx());
    let w = tool_w(label.map(|l| text_w(ui, l)), chevron);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, H), Sense::click());
    hover_fill(ui, &resp, on == Some(true), rect);
    let mut x = rect.left() + 6.0 + ICON / 2.0;
    paint_icon(ui, pos2(x, rect.center().y), icon, ICON, t.text);
    x += ICON / 2.0 + 5.0;
    if let Some(l) = label {
        ui.painter().text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, l, theme::ui_font(12.5), t.text);
    }
    if chevron {
        paint_icon(ui, pos2(rect.right() - 11.0, rect.center().y + 1.0), "chevron_down", 10.0, t.text_dim);
    }
    resp.widget_info(|| match on {
        Some(on) => egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, name),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name),
    });
    resp.on_hover_text(tip)
}

/// A button plus an arrow (colours, borders). Returns (main clicked, arrow response).
fn split(ui: &mut Ui, icon: &str, bar: Option<Color32>, name: &str, tip: String) -> (bool, Response) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(H + 14.0, H), Sense::hover());
    let main = Rect::from_min_size(rect.min, vec2(H, H));
    let arrow = Rect::from_min_max(pos2(main.right(), rect.top()), rect.max);
    let m = ui.interact(main, ui.id().with((name, "main")), Sense::click());
    let a = ui.interact(arrow, ui.id().with((name, "arrow")), Sense::click());
    if m.hovered() || a.hovered() {
        ui.painter().rect_filled(rect, 4.0, t.hover);
        ui.painter()
            .line_segment([pos2(main.right(), rect.top() + 4.0), pos2(main.right(), rect.bottom() - 4.0)], egui::Stroke::new(1.0, t.separator));
    }
    paint_icon(ui, main.center() - vec2(0.0, 2.0), icon, ICON, t.text);
    if let Some(c) = bar {
        ui.painter().rect_filled(Rect::from_min_size(pos2(main.left() + 6.0, main.bottom() - 7.0), vec2(16.0, 3.0)), 0.0, c);
    }
    paint_icon(ui, pos2(arrow.center().x, arrow.center().y + 1.0), "chevron_down", 10.0, t.text_dim);
    m.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    a.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{name} \u{25be}")));
    (m.on_hover_text(tip).clicked(), a)
}

/// Consumes the one-shot keytip request for control `id` (Alt,H,H and Alt,H,F,C open the colour popups).
fn invoke(app: &mut SheetApp, id: &str) -> bool {
    let hit = app.ui.keytip_invoke == Some(id);
    if hit {
        app.ui.keytip_invoke = None;
    }
    hit
}

fn popup_command(forced: bool, clicked: bool) -> Option<egui::SetOpenCommand> {
    if forced { Some(egui::SetOpenCommand::Bool(true)) } else { clicked.then_some(egui::SetOpenCommand::Toggle) }
}

fn params(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn entries(list: &[Entry]) -> Vec<(&str, &str, Value)> {
    list.iter().map(|(label, id, p)| (*label, *id, params(p))).collect()
}

fn is_on(app: &SheetApp, st: &Style, id: &str) -> bool {
    let sheet = || app.session.active().and_then(|d| d.wb.active().map(|sh| (d, sh)));
    match id {
        "home.bold" => st.font.bold,
        "home.italic" => st.font.italic,
        "home.underline" => st.font.underline != Underline::None,
        "home.alignLeft" => st.align.h == HAlign::Left,
        "home.alignCenter" => st.align.h == HAlign::Center,
        "home.alignRight" => st.align.h == HAlign::Right,
        "home.wrapText" => st.align.wrap,
        "home.mergeCenter" => sheet().is_some_and(|(d, sh)| sh.merge_at(d.selection.active).is_some()),
        "data.filter" => sheet().is_some_and(|(_, sh)| sh.autofilter.is_some()),
        "formulas.showFormulas" => sheet().is_some_and(|(_, sh)| sh.show_formulas),
        _ => false,
    }
}

fn shown(item: &Item) -> bool {
    match item.kind {
        Kind::Plugins => !office_overlay::plugin_menu().is_empty(),
        _ => item.id != "office.history" || gridcraft_engine::find_command(item.id).is_some(),
    }
}

/// The item's label as drawn (or `None` for an icon-only item).
fn label_of(app: &SheetApp, item: &Item, labels: bool) -> Option<String> {
    ((labels || matches!(item.kind, Kind::Plugins)) && item.text).then(|| app.ui.language.tr(item.label).to_string())
}

/// The plugin menus: one submenu per plugin. A finished command's status (the plugin's `ui.status`) is the toast, an error the
/// usual alert. On the web a command answers `{pending}` at once and the page shows the outcome.
fn plugin_items(app: &mut SheetApp, ui: &mut Ui) {
    for (plugin, commands) in office_overlay::plugin_menu() {
        ui.menu_button(plugin, |ui| plugin_commands(app, ui, commands));
    }
}

fn plugin_commands(app: &mut SheetApp, ui: &mut Ui, commands: Vec<(String, &'static str)>) {
    for (label, id) in commands {
        if ui.button(label).clicked() {
            match app.run(id, json!({})) {
                Ok(v) => {
                    if let Some(t) = v.get("status").and_then(Value::as_str) {
                        app.toast = Some((t.to_string(), crate::now_ms()));
                    }
                }
                Err(e) => app.alert(e),
            }
            ui.close();
        }
    }
}

/// The full ribbon's Plugins tab (gridcraft/0187): the same menus as the simple toolbar's, one ribbon button per plugin.
pub(crate) fn plugins_tab(app: &mut SheetApp, ui: &mut Ui) {
    for (plugin, commands) in office_overlay::plugin_menu() {
        let r = crate::widgets::big_button(ui, crate::icons::Icon::Script, &plugin, &plugin, true);
        egui::Popup::menu(&r).show(|ui| plugin_commands(app, ui, commands));
    }
}

fn item_w(ui: &Ui, app: &SheetApp, item: &Item, labels: bool) -> f32 {
    match item.kind {
        Kind::Font | Kind::Number => 150.0,
        Kind::Size => 52.0,
        Kind::Color | Kind::Borders => H + 14.0,
        Kind::Menu(_) | Kind::Plugins => tool_w(label_of(app, item, labels).map(|l| text_w(ui, &l)), true),
        Kind::Toggle | Kind::Button => tool_w(label_of(app, item, labels).map(|l| text_w(ui, &l)), false),
    }
}

fn draw_item(app: &mut SheetApp, ui: &mut Ui, item: &Item, st: &Style, labels: bool) {
    let name = app.ui.language.tr(item.label).to_string();
    let label = label_of(app, item, labels);
    match item.kind {
        Kind::Font => ribbon::font_combo(app, ui, st),
        Kind::Size => ribbon::size_combo(app, ui, st),
        Kind::Number => ribbon::number_combo_of(app, ui, st, NUMBER_NAMES),
        Kind::Toggle | Kind::Button => {
            let on = matches!(item.kind, Kind::Toggle).then(|| is_on(app, st, item.id));
            let r = tool(ui, item.icon, label.as_deref(), false, on, &name, tip(app, item.label, item.id));
            ribbon::control_keytip(app, ui, &r, item.id);
            if r.clicked() {
                app.run_or_alert(item.id, json!({}));
            }
        }
        Kind::Menu(list) => {
            let r = tool(ui, item.icon, label.as_deref(), true, None, &name, tip(app, item.label, item.id));
            let targets: Vec<&str> = list.iter().map(|e| e.1).collect();
            for sequence in crate::keytips::keytip_table::group_extension(app.keytips.prefix().unwrap_or_default(), &targets) {
                ribbon::keytip(app, ui, sequence, r.rect.center_bottom());
            }
            egui::Popup::menu(&r).show(|ui| ribbon::menu_items(app, ui, &entries(list)));
        }
        Kind::Plugins => {
            let r = tool(ui, item.icon, label.as_deref(), true, None, &name, name.clone());
            egui::Popup::menu(&r).show(|ui| plugin_items(app, ui));
        }
        Kind::Color => {
            let (last, bar_id) =
                if item.id == "home.fillColor" { (app.grid.last_fill.clone(), "Fill") } else { (app.grid.last_font_color.clone(), "Font") };
            let bar = gridcraft_engine::model::Color::from_hex(&last).and_then(|c| c.resolve(&Default::default())).map(theme::color32);
            let (clicked, arrow) = split(ui, item.icon, bar, &name, tip(app, item.label, item.id));
            ribbon::control_keytip(app, ui, &arrow, item.id);
            if clicked {
                app.run_or_alert(item.id, json!({"color": last}));
            }
            let forced = invoke(app, item.id);
            let none_label = if bar_id == "Fill" { "No Fill" } else { "Automatic" };
            let colors = app.session.active().map(|d| d.wb.theme.colors).unwrap_or(gridcraft_engine::model::Theme::default().colors);
            egui::Popup::menu(&arrow).open_memory(popup_command(forced, arrow.clicked())).show(|ui| {
                if let Some(c) = color_palette(ui, &colors, none_label) {
                    if c != "none" {
                        if bar_id == "Fill" {
                            app.grid.last_fill = c.clone();
                        } else {
                            app.grid.last_font_color = c.clone();
                        }
                    }
                    let c = if bar_id == "Font" && c == "none" { "auto".to_string() } else { c };
                    app.run_or_alert(item.id, json!({"color": c}));
                    ui.close();
                }
            });
        }
        Kind::Borders => {
            let (clicked, arrow) = split(ui, item.icon, None, &name, tip(app, item.label, item.id));
            ribbon::control_keytip(app, ui, &arrow, item.id);
            if clicked {
                app.run_or_alert("home.borders", json!({"preset": app.grid.last_border.clone()}));
            }
            egui::Popup::menu(&arrow).show(|ui| {
                for (label, preset) in BORDER_PRESETS {
                    if crate::border_preview::preset_button(ui, label, preset).clicked() {
                        app.grid.last_border = preset.to_string();
                        app.run_or_alert("home.borders", json!({"preset": preset}));
                    }
                }
                ui.separator();
                if ui.button(app.ui.language.tr("More Borders…")).clicked() {
                    app.open_dialog("formatCells", json!({"tab": "Border"}));
                }
            });
        }
    }
}

/// How the row is laid out at a given width: labels on or off, and how many items stay in the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub labels: bool,
    pub shown: usize,
    pub total: usize,
}

fn visible() -> Vec<&'static Item> {
    ITEMS.iter().filter(|i| shown(i)).collect()
}

fn row_w(ui: &Ui, app: &SheetApp, items: &[&Item], labels: bool) -> f32 {
    let mut w = 0.0;
    for (i, it) in items.iter().enumerate() {
        w += item_w(ui, app, it, labels) + GAP;
        if i > 0 && items[i - 1].group != it.group {
            w += SEP;
        }
    }
    w
}

/// The width left for the items in a window `window_w` wide: the card's margins, the toggle and its gaps are outside it.
fn room_for(window_w: f32, toggle_w: f32) -> f32 {
    window_w - 40.0 - toggle_w - 2.0 * GAP - 10.0
}

/// `room` is the width for the items; the toggle and the "..." button are outside it.
fn plan(ui: &Ui, app: &SheetApp, items: &[&Item], room: f32) -> Plan {
    let total = items.len();
    if row_w(ui, app, items, true) <= room {
        return Plan { labels: true, shown: total, total };
    }
    // The left groups (fonts, alignment, number) never move into the menu; below their width the row is clipped.
    let keep = items.iter().take_while(|i| i.group < 3).count();
    let mut shown = total;
    while shown > keep && row_w(ui, app, &items[..shown], false) + if shown < total { H + GAP } else { 0.0 } > room {
        shown -= 1;
    }
    Plan { labels: false, shown, total }
}

/// The plan the last drawn row used (the width tests read it).
pub fn last_plan() -> Option<Plan> {
    *LAST_PLAN.lock().unwrap_or_else(|e| e.into_inner())
}

static LAST_PLAN: std::sync::Mutex<Option<Plan>> = std::sync::Mutex::new(None);

fn toggle_w(ui: &Ui, app: &SheetApp) -> f32 {
    if offered() { tool_w(Some(text_w(ui, app.ui.language.tr("Full ribbon"))), false) } else { 0.0 }
}

/// The overflow menu: a flat entry for a button or toggle, a submenu for a menu.
fn overflow(app: &mut SheetApp, ui: &mut Ui, items: &[&Item]) {
    for it in items {
        match it.kind {
            Kind::Menu(list) => {
                let label = app.ui.language.tr(it.label).to_string();
                ui.menu_button(label, |ui| ribbon::menu_items(app, ui, &entries(list)));
            }
            Kind::Plugins => {
                let label = app.ui.language.tr(it.label).to_string();
                ui.menu_button(label, |ui| plugin_items(app, ui));
            }
            _ => ribbon::menu_items(app, ui, &[(it.label, it.id, json!({}))]),
        }
    }
}

/// The row, in place of upstream's ribbon. Full mode (or no overlay): upstream's ribbon, with a pill to go back.
pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    if !simple() {
        let top = ui.cursor().top();
        ribbon::show(app, ui);
        if offered() {
            simple_pill(app, ui, top);
        }
        return;
    }
    let t = Tokens::get(ui.ctx());
    let st = app.session.active().and_then(|d| Some(d.wb.styles.get(d.wb.active()?.style_id(d.selection.active)).clone())).unwrap_or_default();
    egui::Panel::top("ribbon")
        .exact_size(CARD_H + 10.0)
        .frame(egui::Frame::NONE.fill(t.window).inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 6 }))
        .show(ui, |ui| {
            let card = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), CARD_H));
            ui.painter().rect_filled(card, 10.0, t.ribbon);
            ui.painter().rect_stroke(card, 10.0, egui::Stroke::new(1.0, t.ribbon_border), egui::StrokeKind::Inside);
            let inner = card.shrink2(vec2(10.0, 0.0));
            let items = visible();
            let tw = toggle_w(ui, app);
            let room = room_for(ui.max_rect().width() + 20.0, tw);
            let plan = plan(ui, app, &items, room);
            *LAST_PLAN.lock().unwrap_or_else(|e| e.into_inner()) = Some(plan);
            let row = Rect::from_min_max(inner.min, pos2(inner.right() - tw - 10.0, inner.bottom()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(row));
                ui.spacing_mut().item_spacing = vec2(GAP, 0.0);
                for (i, it) in items[..plan.shown].iter().enumerate() {
                    if i > 0 && items[i - 1].group != it.group {
                        let (r, _) = ui.allocate_exact_size(vec2(SEP - GAP, 20.0), Sense::hover());
                        ui.painter().line_segment([pos2(r.center().x, r.top()), pos2(r.center().x, r.bottom())], egui::Stroke::new(1.0, t.separator));
                    }
                    draw_item(app, ui, it, &st, plan.labels);
                }
                if plan.shown < plan.total {
                    let name = app.ui.language.tr("More commands").to_string();
                    let more = tool(ui, "more_horizontal", None, false, None, &name, name.clone());
                    egui::Popup::menu(&more).show(|ui| overflow(app, ui, &items[plan.shown..]));
                }
            });
            if tw > 0.0 {
                let slot = Rect::from_min_max(pos2(inner.right() - tw, inner.top()), inner.max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(slot).layout(Layout::left_to_right(Align::Center)), |ui| {
                    let name = app.ui.language.tr("Full ribbon").to_string();
                    if tool(ui, "window_header_horizontal", Some(&name), false, None, &name, name.clone()).clicked() {
                        toggle_mode(false);
                    }
                });
            }
        });
}

/// Full mode: a small "Simple ribbon" pill at the right end of the tab row. An `Area`, so `ribbon.rs` is untouched.
fn simple_pill(app: &mut SheetApp, ui: &mut Ui, top: f32) {
    let label = app.ui.language.tr("Simple ribbon").to_string();
    let w = tool_w(Some(text_w(ui, &label)), false);
    let at = pos2(ui.max_rect().right() - w - 14.0, top + 3.0);
    egui::Area::new(egui::Id::new("office_simple_pill")).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
        if tool(ui, "window_header_horizontal_off", Some(&label), false, None, &label, label.clone()).clicked() {
            toggle_mode(true);
        }
    });
}
