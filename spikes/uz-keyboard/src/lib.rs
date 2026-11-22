//! Throwaway Phase 0 spike (docs/keyboard/PLAN.md): the smallest TSF text input processor (TIP) that
//! turns s+h into ş (S+h, S+H into Ş) while typing, with Backspace undo. No product code.
//!
//! Model: the TIP keeps only the pending tail in a TSF composition.
//! - `s` starts a composition holding "s"; `h` turns it into "ş" and the composition stays open;
//! - Backspace while it holds "ş" commits the typed digraph ("sh") instead; while it holds "s", removes it;
//! - any other key commits the composition and then reaches the application unchanged.
//!
//! Written from the public TSF documentation (learn.microsoft.com, "Text Services Framework").
//! No code is copied from Windows-classic-samples SampleIME.

use std::cell::{Cell, RefCell};
use std::io::Write as _;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

pub const CLSID_SPIKE: GUID = GUID::from_u128(0x71a6784f_8eca_4fc0_92c0_0b1756de11cb);
pub const PROFILE_SPIKE: GUID = GUID::from_u128(0xf617e325_d178_40a9_9070_bf4b61b266b4);
/// uz-Latn-UZ.
pub const LANGID_UZ_LATN: u16 = 0x0443;

static OBJECTS: AtomicI32 = AtomicI32::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Spike log: %TEMP%\uzkbd-spike.log (the TIP runs inside other processes, so stdout is not available).
pub fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("uzkbd-spike.log")
}
fn log(line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() % 100_000).unwrap_or(0);
        let _ = writeln!(f, "{} {ms:05} {line}", std::process::id());
    }
}

#[derive(Default)]
struct State {
    tid: Cell<u32>,
    tm: RefCell<Option<ITfThreadMgr>>,
    comp: RefCell<Option<ITfComposition>>,
    /// The keys typed into the composition ("s", "sh", "Sh", ...).
    typed: RefCell<String>,
    /// One probe per activation: can the TIP read the committed text before the caret?
    probed: Cell<bool>,
}

#[implement(ITfTextInputProcessor, ITfKeyEventSink, ITfCompositionSink)]
struct Tip {
    st: Rc<State>,
}

#[implement(ITfEditSession)]
struct Session {
    f: RefCell<Option<Box<dyn FnOnce(u32) -> Result<()>>>>,
}

impl ITfEditSession_Impl for Session_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        match self.f.borrow_mut().take() {
            Some(f) => f(ec),
            None => Ok(()),
        }
    }
}

fn shift_down() -> bool {
    unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 }
}
fn caps_on() -> bool {
    unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 }
}
fn ctrl_or_alt() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 || GetKeyState(VK_MENU.0 as i32) < 0 }
}
fn is_modifier(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_CAPITAL
    )
}
/// shortcut: assumes a US-style layout under the TIP (true for Uzbek Latin); the product uses ToUnicodeEx.
fn letter(base: char) -> char {
    if shift_down() ^ caps_on() { base.to_ascii_uppercase() } else { base }
}

/// What the composition shows for the keys typed into it. R10: the first letter decides the case.
pub fn shown(typed: &str) -> String {
    let t: Vec<char> = typed.chars().collect();
    if t.len() != 2 {
        return typed.into();
    }
    let up = t[0].is_ascii_uppercase();
    let l = match (t[0].to_ascii_lowercase(), t[1].to_ascii_lowercase()) {
        ('s', 'h') => ('ş', 'Ş'),
        ('c', 'h') => ('ç', 'Ç'),
        ('o', '\'') => ('ö', 'Ö'),
        ('g', '\'') => ('ğ', 'Ğ'),
        _ => return typed.into(),
    };
    (if up { l.1 } else { l.0 }).to_string()
}

/// A key as the conversion sees it.
#[derive(Clone, Copy, Debug)]
pub enum Key {
    /// s c o g h (either case) or the apostrophe `'`.
    Ch(char),
    Back,
    Other,
}

#[derive(Debug, PartialEq)]
pub enum Act {
    /// Commit any open composition, then start a new one with this text.
    Start(String),
    /// Replace the composition's text.
    Update(String),
    /// Replace the composition's text (may be empty), then commit it.
    Finish(String),
    /// Commit the composition; the key goes on to the application.
    Commit,
}

