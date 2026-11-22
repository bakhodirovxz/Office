//! The text input processor: key events, the composition that holds the pending digraph tail, the on/off toggle.
//!
//! Key handling, as measured on the owner's PC (PLAN 11.4): a key claimed in OnTestKeyDown has already become
//! VK_PROCESSKEY under CUAS (Notepad), so OnKeyDown must never return FALSE for it, and a key let through overtakes the
//! composition's commit. So a key that only commits is always claimed: a printable key is committed together with the
//! composition in one edit session (Finish(shown + char)); any other key (Enter, Tab, arrows, shortcuts) commits and
//! is sent again with SendInput, so it keeps its meaning.

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::time::Instant;

use kbd_core::{Act, Key, decide, shown};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

use crate::attr::{AttrEnum, AttrInfo};
use crate::langbar::ModeButton;
use crate::{GUID_DISPLAY_ATTR, GUID_PRESERVED_TOGGLE, Live, guard, log, res, set_debug, settings};

/// The on/off key: Ctrl+Shift+` (owner decision O4).
pub const TOGGLE_KEY: TF_PRESERVEDKEY = TF_PRESERVEDKEY {
    uVKey: VK_OEM_3.0 as u32,
    uModifiers: TF_MOD_CONTROL | TF_MOD_SHIFT,
};

/// Per-thread state shared by the TIP and its mode button. Rule: no RefCell borrow is held across a COM call, because
/// TSF calls back into the TIP synchronously (edit sessions, compartment changes).
#[derive(Default)]
pub(crate) struct State {
    tid: Cell<u32>,
    tm: RefCell<Option<ITfThreadMgr>>,
    comp: RefCell<Option<ITfComposition>>,
    /// The context the open composition lives in.
    ctx: RefCell<Option<ITfContext>>,
    /// The keys typed into the composition ("s", "sh", "Sh", "oʻ", ...); empty when none is open.
    typed: RefCell<String>,
    /// Conversion on (GUID_COMPARTMENT_KEYBOARD_OPENCLOSE).
    open: Cell<bool>,
    all_letters: Cell<bool>,
    attr_atom: Cell<u32>,
    compartment_cookie: Cell<Option<u32>>,
    button: RefCell<Option<ITfLangBarItem>>,
    pub(crate) button_sink: RefCell<Option<ITfLangBarItemSink>>,
}

impl State {
    pub(crate) fn is_open(&self) -> bool {
        self.open.get()
    }

    fn thread_mgr(&self) -> Result<ITfThreadMgr> {
        self.tm.borrow().clone().ok_or_else(|| E_UNEXPECTED.into())
    }

    fn openclose(&self) -> Result<ITfCompartment> {
        unsafe {
            self.thread_mgr()?
                .cast::<ITfCompartmentMgr>()?
                .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)
        }
    }

    /// Flips the compartment; the change comes back through OnChange (also when another party sets it).
    pub(crate) fn request_toggle(&self) -> Result<()> {
        let c = self.openclose()?;
        unsafe { c.SetValue(self.tid.get(), &VARIANT::from(!self.open.get() as i32)) }
    }

    fn composing(&self) -> bool {
        self.comp.borrow().is_some()
    }

    /// Forgets the composition (after an error, a panic, or termination by the application).
    fn reset(&self) {
        self.comp.borrow_mut().take();
        self.ctx.borrow_mut().take();
        self.typed.borrow_mut().clear();
    }

    fn notify_button(&self) {
        let sink = self.button_sink.borrow().clone();
        if let Some(s) = sink {
            let _ = unsafe { s.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP) };
        }
    }
}

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfCompartmentEventSink,
    ITfDisplayAttributeProvider
)]
pub(crate) struct Tip {
    st: Rc<State>,
    _live: Live,
}

impl Tip {
    pub(crate) fn new() -> Self {
        Tip {
            st: Rc::new(State::default()),
            _live: Live::new(),
        }
    }
}

