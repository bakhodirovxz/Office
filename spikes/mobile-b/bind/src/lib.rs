//! wordcraft-bind: a thin, shell-neutral binding over the wordcraft engine (Spike 3, approach B).
//!
//! No egui, no winit. One `Editor` owns an engine `Session`. A shell (a WebView page through
//! wasm-bindgen, Flutter through flutter_rust_bridge, a native C ABI) draws pages from
//! `render_tile`, draws the caret and the selection from `caret` / `selection_rects`, and feeds
//! pointer and IME input through `tap`, `text_context`, `replace_utf16`, `insert`, `delete_*`.
//! Everything else is `run(id, json)`, the engine's one command entry point.
//!
//! The engine has no composition API: a shell keeps the preedit itself and commits with
//! `replace_utf16` / `insert` (see `docs/phase0/mobile-b.md`).
use serde_json::{Value, json};
use vello_cpu::kurbo::Affine;
use wordcraft_doc::{Document, Pos, StoryRef};
use wordcraft_engine::{Region, Selection, Session, blank_document, io};
use wordcraft_render::{RenderOptions, render_region};

/// Result type of every call: the engine's error text.
pub type Res<T> = Result<T, String>;

/// Caret geometry in page points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretRect {
    pub page: u32,
    pub x: f32,
    pub top: f32,
    pub height: f32,
    pub width: f32,
}

/// What the IME needs to see around the caret (UTF-16 units, like a DOM textarea).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextContext {
    /// The paragraph's text; a leading `\n` stands for "start of paragraph" so Backspace and
    /// auto-capitalisation have something to act on.
    pub text: String,
    pub sel_start: u32,
    pub sel_end: u32,
}

/// Where the last `text_context` came from, so `replace_utf16` can map offsets back.
#[derive(Clone)]
struct Mirror {
    pos: Pos, // the paragraph (off is ignored)
    base_byte: usize,
    sentinel: bool,
    body: String, // the mirrored text without the sentinel
}

pub struct Editor {
    s: Session,
    mirror: Option<Mirror>,
}

fn u16_to_byte(s: &str, u16_off: usize) -> usize {
    let mut n = 0;
    for (b, c) in s.char_indices() {
        if n >= u16_off {
            return b;
        }
        n += c.len_utf16();
    }
    s.len()
}
fn byte_to_u16(s: &str, byte: usize) -> usize {
    s.get(..byte.min(s.len())).map_or(0, |h| h.encode_utf16().count())
}

impl Editor {
    fn wrap(doc: Document) -> Editor {
        Editor { s: Session::new(doc), mirror: None }
    }
    /// A blank document on the default paper.
    pub fn blank() -> Editor {
        Editor::wrap(blank_document(&Region::default()))
    }
    /// Open `bytes`; `name` gives the format by extension (`.docx`, `.doc`, `.txt`, ...).
    pub fn open(name: &str, bytes: &[u8]) -> Res<Editor> {
        let (doc, _encrypted) = io::open_bytes_with(name, bytes, None)?;
        let mut e = Editor::wrap(doc);
        e.s.file_name = Some(name.to_string());
        Ok(e)
    }
    /// Run any engine command by id with JSON params; returns the JSON result.
    pub fn run(&mut self, id: &str, params_json: &str) -> Res<String> {
        let params: Value = if params_json.trim().is_empty() { json!({}) } else { serde_json::from_str(params_json).map_err(|e| e.to_string())? };
        self.s.run(id, &params).map(|v| v.to_string()).map_err(|e| e.to_string())
    }
    fn cmd(&mut self, id: &str, params: Value) -> Res<()> {
        self.s.run(id, &params).map(|_| ()).map_err(|e| e.to_string())
    }

    // ---- pages and pixels -------------------------------------------------------------
    pub fn page_count(&mut self) -> u32 {
        self.s.layout().pages.len() as u32
    }
    pub fn page_size_pt(&mut self, page: u32) -> (f32, f32) {
        self.s.layout().pages.get(page as usize).map_or((0.0, 0.0), |p| (p.w, p.h))
    }
    /// Layout time of the current layout, ms (the engine's own measure).
    pub fn layout_ms(&mut self) -> f64 {
        self.s.layout().ms
    }
    /// Render the `w`x`h` pixel window at (`x`,`y`) of `page` drawn at `scale` pixels per point.
    /// Premultiplied RGBA8, row-major (pages are opaque, so it is also straight RGBA).
    pub fn render_tile(&mut self, page: u32, scale: f32, x: u32, y: u32, w: u32, h: u32) -> Res<Vec<u8>> {
        let layout = self.s.layout();
        let pg = layout.pages.get(page as usize).ok_or("no such page")?;
        let view = Affine::translate((-(x as f64), -(y as f64))) * Affine::scale(scale as f64);
        Ok(render_region(&self.s.doc, pg, w, h, view, &RenderOptions::default()).pixels)
    }

