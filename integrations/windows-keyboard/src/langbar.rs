//! The mode button for the Input Indicator (GUID_LBI_INPUTMODE): "ş" when the conversion is on, "s" when it is off. A
//! click toggles it, like Ctrl+Shift+`. The brand icon next to it comes from the profile registration (src/reg.rs).

use std::rc::Rc;

use windows::Win32::Foundation::*;
use windows::Win32::System::Ole::{
    CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION,
};
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::tip::State;
use crate::{CLSID_TIP, Live, guard, load_string, module, res};

const SINK_COOKIE: u32 = 1;

#[implement(ITfLangBarItemButton, ITfSource)]
pub(crate) struct ModeButton {
    st: Rc<State>,
    _live: Live,
}

impl ModeButton {
    pub(crate) fn new(st: Rc<State>) -> Self {
        ModeButton {
            st,
            _live: Live::new(),
        }
    }
}

impl ModeButton_Impl {
    fn mode_text(&self) -> String {
        load_string(if self.st.is_open() {
            res::IDS_MODE_ON
        } else {
            res::IDS_MODE_OFF
        })
    }
}

impl ITfLangBarItem_Impl for ModeButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        guard("GetInfo", || {
            if pinfo.is_null() {
                return Err(E_INVALIDARG.into());
            }
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TIP,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
                ulSort: 0,
                szDescription: [0; 32],
            };
            for (d, s) in info
                .szDescription
                .iter_mut()
                .zip(load_string(res::IDS_TOGGLE).encode_utf16().take(31))
            {
                *d = s;
            }
            unsafe { pinfo.write(info) };
            Ok(())
        })
    }
    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }
    fn Show(&self, _fshow: BOOL) -> Result<()> {
        Ok(())
    }
    fn GetTooltipString(&self) -> Result<BSTR> {
        guard("GetTooltipString", || {
            Ok(BSTR::from(format!("{} (Ctrl+Shift+`)", self.mode_text())))
        })
    }
}

impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _area: *const RECT) -> Result<()> {
        guard("OnClick", || self.st.request_toggle())
    }
    fn InitMenu(&self, _menu: Ref<ITfMenu>) -> Result<()> {
        Ok(())
    }
    fn OnMenuSelect(&self, _wid: u32) -> Result<()> {
        Ok(())
    }
    /// The caller destroys the icon, so it is loaded fresh each time (not LR_SHARED).
    fn GetIcon(&self) -> Result<HICON> {
        guard("GetIcon", || {
            let id = if self.st.is_open() {
                res::IDI_MODE_ON
            } else {
                res::IDI_MODE_OFF
            };
            unsafe {
                let cx = GetSystemMetrics(SM_CXSMICON);
                let h = LoadImageW(
                    Some(module().into()),
                    PCWSTR(id as usize as *const u16),
                    IMAGE_ICON,
                    cx,
                    cx,
                    LR_DEFAULTCOLOR,
                )?;
                Ok(HICON(h.0))
            }
        })
    }
    fn GetText(&self) -> Result<BSTR> {
        guard("GetText", || Ok(BSTR::from(self.mode_text())))
    }
}

impl ITfSource_Impl for ModeButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        guard("AdviseSink", || {
            if riid.is_null() || unsafe { *riid } != ITfLangBarItemSink::IID {
                return Err(CONNECT_E_CANNOTCONNECT.into());
            }
            if self.st.button_sink.borrow().is_some() {
                return Err(CONNECT_E_ADVISELIMIT.into());
            }
            let sink: ITfLangBarItemSink = punk.ok()?.cast()?;
            *self.st.button_sink.borrow_mut() = Some(sink);
            Ok(SINK_COOKIE)
        })
    }
    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != SINK_COOKIE || self.st.button_sink.borrow_mut().take().is_none() {
            return Err(CONNECT_E_NOCONNECTION.into());
        }
        Ok(())
    }
}