// ---------- key mapping (pure, unit-tested) ----------

fn is_modifier(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_CAPITAL
    )
}

/// A key as the conversion sees it; `None` for a lone modifier (Shift between `s` and `H` must not commit, R10).
/// `ch` is the character the key types in the current layout and modifier state. kbd-core treats every `Ch` that is
/// not one of its letters or set-A apostrophes as `Other`. With the "sh and ch only" option, `o` and `g` start nothing.
pub fn key_of(vk: VIRTUAL_KEY, ch: Option<char>, all_letters: bool) -> Option<Key> {
    if is_modifier(vk) {
        return None;
    }
    if vk == VK_BACK {
        return Some(Key::Back);
    }
    Some(match ch {
        Some(c) if !all_letters && matches!(c, 'o' | 'O' | 'g' | 'G') => Key::Other,
        Some(c) => Key::Ch(c),
        None => Key::Other,
    })
}

fn ctrl_or_alt() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 || GetKeyState(VK_MENU.0 as i32) < 0 }
}

/// The single printable character this key types in the current layout and modifier state (Shift, Caps Lock), if any.
/// Ctrl and Alt chords are never characters here: shortcuts must keep their meaning.
fn printable(vk: VIRTUAL_KEY, lparam: LPARAM) -> Option<char> {
    if ctrl_or_alt() {
        return None;
    }
    let mut ks = [0u8; 256];
    unsafe { GetKeyboardState(&mut ks).ok()? };
    let mut buf = [0u16; 4];
    // wFlags bit 2: do not change the kernel's keyboard state (dead keys stay pending). VK_PACKET (touch keyboard,
    // SendInput Unicode) maps to its character the same way (IME requirements page).
    let n = unsafe {
        ToUnicodeEx(
            vk.0 as u32,
            ((lparam.0 >> 16) & 0xff) as u32,
            &ks,
            &mut buf,
            4,
            Some(GetKeyboardLayout(0)),
        )
    };
    let s = String::from_utf16(&buf[..n.max(0) as usize]).ok()?;
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if c >= ' ' && c != '\u{7f}' => Some(c),
        _ => None,
    }
}

/// Sends the key down again after a commit (the physical key-up follows by itself).
fn resend_key(vk: VIRTUAL_KEY, lparam: LPARAM) {
    let ext = if (lparam.0 >> 24) & 1 != 0 {
        KEYEVENTF_EXTENDEDKEY
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    let i = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: ((lparam.0 >> 16) & 0xff) as u16,
                dwFlags: ext,
                ..Default::default()
            },
        },
    };
    unsafe { SendInput(&[i], std::mem::size_of::<INPUT>() as i32) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

// ---------- edit sessions ----------

type EditFn = Box<dyn FnOnce(u32) -> Result<()>>;

#[implement(ITfEditSession)]
struct Session {
    f: RefCell<Option<EditFn>>,
    _live: Live,
}

impl ITfEditSession_Impl for Session_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guard("DoEditSession", || match self.f.borrow_mut().take() {
            Some(f) => f(ec),
            None => Ok(()),
        })
    }
}

/// Moves the selection to the end of `range`, so the caret follows the composition.
unsafe fn caret_to_end(ec: u32, ctx: &ITfContext, range: &ITfRange) -> Result<()> {
    unsafe {
        let r = range.Clone()?;
        r.Collapse(ec, TF_ANCHOR_END)?;
        let sel = TF_SELECTION {
            range: ManuallyDrop::new(Some(r)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: false.into(),
            },
        };
        let res = ctx.SetSelection(ec, std::slice::from_ref(&sel));
        drop(ManuallyDrop::into_inner(sel.range));
        res
    }
}