/// The conversion state machine (docs/keyboard/PLAN.md 2.1). `typed`: keys in the composition; `None`: the key is
/// not ours and nothing is pending.
pub fn decide(typed: &str, composing: bool, key: Key) -> Option<Act> {
    let t: Vec<char> = typed.chars().collect();
    let first = t.first().map(|c| c.to_ascii_lowercase());
    match key {
        Key::Ch(c) => {
            let lc = c.to_ascii_lowercase();
            if composing {
                match (first, t.len(), lc) {
                    (Some('s' | 'c'), 1, 'h') | (Some('o' | 'g' | 's'), 1, '\'') => return Some(Act::Update(format!("{typed}{c}"))),
                    // s'h: the 1995 separator; 2026 writes plain sh (rules.md 15.2)
                    (Some('s'), 2, 'h') if t[1] == '\'' => return Some(Act::Finish(format!("{}{c}", t[0]))),
                    _ => {}
                }
            }
            if matches!(lc, 's' | 'c' | 'o' | 'g') {
                Some(Act::Start(c.to_string()))
            } else {
                composing.then_some(Act::Commit)
            }
        }
        Key::Back if composing => match t.len() {
            1 => Some(Act::Finish(String::new())),
            _ if first == Some('s') && t[1] == '\'' => Some(Act::Update(t[0].to_string())),
            _ => Some(Act::Finish(typed.into())),
        },
        Key::Back => None,
        Key::Other => composing.then_some(Act::Commit),
    }
}

/// Test cases shared by the unit test and kbdctl: (typed, expected with the TIP). '\u{8}' = Backspace, '\n' = Enter.
pub const CASES: &[(&str, &str)] = &[
    ("shahar ", "şahar "),
    ("Shahar ", "Şahar "),
    ("SHAHAR ", "ŞAHAR "),
    ("sHa ", "şa "),
    ("choy ", "çoy "),
    ("Choy ", "Çoy "),
    ("CHOY ", "ÇOY "),
    ("o'zbek ", "özbek "),
    ("O'zbek ", "Özbek "),
    ("O'ZBEK ", "ÖZBEK "),
    ("g'isht ", "ğişt "),
    ("G'ani ", "Ğani "),
    ("to'g'ri ", "töğri "),
    ("Is'hoq ", "Ishoq "),
    ("Ishoq ", "Işoq "),
    ("Ish\u{8}oq ", "Ishoq "),
    ("o'\u{8}zbek ", "o'zbek "),
    ("a'lo ", "a'lo "),
    ("maktab ", "maktab "),
    ("as\u{8}b ", "ab "),
    ("ssh ", "sş "),
    ("tosh\n", "toş\n"),
    ("qush\u{8}\u{8}x ", "qusx "),
];

/// What the text is without the TIP: Backspace deletes, everything else is typed.
pub fn plain(typed: &str) -> String {
    let mut s = String::new();
    for c in typed.chars() {
        if c == '\u{8}' {
            s.pop();
        } else {
            s.push(c);
        }
    }
    s
}

/// Moves the selection to the end of `range` (so the caret follows the composition).
unsafe fn caret_to_end(ec: u32, ctx: &ITfContext, range: &ITfRange) -> Result<()> {
    unsafe {
        let r = range.Clone()?;
        r.Collapse(ec, TF_ANCHOR_END)?;
        let sel = TF_SELECTION {
            range: ManuallyDrop::new(Some(r)),
            style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: false.into() },
        };
        let res = ctx.SetSelection(ec, std::slice::from_ref(&sel));
        drop(ManuallyDrop::into_inner(sel.range));
        res
    }
}

