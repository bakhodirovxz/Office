//! Registration (PLAN 3, 8; WP3). Per machine, elevated, once per architecture: `DllRegisterServer` writes the COM class
//! and registers the profile and categories through the TSF API ("don't write registry entries directly", IME
//! requirements page); `DllUnregisterServer` reverses it. Per user, not elevated: `KbdInstallForUser` adds the keyboard
//! to the user's input list (InstallLayoutOrTip). For every user at install (owner decision O6): the same for the
//! default-user hive, which new profiles copy (InstallLayoutOrTipUserReg).
//!
//! The four `Kbd*` exports have the signature of an MSI DLL custom action, `UINT __stdcall f(MSIHANDLE)`; the handle is
//! not used. They return ERROR_SUCCESS (0) or ERROR_INSTALL_FAILURE (1603).

use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

use crate::{
    CLSID_TIP, LANGID_UZ_LATN, PROFILE, braced, guard, guard_hr, module_path, res, tip_string,
};

/// Categories (PLAN 3; O5: no SECUREMODE, the lock screen is not in v1).
pub const CATEGORIES: [GUID; 4] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
];

/// The profile's plain description; Windows shows the localized MUI name (SetLanguageProfileDisplayName) instead.
const NAME_UZ_LATN: &str = env!("KBD_NAME_UZ_LATN");

// input.dll flags (documented values, no public header).
const ILOT_UNINSTALL: u32 = 0x0000_0001;

/// Where the default-user hive is mounted while it is changed.
pub const DEFAULT_HIVE_MOUNT: &str = "KbdDefaultUserHive";

const ERROR_INSTALL_FAILURE: u32 = 1603;

fn com_key() -> String {
    format!(r"SOFTWARE\Classes\CLSID\{}", braced(&CLSID_TIP))
}

/// The TIP's key below HKLM, in this process's registry view (WOW64 redirects it for the x86 DLL).
fn tip_key() -> String {
    format!(r"SOFTWARE\Microsoft\CTF\TIP\{}", braced(&CLSID_TIP))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Runs `f` with COM initialized on this thread (regsvr32 does it; an MSI custom action host may not).
fn with_com<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let r = f();
    if hr.is_ok() {
        unsafe { CoUninitialize() };
    }
    r
}

// ---------- registry helpers ----------

fn set_sz(root: HKEY, path: &str, name: Option<&str>, value: &str) -> Result<()> {
    unsafe {
        let mut k = HKEY::default();
        RegCreateKeyExW(
            root,
            &HSTRING::from(path),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut k,
            None,
        )
        .ok()?;
        let data: Vec<u8> = value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        let name = name.map(HSTRING::from);
        let r = RegSetValueExW(
            k,
            name.as_ref()
                .map(|n| PCWSTR(n.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            None,
            REG_SZ,
            Some(&data),
        );
        let _ = RegCloseKey(k);
        r.ok()
    }
}

fn open(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Option<HKEY> {
    let mut k = HKEY::default();
    unsafe { RegOpenKeyExW(root, &HSTRING::from(path), None, access, &mut k) }
        .is_ok()
        .then_some(k)
}

fn subkeys(root: HKEY, path: &str) -> Vec<String> {
    let Some(k) = open(root, path, KEY_READ) else {
        return vec![];
    };
    let mut out = vec![];
    for i in 0.. {
        let mut buf = [0u16; 256];
        let mut n = buf.len() as u32;
        if unsafe {
            RegEnumKeyExW(
                k,
                i,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut n,
                None,
                None,
                None,
                None,
            )
        }
        .is_err()
        {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..n as usize]));
    }
    unsafe {
        let _ = RegCloseKey(k);
    }
    out
}

fn value_count(root: HKEY, path: &str) -> u32 {
    let Some(k) = open(root, path, KEY_READ) else {
        return 0;
    };
    let mut n = 0u32;
    unsafe {
        let _ = RegQueryInfoKeyW(
            k,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&mut n),
            None,
            None,
            None,
            None,
        );
        let _ = RegCloseKey(k);
    }
    n
}

