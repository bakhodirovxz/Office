//! The phone layout of Docs: a top row, a contextual bottom toolbar and the sheets behind it (Phase 6 WP6, docs/phase6/PLAN.md section 5.1,
//! docs/phase6/phone-layout.md). The app calls `active` every frame and, when it says yes, `chrome` instead of the title bar and the ribbon
//! (the carried hook `patches/wordcraft/0140`). Nothing here runs the desktop layout; with no hook set the app is upstream's.
//!
//! The chrome only reads the session and returns the commands to run (`(id, params)`); the app runs them with its own `run`, so a command
//! that opens a dialog or a file picker behaves as it does from the ribbon. Every control is a command id upstream already has.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use egui::{Align2, Color32, Context, FontId, Id, Painter, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::Session;

/// A window at most this wide (CSS px) is a phone (PLAN D5).
pub const MAX_WIDTH: f32 = 600.0;
/// The height of the top row, the bottom toolbar and every row of a sheet (dp = CSS px = points).
pub const ROW: f32 = 48.0;
/// A sheet is at most this share of the window's height.
pub const SHEET_SHARE: f32 = 0.6;

/// Indices of the colours the app passes (`Tokens` of its theme), so this crate does not depend on the UI crate.
pub mod color {
    pub const BAR: usize = 0;
    pub const BORDER: usize = 1;
    pub const TEXT: usize = 2;
    pub const DIM: usize = 3;
    pub const ACCENT: usize = 4;
    pub const ON_ACCENT: usize = 5;
    pub const PRESSED: usize = 6;
    pub const SHEET: usize = 7;
    pub const COUNT: usize = 8;
}

// ---------------------------------------------------------------------------------------------------------------- trigger

static FORCE: AtomicU8 = AtomicU8::new(0);
static COARSE: AtomicU8 = AtomicU8::new(0);

/// `?phone=on|off` or a setting: `Some(true)` is always the phone layout, `Some(false)` never, `None` automatic.
pub fn set_force(on: Option<bool>) {
    FORCE.store(on.map_or(0, |b| if b { 1 } else { 2 }), Ordering::SeqCst);
}

/// The device's primary pointer is a finger (`(pointer: coarse)`), known before the first touch.
pub fn set_coarse_pointer(coarse: bool) {
    COARSE.store(u8::from(coarse), Ordering::SeqCst);
}

/// The rule (D5), pure: forced on or off, else a finger as the last pointer and a narrow window or a small screen (its short side, so a phone
/// turned to landscape keeps the phone layout; `short_side` is `f32::INFINITY` where the page cannot say, which leaves the window width alone).
pub fn decide(force: Option<bool>, width: f32, short_side: f32, touched: bool) -> bool {
    force.unwrap_or(touched && (width <= MAX_WIDTH || short_side <= MAX_WIDTH))
}

