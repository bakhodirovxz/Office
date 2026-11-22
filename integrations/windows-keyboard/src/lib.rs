//! Uzbek 2026 keyboard for Windows: a TSF text input processor (TIP) in an in-process COM DLL (docs/keyboard/PLAN.md).
//!
//! While the user types, `sh`, `ch`, `oʻ`, `gʻ` become `ş`, `ç`, `ö`, `ğ`; the pending tail of a digraph waits in a TSF
//! composition with no underline, so one Backspace gives the typed keys back. The conversion itself is `kbd-core`.
//!
//! Written from the public Microsoft documentation (Text Services Framework reference, "Input Method Editor (IME)
//! requirements"). No code is taken from Windows-classic-samples SampleIME or any other IME.
//!
//! Rules for every COM entry point in this crate: it runs inside [`guard`] (a panic becomes an error HRESULT, never an
//! unwind into the host), and it never writes to disk unless the HKCU `Debug` flag is set ([`settings`]).

mod attr;
mod langbar;
pub mod reg;
pub mod res;
mod tip;

pub use tip::{TOGGLE_KEY, key_of};

use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::WindowsAndMessaging::LoadStringW;
use windows::core::*;

// Frozen identities (2026-11-22). Never change them: the registration, the users' input lists and the MSI refer to them.
/// COM class of the TIP.
pub const CLSID_TIP: GUID = GUID::from_u128(0xe384aabe_7f88_4bb2_bfb5_3aa607601e3e);
/// The language profile under uz-Latn-UZ.
pub const PROFILE: GUID = GUID::from_u128(0xbb9f7540_b6bf_43ac_ac2c_0ee5fec092a6);
/// The composition's display attribute (no underline, no colours).
pub const GUID_DISPLAY_ATTR: GUID = GUID::from_u128(0x508f6aa0_f1bf_406a_93c1_d4bc7fb71699);
/// The on/off preserved key, Ctrl+Shift+` (owner decision O4).
pub const GUID_PRESERVED_TOGGLE: GUID = GUID::from_u128(0x10a834ec_8648_4d1a_91ab_fd4e884f83b9);
/// uz-Latn-UZ.
pub const LANGID_UZ_LATN: u16 = 0x0443;
/// The profile as InstallLayoutOrTip and the input list spell it.
pub fn tip_string() -> String {
    format!(
        "{LANGID_UZ_LATN:04X}:{}{}",
        braced(&CLSID_TIP),
        braced(&PROFILE)
    )
}
/// `{XXXXXXXX-XXXX-...}`, upper case, as the registry writes GUIDs.
pub fn braced(g: &GUID) -> String {
    format!("{{{g:?}}}")
}

/// Product name, rendered from brand/brand.toml at build time (never hard-coded).
pub const BRAND: &str = env!("KBD_BRAND");

/// Live COM objects and server locks (DllCanUnloadNow).
static OBJECTS: AtomicI32 = AtomicI32::new(0);

/// A field of every COM object of this DLL: counts it for DllCanUnloadNow.
pub(crate) struct Live;
impl Live {
    pub(crate) fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Live
    }
}
impl Drop for Live {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Runs a COM entry point; a panic becomes `E_UNEXPECTED` (logged when debugging) instead of unwinding into the host.
/// The DLL never installs a panic hook: the hook is process-wide and belongs to the host.
pub(crate) fn guard<T>(what: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned());
            log(&format!("PANIC in {what}: {}", msg.unwrap_or_default()));
            Err(E_UNEXPECTED.into())
        }
    }
}