fn read_sz(root: HKEY, path: &str, name: &str) -> Option<String> {
    let mut buf = [0u16; 128];
    let mut n = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(path),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as _),
            Some(&mut n),
        )
    }
    .ok()
    .ok()?;
    let len = (n as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// Deletes `path` and everything below it; a missing key is not an error.
fn delete_tree(root: HKEY, path: &str) -> Result<()> {
    match unsafe { RegDeleteTreeW(root, &HSTRING::from(path)) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        e => e.ok(),
    }
}

/// Below `path`: deletes each key whose `CLSID` value names this TIP, then the keys that are left empty, bottom-up
/// (including `path` itself). Keys of other keyboards are kept.
fn prune(root: HKEY, path: &str) {
    for s in subkeys(root, path) {
        prune(root, &format!(r"{path}\{s}"));
    }
    let ours =
        read_sz(root, path, "CLSID").is_some_and(|v| v.eq_ignore_ascii_case(&braced(&CLSID_TIP)));
    if ours
        || (subkeys(root, path).is_empty()
            && value_count(root, path) == 0
            && open(root, path, KEY_READ).is_some())
    {
        let _ = delete_tree(root, path);
    }
}

/// The per-user keys Windows leaves after the profile is removed from a user's input list (measured, PLAN 11.4):
/// `CTF\TIP\{clsid}` (per-user Enable override) and this TIP's entries under `CTF\SortOrder\AssemblyItem\0x00000443`.
/// `prefix` is "" for HKEY_CURRENT_USER or `<hive>\` below HKEY_USERS.
pub fn clean_user_keys(root: HKEY, prefix: &str) {
    let _ = delete_tree(
        root,
        &format!(r"{prefix}Software\Microsoft\CTF\TIP\{}", braced(&CLSID_TIP)),
    );
    prune(
        root,
        &format!(
            r"{prefix}Software\Microsoft\CTF\SortOrder\AssemblyItem\0x{:08x}",
            LANGID_UZ_LATN
        ),
    );
}

/// The same for every user hive that is loaded now (logged-on users, services). Profiles that are not loaded keep their
/// leftovers: they are inert once the profile is unregistered, and loading every NTUSER.DAT is out of scope.
pub fn clean_loaded_user_hives() {
    for sid in subkeys(HKEY_USERS, "") {
        if !sid.ends_with("_Classes") {
            clean_user_keys(HKEY_USERS, &format!(r"{sid}\"));
        }
    }
}

// ---------- per machine ----------

fn register() -> Result<()> {
    let dll = module_path()?;
    let key = com_key();
    set_sz(HKEY_LOCAL_MACHINE, &key, None, NAME_UZ_LATN)?;
    set_sz(
        HKEY_LOCAL_MACHINE,
        &format!(r"{key}\InprocServer32"),
        None,
        &dll,
    )?;
    set_sz(
        HKEY_LOCAL_MACHINE,
        &format!(r"{key}\InprocServer32"),
        Some("ThreadingModel"),
        "Apartment",
    )?;
    unsafe {
        let pm: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        // Icon index 0 = the first icon group in the DLL, the brand icon (res::IDI_BRAND).
        pm.RegisterProfile(
            &CLSID_TIP,
            LANGID_UZ_LATN,
            &PROFILE,
            &wide(NAME_UZ_LATN),
            &wide(&dll),
            0,
            HKL::default(),
            0,
            true,
            0,
        )?;
        let ex: ITfInputProcessorProfilesEx = pm.cast()?;
        ex.SetLanguageProfileDisplayName(
            &CLSID_TIP,
            LANGID_UZ_LATN,
            &PROFILE,
            &wide(&dll),
            res::IDS_NAME,
        )?;
        let cm: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for c in CATEGORIES {
            cm.RegisterCategory(&CLSID_TIP, &c, &CLSID_TIP)?;
        }
    }
    Ok(())
}

/// Idempotent: what is missing is skipped. Returns the first API error, after trying everything.
fn unregister() -> Result<()> {
    let mut first: Result<()> = Ok(());
    let mut keep = |r: Result<()>| {
        if first.is_ok() && r.is_err() {
            first = r;
        }
    };
    unsafe {
        match CoCreateInstance::<_, ITfCategoryMgr>(
            &CLSID_TF_CategoryMgr,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(cm) => CATEGORIES.iter().for_each(|c| {
                let _ = cm.UnregisterCategory(&CLSID_TIP, c, &CLSID_TIP);
            }),
            Err(e) => keep(Err(e)),
        }
        match CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(pm) => {
                let _ = pm.UnregisterProfile(&CLSID_TIP, LANGID_UZ_LATN, &PROFILE, 0);
            }
            Err(e) => keep(Err(e)),
        }
    }
    // UnregisterProfile and UnregisterCategory leave the TIP key itself (machine run 5, 64-bit view); the MSI uninstall
    // relies on this function to remove it.
    keep(delete_tree(HKEY_LOCAL_MACHINE, &tip_key()));
    keep(delete_tree(HKEY_LOCAL_MACHINE, &com_key()));
    clean_loaded_user_hives();
    first
}

/// regsvr32 / the MSI's elevated deferred action. On failure, what was registered is removed again.
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    guard_hr("DllRegisterServer", || {
        with_com(|| {
            let r = register();
            if r.is_err() {
                let _ = unregister();
            }
            r
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    guard_hr("DllUnregisterServer", || with_com(unregister))
}

// ---------- per user (input.dll) ----------

/// Calls a System32 DLL's export without a static import (the TIP loads into every process; these are install-time only).
fn system_proc<T>(
    dll: PCWSTR,
    name: PCSTR,
    call: impl FnOnce(unsafe extern "system" fn() -> isize) -> T,
) -> Result<T> {
    unsafe {
        let m = LoadLibraryExW(dll, None, LOAD_LIBRARY_SEARCH_SYSTEM32)?;
        let r = GetProcAddress(m, name)
            .map(call)
            .ok_or_else(Error::from_thread);
        let _ = FreeLibrary(m);
        r
    }
}

type InstallLayoutOrTip = unsafe extern "system" fn(PCWSTR, u32) -> BOOL;
type InstallLayoutOrTipUserReg =
    unsafe extern "system" fn(PCWSTR, PCWSTR, PCWSTR, PCWSTR, u32) -> BOOL;

fn install_layout_or_tip(flags: u32) -> Result<()> {
    let psz = HSTRING::from(tip_string());
    let ok = system_proc(w!("input.dll"), s!("InstallLayoutOrTip"), |f| unsafe {
        let f: InstallLayoutOrTip = std::mem::transmute(f);
        f(PCWSTR(psz.as_ptr()), flags)
    })?;
    if ok.as_bool() {
        Ok(())
    } else {
        Err(E_FAIL.into())
    }
}

/// Logs the failing step of an install-time export, with its Win32 error, and passes the result on.
fn step<T>(what: &str, r: Result<T>) -> Result<T> {
    if let Err(e) = &r {
        crate::log(&format!(
            "  step {what} failed: {:#010x} {}",
            e.code().0,
            e.message()
        ));
    }
    r
}

/// Whether the hive mounted at `HKEY_USERS\<m>` lists the keyboard (debug log only): the language list
/// (`Control Panel\International\User Profile\uz-Latn-UZ`, a value named after the profile) or a SortOrder entry.
fn hive_lists_tip(m: &str) -> bool {
    let lang = format!(r"{m}\Control Panel\International\User Profile\uz-Latn-UZ");
    let in_list = unsafe {
        RegGetValueW(
            HKEY_USERS,
            &HSTRING::from(lang),
            &HSTRING::from(tip_string()),
            RRF_RT_ANY,
            None,
            None,
            None,
        )
    } == ERROR_SUCCESS;
    let sort = format!(
        r"{m}\Software\Microsoft\CTF\SortOrder\AssemblyItem\0x{:08x}",
        LANGID_UZ_LATN
    );
    in_list || names_tip(HKEY_USERS, &sort)
}

/// Whether a key at or below `path` has a `CLSID` value naming this TIP.
fn names_tip(root: HKEY, path: &str) -> bool {
    read_sz(root, path, "CLSID").is_some_and(|v| v.eq_ignore_ascii_case(&braced(&CLSID_TIP)))
        || subkeys(root, path)
            .iter()
            .any(|s| names_tip(root, &format!(r"{path}\{s}")))
}

/// InstallLayoutOrTipUserReg for the hive mounted at `HKEY_USERS\<m>`. The documentation calls pszUserReg "the registry
/// path of the user" (NULL = HKEY_CURRENT_USER) and does not give its format; machine run 5 got 1603 with the first form
/// below and no log. So each form is tried in turn until one returns TRUE, and every result is logged with what the
/// hive shows afterwards.
/// shortcut: three guessed path forms; keep only the one the machine test shows working.
fn install_layout_or_tip_user_reg(m: &str, flags: u32) -> Result<()> {
    let psz = HSTRING::from(tip_string());
    for user_reg in [
        format!(r"HKEY_USERS\{m}"),
        format!(r"HKU\{m}"),
        format!(r"\Registry\User\{m}"),
    ] {
        let user = HSTRING::from(user_reg.as_str());
        let (ok, err) = system_proc(
            w!("input.dll"),
            s!("InstallLayoutOrTipUserReg"),
            |f| unsafe {
                let f: InstallLayoutOrTipUserReg = std::mem::transmute(f);
                let ok = f(
                    PCWSTR(user.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    PCWSTR(psz.as_ptr()),
                    flags,
                );
                (ok, GetLastError())
            },
        )?;
        crate::log(&format!(
            "  InstallLayoutOrTipUserReg({user_reg:?}, flags={flags:#x}) -> {} (last error {}); hive lists the keyboard: {}",
            ok.as_bool(),
            err.0,
            hive_lists_tip(m)
        ));
        if ok.as_bool() {
            return Ok(());
        }
    }
    Err(E_FAIL.into())
}

fn enable_privilege(name: PCWSTR) -> Result<()> {
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut tok,
        )?;
        let mut luid = LUID::default();
        let r = LookupPrivilegeValueW(None, name, &mut luid).and_then(|_| {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            AdjustTokenPrivileges(tok, false, Some(&tp), 0, None, None)?;
            // AdjustTokenPrivileges succeeds even when the privilege is not held.
            match GetLastError() {
                ERROR_NOT_ALL_ASSIGNED => Err(ERROR_NOT_ALL_ASSIGNED.into()),
                _ => Ok(()),
            }
        });
        let _ = CloseHandle(tok);
        r
    }
}

/// Mounts C:\Users\Default\NTUSER.DAT under HKEY_USERS\<DEFAULT_HIVE_MOUNT> for `f`, then unloads it. Needs an
/// elevated caller (SeBackupPrivilege and SeRestorePrivilege). Every step logs its own failure (machine run 5: 1603
/// with no clue which step).
fn with_default_hive(f: impl FnOnce(&str) -> Result<()>) -> Result<()> {
    step("enable SeBackupPrivilege", enable_privilege(SE_BACKUP_NAME))?;
    step(
        "enable SeRestorePrivilege",
        enable_privilege(SE_RESTORE_NAME),
    )?;
    let mut buf = vec![0u16; 1024];
    let mut n = buf.len() as u32;
    type GetDefaultUserProfileDirectoryW = unsafe extern "system" fn(PWSTR, *mut u32) -> BOOL;
    let ok = step(
        "load userenv.dll",
        system_proc(
            w!("userenv.dll"),
            s!("GetDefaultUserProfileDirectoryW"),
            |f| unsafe {
                let f: GetDefaultUserProfileDirectoryW = std::mem::transmute(f);
                f(PWSTR(buf.as_mut_ptr()), &mut n).ok()
            },
        ),
    )?;
    step("GetDefaultUserProfileDirectoryW", ok)?;
    let dir = String::from_utf16_lossy(&buf[..(n as usize).saturating_sub(1)]);
    let hive = format!(r"{dir}\NTUSER.DAT");
    let mount = HSTRING::from(DEFAULT_HIVE_MOUNT);
    step(
        &format!(r"RegLoadKey(HKEY_USERS\{DEFAULT_HIVE_MOUNT}, {hive})"),
        unsafe { RegLoadKeyW(HKEY_USERS, &mount, &HSTRING::from(&hive)) }.ok(),
    )?;
    crate::log(&format!(
        r"  default-user hive {hive} loaded at HKEY_USERS\{DEFAULT_HIVE_MOUNT}"
    ));
    let r = f(DEFAULT_HIVE_MOUNT);
    // input.dll may still hold a handle for a moment; the hive cannot be unloaded while a key is open.
    let mut u = unsafe { RegUnLoadKeyW(HKEY_USERS, &mount) };
    for _ in 0..20 {
        if u.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        u = unsafe { RegUnLoadKeyW(HKEY_USERS, &mount) };
    }
    r.and(step("RegUnLoadKey", u.ok()))
}

/// Runs an MSI-style export: debug logging is set up first (HKCU `Debug`, or the `KBD_DEBUG` environment variable that
/// kbd-harness `call` sets), so every step of `f` can log; then 0 or 1603.
fn msi_result(what: &str, f: impl FnOnce() -> Result<()>) -> u32 {
    crate::set_debug(crate::settings().debug || std::env::var_os("KBD_DEBUG").is_some());
    crate::log(&format!("{what} start"));
    match guard(what, f) {
        Ok(()) => {
            crate::log(&format!("{what} ok"));
            0
        }
        Err(e) => {
            crate::log(&format!(
                "{what} failed {:#010x} {}",
                e.code().0,
                e.message()
            ));
            ERROR_INSTALL_FAILURE
        }
    }
}

/// Adds the keyboard to the calling user's input list (impersonated, not elevated). Adds uz-Latn if missing.
#[unsafe(no_mangle)]
pub extern "system" fn KbdInstallForUser(_msi: u32) -> u32 {
    msi_result("KbdInstallForUser", || install_layout_or_tip(0))
}

/// Removes it from the calling user's list and deletes the per-user keys Windows leaves behind.
#[unsafe(no_mangle)]
pub extern "system" fn KbdUninstallForUser(_msi: u32) -> u32 {
    msi_result("KbdUninstallForUser", || {
        let r = install_layout_or_tip(ILOT_UNINSTALL);
        clean_user_keys(HKEY_CURRENT_USER, "");
        r
    })
}

/// Enables the keyboard in the default-user hive, so every profile created later starts with it (elevated).
#[unsafe(no_mangle)]
pub extern "system" fn KbdInstallForDefaultUser(_msi: u32) -> u32 {
    msi_result("KbdInstallForDefaultUser", || {
        with_default_hive(|m| install_layout_or_tip_user_reg(m, 0))
    })
}

/// The reverse, plus the leftover keys in that hive (elevated).
#[unsafe(no_mangle)]
pub extern "system" fn KbdUninstallForDefaultUser(_msi: u32) -> u32 {
    msi_result("KbdUninstallForDefaultUser", || {
        with_default_hive(|m| {
            let r = install_layout_or_tip_user_reg(m, ILOT_UNINSTALL);
            clean_user_keys(HKEY_USERS, &format!(r"{m}\"));
            r
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// prune() on a scratch tree under HKCU (removed again): our entries and the keys they leave empty go, another
    /// keyboard's entry stays.
    #[test]
    fn prune_removes_only_our_entries() {
        let root = format!(r"Software\kbd-tip-test-{}", std::process::id());
        let lang = format!(r"{root}\AssemblyItem\0x00000443");
        let cat = r"{34745C63-B2F0-4784-8B67-5E12C8701A31}";
        set_sz(
            HKEY_CURRENT_USER,
            &format!(r"{lang}\{cat}\00000000"),
            Some("CLSID"),
            &braced(&CLSID_TIP).to_lowercase(),
        )
        .unwrap();
        set_sz(
            HKEY_CURRENT_USER,
            &format!(r"{lang}\{cat}\00000001"),
            Some("CLSID"),
            "{00000000-0000-0000-0000-000000000001}",
        )
        .unwrap();
        set_sz(
            HKEY_CURRENT_USER,
            &format!(r"{root}\Other\{cat}\00000000"),
            Some("CLSID"),
            &braced(&CLSID_TIP),
        )
        .unwrap();
        clean_user_keys(HKEY_CURRENT_USER, &format!(r"{root}\")); // wrong layout: nothing below `root` matches its paths
        prune(HKEY_CURRENT_USER, &lang);
        assert_eq!(
            subkeys(HKEY_CURRENT_USER, &format!(r"{lang}\{cat}")),
            vec!["00000001".to_string()]
        );
        set_sz(
            HKEY_CURRENT_USER,
            &format!(r"{lang}\{cat}\00000001"),
            Some("CLSID"),
            &braced(&CLSID_TIP),
        )
        .unwrap();
        prune(HKEY_CURRENT_USER, &lang);
        assert!(
            open(HKEY_CURRENT_USER, &lang, KEY_READ).is_none(),
            "emptied keys are removed up to the language key"
        );
        assert!(
            open(
                HKEY_CURRENT_USER,
                &format!(r"{root}\Other\{cat}\00000000"),
                KEY_READ
            )
            .is_some(),
            "outside the language key: kept"
        );
        delete_tree(HKEY_CURRENT_USER, &root).unwrap();
        assert!(open(HKEY_CURRENT_USER, &root, KEY_READ).is_none());
    }
}
