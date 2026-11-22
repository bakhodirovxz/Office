//! Machine-test driver for the product TIP (machine-test/run.sh). Never shipped. Adapted from the Phase 0 spike's kbdctl:
//! the SendInput suite against real applications is the same; registration now goes through the DLL's own exports.
//!
//!   kbd-harness wait-unlock <min>        wait until the desktop is unlocked (no keys are sent while it is locked)
//!   kbd-harness enum                     what TSF sees for uz-Latn (0x0443)
//!   kbd-harness call <dll> <export>      LoadLibrary + call an MSI-style export (KbdInstallForUser, ...) with handle 0
//!   kbd-harness debug on|off             set or remove the HKCU `Debug` flag (the TIP then logs per-key times)
//!   kbd-harness log-path                 print the TIP's debug log path
//!   kbd-harness settings-key             print the HKCU settings key (below HKCU)
//!   kbd-harness suite notepad wordpad edge   per app: a plain run, then a TIP run (session-wide profile switch, restored)

use std::time::{Duration, Instant};

use kbd::{CLSID_TIP, LANGID_UZ_LATN, PROFILE};
use kbd_core::plain;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

/// (typed, expected with the TIP). '\u{8}' = Backspace, '\n' = Enter. The spike's 23 machine-test cases.
const CASES: &[(&str, &str)] = &[
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

fn profiles() -> Result<ITfInputProcessorProfileMgr> {
    unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
}
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    match a(0) {
        "enum" => unsafe { enum_profiles()? },
        "wait-unlock" => {
            // SendInput needs an unlocked, interactive desktop: wait until the lock screen is gone.
            let mins: u64 = a(1).parse().unwrap_or(30);
            let t0 = Instant::now();
            while fg_exe().to_ascii_lowercase().ends_with("lockapp.exe") || fg_exe().is_empty() {
                if t0.elapsed() > Duration::from_secs(mins * 60) {
                    println!("still locked after {mins} min");
                    std::process::exit(4);
                }
                std::thread::sleep(Duration::from_secs(5));
            }
            println!(
                "unlocked after {:.0}s (foreground {})",
                t0.elapsed().as_secs_f64(),
                fg_exe()
            );
        }
        "call" => {
            let rc = call_export(a(1), a(2))?;
            println!("{} -> {rc}", a(2));
            std::process::exit(rc as i32);
        }
        "debug" => debug_flag(a(1) == "on")?,
        "log-path" => println!("{}", kbd::log_path().display()),
        "settings-key" => println!("{}", kbd::settings_key()),
        "suite" => suite(&args[1..].iter().map(String::as_str).collect::<Vec<_>>())?,
        _ => eprintln!("usage: see the file header"),
    }
    Ok(())
}

/// Calls the export with the DLL's debug log on for this process (KBD_DEBUG), then prints what the export logged, so the
/// failing step shows on the console and in elevated.log.
fn call_export(dll: &str, export: &str) -> Result<u32> {
    // Single-threaded here: nothing else reads the environment.
    unsafe { std::env::set_var("KBD_DEBUG", "1") };
    let log = kbd::log_path();
    let start = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0) as usize;
    let rc = call_raw(dll, export);
    if let Ok(b) = std::fs::read(&log) {
        for l in String::from_utf8_lossy(b.get(start..).unwrap_or_default()).lines() {
            eprintln!("  log: {l}");
        }
    }
    rc
}

fn call_raw(dll: &str, export: &str) -> Result<u32> {
    unsafe {
        let m = LoadLibraryW(&HSTRING::from(dll))?;
        let name = std::ffi::CString::new(export).unwrap();
        let f =
            GetProcAddress(m, PCSTR(name.as_ptr() as *const u8)).ok_or_else(Error::from_thread)?;
        let f: unsafe extern "system" fn(u32) -> u32 = std::mem::transmute(f);
        Ok(f(0))
    }
}