/// HRESULT-returning exports: the same guard.
pub(crate) fn guard_hr(what: &str, f: impl FnOnce() -> Result<()>) -> HRESULT {
    match guard(what, f) {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

// ---------- settings (HKCU only; the TIP reads them and never writes them) ----------

/// `HKCU\Software\<brand>\Keyboard`.
pub fn settings_key() -> String {
    format!(r"Software\{BRAND}\Keyboard")
}

/// Options of PLAN 7, read at each activation. There is no settings UI yet; missing values mean the defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `Letters` DWORD: 0 (default) = all four letters (owner decision O2); 1 = sh and ch only.
    pub all_letters: bool,
    /// `Debug` DWORD: 1 = append a log to %TEMP%\<brand>-keyboard.log. Off by default: nothing is written to disk.
    pub debug: bool,
}

pub fn settings() -> Settings {
    let key = HSTRING::from(settings_key());
    let dword = |name: &str| -> Option<u32> {
        let mut v = 0u32;
        let mut n = 4u32;
        let name = HSTRING::from(name);
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key,
                &name,
                RRF_RT_REG_DWORD,
                None,
                Some(&mut v as *mut u32 as _),
                Some(&mut n),
            )
        };
        (r == ERROR_SUCCESS).then_some(v)
    };
    Settings {
        all_letters: dword("Letters").unwrap_or(0) != 1,
        debug: dword("Debug").unwrap_or(0) == 1,
    }
}

static DEBUG: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_debug(on: bool) {
    DEBUG.store(on, Ordering::Relaxed);
}

pub(crate) fn debug_on() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

/// The debug log (machine tests read the per-key times from it).
pub fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{BRAND}-keyboard.log"))
}

pub(crate) fn log(line: &str) {
    if !debug_on() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() % 100_000)
            .unwrap_or(0);
        let _ = writeln!(f, "{} {ms:05} {line}", std::process::id());
    }
}

// ---------- this module ----------

/// The DLL's own module handle (the test executable when linked into a test).
pub(crate) fn module() -> HMODULE {
    let mut h = HMODULE::default();
    unsafe {
        let _ = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(module as *const () as *const u16),
            &mut h,
        );
    }
    h
}

/// Full path of this DLL, as registered for COM and as the icon and string resource file.
pub(crate) fn module_path() -> Result<String> {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(module()), &mut buf) } as usize;
    if n == 0 || n >= buf.len() {
        return Err(Error::from_thread());
    }
    Ok(String::from_utf16_lossy(&buf[..n]))
}

/// A string resource in the thread's UI language (the loader picks the string table: uz-Latn, uz-Cyrl, ru, else en).
pub fn load_string(id: u32) -> String {
    let mut p: *const u16 = std::ptr::null();
    // cchBufferMax 0: the loader returns a read-only pointer into the resource itself.
    let n = unsafe {
        LoadStringW(
            Some(module().into()),
            id,
            PWSTR(&mut p as *mut *const u16 as *mut u16),
            0,
        )
    };
    if n <= 0 || p.is_null() {
        return String::new();
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n as usize) })
}

// ---------- COM server exports ----------

#[implement(IClassFactory)]
struct Factory {
    _live: Live,
}

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        guard("CreateInstance", || {
            if ppv.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe { *ppv = std::ptr::null_mut() };
            if outer.is_some() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let unk: IUnknown = tip::Tip::new().into();
            unsafe { unk.query(riid, ppv).ok() }
        })
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        OBJECTS.fetch_add(if lock.as_bool() { 1 } else { -1 }, Ordering::SeqCst);
        Ok(())
    }
}

/// # Safety
/// COM contract: valid pointers from the COM runtime.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    guard_hr("DllGetClassObject", || unsafe {
        if ppv.is_null() || clsid.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        *ppv = std::ptr::null_mut();
        if *clsid != CLSID_TIP {
            return Err(CLASS_E_CLASSNOTAVAILABLE.into());
        }
        let f: IClassFactory = Factory { _live: Live::new() }.into();
        f.query(iid, ppv).ok()
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) <= 0 {
        S_OK
    } else {
        S_FALSE
    }
}

/// Number of live objects (tests).
pub fn live_objects() -> i32 {
    OBJECTS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_becomes_an_error_hresult() {
        let r: Result<()> = guard("test", || panic!("boom"));
        assert_eq!(r.unwrap_err().code(), E_UNEXPECTED);
        assert_eq!(guard_hr("test", || Err(E_FAIL.into())), E_FAIL);
        assert_eq!(guard_hr("test", || Ok(())), S_OK);
    }

    #[test]
    fn settings_default_to_all_letters_and_no_log() {
        // The test user has no settings key (the TIP never writes one).
        if std::env::var_os("KBD_TEST_HAS_SETTINGS").is_none() {
            assert_eq!(
                settings(),
                Settings {
                    all_letters: true,
                    debug: false
                }
            );
        }
    }
}