/// Sets (or clears) the no-underline display attribute on the composition's range. Best effort: an app without the
/// attribute property still shows the text, only with its own default composition style.
unsafe fn mark(st: &State, ctx: &ITfContext, ec: u32, r: &ITfRange, on: bool) {
    let atom = st.attr_atom.get();
    if atom == 0 {
        return;
    }
    unsafe {
        if let Ok(p) = ctx.GetProperty(&GUID_PROP_ATTRIBUTE) {
            let _ = if on {
                p.SetValue(ec, r, &VARIANT::from(atom as i32))
            } else {
                p.Clear(ec, r)
            };
        }
    }
}

unsafe fn set_typed(st: &State, ctx: &ITfContext, ec: u32, typed: &str) -> Result<()> {
    *st.typed.borrow_mut() = typed.to_string();
    let comp = st.comp.borrow().clone();
    if let Some(c) = comp {
        unsafe {
            let r = c.GetRange()?;
            r.SetText(ec, 0, &wide(&shown(typed)))?;
            mark(st, ctx, ec, &r, true);
            caret_to_end(ec, ctx, &r)?;
        }
    }
    Ok(())
}

/// Ends the composition; its text stays in the document. The state is cleared first, so an error cannot leave it stale.
unsafe fn end(st: &State, ctx: &ITfContext, ec: u32) -> Result<()> {
    let comp = st.comp.borrow_mut().take();
    st.ctx.borrow_mut().take();
    st.typed.borrow_mut().clear();
    if let Some(c) = comp {
        unsafe {
            let r = c.GetRange()?;
            mark(st, ctx, ec, &r, false);
            c.EndComposition(ec)?;
            caret_to_end(ec, ctx, &r)?;
        }
    }
    Ok(())
}

unsafe fn apply(
    st: &State,
    ctx: &ITfContext,
    sink: &ITfCompositionSink,
    ec: u32,
    act: Act,
) -> Result<()> {
    unsafe {
        match act {
            Act::Start(t) => {
                end(st, ctx, ec)?;
                let range = ctx.cast::<ITfInsertAtSelection>()?.InsertTextAtSelection(
                    ec,
                    TF_IAS_QUERYONLY,
                    &[],
                )?;
                let comp = ctx
                    .cast::<ITfContextComposition>()?
                    .StartComposition(ec, &range, sink)?;
                *st.comp.borrow_mut() = Some(comp);
                *st.ctx.borrow_mut() = Some(ctx.clone());
                set_typed(st, ctx, ec, &t)
            }
            Act::Update(t) => set_typed(st, ctx, ec, &t),
            Act::Finish(t) => {
                let comp = st.comp.borrow().clone();
                if let Some(c) = comp {
                    c.GetRange()?.SetText(ec, 0, &wide(&t))?;
                }
                end(st, ctx, ec)
            }
            Act::Commit => end(st, ctx, ec),
        }
    }
}

impl Tip_Impl {
    /// What to do with a key, without changing anything (OnTestKeyDown and OnKeyDown ask the same question).
    fn plan(&self, vk: VIRTUAL_KEY, lparam: LPARAM) -> Option<(Act, Option<char>)> {
        let composing = self.st.composing();
        if is_modifier(vk) {
            return None;
        }
        if !self.st.open.get() || ctrl_or_alt() {
            return composing.then_some((Act::Commit, None));
        }
        let ch = printable(vk, lparam);
        let key = key_of(vk, ch, self.st.all_letters.get())?;
        let typed = self.st.typed.borrow().clone();
        decide(&typed, composing, key).map(|a| (a, ch))
    }

    fn run(&self, ctx: &ITfContext, act: Act, flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS) -> Result<()> {
        let st = self.st.clone();
        let ctx2 = ctx.clone();
        let sink: ITfCompositionSink = self.to_interface();
        let f = move |ec: u32| unsafe { apply(&st, &ctx2, &sink, ec, act) };
        let session: ITfEditSession = Session {
            f: RefCell::new(Some(Box::new(f))),
            _live: Live::new(),
        }
        .into();
        let hr = unsafe {
            ctx.RequestEditSession(self.st.tid.get(), &session, flags | TF_ES_READWRITE)?
        };
        hr.ok()
    }