/// `HKCU\Software\<brand>\Keyboard` `Debug`. Off removes the value, then the key and its parent if they are empty.
fn debug_flag(on: bool) -> Result<()> {
    let key = kbd::settings_key();
    let run = |args: &[&str]| {
        std::process::Command::new("reg.exe")
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    let hk = format!(r"HKCU\{key}");
    if on {
        println!(
            "debug on: {}",
            run(&[
                "add",
                &hk,
                "/v",
                "Debug",
                "/t",
                "REG_DWORD",
                "/d",
                "1",
                "/f"
            ])
        );
        return Ok(());
    }
    println!("debug off: {}", run(&["delete", &hk, "/v", "Debug", "/f"]));
    for k in [
        key.as_str(),
        key.rsplit_once('\\').map(|p| p.0).unwrap_or(""),
    ] {
        // RegDeleteKeyW fails on a key that still has subkeys; values are checked first.
        let mut n_values = 0u32;
        let mut n_keys = 0u32;
        let mut h = HKEY::default();
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(k), None, KEY_READ, &mut h).is_ok() {
                let _ = RegQueryInfoKeyW(
                    h,
                    None,
                    None,
                    None,
                    Some(&mut n_keys),
                    None,
                    None,
                    Some(&mut n_values),
                    None,
                    None,
                    None,
                    None,
                );
                let _ = RegCloseKey(h);
                if n_keys == 0 && n_values == 0 {
                    println!(
                        r"delete empty HKCU\{k}:{:?}",
                        RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(k))
                    );
                }
            }
        }
    }
    Ok(())
}

unsafe fn enum_profiles() -> Result<()> {
    unsafe {
        let pm = profiles()?;
        let e = pm.EnumProfiles(LANGID_UZ_LATN)?;
        let mut p = [TF_INPUTPROCESSORPROFILE::default()];
        let mut n = 0u32;
        let mut count = 0;
        while e.Next(&mut p, &mut n).is_ok() && n == 1 {
            count += 1;
            let ours = if p[0].clsid == CLSID_TIP {
                "  <- ours"
            } else {
                ""
            };
            println!(
                "  type={} clsid={:?} hkl={:?} flags={:#x}{ours}",
                p[0].dwProfileType, p[0].clsid, p[0].hkl, p[0].dwFlags
            );
        }
        println!("lang {LANGID_UZ_LATN:#06x}: {count} profiles");
        let mut p = TF_INPUTPROCESSORPROFILE::default();
        let r = pm.GetProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            LANGID_UZ_LATN,
            &CLSID_TIP,
            &PROFILE,
            HKL::default(),
            &mut p,
        );
        println!(
            "GetProfile(ours) -> {:?} flags={:#x}",
            r.map_err(|e| e.code()),
            p.dwFlags
        );
        // The name Windows shows (the MUI string when the display name is registered, in this user's UI language).
        let old: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let d = old.GetLanguageProfileDescription(&CLSID_TIP, LANGID_UZ_LATN, &PROFILE);
        println!(
            "description -> {:?}",
            d.map(|b| b.to_string()).map_err(|e| e.code())
        );
        Ok(())
    }
}

// ---------- automation ----------

/// One scripted step: a character to type ('\u{8}' = Backspace, '\n' = Enter).
fn inputs_for(c: char) -> Vec<INPUT> {
    let kb = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let (vk, shift) = match c {
        '\u{8}' => (VK_BACK, false),
        '\n' => (VK_RETURN, false),
        ' ' => (VK_SPACE, false),
        '\'' => (VK_OEM_7, false),
        'a'..='z' => (VIRTUAL_KEY(c.to_ascii_uppercase() as u16), false),
        'A'..='Z' => (VIRTUAL_KEY(c as u16), true),
        _ => panic!("script char {c:?}"),
    };
    let mut v = vec![];
    if shift {
        v.push(kb(VK_SHIFT, false));
    }
    v.push(kb(vk, false));
    v.push(kb(vk, true));
    if shift {
        v.push(kb(VK_SHIFT, true));
    }
    v
}

fn get_text(hwnd: HWND) -> Option<String> {
    let mut buf = vec![0u16; 4096];
    let mut res = 0usize;
    let ok = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETTEXT,
            WPARAM(buf.len()),
            LPARAM(buf.as_mut_ptr() as isize),
            SMTO_ABORTIFHUNG,
            2000,
            Some(&mut res),
        )
    };
    (ok.0 != 0).then(|| String::from_utf16_lossy(&buf[..res]))
}