    // ---- pointer, caret, selection ---------------------------------------------------------
    /// Pointer at (`x_pt`,`y_pt`) on `page`: sets the caret, or with `extend` the selection end.
    pub fn tap(&mut self, page: u32, x_pt: f32, y_pt: f32, extend: bool) -> Res<()> {
        self.cmd("caret.set", json!({"page": page, "x": x_pt, "y": y_pt, "extend": extend}))
    }
    pub fn select_word(&mut self) -> Res<()> {
        self.cmd("select.word", json!({}))
    }
    pub fn caret(&mut self) -> Option<CaretRect> {
        let l = self.s.layout();
        let c = l.caret_on(&self.s.sel.focus, self.s.page_hint)?;
        Some(CaretRect { page: c.page as u32, x: c.x, top: c.top, height: c.height, width: c.width })
    }
    /// Highlight rectangles `(page, x, y, w, h)` in page points; empty for a caret.
    pub fn selection_rects(&mut self) -> Vec<(u32, f32, f32, f32, f32)> {
        if self.s.sel.is_collapsed() {
            return Vec::new();
        }
        let l = self.s.layout();
        let (a, b) = self.s.sel.ordered();
        l.selection_rects(&self.s.doc, &a, &b, self.s.page_hint).into_iter().map(|(p, r)| (p as u32, r.x, r.y, r.w, r.h)).collect()
    }
    pub fn selected_text(&self) -> String {
        self.s.selected_text()
    }

    // ---- text input ------------------------------------------------------------------------
    /// The paragraph around the selection as an IME would see it, at most `before` chars before
    /// and `after` after the caret. Remembers the mapping for `replace_utf16`.
    pub fn text_context(&mut self, before: usize, after: usize) -> TextContext {
        let f = self.s.sel.focus.clone();
        let Some(p) = self.s.doc.para_at(&f) else { return TextContext::default() };
        let full = p.text.as_str();
        let off = f.off.min(full.len());
        let start = if before == 0 { off } else { full[..off].char_indices().rev().nth(before - 1).map_or(0, |(b, _)| b) };
        let end = full[off..].char_indices().nth(after).map_or(full.len(), |(b, _)| off + b);
        let sentinel = start == 0;
        let body = full[start..end].to_string();
        let (a, b) = self.s.sel.ordered();
        let same_para = a.story == b.story && a.path == b.path && a.path == f.path;
        let (sa, sb) = if same_para { (a.off, b.off) } else { (f.off, f.off) };
        let conv = |o: usize| (usize::from(sentinel) + byte_to_u16(&body, o.clamp(start, end) - start)) as u32;
        let ctx = TextContext { text: format!("{}{}", if sentinel { "\n" } else { "" }, body), sel_start: conv(sa), sel_end: conv(sb) };
        self.mirror = Some(Mirror { pos: f, base_byte: start, sentinel, body });
        ctx
    }
    /// Insert `s` at the caret (replacing the selection). `\n` becomes a new paragraph.
    pub fn insert(&mut self, s: &str) -> Res<()> {
        if s.is_empty() {
            return Ok(());
        }
        self.cmd("text.insert", json!({"text": s.replace('\n', "\r")}))
    }
    /// IME `deleteSurroundingText(before, 0)`.
    pub fn delete_before(&mut self, n: usize) -> Res<()> {
        (0..n).try_for_each(|_| self.cmd("text.backspace", json!({})))
    }
    /// IME `deleteSurroundingText(0, after)`.
    pub fn delete_after(&mut self, n: usize) -> Res<()> {
        (0..n).try_for_each(|_| self.cmd("text.delete", json!({})))
    }
    fn mirror_pos(m: &Mirror, u16_off: u32) -> Pos {
        let rel = (u16_off as usize).saturating_sub(usize::from(m.sentinel));
        Pos { off: m.base_byte + u16_to_byte(&m.body, rel), ..m.pos.clone() }
    }
    /// Replace `from..to` (UTF-16 units of the last `text_context().text`) with `text`: the one
    /// primitive a textarea diff needs (typing, autocorrect, delete, Enter, cut of a word).
    /// Deleting the leading sentinel joins the paragraph with the previous one.
    pub fn replace_utf16(&mut self, from: u32, to: u32, text: &str) -> Res<()> {
        let m = self.mirror.clone().ok_or("call text_context first")?;
        let join_back = m.sentinel && from == 0 && to >= 1;
        let from = if join_back { 1 } else { from };
        let to = to.max(from);
        let (a, b) = (Self::mirror_pos(&m, from), Self::mirror_pos(&m, to));
        self.s.sel = Selection { anchor: a, focus: b };
        if !text.is_empty() {
            self.insert(text)?;
        } else if from != to {
            self.cmd("text.backspace", json!({}))?;
        }
        if join_back {
            self.s.sel = Selection::caret(Pos { off: 0, ..m.pos.clone() });
            self.cmd("text.backspace", json!({}))?;
        }
        Ok(())
    }
    /// Move the caret or select inside the mirrored paragraph (IME cursor moves).
    pub fn select_utf16(&mut self, start: u32, end: u32) -> Res<()> {
        let m = self.mirror.clone().ok_or("call text_context first")?;
        self.s.sel = Selection { anchor: Self::mirror_pos(&m, start), focus: Self::mirror_pos(&m, end) };
        Ok(())
    }