    /// Commits a pending composition outside a key event (toggle off, deactivation). The session clears the state
    /// itself when it runs (an asynchronous one may run later); only a refused request clears it here.
    fn commit_pending(&self, flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS) {
        let ctx = self.st.ctx.borrow().clone();
        if let Some(ctx) = ctx
            && let Err(e) = self.run(&ctx, Act::Commit, flags)
        {
            log(&format!("commit pending failed {e}"));
            self.st.reset();
        }
    }

    fn key_down(&self, pic: Ref<ITfContext>, vk: VIRTUAL_KEY, lparam: LPARAM) -> Result<BOOL> {
        let t0 = Instant::now();
        let Some(ctx) = pic.as_ref() else {
            return Ok(false.into());
        };
        let Some((mut act, ch)) = self.plan(vk, lparam) else {
            return Ok(false.into());
        };
        let mut resend = false;
        if act == Act::Commit {
            match ch {
                Some(c) => act = Act::Finish(format!("{}{c}", shown(&self.st.typed.borrow()))),
                None => resend = true,
            }
        }
        let what = format!("{act:?}");
        let res = self.run(ctx, act, TF_ES_SYNC);
        let us = t0.elapsed().as_micros();
        match &res {
            Ok(()) => {
                if resend {
                    resend_key(vk, lparam);
                }
                log(&format!(
                    "key vk={:#04x} {what} resend={resend} us={us} typed={:?}",
                    vk.0,
                    self.st.typed.borrow()
                ));
                Ok(true.into())
            }
            Err(e) => {
                // The composition is gone or unusable: forget it and let the key through (best effort).
                self.st.reset();
                log(&format!("key vk={:#04x} {what} error {e} us={us}", vk.0));
                Ok(false.into())
            }
        }
    }

    fn activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        let tm = ptim.ok()?.clone();
        let s = settings();
        set_debug(s.debug);
        self.st.all_letters.set(s.all_letters);
        self.st.tid.set(tid);
        *self.st.tm.borrow_mut() = Some(tm.clone());
        unsafe {
            let km: ITfKeystrokeMgr = tm.cast()?;
            let sink: ITfKeyEventSink = self.to_interface();
            if let Err(e) = km.AdviseKeyEventSink(tid, &sink, true) {
                log(&format!("AdviseKeyEventSink failed {e}"));
                self.st.tm.borrow_mut().take();
                return Err(e);
            }
            // The rest is best effort: without it the keyboard still converts.
            let desc = wide(&crate::load_string(res::IDS_TOGGLE));
            if let Err(e) = km.PreserveKey(tid, &GUID_PRESERVED_TOGGLE, &TOGGLE_KEY, &desc) {
                log(&format!("PreserveKey failed {e}"));
            }
            match CoCreateInstance::<_, ITfCategoryMgr>(
                &CLSID_TF_CategoryMgr,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .and_then(|cm| cm.RegisterGUID(&GUID_DISPLAY_ATTR))
            {
                Ok(a) => self.st.attr_atom.set(a),
                Err(e) => log(&format!("RegisterGUID(display attribute) failed {e}")),
            }
            // IME requirements: "Set the default IME mode to on".
            self.st.open.set(true);
            match self.st.openclose() {
                Ok(c) => {
                    let _ = c.SetValue(tid, &VARIANT::from(1i32));
                    let me: ITfCompartmentEventSink = self.to_interface();
                    match c
                        .cast::<ITfSource>()
                        .and_then(|src| src.AdviseSink(&ITfCompartmentEventSink::IID, &me))
                    {
                        Ok(cookie) => self.st.compartment_cookie.set(Some(cookie)),
                        Err(e) => log(&format!("compartment AdviseSink failed {e}")),
                    }
                }
                Err(e) => log(&format!("open/close compartment failed {e}")),
            }
            let button: ITfLangBarItemButton = ModeButton::new(self.st.clone()).into();
            match button
                .cast::<ITfLangBarItem>()
                .and_then(|b| tm.cast::<ITfLangBarItemMgr>()?.AddItem(&b).map(|_| b))
            {
                Ok(b) => *self.st.button.borrow_mut() = Some(b),
                Err(e) => log(&format!("language bar AddItem failed {e}")),
            }
        }
        log(&format!(
            "activate tid={tid} exe={:?} settings={s:?}",
            std::env::current_exe().ok()
        ));
        Ok(())
    }