/// Reads up to `n` characters before the caret (probe for word-level rules in the plan).
unsafe fn text_before_caret(ec: u32, ctx: &ITfContext, n: i32) -> Result<String> {
    unsafe {
        let mut sel = [TF_SELECTION::default()];
        let mut got = 0u32;
        ctx.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut got)?;
        let range = ManuallyDrop::into_inner(std::mem::take(&mut sel[0].range)).ok_or(Error::from(E_FAIL))?;
        let r = range.Clone()?;
        r.Collapse(ec, TF_ANCHOR_START)?;
        let mut moved = 0i32;
        r.ShiftStart(ec, -n, &mut moved, std::ptr::null())?;
        let mut buf = vec![0u16; n as usize];
        let mut len = 0u32;
        r.GetText(ec, 0, &mut buf, &mut len)?;
        Ok(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

impl Tip_Impl {
    fn composing(&self) -> bool {
        self.st.comp.borrow().is_some()
    }

    /// Maps the key and asks the state machine.
    fn plan(&self, vk: u16) -> Option<Act> {
        if is_modifier(vk) {
            return None; // Shift for "sH" must not commit the pending "s"
        }
        let composing = self.composing();
        if ctrl_or_alt() {
            return composing.then_some(Act::Commit);
        }
        let key = match VIRTUAL_KEY(vk) {
            VK_BACK => Key::Back,
            // shortcut: apostrophe = the US-layout ' or ` key unshifted; the product uses ToUnicodeEx
            VK_OEM_7 | VK_OEM_3 if !shift_down() => Key::Ch('\''),
            _ => match (vk as u8 as char).to_ascii_lowercase() {
                c @ ('s' | 'c' | 'o' | 'g' | 'h') if vk < 0x80 => Key::Ch(letter(c)),
                _ => Key::Other,
            },
        };
        decide(&self.st.typed.borrow(), composing, key)
    }

    fn run(&self, ctx: &ITfContext, act: Act) -> Result<()> {
        let st = self.st.clone();
        let ctx2 = ctx.clone();
        let sink: ITfCompositionSink = self.to_interface();
        let f = move |ec: u32| -> Result<()> {
            unsafe {
                let end = |st: &State| -> Result<()> {
                    if let Some(c) = st.comp.borrow_mut().take() {
                        let r = c.GetRange()?;
                        c.EndComposition(ec)?;
                        caret_to_end(ec, &ctx2, &r)?;
                    }
                    st.typed.borrow_mut().clear();
                    Ok(())
                };
                let set = |st: &State, typed: &str| -> Result<()> {
                    *st.typed.borrow_mut() = typed.to_string();
                    if let Some(c) = st.comp.borrow().as_ref() {
                        let r = c.GetRange()?;
                        r.SetText(ec, 0, &wide(&shown(typed)))?;
                        caret_to_end(ec, &ctx2, &r)?;
                    }
                    Ok(())
                };
                match act {
                    Act::Start(t) => {
                        end(&st)?;
                        if !st.probed.replace(true) {
                            match text_before_caret(ec, &ctx2, 8) {
                                Ok(s) => log(&format!("probe before-caret ok {:?}", s)),
                                Err(e) => log(&format!("probe before-caret err {e}")),
                            }
                        }
                        let ins: ITfInsertAtSelection = ctx2.cast()?;
                        let range = ins.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
                        let cc: ITfContextComposition = ctx2.cast()?;
                        let comp = cc.StartComposition(ec, &range, &sink)?;
                        *st.comp.borrow_mut() = Some(comp);
                        set(&st, &t)?;
                    }
                    Act::Update(t) => set(&st, &t)?,
                    Act::Finish(t) => {
                        if let Some(c) = st.comp.borrow().as_ref() {
                            c.GetRange()?.SetText(ec, 0, &wide(&t))?;
                        }
                        end(&st)?;
                    }
                    Act::Commit => end(&st)?,
                }
                Ok(())
            }
        };
        let session: ITfEditSession = Session { f: RefCell::new(Some(Box::new(f))) }.into();
        let hr = unsafe { ctx.RequestEditSession(self.st.tid.get(), &session, TF_ES_SYNC | TF_ES_READWRITE)? };
        hr.ok()
    }
}

impl ITfTextInputProcessor_Impl for Tip_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        let tm = ptim.ok()?.clone();
        let km: ITfKeystrokeMgr = tm.cast()?;
        let sink: ITfKeyEventSink = self.to_interface();
        // UZKBD_BG: harness only (kbdctl host feed), a background sink when the TIP is activated by hand.
        let fg = std::env::var_os("UZKBD_BG").is_none();
        if let Err(e) = unsafe { km.AdviseKeyEventSink(tid, &sink, fg) } {
            log(&format!("AdviseKeyEventSink(foreground={fg}) failed {e}"));
            return Err(e);
        }
        self.st.tid.set(tid);
        *self.st.tm.borrow_mut() = Some(tm);
        log(&format!("activate tid={tid} exe={:?}", std::env::current_exe().ok()));
        Ok(())
    }
    fn Deactivate(&self) -> Result<()> {
        if let Some(tm) = self.st.tm.borrow_mut().take() {
            if let Ok(km) = tm.cast::<ITfKeystrokeMgr>() {
                unsafe {
                    let _ = km.UnadviseKeyEventSink(self.st.tid.get());
                }
            }
        }
        *self.st.comp.borrow_mut() = None;
        self.st.typed.borrow_mut().clear();
        log("deactivate");
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for Tip_Impl {
    fn OnSetFocus(&self, fg: BOOL) -> Result<()> {
        log(&format!("key sink focus={}", fg.as_bool()));
        Ok(())
    }
    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let eat = self.plan(wparam.0 as u16).is_some();
        if std::env::var_os("UZKBD_TRACE").is_some() {
            log(&format!("test vk={:#04x} eat={eat} ctx={}", wparam.0, pic.is_some()));
        }
        Ok(eat.into())
    }
    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let t0 = Instant::now();
        let vk = wparam.0 as u16;
        let Some(act) = self.plan(vk) else { return Ok(false.into()) };
        let eaten = !matches!(act, Act::Commit);
        let res = match pic.ok() {
            Ok(ctx) => self.run(ctx, act),
            Err(e) => Err(e),
        };
        let us = t0.elapsed().as_micros();
        match &res {
            Ok(()) => log(&format!("key vk={vk:#04x} eaten={eaten} us={us} typed={:?}", self.st.typed.borrow())),
            Err(e) => log(&format!("key vk={vk:#04x} error {e} us={us}")),
        }
        // On failure, let the key through rather than lose it.
        Ok((eaten && res.is_ok()).into())
    }
    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _g: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for Tip_Impl {
    fn OnCompositionTerminated(&self, _ec: u32, _c: Ref<ITfComposition>) -> Result<()> {
        // The application ended it (focus change, click, ...): the text stays as shown.
        *self.st.comp.borrow_mut() = None;
        self.st.typed.borrow_mut().clear();
        log("composition terminated by the application");
        Ok(())
    }
}

impl Drop for Tip {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut core::ffi::c_void) -> Result<()> {
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        let unk: IUnknown = Tip { st: Rc::new(State::default()) }.into();
        unsafe { unk.query(riid, ppv).ok() }
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        OBJECTS.fetch_add(if lock.as_bool() { 1 } else { -1 }, Ordering::SeqCst);
        Ok(())
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(clsid: *const GUID, iid: *const GUID, ppv: *mut *mut core::ffi::c_void) -> HRESULT {
    unsafe {
        if ppv.is_null() {
            return E_POINTER;
        }
        *ppv = std::ptr::null_mut();
        if *clsid != CLSID_SPIKE {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let f: IClassFactory = Factory.into();
        f.query(iid, ppv)
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) <= 0 { S_OK } else { S_FALSE }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays keys through `decide` against a fake document (what an app would hold).
    fn simulate(keys: &str) -> String {
        let (mut doc, mut comp): (String, Option<String>) = (String::new(), None);
        for k in keys.chars() {
            let key = match k {
                '\u{8}' => Key::Back,
                's' | 'c' | 'o' | 'g' | 'h' | 'S' | 'C' | 'O' | 'G' | 'H' | '\'' => Key::Ch(k),
                _ => Key::Other,
            };
            match decide(comp.as_deref().unwrap_or(""), comp.is_some(), key) {
                Some(Act::Start(t)) => {
                    if let Some(c) = comp.take() { doc += &shown(&c); }
                    comp = Some(t);
                }
                Some(Act::Update(t)) => comp = Some(t),
                Some(Act::Finish(t)) => { doc += &t; comp = None; }
                Some(Act::Commit) | None => {
                    if let Some(c) = comp.take() { doc += &shown(&c); }
                    if k == '\u{8}' { doc.pop(); } else { doc.push(k); }
                }
            }
        }
        doc + &comp.map(|c| shown(&c)).unwrap_or_default()
    }

    #[test]
    fn cases_hold() {
        for (typed, want) in CASES {
            assert_eq!(simulate(typed), *want, "typed {typed:?}");
        }
    }

    #[test]
    fn shown_follows_r10() {
        assert_eq!(shown("s"), "s");
        assert_eq!(shown("sh"), "ş");
        assert_eq!(shown("Sh"), "Ş");
        assert_eq!(shown("SH"), "Ş");
        assert_eq!(shown("sH"), "ş");
        assert_eq!(shown("O'"), "Ö");
        assert_eq!(shown("s'"), "s'");
    }
}
