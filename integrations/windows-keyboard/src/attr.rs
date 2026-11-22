//! The composition's display attribute: no underline, no colours (PLAN 2.1), so the pending letter looks like ordinary
//! text. TSF reaches it through ITfDisplayAttributeProvider on the TIP's class (category DISPLAYATTRIBUTEPROVIDER).

use std::cell::Cell;

use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

use crate::{GUID_DISPLAY_ATTR, Live, guard};

/// The attribute itself.
pub fn attribute() -> TF_DISPLAYATTRIBUTE {
    let none = TF_DA_COLOR {
        r#type: TF_CT_NONE,
        ..Default::default()
    };
    TF_DISPLAYATTRIBUTE {
        crText: none,
        crBk: none,
        lsStyle: TF_LS_NONE,
        fBoldLine: false.into(),
        crLine: none,
        bAttr: TF_ATTR_INPUT,
    }
}

#[implement(ITfDisplayAttributeInfo)]
pub struct AttrInfo {
    _live: Live,
}

impl AttrInfo {
    pub fn new() -> Self {
        AttrInfo { _live: Live::new() }
    }
}

impl ITfDisplayAttributeInfo_Impl for AttrInfo_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID_DISPLAY_ATTR)
    }
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("Pending letter (no underline)"))
    }
    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        guard("GetAttributeInfo", || {
            if pda.is_null() {
                return Err(E_INVALIDARG.into());
            }
            unsafe { pda.write(attribute()) };
            Ok(())
        })
    }
    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// Enumerates the one attribute.
#[implement(IEnumTfDisplayAttributeInfo)]
pub struct AttrEnum {
    next: Cell<u32>,
    _live: Live,
}

impl AttrEnum {
    pub fn new(next: u32) -> Self {
        AttrEnum {
            next: Cell::new(next),
            _live: Live::new(),
        }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for AttrEnum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(AttrEnum::new(self.next.get()).into())
    }
    fn Next(
        &self,
        count: u32,
        out: *mut Option<ITfDisplayAttributeInfo>,
        fetched: *mut u32,
    ) -> Result<()> {
        guard("AttrEnum::Next", || {
            if out.is_null() && count > 0 {
                return Err(E_INVALIDARG.into());
            }
            let mut n = 0;
            if count > 0 && self.next.get() == 0 {
                unsafe { out.write(Some(AttrInfo::new().into())) };
                self.next.set(1);
                n = 1;
            }
            if !fetched.is_null() {
                unsafe { fetched.write(n) };
            }
            // S_FALSE is a success code; the vtable shim returns it as is.
            if n == count {
                Ok(())
            } else {
                Err(Error::from_hresult(S_FALSE))
            }
        })
    }
    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }
    fn Skip(&self, count: u32) -> Result<()> {
        let left = 1u32.saturating_sub(self.next.get());
        self.next.set(self.next.get().saturating_add(count));
        if count <= left {
            Ok(())
        } else {
            Err(Error::from_hresult(S_FALSE))
        }
    }
}