    fn deactivate(&self) -> Result<()> {
        self.commit_pending(TF_ES_SYNC);
        let tm = self.st.tm.borrow_mut().take();
        let button = self.st.button.borrow_mut().take();
        if let Some(tm) = tm {
            unsafe {
                if let Some(b) = button
                    && let Ok(m) = tm.cast::<ITfLangBarItemMgr>()
                {
                    let _ = m.RemoveItem(&b);
                }
                if let Some(cookie) = self.st.compartment_cookie.take()
                    && let Ok(c) = tm
                        .cast::<ITfCompartmentMgr>()
                        .and_then(|m| m.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE))
                    && let Ok(src) = c.cast::<ITfSource>()
                {
                    let _ = src.UnadviseSink(cookie);
                }
                if let Ok(km) = tm.cast::<ITfKeystrokeMgr>() {
                    let _ = km.UnpreserveKey(&GUID_PRESERVED_TOGGLE, &TOGGLE_KEY);
                    let _ = km.UnadviseKeyEventSink(self.st.tid.get());
                }
            }
        }
        self.st.button_sink.borrow_mut().take(); // after RemoveItem, which unadvises it
        self.st.reset();
        log("deactivate");
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for Tip_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        guard("Activate", || self.activate(ptim, tid))
    }
    fn Deactivate(&self) -> Result<()> {
        guard("Deactivate", || self.deactivate())
    }
}

impl ITfTextInputProcessorEx_Impl for Tip_Impl {
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        guard("ActivateEx", || self.activate(ptim, tid))
    }
}

impl ITfKeyEventSink_Impl for Tip_Impl {
    fn OnSetFocus(&self, _fg: BOOL) -> Result<()> {
        Ok(())
    }
    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let r = guard("OnTestKeyDown", || {
            Ok((pic.is_some() && self.plan(VIRTUAL_KEY(wparam.0 as u16), lparam).is_some()).into())
        });
        Ok(r.unwrap_or_else(|_| {
            self.st.reset();
            false.into()
        }))
    }
    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let r = guard("OnKeyDown", || {
            self.key_down(pic, VIRTUAL_KEY(wparam.0 as u16), lparam)
        });
        Ok(r.unwrap_or_else(|_| {
            self.st.reset();
            false.into()
        }))
    }
    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        guard("OnPreservedKey", || {
            if rguid.is_null() || unsafe { *rguid } != GUID_PRESERVED_TOGGLE {
                return Ok(false.into());
            }
            self.st.request_toggle()?;
            Ok(true.into())
        })
    }
}

impl ITfCompositionSink_Impl for Tip_Impl {
    fn OnCompositionTerminated(&self, _ec: u32, _c: Ref<ITfComposition>) -> Result<()> {
        // The application ended it (focus change, click, ...): the text stays as shown.
        self.st.reset();
        log("composition terminated by the application");
        Ok(())
    }
}