fn forced() -> Option<bool> {
    match FORCE.load(Ordering::SeqCst) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// Points per dp (CSS px): egui's width in points over the page's width in CSS px, 1.0 where the page cannot say (tests, native).
pub fn dp(ctx: &Context) -> f32 {
    CSS_WIDTH.get().map(|f| ctx.content_rect().width() / f()).filter(|d| d.is_finite() && *d > 0.0).unwrap_or(1.0)
}

static CSS_WIDTH: OnceLock<fn() -> f32> = OnceLock::new();
static SCREEN_SHORT: OnceLock<fn() -> f32> = OnceLock::new();

/// The web app tells the layout the short side of the device screen in CSS px (`min(screen.width, screen.height)`): it does not change with the keyboard.
pub fn set_screen_short_side(f: fn() -> f32) {
    let _ = SCREEN_SHORT.set(f);
}

/// The web app tells the layout how wide the window is in CSS px (`window.innerWidth`).
pub fn set_css_width(f: fn() -> f32) {
    let _ = CSS_WIDTH.set(f);
}

/// Phone mode for this frame. "Touched" starts as the pointer type and follows the events: a touch sets it, a mouse button clears it.
pub fn active(ctx: &Context) -> bool {
    let (touched_id, at_id) = (Id::new("docs-office-phone-touched"), Id::new("docs-office-phone-touch-at"));
    let (touch, mouse, now) = ctx.input(|i| {
        let mut r = (false, false, i.time);
        for e in &i.events {
            match e {
                egui::Event::Touch { .. } => r.0 = true,
                egui::Event::PointerButton { .. } => r.1 = true,
                _ => {}
            }
        }
        r
    });
    let before = ctx.data(|d| d.get_temp::<bool>(touched_id)).unwrap_or(COARSE.load(Ordering::SeqCst) == 1);
    let at = ctx.data(|d| d.get_temp::<f64>(at_id)).unwrap_or(f64::NEG_INFINITY);
    // The web runner also reports a finger as pointer events, in frames of their own: a mouse button only counts once the last touch is a second old.
    let after = touched_after(before, touch, mouse, now - at, COARSE.load(Ordering::SeqCst) == 1);
    if touch {
        ctx.data_mut(|d| d.insert_temp(at_id, now));
    }
    if after != before {
        ctx.data_mut(|d| d.insert_temp(touched_id, after));
    }
    let short = SCREEN_SHORT.get().map(|f| f()).filter(|d| *d > 0.0).unwrap_or(f32::INFINITY);
    let on = decide(forced(), ctx.content_rect().width() / dp(ctx), short, after);
    ON.store(on, Ordering::SeqCst);
    on
}

/// A touch sets "touched"; a mouse button clears it once the last touch is `since_touch` > `TOUCH_GRACE` old, except on a device whose primary pointer is a finger
/// (`coarse`): there a pointer press with no touch event in its frame is the finger's own (Android Chrome reports `pointerdown` before `touchstart`), and
/// clearing it made the phone layout flicker off for the press frame, so a tap on the Edit button reached the page under it.
fn touched_after(before: bool, touch: bool, mouse: bool, since_touch: f64, coarse: bool) -> bool {
    if touch {
        true
    } else if mouse && since_touch > TOUCH_GRACE && !coarse {
        false
    } else {
        before
    }
}

static ON: AtomicBool = AtomicBool::new(false);
static REPORTED: AtomicBool = AtomicBool::new(false);
static SHELL: AtomicBool = AtomicBool::new(false);
static BACK: AtomicBool = AtomicBool::new(false);

/// The editor runs inside the shell (`?host=parent`): the top row then has the Back arrow, and the shell hides its own header in phone mode.
pub fn set_shell(inside: bool) {
    SHELL.store(inside, Ordering::SeqCst);
}

/// `Some(on)` once after the phone layout turned on or off, for the shell (`wordcraft:phone`): it hides its header while the editor's row is the only bar.
pub fn take_phone_change() -> Option<bool> {
    change(&REPORTED, ON.load(Ordering::SeqCst))
}

fn change(reported: &AtomicBool, on: bool) -> Option<bool> {
    (reported.swap(on, Ordering::SeqCst) != on).then_some(on)
}

/// True once after the Back arrow was tapped, for the shell (`wordcraft:back`).
pub fn take_back() -> bool {
    BACK.swap(false, Ordering::SeqCst)
}

// ---------------------------------------------------------------------------------------------------------------- reading first

/// A document opens for editing (owner decision 2026-11-03); "Done editing" switches to reading (no caret, no keyboard, no toolbar), the pencil back. The
/// state belongs to one document: another one replacing it opens for editing again.
fn reading_doc_id() -> Id {
    Id::new("docs-office-phone-reading-doc")
}

fn reading_id() -> Id {
    Id::new("docs-office-phone-reading")
}

/// Where the page must leave a finger to the chrome besides the bars (the pencil): read by the app (`office_overlay`), see wordcraft/0143.
fn extra_id() -> Id {
    Id::new("docs-office-phone-extra")
}

/// Sessions that cannot edit (a view session: read-only protection) get no pencil.
pub fn can_edit(s: &Session) -> bool {
    s.doc.settings.protection.as_deref() != Some("readOnly")
}

pub fn is_editing(ctx: &Context, s: &Session) -> bool {
    can_edit(s) && ctx.data(|d| d.get_temp::<Option<u64>>(reading_doc_id())).flatten() != Some(s.document_id())
}

pub fn set_editing(ctx: &Context, s: &Session, on: bool) {
    ctx.data_mut(|d| d.insert_temp(reading_doc_id(), (!on).then(|| s.document_id())));
}

/// The page is for reading this frame: the app lets no tap reach it and gives it no keyboard focus (wordcraft/0143). Set by `chrome`.
pub fn reading(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(reading_id())).unwrap_or(false)
}

/// The Format sheet's two tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Text,
    Paragraph,
}

fn tab_id() -> Id {
    Id::new("docs-office-phone-tab")
}

pub fn tab(ctx: &Context) -> Tab {
    ctx.data(|d| d.get_temp::<Tab>(tab_id())).unwrap_or(Tab::Text)
}

pub fn set_tab(ctx: &Context, t: Tab) {
    ctx.data_mut(|d| d.insert_temp(tab_id(), t));
}

/// Seconds after a touch during which pointer events do not mean a mouse.
const TOUCH_GRACE: f64 = 1.0;

// ---------------------------------------------------------------------------------------------------------------- contexts

/// What the bottom toolbar offers, chosen from the selection (the order is the precedence).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Set {
    Picture,
    Table,
    Selection,
    Caret,
}

pub fn set_of(s: &Session) -> Set {
    if matches!(wordcraft_engine::cmd::objects::selected(s), Some((_, wordcraft_doc::para::InlineObject::Image { .. }))) {
        Set::Picture
    } else if s.sel.focus.path.cell().is_some() {
        Set::Table
    } else if !s.sel.is_collapsed() {
        Set::Selection
    } else {
        Set::Caret
    }
}

/// The sheets (and the top row's menu) that slide up over the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Menu,
    Format,
    List,
    Insert,
    Table,
    Delete,
    Wrap,
    Picture,
}

#[derive(Clone, Copy, Debug)]
pub enum Act {
    Sheet(Sheet),
    /// A command with its parameters as JSON text.
    Cmd(&'static str, &'static str),
    Keyboard,
}