    // ---- state for toolbars --------------------------------------------------------------------
    pub fn can_undo(&self) -> bool {
        self.s.can_undo()
    }
    pub fn dirty(&self) -> bool {
        self.s.dirty
    }
    pub fn bold_at_caret(&self) -> bool {
        self.s.typing_props().bold == Some(true)
    }
    pub fn word_count(&self) -> u32 {
        self.s.word_count() as u32
    }
    pub fn plain_text(&self) -> String {
        self.s.doc.plain_text(StoryRef::Body)
    }
    /// Serialise; `name` gives the format by extension (`x.docx`, `x.pdf`, `x.txt`, ...).
    pub fn save(&mut self, name: &str) -> Res<Vec<u8>> {
        self.s.stamp_save();
        let bytes = io::save_bytes_with(name, &self.s.doc, None, &io::ExportOptions::default())?;
        self.s.dirty = false;
        Ok(bytes)
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = console)]
        fn error(s: &str);
    }
    fn js(e: String) -> JsValue {
        JsValue::from_str(&e)
    }

    #[wasm_bindgen]
    pub fn init() {
        std::panic::set_hook(Box::new(|i| error(&i.to_string())));
    }

    #[wasm_bindgen]
    pub struct WcEditor(Editor);

    #[wasm_bindgen(getter_with_clone)]
    pub struct TextCtx {
        pub text: String,
        pub sel_start: u32,
        pub sel_end: u32,
    }

    #[wasm_bindgen]
    impl WcEditor {
        pub fn blank() -> WcEditor {
            WcEditor(Editor::blank())
        }
        pub fn open(name: &str, bytes: &[u8]) -> Result<WcEditor, JsValue> {
            Editor::open(name, bytes).map(WcEditor).map_err(js)
        }
        pub fn run(&mut self, id: &str, params_json: &str) -> Result<String, JsValue> {
            self.0.run(id, params_json).map_err(js)
        }
        pub fn page_count(&mut self) -> u32 {
            self.0.page_count()
        }
        /// `[w, h]` in points.
        pub fn page_size_pt(&mut self, page: u32) -> Vec<f32> {
            let (w, h) = self.0.page_size_pt(page);
            vec![w, h]
        }
        pub fn layout_ms(&mut self) -> f64 {
            self.0.layout_ms()
        }
        pub fn render_tile(&mut self, page: u32, scale: f32, x: u32, y: u32, w: u32, h: u32) -> Result<wasm_bindgen::Clamped<Vec<u8>>, JsValue> {
            self.0.render_tile(page, scale, x, y, w, h).map(wasm_bindgen::Clamped).map_err(js)
        }
        pub fn tap(&mut self, page: u32, x_pt: f32, y_pt: f32, extend: bool) -> Result<(), JsValue> {
            self.0.tap(page, x_pt, y_pt, extend).map_err(js)
        }
        pub fn select_word(&mut self) -> Result<(), JsValue> {
            self.0.select_word().map_err(js)
        }
        /// `[page, x, top, height, width]` or empty.
        pub fn caret(&mut self) -> Vec<f32> {
            self.0.caret().map_or_else(Vec::new, |c| vec![c.page as f32, c.x, c.top, c.height, c.width])
        }
        /// Flat `[page, x, y, w, h, ...]`.
        pub fn selection_rects(&mut self) -> Vec<f32> {
            self.0.selection_rects().into_iter().flat_map(|(p, x, y, w, h)| [p as f32, x, y, w, h]).collect()
        }
        pub fn selected_text(&self) -> String {
            self.0.selected_text()
        }
        pub fn text_context(&mut self, before: usize, after: usize) -> TextCtx {
            let c = self.0.text_context(before, after);
            TextCtx { text: c.text, sel_start: c.sel_start, sel_end: c.sel_end }
        }
        pub fn insert(&mut self, s: &str) -> Result<(), JsValue> {
            self.0.insert(s).map_err(js)
        }
        pub fn delete_before(&mut self, n: usize) -> Result<(), JsValue> {
            self.0.delete_before(n).map_err(js)
        }
        pub fn delete_after(&mut self, n: usize) -> Result<(), JsValue> {
            self.0.delete_after(n).map_err(js)
        }
        pub fn replace_utf16(&mut self, from: u32, to: u32, text: &str) -> Result<(), JsValue> {
            self.0.replace_utf16(from, to, text).map_err(js)
        }
        pub fn select_utf16(&mut self, start: u32, end: u32) -> Result<(), JsValue> {
            self.0.select_utf16(start, end).map_err(js)
        }
        pub fn can_undo(&self) -> bool {
            self.0.can_undo()
        }
        pub fn dirty(&self) -> bool {
            self.0.dirty()
        }
        pub fn bold_at_caret(&self) -> bool {
            self.0.bold_at_caret()
        }
        pub fn word_count(&self) -> u32 {
            self.0.word_count()
        }
        pub fn plain_text(&self) -> String {
            self.0.plain_text()
        }
        pub fn save(&mut self, name: &str) -> Result<Vec<u8>, JsValue> {
            self.0.save(name).map_err(js)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn fixture() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/sample.docx")).expect("fixture")
    }
    fn docx_xml(bytes: &[u8]) -> String {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
        let mut s = String::new();
        z.by_name("word/document.xml").expect("document.xml").read_to_string(&mut s).expect("utf8");
        s
    }

    #[test]
    fn open_insert_save_contains_x() {
        let mut e = Editor::open("sample.docx", &fixture()).expect("open");
        assert!(e.page_count() >= 1);
        assert!(!docx_xml(&fixture()).contains('X'), "fixture must not contain X already");
        e.run("caret.docEnd", "{}").expect("docEnd");
        e.insert("X").expect("insert");
        let out = e.save("out.docx").expect("save");
        assert!(docx_xml(&out).contains('X'), "saved docx lacks X");
        let again = Editor::open("out.docx", &out).expect("reopen");
        assert!(again.plain_text().contains('X'));
    }

    #[test]
    fn tap_render_caret_and_ime_edits() {
        let mut e = Editor::open("sample.docx", &fixture()).expect("open");
        let (w, h) = e.page_size_pt(0);
        assert!(w > 100.0 && h > 100.0);
        let px = e.render_tile(0, 1.0, 0, 0, w as u32, h as u32).expect("tile");
        assert_eq!(px.len(), (w as u32 * h as u32 * 4) as usize);
        assert!(px.chunks(4).any(|p| p[0] < 128), "page has dark (ink) pixels");
        let tile = e.render_tile(0, 1.0, 10, 10, 20, 20).expect("tile");
        assert_eq!(tile.len(), 20 * 20 * 4);
        e.tap(0, 100.0, 100.0, false).expect("tap");
        assert!(e.caret().is_some());
        // IME mirror: replace a word like autocorrect does (teh -> the)
        e.run("caret.docStart", "{}").expect("start");
        e.insert("teh ").expect("type");
        let c = e.text_context(400, 400);
        let at = c.text.find("teh ").expect("teh in context") as u32;
        e.replace_utf16(at, at + 4, "the ").expect("replace");
        assert!(e.plain_text().starts_with("the "), "{}", e.plain_text());
        // Enter, then Backspace through the sentinel joins the paragraphs again
        let n0 = e.plain_text().matches('\n').count();
        e.insert("\n").expect("enter");
        assert_eq!(e.plain_text().matches('\n').count(), n0 + 1);
        let c = e.text_context(400, 400);
        assert_eq!(c.sel_start, 1, "caret at paragraph start, after the sentinel");
        e.replace_utf16(0, 1, "").expect("join");
        assert_eq!(e.plain_text().matches('\n').count(), n0);
    }
}