impl ITfCompartmentEventSink_Impl for Tip_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        guard("OnChange", || {
            if rguid.is_null() || unsafe { *rguid } != GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                return Ok(());
            }
            let v = self.st.openclose().and_then(|c| unsafe { c.GetValue() })?;
            let open = v.vt() == VT_I4 && unsafe { v.Anonymous.Anonymous.Anonymous.lVal } != 0;
            if !open && self.st.composing() {
                self.commit_pending(TF_ES_ASYNCDONTCARE);
            }
            self.st.open.set(open);
            self.st.notify_button();
            log(&format!("open={open}"));
            Ok(())
        })
    }
}

impl ITfDisplayAttributeProvider_Impl for Tip_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        guard("EnumDisplayAttributeInfo", || Ok(AttrEnum::new(0).into()))
    }
    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        guard("GetDisplayAttributeInfo", || {
            if guid.is_null() || unsafe { *guid } != GUID_DISPLAY_ATTR {
                return Err(E_INVALIDARG.into());
            }
            Ok(AttrInfo::new().into())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_are_ignored_and_backspace_is_back() {
        for vk in [
            VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_CAPITAL, VK_CONTROL, VK_MENU,
        ] {
            assert!(key_of(vk, None, true).is_none(), "{vk:?}");
        }
        assert!(matches!(
            key_of(VK_BACK, Some('\u{8}'), true),
            Some(Key::Back)
        ));
    }

    #[test]
    fn characters_come_from_the_layout() {
        // The character, not the key: the apostrophe of any layout key, either case of a letter.
        for c in ['s', 'S', 'h', 'H', 'o', 'G', '\'', '`', 'ʻ', '’'] {
            assert!(
                matches!(key_of(VIRTUAL_KEY(0x41), Some(c), true), Some(Key::Ch(x)) if x == c),
                "{c:?}"
            );
        }
        assert!(matches!(key_of(VK_RETURN, None, true), Some(Key::Other)));
    }

    #[test]
    fn sh_ch_only_leaves_o_and_g_alone() {
        assert!(matches!(
            key_of(VIRTUAL_KEY(0x4F), Some('o'), false),
            Some(Key::Other)
        ));
        assert!(matches!(
            key_of(VIRTUAL_KEY(0x47), Some('G'), false),
            Some(Key::Other)
        ));
        assert!(matches!(
            key_of(VIRTUAL_KEY(0x53), Some('s'), false),
            Some(Key::Ch('s'))
        ));
        // o' types plainly; s'h still means sh (the apostrophe is still a key of the machine)
        assert!(matches!(
            key_of(VK_OEM_7, Some('\''), false),
            Some(Key::Ch('\''))
        ));
    }

    /// Plays characters through key_of + decide against a model document, as OnKeyDown does (Commit of a printable
    /// key = Finish(shown + char)).
    fn play(keys: &str, all: bool) -> String {
        let (mut doc, mut typed) = (String::new(), String::new());
        for c in keys.chars() {
            let vk = if c == '\u{8}' {
                VK_BACK
            } else {
                VIRTUAL_KEY(0x41)
            };
            let key = key_of(vk, (c != '\u{8}').then_some(c), all).unwrap();
            match decide(&typed, !typed.is_empty(), key) {
                Some(Act::Start(t)) => {
                    doc += &shown(&typed);
                    typed = t;
                }
                Some(Act::Update(t)) => typed = t,
                Some(Act::Finish(t)) => {
                    doc += &t;
                    typed.clear();
                }
                Some(Act::Commit) => {
                    doc += &format!("{}{c}", shown(&typed));
                    typed.clear();
                }
                None if c == '\u{8}' => {
                    doc.pop();
                }
                None => doc.push(c),
            }
        }
        doc + &shown(&typed)
    }

    #[test]
    fn options_change_the_output() {
        assert_eq!(play("toʻgʻri qush", true), "töğri quş");
        assert_eq!(play("toʻgʻri qush", false), "toʻgʻri quş");
        assert_eq!(play("Is'hoq choy", false), "Ishoq çoy");
        assert_eq!(play("o’\u{8}zbek", true), "o’zbek"); // Backspace gives back the apostrophe that was typed
    }
}