/// One slot of the bottom toolbar: the catalog key of its label, its icon, what it does and the `format.state` flag that shows it pressed.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub label: &'static str,
    pub icon: &'static str,
    pub act: Act,
    pub flag: &'static str,
}

const fn slot(label: &'static str, icon: &'static str, act: Act) -> Slot {
    Slot { label, icon, act, flag: "" }
}
const fn flagged(label: &'static str, icon: &'static str, act: Act, flag: &'static str) -> Slot {
    Slot { label, icon, act, flag }
}

const MORE: Slot = slot("More", "more", Act::Sheet(Sheet::Format));

/// At most five slots (PLAN 5.1), each one at least `ROW` wide on the narrowest phone.
pub fn slots(set: Set) -> Vec<Slot> {
    match set {
        Set::Caret => vec![
            slot("Format", "case", Act::Sheet(Sheet::Format)),
            slot("List", "bullets", Act::Sheet(Sheet::List)),
            slot("Insert", "plus", Act::Sheet(Sheet::Insert)),
            slot("Keyboard", "keyboard", Act::Keyboard),
            slot("More", "more", Act::Sheet(Sheet::Menu)),
        ],
        Set::Selection => vec![
            flagged("Bold", "bold", Act::Cmd("format.bold", "{}"), "bold"),
            flagged("Italic", "italic", Act::Cmd("format.italic", "{}"), "italic"),
            flagged("Underline", "underline", Act::Cmd("format.underline", "{}"), "underline"),
            slot("Highlight", "highlight", Act::Cmd("format.highlight", r#"{"color":"yellow"}"#)),
            MORE,
        ],
        Set::Table => vec![
            slot("Row", "insertBelow", Act::Cmd("table.insertRowBelow", "{}")),
            slot("Column", "insertRight", Act::Cmd("table.insertColumnRight", "{}")),
            slot("Delete", "deleteTable", Act::Sheet(Sheet::Delete)),
            slot("Format", "case", Act::Sheet(Sheet::Format)),
            slot("Table", "table", Act::Sheet(Sheet::Table)),
        ],
        Set::Picture => vec![
            slot("Wrap Text", "wrapText", Act::Sheet(Sheet::Wrap)),
            slot("Size", "size", Act::Cmd("picture.size", "{}")),
            slot("Delete", "deleteTable", Act::Cmd("text.delete", "{}")),
            slot("More", "more", Act::Sheet(Sheet::Picture)),
        ],
    }
}

/// One control of a sheet. `label` is the catalog key (English), `flag` a `format.state` key that shows it pressed, `keep` leaves the sheet open
/// after the command (formatting you repeat), `rgb` draws a colour swatch instead of an icon.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub label: &'static str,
    pub icon: &'static str,
    pub cmd: &'static str,
    pub params: &'static str,
    pub flag: &'static str,
    pub keep: bool,
    pub rgb: u32,
    /// A paragraph style row: its name is drawn in that style's size and weight (1 Normal, 2 Heading 1, 3 Heading 2).
    pub preview: u8,
}

const fn item(label: &'static str, icon: &'static str, cmd: &'static str) -> Item {
    Item { label, icon, cmd, params: "{}", flag: "", keep: false, rgb: 0, preview: 0 }
}
const fn with(i: Item, params: &'static str) -> Item {
    Item { params, ..i }
}
const fn toggle(i: Item, flag: &'static str) -> Item {
    Item { flag, keep: true, ..i }
}
const fn kept(i: Item) -> Item {
    Item { keep: true, ..i }
}
const fn styled(label: &'static str, style: &'static str, level: u8) -> Item {
    Item { params: style, keep: true, preview: level, ..item(label, "", "para.style") }
}
const fn swatch(label: &'static str, cmd: &'static str, params: &'static str, rgb: u32) -> Item {
    Item { label, icon: "", cmd, params, flag: "", keep: true, rgb, preview: 0 }
}

/// The font size between the two buttons, drawn from `format.state`.
pub(super) const SIZE_LABEL: &str = "$size";

type Rows = Vec<Vec<Item>>;

/// The rows of a sheet. A row with one item is a list row (icon and label side by side); more share the width.
pub fn rows(sheet: Sheet, tab: Tab) -> Rows {
    let one = |i: Item| vec![i];
    match sheet {
        Sheet::Menu => {
            // No Print: it opens the Backstage, whose 200 px rail leaves 160 px of page at 360 dp (docs/phase6/phone-layout.md, section 4). No Print/Web
            // Layout switch either: the phone shows the Web Layout, the rest is "Desktop layout".
            let mut v = vec![one(item("Save", "save", "file.save")), one(item("Save As", "saveAs", "file.saveAs"))];
            v.push(one(item("Find", "find", "edit.findReplace")));
            v.push(one(item("Desktop layout", "options", DESKTOP)));
            v
        }
        Sheet::Format if tab == Tab::Text => vec![
            vec![
                toggle(item("Bold", "bold", "format.bold"), "bold"),
                toggle(item("Italic", "italic", "format.italic"), "italic"),
                toggle(item("Underline", "underline", "format.underline"), "underline"),
                toggle(item("Strikethrough", "strike", "format.strikethrough"), "strike"),
            ],
            vec![
                kept(item("Decrease Font Size", "shrink", "format.shrinkFont")),
                Item { keep: true, ..item(SIZE_LABEL, "", "") },
                kept(item("Increase Font Size", "grow", "format.growFont")),
            ],
            vec![
                swatch("Font Color", "format.color", r#"{"color":"auto"}"#, 0x202020),
                swatch("Font Color", "format.color", r#"{"color":"C00000"}"#, 0xC00000),
                swatch("Font Color", "format.color", r#"{"color":"0050C8"}"#, 0x0050C8),
                swatch("Font Color", "format.color", r#"{"color":"2E8B57"}"#, 0x2E8B57),
            ],
            vec![
                swatch("Text Highlight Color", "format.highlight", r#"{"color":"yellow"}"#, 0xFFFF00),
                swatch("Text Highlight Color", "format.highlight", r#"{"color":"brightGreen"}"#, 0x00FF00),
                swatch("Text Highlight Color", "format.highlight", r#"{"color":"turquoise"}"#, 0x00FFFF),
                with(kept(item("No Color", "clear", "format.highlight")), r#"{"color":"none"}"#),
            ],
        ],
        Sheet::Format => vec![
            one(styled("Normal", r#"{"style":"Normal"}"#, 1)),
            one(styled("Heading 1", r#"{"style":"Heading1"}"#, 2)),
            one(styled("Heading 2", r#"{"style":"Heading2"}"#, 3)),
            vec![
                with(kept(item("Align Left", "alignLeft", "para.align")), r#"{"value":"left"}"#),
                with(kept(item("Center", "alignCenter", "para.align")), r#"{"value":"center"}"#),
                with(kept(item("Align Right", "alignRight", "para.align")), r#"{"value":"right"}"#),
                with(kept(item("Justify", "justify", "para.align")), r#"{"value":"justify"}"#),
            ],
            vec![
                item("Bullets", "bullets", "para.bullets"),
                item("Numbering", "numbering", "para.numbering"),
                kept(item("Decrease Indent", "outdent", "para.outdent")),
                kept(item("Increase Indent", "indent", "para.indent")),
            ],
        ],
        Sheet::List => vec![vec![
            item("Bullets", "bullets", "para.bullets"),
            item("Numbering", "numbering", "para.numbering"),
            kept(item("Decrease Indent", "outdent", "para.outdent")),
            kept(item("Increase Indent", "indent", "para.indent")),
        ]],
        Sheet::Insert => vec![
            vec![
                with(item("2 × 2", "table", "insert.table"), r#"{"rows":2,"cols":2}"#),
                with(item("3 × 3", "table", "insert.table"), r#"{"rows":3,"cols":3}"#),
                with(item("4 × 4", "table", "insert.table"), r#"{"rows":4,"cols":4}"#),
            ],
            one(item("Picture", "picture", "insert.picture")),
            one(item("Page Break", "pageBreak", "insert.pageBreak")),
        ],
        Sheet::Table => vec![
            vec![item("Insert Above", "insertAbove", "table.insertRowAbove"), item("Insert Below", "insertBelow", "table.insertRowBelow")],
            vec![item("Insert Left", "insertLeft", "table.insertColumnLeft"), item("Insert Right", "insertRight", "table.insertColumnRight")],
            vec![item("Merge Cells", "merge", "table.merge"), item("Split Cells", "splitCells", "table.split")],
        ],
        Sheet::Delete => vec![
            one(item("Delete Rows", "deleteTable", "table.deleteRow")),
            one(item("Delete Columns", "deleteTable", "table.deleteColumn")),
            one(item("Delete Table", "deleteTable", "table.deleteTable")),
        ],
        Sheet::Wrap => vec![
            one(with(item("In Line with Text", "wrapText", "arrange.wrap"), r#"{"wrap":"inline"}"#)),
            one(with(item("Square", "wrapText", "arrange.wrap"), r#"{"wrap":"square"}"#)),
            one(with(item("Tight", "wrapText", "arrange.wrap"), r#"{"wrap":"tight"}"#)),
            one(with(item("Top and Bottom", "wrapText", "arrange.wrap"), r#"{"wrap":"topAndBottom"}"#)),
            one(with(item("Behind Text", "wrapText", "arrange.wrap"), r#"{"wrap":"behindText"}"#)),
            one(with(item("In Front of Text", "wrapText", "arrange.wrap"), r#"{"wrap":"inFrontOfText"}"#)),
        ],
        Sheet::Picture => vec![one(item("Change Picture", "picture", "picture.change")), one(item("Reset Picture", "resetGraphic", "picture.reset"))],
    }
}

/// The pseudo command of the menu's "Desktop layout": turns the phone layout off for this run (`?phone=on` or a reload brings it back).
pub const DESKTOP: &str = "office.phoneOff";

// ---------------------------------------------------------------------------------------------------------------- keyboard

static KEYBOARD: OnceLock<fn()> = OnceLock::new();

/// The web app's way to show or hide the soft keyboard (the page owns the text input, not this crate).
pub fn set_keyboard(toggle: fn()) {
    let _ = KEYBOARD.set(toggle);
}

// ---------------------------------------------------------------------------------------------------------------- drawing

type IconFn<'a> = &'a dyn Fn(&Painter, Rect, &str, Color32);

fn sheet_id() -> Id {
    Id::new("docs-office-phone-sheet")
}

fn open_sheet(ctx: &Context) -> Option<Sheet> {
    ctx.data(|d| d.get_temp::<Option<Sheet>>(sheet_id())).flatten()
}

fn set_sheet(ctx: &Context, s: Option<Sheet>) {
    ctx.data_mut(|d| d.insert_temp(sheet_id(), s));
}

fn params(p: &str) -> Value {
    serde_json::from_str(p).unwrap_or_else(|_| json!({}))
}

/// Truncate to one row with an ellipsis (a long file name never reaches the buttons beside it).
fn one_line(p: &Painter, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…'), ..Default::default() };
    p.layout_job(job)
}

/// What a finger did on one touch target this frame.
struct Tap {
    clicked: bool,
    down: bool,
}

impl Tap {
    fn clicked(&self) -> bool {
        self.clicked
    }
    fn is_pointer_button_down_on(&self) -> bool {
        self.down
    }
}

fn down_id() -> Id {
    Id::new("docs-office-phone-down")
}

/// A touch target (at least `ROW` in both directions) and the tap on it. egui's own hit test did not report the bar's widgets as under a finger on the
/// web (`contains_pointer` stayed false with the pointer inside, docs/phase6/phone-layout.md section 6), so a tap is read from the pointer events itself: it went down
/// and came up inside `rect`. The widget is still registered so the page does not treat the press as its own.
fn target(ui: &mut Ui, rect: Rect, key: impl std::hash::Hash + std::fmt::Debug) -> Tap {
    let id = ui.id().with(key);
    let _ = ui.interact(rect, id, Sense::click());
    tap_in(ui.ctx(), rect, id)
}

/// The tap on a target that has no widget (the round Edit button is painted on a layer of its own).
fn tap_in(ctx: &Context, rect: Rect, id: Id) -> Tap {
    let (press, release) = button_events(ctx);
    let mut down = ctx.data(|d| d.get_temp::<Option<Id>>(down_id())).flatten();
    if press.is_some_and(|p| rect.contains(p)) {
        down = Some(id);
        ctx.data_mut(|d| d.insert_temp(down_id(), down));
    }
    let up = release.is_some_and(|p| rect.contains(p));
    Tap { clicked: up && down == Some(id), down: down == Some(id) && release.is_none() }
}

/// Where the primary button went down and came up in this frame's events. A quick tap puts both in one frame, and the pointer is already gone
/// by the frame's end (`interact_pos` is `None`), so the events are read, not the pointer state.
fn button_events(ctx: &Context) -> (Option<egui::Pos2>, Option<egui::Pos2>) {
    ctx.input(|i| {
        let mut r = (None, None);
        for e in &i.events {
            if let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, .. } = e {
                if *pressed { r.0 = Some(*pos) } else { r.1 = Some(*pos) }
            }
        }
        r
    })
}

struct Look<'a> {
    /// Points per dp: egui's point need not be a CSS px, so every size here is in dp times this.
    dp: f32,
    c: &'a [Color32; color::COUNT],
    icon: IconFn<'a>,
    t: &'a dyn Fn(&str) -> String,
}

impl Look<'_> {
    fn press(&self, ui: &Ui, r: Rect, resp: &Tap, on: bool) {
        if on {
            ui.painter().rect_filled(r.shrink(3.0 * self.dp), 8.0 * self.dp, self.c[color::ACCENT].gamma_multiply(0.22));
        } else if resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(r.shrink(3.0 * self.dp), 8.0 * self.dp, self.c[color::PRESSED]);
        }
    }

    /// Our own glyphs first (`glyph`), then the app's icon set.
    fn ic(&self, p: &Painter, r: Rect, name: &str, c: Color32) {
        if !glyph::paint(p, r, name, c) {
            (self.icon)(p, r, name, c);
        }
    }

    /// Icon over a label, centred in `r`.
    fn stacked(&self, ui: &Ui, r: Rect, icon: &str, label: &str, tint: Color32) {
        let p = ui.painter();
        self.ic(p, Rect::from_center_size(pos2(r.center().x, r.min.y + 17.0 * self.dp), vec2(22.0, 22.0) * self.dp), icon, tint);
        // a long translated label is set smaller (9 dp at least) before it is cut with an ellipsis
        let room = r.width() - 4.0 * self.dp;
        let natural = p.layout_no_wrap(label.to_string(), FontId::proportional(10.5 * self.dp), tint).size().x;
        let size = (10.5 * self.dp * (room / natural).min(1.0)).max(9.0 * self.dp);
        let g = one_line(p, label, FontId::proportional(size), tint, room);
        p.galley(pos2(r.center().x - g.size().x / 2.0, r.max.y - 5.0 * self.dp - g.size().y), g, tint);
    }

    /// Icon, then label, left-aligned (a list row).
    fn wide(&self, ui: &Ui, r: Rect, icon: &str, label: &str, tint: Color32) {
        let p = ui.painter();
        self.ic(p, Rect::from_center_size(pos2(r.min.x + 28.0 * self.dp, r.center().y), vec2(22.0, 22.0) * self.dp), icon, tint);
        let g = one_line(p, label, FontId::proportional(14.5 * self.dp), tint, r.width() - 64.0 * self.dp);
        p.galley(pos2(r.min.x + 52.0 * self.dp, r.center().y - g.size().y / 2.0), g, tint);
    }

    /// A paragraph style row: the name in that style's size and weight (bold is drawn twice, half a dp apart: the app has one font weight here).
    fn preview(&self, ui: &Ui, r: Rect, level: u8, label: &str, tint: Color32) {
        let (size, bold) = match level {
            2 => (22.0, true),
            3 => (18.0, true),
            _ => (15.0, false),
        };
        let p = ui.painter();
        let g = one_line(p, label, FontId::proportional(size * self.dp), tint, r.width() - 32.0 * self.dp);
        let at = pos2(r.min.x + 16.0 * self.dp, r.center().y - g.size().y / 2.0);
        if bold {
            p.galley(at + vec2(0.6 * self.dp, 0.0), g.clone(), tint);
        }
        p.galley(at, g, tint);
    }

    fn name(&self, key: &str) -> String {
        (self.t)(key)
    }
}

fn state(s: &mut Session) -> Value {
    s.run("format.state", &json!({})).unwrap_or_default()
}

fn flag(st: &Value, k: &str) -> bool {
    !k.is_empty() && st.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn size_text(st: &Value) -> String {
    st.get("size").and_then(Value::as_f64).map(|v| if v.fract() == 0.0 { format!("{v:.0}") } else { v.to_string() }).unwrap_or_default()
}

fn enabled(s: &Session, cmd: &str) -> bool {
    s.registry.get(cmd).is_some_and(|c| (c.enabled)(s).is_none())
}

/// Rows whose long names ("Decrease Font Size") do not fit a quarter of 360 dp: their icon alone, bigger.
const ICON_ONLY: [&str; 4] = ["Decrease Font Size", "Increase Font Size", "Decrease Indent", "Increase Indent"];

/// The sheet header: the grab handle above, the title left and the close button right.
pub const HEAD: f32 = 52.0;
/// The round Edit button's size and its distance from the window's edges.
pub const FAB: f32 = 56.0;
pub const FAB_GAP: f32 = 16.0;
/// The pseudo command of the menu's "Done editing" (the reading view again).
pub const DONE: &str = "office.phoneDone";

/// The title of a sheet (catalog key).
fn sheet_title(sh: Sheet) -> &'static str {
    match sh {
        Sheet::Menu => "More",
        Sheet::Format => "Format",
        Sheet::List => "List",
        Sheet::Insert => "Insert",
        Sheet::Table => "Table",
        Sheet::Delete => "Delete",
        Sheet::Wrap => "Wrap Text",
        Sheet::Picture => "Picture",
    }
}

/// The rows of `sheet` as drawn: the Menu also leads with "Done editing" while editing.
fn sheet_rows(sh: Sheet, tab: Tab, editing: bool) -> Rows {
    let mut r = rows(sh, tab);
    if sh == Sheet::Menu && editing {
        r.insert(0, vec![item("Done editing", "done", DONE)]);
    }
    r
}

/// The Format sheet is as tall as its taller tab, so the sheet does not jump when the tab changes.
fn format_rows() -> usize {
    rows(Sheet::Format, Tab::Text).len().max(rows(Sheet::Format, Tab::Paragraph).len())
}

/// The sheet's height: header, the tabs of the Format sheet, one row each and a little air (the caller caps it).
pub fn sheet_height(sh: Sheet, rows: usize, dp: f32) -> f32 {
    (HEAD + if sh == Sheet::Format { ROW } else { 0.0 } + rows as f32 * ROW + 8.0) * dp
}

/// Draw the top row, the sheet (when one is open), the bottom toolbar (editing only) and the Edit button (reading, when the document can be edited),
/// and return the commands the user asked for. `title` is the file name; `c` the theme colours (see [`color`]); `t` the catalog; `icon` the app's icon painter.
pub fn chrome(s: &mut Session, ui: &mut Ui, title: &str, c: &[Color32; color::COUNT], t: &dyn Fn(&str) -> String, icon: IconFn) -> Vec<(String, Value)> {
    let ctx = ui.ctx().clone();
    let dp = dp(&ctx);
    let row = ROW * dp;
    let look = Look { dp, c, icon, t };
    let st = state(s);
    let set = set_of(s);
    let editing = is_editing(&ctx, s);
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut open = open_sheet(&ctx);
    // reading has the menu only; the sheets of the toolbar belong to editing
    if !editing && open != Some(Sheet::Menu) {
        open = None;
    }
    let frame = |fill: Color32| egui::Frame::NONE.fill(fill).stroke(Stroke::new(1.0, c[color::BORDER]));
    let (mut press, mut release) = button_events(&ctx);

    // --- top row: Back, the file name, then (editing) Undo and Redo, and More
    let shell = SHELL.load(Ordering::SeqCst);
    let top = egui::Panel::top("phone_top").exact_size(row).frame(frame(c[color::BAR])).show(ui, |ui| {
        let full = ui.max_rect();
        let mut x = full.max.x;
        let mut buttons = vec![("More", "more", true)];
        if editing {
            buttons.extend([("Redo", "redo", enabled(s, "edit.redo")), ("Undo", "undo", enabled(s, "edit.undo"))]);
        }
        for (label, ic, on) in buttons {
            x -= row;
            let r = Rect::from_min_size(pos2(x, full.min.y), vec2(row, row));
            let resp = target(ui, r, label);
            look.press(ui, r, &resp, open == Some(Sheet::Menu) && label == "More");
            look.ic(ui.painter(), Rect::from_center_size(r.center(), vec2(22.0, 22.0) * dp), ic, if on { c[color::TEXT] } else { c[color::DIM].gamma_multiply(0.5) });
            if resp.clicked() && on {
                match label {
                    "More" => open = if open == Some(Sheet::Menu) { None } else { Some(Sheet::Menu) },
                    "Undo" => out.push(("edit.undo".into(), json!({}))),
                    _ => out.push(("edit.redo".into(), json!({}))),
                }
            }
        }
        let mut left = full.min.x + 12.0 * dp;
        if shell {
            let r = Rect::from_min_size(full.min, vec2(row, row));
            let resp = target(ui, r, "Back");
            look.press(ui, r, &resp, false);
            look.ic(ui.painter(), Rect::from_center_size(r.center(), vec2(22.0, 22.0) * dp), "phoneBack", c[color::TEXT]);
            if resp.clicked() {
                BACK.store(true, Ordering::SeqCst);
            }
            left = full.min.x + row;
        }
        // the dirty dot, then the name in what is left
        if s.dirty {
            ui.painter().circle_filled(pos2(x - 12.0 * dp, full.center().y), 4.0 * dp, c[color::ACCENT]);
            x -= 24.0 * dp;
        }
        let g = one_line(ui.painter(), title, FontId::proportional(15.0 * dp), c[color::TEXT], x - left - 8.0 * dp);
        ui.painter().galley(pos2(left, full.center().y - g.size().y / 2.0), g, c[color::TEXT]);
    });

    // --- bottom toolbar (declared before the sheet so the sheet sits above it)
    let slots = slots(set);
    let bar = editing.then(|| {
        egui::Panel::bottom("phone_bar").exact_size(row).frame(frame(c[color::BAR])).show(ui, |ui| {
            let full = ui.max_rect();
            let w = full.width() / slots.len() as f32;
            for (i, sl) in slots.iter().enumerate() {
                let r = Rect::from_min_size(pos2(full.min.x + w * i as f32, full.min.y), vec2(w, row));
                let resp = target(ui, r, ("slot", i));
                let on = flag(&st, sl.flag) || matches!(sl.act, Act::Sheet(sh) if open == Some(sh));
                look.press(ui, r, &resp, on);
                let tint = if on { c[color::ACCENT] } else { c[color::TEXT] };
                look.stacked(ui, r, sl.icon, &look.name(sl.label), tint);
                if resp.clicked() {
                    match sl.act {
                        Act::Sheet(sh) => open = if open == Some(sh) { None } else { Some(sh) },
                        Act::Cmd(id, p) => out.push((id.into(), params(p))),
                        Act::Keyboard => {
                            if let Some(k) = KEYBOARD.get() {
                                k();
                            }
                        }
                    }
                }
            }
        })
    });

    // --- the sheet: a grab handle, a header (title, close), the Format tabs and the rows
    let mut sheet_rect = None;
    let mut done = false;
    if let Some(sh) = open {
        let tb = tab(&ctx);
        let rws = sheet_rows(sh, tb, editing);
        let max_h = ctx.content_rect().height() * SHEET_SHARE;
        let r = egui::Panel::bottom("phone_sheet").exact_size(sheet_height(sh, if sh == Sheet::Format { format_rows() } else { rws.len() }, dp).min(max_h)).frame(frame(c[color::SHEET])).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD * dp), Sense::hover());
            let p = ui.painter();
            p.rect_filled(Rect::from_center_size(pos2(head.center().x, head.min.y + 6.0 * dp), vec2(32.0, 4.0) * dp), 2.0 * dp, c[color::BORDER]);
            let g = one_line(p, &look.name(sheet_title(sh)), FontId::proportional(17.0 * dp), c[color::TEXT], head.width() - row - 32.0 * dp);
            p.galley(pos2(head.min.x + 16.0 * dp, head.min.y + 4.0 * dp + (row - g.size().y) / 2.0), g, c[color::TEXT]);
            let x = Rect::from_min_size(pos2(head.max.x - row, head.min.y + 4.0 * dp), vec2(row, row));
            let resp = target(ui, x, "close");
            look.press(ui, x, &resp, false);
            look.ic(ui.painter(), Rect::from_center_size(x.center(), vec2(20.0, 20.0) * dp), "close", c[color::TEXT]);
            if resp.clicked() {
                open = None;
            }
            if sh == Sheet::Format {
                let (tabs, _) = ui.allocate_exact_size(vec2(ui.available_width(), row), Sense::hover());
                for (i, (t, name)) in [(Tab::Text, "Text"), (Tab::Paragraph, "Paragraph")].into_iter().enumerate() {
                    let r = Rect::from_min_size(pos2(tabs.min.x + tabs.width() / 2.0 * i as f32, tabs.min.y), vec2(tabs.width() / 2.0, row));
                    let resp = target(ui, r, ("tab", i));
                    let tint = if t == tb { c[color::ACCENT] } else { c[color::DIM] };
                    look.press(ui, r, &resp, false);
                    let g = one_line(ui.painter(), &look.name(name), FontId::proportional(15.0 * dp), tint, r.width() - 8.0 * dp);
                    ui.painter().galley(pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, tint);
                    let (y, w) = (r.max.y - 1.0 * dp, if t == tb { 3.0 } else { 1.0 } * dp);
                    ui.painter().line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(w, if t == tb { c[color::ACCENT] } else { c[color::BORDER] }));
                    if resp.clicked() {
                        set_tab(&ctx, t);
                    }
                }
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for cells in &rws {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row), Sense::hover());
                    let w = rect.width() / cells.len() as f32;
                    for (i, it) in cells.iter().enumerate() {
                        let r = Rect::from_min_size(pos2(rect.min.x + w * i as f32, rect.min.y), vec2(w, row));
                        let size = it.label == SIZE_LABEL;
                        let live = it.cmd.is_empty() || it.cmd.starts_with("ui.") || enabled(s, it.cmd) || it.cmd == DESKTOP || it.cmd == DONE;
                        let resp = target(ui, r, (it.label, it.params, i));
                        let on = flag(&st, it.flag);
                        if !size {
                            look.press(ui, r, &resp, on);
                        }
                        let tint = match (live, on) {
                            (false, _) => c[color::DIM].gamma_multiply(0.5),
                            (_, true) => c[color::ACCENT],
                            _ => c[color::TEXT],
                        };
                        if size {
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, size_text(&st), FontId::proportional(16.0 * dp), c[color::TEXT]);
                        } else if it.rgb != 0 {
                            let col = Color32::from_rgb((it.rgb >> 16) as u8, (it.rgb >> 8) as u8, it.rgb as u8);
                            ui.painter().circle_filled(r.center(), 13.0 * dp, col);
                            ui.painter().circle_stroke(r.center(), 13.0 * dp, Stroke::new(1.0, c[color::BORDER]));
                        } else if ICON_ONLY.contains(&it.label) {
                            look.ic(ui.painter(), Rect::from_center_size(r.center(), vec2(26.0, 26.0) * dp), it.icon, tint);
                        } else if it.preview != 0 {
                            look.preview(ui, r, it.preview, &look.name(it.label), tint);
                        } else if cells.len() == 1 {
                            look.wide(ui, r, it.icon, &look.name(it.label), tint);
                        } else {
                            look.stacked(ui, r, it.icon, &look.name(it.label), tint);
                        }
                        if resp.clicked() && live && !size {
                            if it.cmd == DESKTOP {
                                set_force(Some(false));
                                open = None;
                            } else if it.cmd == DONE {
                                done = true;
                                open = None;
                            } else {
                                out.push((it.cmd.into(), params(it.params)));
                                if !it.keep {
                                    open = None;
                                }
                            }
                        }
                    }
                }
            });
        });
        sheet_rect = Some(r.response.rect);
    }

    // --- the Edit button: reading a document that can be edited, bottom right above the page
    let mut fab = None;
    if !editing && can_edit(s) {
        let page = ui.available_rect_before_wrap();
        let d = FAB * dp;
        let r = Rect::from_min_size(pos2(page.max.x - FAB_GAP * dp - d, page.max.y - FAB_GAP * dp - d), vec2(d, d));
        let resp = tap_in(&ctx, r, Id::new("docs-office-phone-fab"));
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("docs-office-phone-fab-layer")));
        p.circle_filled(r.center() + vec2(0.0, 2.0 * dp), d / 2.0, Color32::from_black_alpha(60));
        p.circle_filled(r.center(), d / 2.0 - if resp.is_pointer_button_down_on() { 1.5 * dp } else { 0.0 }, c[color::ACCENT]);
        look.ic(&p, Rect::from_center_size(r.center(), vec2(24.0, 24.0) * dp), "phonePencil", c[color::ON_ACCENT]);
        if resp.clicked() {
            set_editing(&ctx, s, true);
            open = None;
        }
        fab = Some(r);
    }
    ctx.data_mut(|d| d.insert_temp(extra_id(), fab));

    // a press outside the bars and the sheet closes the sheet; one on a bar decides for itself above
    if open.is_some()
        && let Some(p) = press.take()
        && ![Some(top.response.rect), bar.as_ref().map(|b| b.response.rect), sheet_rect].into_iter().flatten().any(|r| r.contains(p))
    {
        open = None;
    }
    if done {
        set_editing(&ctx, s, false);
    }
    set_sheet(&ctx, open);
    // the finger is up: nothing is pressed any more (after the sheet, whose rows read it too)
    if release.take().is_some() {
        ctx.data_mut(|d| d.insert_temp(down_id(), None::<Id>));
    }
    let editing = is_editing(&ctx, s);
    ctx.data_mut(|d| d.insert_temp(reading_id(), !editing));
    out
}

mod glyph;

#[cfg(test)]
mod tests;