struct Lat(Vec<f64>);
impl Lat {
    fn report(&mut self, name: &str) {
        self.0.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = self.0.len();
        if n == 0 {
            return;
        }
        let p = |q: f64| self.0[((n as f64 - 1.0) * q).round() as usize];
        println!(
            "{name}: keys={n} median={:.2}ms p95={:.2}ms max={:.2}ms",
            p(0.5),
            p(0.95),
            self.0[n - 1]
        );
    }
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Where the typed text is read back: an edit control (WM_GETTEXT) or a browser page that mirrors its textarea into
/// the window title as `uzkbd[<encodeURIComponent(value)>]`.
#[derive(Clone, Copy)]
enum Target {
    Wm { top: isize, edit: isize },
    Title { top: isize },
}

impl Target {
    fn top(&self) -> HWND {
        match *self {
            Target::Wm { top, .. } | Target::Title { top } => HWND(top as _),
        }
    }
    fn read(&self) -> Option<String> {
        match *self {
            Target::Wm { edit, .. } => get_text(HWND(edit as _)),
            Target::Title { top } => title_text(HWND(top as _)),
        }
    }
    fn clear(&self) {
        match *self {
            Target::Wm { edit, .. } => unsafe {
                SendMessageTimeoutW(
                    HWND(edit as _),
                    WM_SETTEXT,
                    WPARAM(0),
                    LPARAM(w("\0").as_ptr() as isize),
                    SMTO_ABORTIFHUNG,
                    2000,
                    None,
                );
            },
            Target::Title { .. } => {
                let t0 = Instant::now();
                while self.read().as_deref() != Some("") && t0.elapsed() < Duration::from_secs(3) {
                    if unsafe { GetForegroundWindow() } != self.top() {
                        return;
                    }
                    let a = VIRTUAL_KEY(b'A' as u16);
                    let v = [
                        key(VK_CONTROL, false),
                        key(a, false),
                        key(a, true),
                        key(VK_CONTROL, true),
                        key(VK_BACK, false),
                        key(VK_BACK, true),
                    ];
                    unsafe { SendInput(&v, std::mem::size_of::<INPUT>() as i32) };
                    std::thread::sleep(Duration::from_millis(150));
                }
            }
        }
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn window_title(h: HWND) -> String {
    let mut buf = [0u16; 1024];
    let n = unsafe { GetWindowTextW(h, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Decodes `uzkbd[%..]` from a window title.
fn title_text(top: HWND) -> Option<String> {
    let t = window_title(top);
    let a = t.find("uzkbd[")? + 6;
    let b = a + t[a..].find(']')?;
    let enc = &t.as_bytes()[a..b];
    let mut out = vec![];
    let mut i = 0;
    while i < enc.len() {
        if enc[i] == b'%' && i + 2 < enc.len() {
            out.push(u8::from_str_radix(std::str::from_utf8(&enc[i + 1..i + 3]).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(enc[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Types every case into the target, checks the text, returns per-key latencies (ms). Stops (Err) the moment the
/// target is not the foreground window or the desktop is locked, so no key ever reaches another window.
fn drive(t: Target, with_tip: bool) -> std::result::Result<(usize, usize, Lat), String> {
    warm_up(t)?;
    let mut lat = Lat(vec![]);
    let (mut pass, mut fail) = (0, 0);
    for (typed, want_tip) in CASES {
        let want = norm(&if with_tip {
            want_tip.to_string()
        } else {
            plain(typed)
        });
        t.clear();
        // Caps Lock turned on mid-run twice (machine runs 3 and 4, cause unknown); it inverts every case. Never while
        // the TIP is on: caps_in attaches to the target's input thread, and that ended the TIP within milliseconds in
        // every app of machine run 6 (suite() checks once before the TIP is activated; a FAIL line flags an inversion).
        if !with_tip && caps_in(t.top()) {
            toggle_caps();
            println!(
                "  caps lock found on before {typed:?}: switched off -> on={}",
                caps_in(t.top())
            );
        }
        for c in typed.chars() {
            if unsafe { GetForegroundWindow() } != t.top()
                || fg_exe().to_ascii_lowercase().ends_with("lockapp.exe")
            {
                return Err(format!(
                    "lost the foreground (now {}); no keys sent elsewhere",
                    fg_exe()
                ));
            }
            let before = t.read().unwrap_or_default();
            let inp = inputs_for(c);
            let t0 = Instant::now();
            unsafe { SendInput(&inp, std::mem::size_of::<INPUT>() as i32) };
            // Latency: until the visible text changes.
            loop {
                if t.read().unwrap_or_default() != before {
                    lat.0.push(t0.elapsed().as_secs_f64() * 1000.0);
                    break;
                }
                if t0.elapsed() > Duration::from_millis(1000) {
                    println!("  no text change after {c:?}");
                    break;
                }
                std::thread::yield_now();
            }
        }
        std::thread::sleep(Duration::from_millis(50));
        let got = norm(&t.read().unwrap_or_default());
        if got == want {
            pass += 1;
            println!("  ok   {typed:?} -> {got:?}");
        } else {
            fail += 1;
            let inverted = got == swap_case(&want);
            println!(
                "  FAIL {typed:?} -> {got:?} (want {want:?}){}",
                if inverted {
                    " CASE INVERTED: Caps Lock was on"
                } else {
                    ""
                }
            );
        }
    }
    Ok((pass, fail, lat))
}

fn swap_case(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_uppercase() {
                c.to_lowercase().collect::<Vec<_>>()
            } else {
                c.to_uppercase().collect()
            }
        })
        .collect()
}

/// The session's keyboard input right now.
fn active_input(pm: &ITfInputProcessorProfileMgr) -> String {
    let mut ap = TF_INPUTPROCESSORPROFILE::default();
    let _ = unsafe { pm.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut ap) };
    format!(
        "lang={:#06x} tip={} hkl={:?}",
        ap.langid,
        ap.clsid == CLSID_TIP,
        ap.hkl
    )
}

/// Machine run 6: the TIP was deactivated in all three apps right after the harness attached to the target's input
/// thread (caps_in). Shows whether attaching alone changes the session's input, without sending a key.
fn attach_check(pm: &ITfInputProcessorProfileMgr, top: HWND, label: &str) {
    let before = active_input(pm);
    let _ = caps_in(top);
    std::thread::sleep(Duration::from_millis(500));
    println!(
        "  attach check ({label}): before {before}; after AttachThreadInput {}",
        active_input(pm)
    );
}

/// A loaded keyboard layout of the user's that is not US English (ru on the owner's PC), if any.
fn other_layout() -> Option<HKL> {
    let mut list = [HKL::default(); 32];
    let n = unsafe { GetKeyboardLayoutList(Some(&mut list)) } as usize;
    list[..n.min(32)]
        .iter()
        .copied()
        .find(|h| !matches!((h.0 as usize & 0xffff) as u16, 0x0409 | LANGID_UZ_LATN))
}

/// The first key after start-up can be lost (machine run 5: Edge's first case gave "hahar "). Types a space until the
/// target shows a change (at most 10 tries, 1 s each), then clears it.
fn warm_up(t: Target) -> std::result::Result<(), String> {
    t.clear();
    for i in 0..10 {
        if unsafe { GetForegroundWindow() } != t.top() {
            return Err(format!("warm-up: lost the foreground (now {})", fg_exe()));
        }
        let before = t.read().unwrap_or_default();
        let inp = inputs_for(' ');
        unsafe { SendInput(&inp, std::mem::size_of::<INPUT>() as i32) };
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(1) {
            if t.read().unwrap_or_default() != before {
                println!("  warm-up: ready after {} key(s)", i + 1);
                t.clear();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    println!("  warm-up: no change after 10 keys (continuing)");
    t.clear();
    Ok(())
}

/// Caps Lock as the window's input thread sees it (a console thread's own GetKeyState can be stale).
fn caps_in(hwnd: HWND) -> bool {
    unsafe {
        let th = GetWindowThreadProcessId(hwnd, None);
        let me = GetCurrentThreadId();
        let attached = th != 0 && th != me && AttachThreadInput(me, th, true).as_bool();
        let on = GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0;
        if attached {
            let _ = AttachThreadInput(me, th, false);
        }
        on
    }
}

fn toggle_caps() {
    unsafe {
        SendInput(
            &[key(VK_CAPITAL, false), key(VK_CAPITAL, true)],
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    std::thread::sleep(Duration::from_millis(200));
}

fn force_foreground(hwnd: HWND) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        let attached =
            fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        std::thread::sleep(Duration::from_millis(200));
        GetForegroundWindow() == hwnd
    }
}

fn fg_exe() -> String {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 512];
        let mut n = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut n)
            .is_ok();
        let _ = CloseHandle(h);
        if ok {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            String::new()
        }
    }
}

// ---------- real applications ----------

const EDGE: &str = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
/// Harness scratch on E: (the Edge profile and the page live here; the runner deletes them).
const SCRATCH: &str = r"E:\office-target\kbd-tip\m";

fn wait_window(f: impl Fn() -> HWND, secs: u64) -> HWND {
    let t0 = Instant::now();
    loop {
        let h = f();
        if !h.is_invalid() || t0.elapsed() > Duration::from_secs(secs) {
            return h;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn find_title_prefix(prefix: &str) -> HWND {
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        unsafe {
            let st = &mut *(l.0 as *mut (String, HWND));
            if IsWindowVisible(h).as_bool() && window_title(h).starts_with(&st.0) {
                st.1 = h;
                return false.into();
            }
            true.into()
        }
    }
    let mut st = (prefix.to_string(), HWND::default());
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut st as *mut _ as isize));
    }
    st.1
}

fn kill_tree(pid: u32) {
    let _ = std::process::Command::new("taskkill.exe")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}

/// Starts the app; returns (pid to kill, target).
fn start_app(app: &str) -> std::result::Result<(u32, Target), String> {
    unsafe {
        match app {
            "notepad" | "wordpad" => {
                let (exe, class) = if app == "notepad" {
                    (r"C:\Windows\System32\notepad.exe", w!("Edit"))
                } else {
                    (
                        r"C:\Program Files\Windows NT\Accessories\wordpad.exe",
                        w!("RICHEDIT50W"),
                    )
                };
                let child = std::process::Command::new(exe)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                let pid = child.id();
                let top = wait_window(|| find_top(pid), 10);
                std::thread::sleep(Duration::from_millis(500));
                let edit = FindWindowExW(Some(top), None, class, None)
                    .map_err(|e| format!("{app}: no edit child: {e}"))?;
                Ok((
                    pid,
                    Target::Wm {
                        top: top.0 as isize,
                        edit: edit.0 as isize,
                    },
                ))
            }
            "edge" => {
                std::fs::create_dir_all(SCRATCH).map_err(|e| e.to_string())?;
                let page = format!(r"{SCRATCH}\page.html");
                std::fs::write(
                    &page,
                    "<!doctype html><meta charset=utf-8><title>uzkbd[]</title><textarea id=t autofocus style='width:90%;height:300px' \
                     oninput=\"document.title='uzkbd['+encodeURIComponent(this.value)+']'\"></textarea><script>t.focus()</script>",
                )
                .map_err(|e| e.to_string())?;
                let child = std::process::Command::new(EDGE)
                    .args([
                        &format!(r"--user-data-dir={SCRATCH}\edge-ud"),
                        "--no-first-run",
                        "--no-default-browser-check",
                        "--disable-sync",
                        "--new-window",
                        &format!("file:///{}", page.replace('\\', "/")),
                    ])
                    .spawn()
                    .map_err(|e| e.to_string())?;
                let top = wait_window(|| find_title_prefix("uzkbd["), 30);
                if top.is_invalid() {
                    kill_tree(child.id());
                    return Err("edge: page window not found".into());
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(top, Some(&mut pid));
                Ok((
                    pid,
                    Target::Title {
                        top: top.0 as isize,
                    },
                ))
            }
            _ => Err(format!("unknown app {app}")),
        }
    }
}

fn activate(pm: &ITfInputProcessorProfileMgr, tip: bool) -> Result<()> {
    unsafe {
        if tip {
            pm.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                LANGID_UZ_LATN,
                &CLSID_TIP,
                &PROFILE,
                HKL::default(),
                TF_IPPMF_FORSESSION,
            )
        } else {
            pm.ActivateProfile(
                TF_PROFILETYPE_KEYBOARDLAYOUT,
                0x0409,
                &GUID::zeroed(),
                &GUID::zeroed(),
                HKL(0x0409_0409 as _),
                TF_IPPMF_FORSESSION,
            )
        }
    }
}

fn suite(apps: &[&str]) -> Result<()> {
    unsafe {
        if fg_exe().to_ascii_lowercase().ends_with("lockapp.exe") {
            eprintln!("ABORT: the desktop is locked; no keys sent");
            std::process::exit(4);
        }
        let pm = profiles()?;
        let mut saved = TF_INPUTPROCESSORPROFILE::default();
        pm.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut saved)?;
        println!(
            "session input before: type={} lang={:#06x} clsid={:?} hkl={:?}",
            saved.dwProfileType, saved.langid, saved.clsid, saved.hkl
        );
        // drive() switches Caps Lock off before each case; put the user's state back at the end.
        let caps_was_on = caps_in(GetForegroundWindow());
        println!("caps lock at start: on={caps_was_on}");
        for app in apps {
            println!("=== {app}");
            let (pid, target) = match start_app(app) {
                Ok(x) => x,
                Err(e) => {
                    println!("  SKIP: {e}");
                    continue;
                }
            };
            for tip in [false, true] {
                if !force_foreground(target.top()) {
                    println!("  ABORT: cannot take the foreground (now {})", fg_exe());
                    break;
                }
                if tip && caps_in(target.top()) {
                    toggle_caps();
                    println!(
                        "  caps lock found on before the TIP run: switched off -> on={}",
                        caps_in(target.top())
                    );
                }
                let r = activate(&pm, tip);
                std::thread::sleep(Duration::from_millis(500));
                let mut ap = TF_INPUTPROCESSORPROFILE::default();
                let _ = pm.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut ap);
                println!(
                    "  {}: activate -> {:?}; active lang={:#06x} clsid={:?}",
                    if tip { "TIP" } else { "plain" },
                    r.map_err(|e| e.code()),
                    ap.langid,
                    ap.clsid
                );
                match drive(target, tip) {
                    Ok((pass, fail, mut lat)) => {
                        println!(
                            "  {app} {}: pass={pass} fail={fail}",
                            if tip { "TIP" } else { "plain" }
                        );
                        lat.report("  latency SendInput->text change");
                    }
                    Err(e) => {
                        println!("  ABORT: {e}");
                        break;
                    }
                }
                if tip && GetForegroundWindow() == target.top() {
                    attach_check(&pm, target.top(), "TIP");
                    // The same with another of the user's layouts: tells a general Windows effect from one of our TIP.
                    if let Some(hkl) = other_layout() {
                        let lang = (hkl.0 as usize & 0xffff) as u16;
                        let r = pm.ActivateProfile(
                            TF_PROFILETYPE_KEYBOARDLAYOUT,
                            lang,
                            &GUID::zeroed(),
                            &GUID::zeroed(),
                            hkl,
                            TF_IPPMF_FORSESSION,
                        );
                        std::thread::sleep(Duration::from_millis(500));
                        println!(
                            "  layout {hkl:?}: activate -> {:?}",
                            r.map_err(|e| e.code())
                        );
                        attach_check(&pm, target.top(), "layout");
                    }
                }
            }
            kill_tree(pid);
            std::thread::sleep(Duration::from_millis(500));
        }
        let r = pm.ActivateProfile(
            saved.dwProfileType,
            saved.langid,
            &saved.clsid,
            &saved.guidProfile,
            saved.hkl,
            TF_IPPMF_FORSESSION,
        );
        println!("session input restored -> {:?}", r.map_err(|e| e.code()));
        let fg = GetForegroundWindow();
        if caps_in(fg) != caps_was_on {
            toggle_caps();
        }
        println!(
            "caps lock at end: on={} (start: {caps_was_on})",
            caps_in(fg)
        );
        Ok(())
    }
}

fn find_top(pid: u32) -> HWND {
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        unsafe {
            let st = &mut *(l.0 as *mut (u32, HWND));
            let mut p = 0u32;
            GetWindowThreadProcessId(h, Some(&mut p));
            if p == st.0 && IsWindowVisible(h).as_bool() {
                st.1 = h;
                return false.into();
            }
            true.into()
        }
    }
    let mut st = (pid, HWND::default());
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut st as *mut _ as isize));
    }
    st.1
}
