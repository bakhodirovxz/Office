//! Spike driver: per-user registration experiments, a test host, and SendInput automation with latency.
//! Everything it writes is under HKCU and is removed by `unreg` (see docs/keyboard/PLAN.md, spike log).
//!
//!   kbdctl reg-com <dll>      HKCU\Software\Classes\CLSID\{spike} -> dll (per-user COM)
//!   kbdctl api-register       ITfInputProcessorProfileMgr::RegisterProfile + ITfCategoryMgr::RegisterCategory (system-wide)
//!   kbdctl hkcu-tip           write the TIP profile keys under HKCU\Software\Microsoft\CTF\TIP (does TSF read them?)
//!   kbdctl enum               what TSF sees for LANGID 0x0443
//!   kbdctl host [edit|rich] [tip|plain] [feed]   own window, profile registered TF_RP_LOCALPROCESS, typed by SendInput
//!   kbdctl notepad [tip|plain]            type into a Notepad started here (needs a profile TSF can activate)
//!   kbdctl unreg              remove every HKCU key the spike creates

use std::time::{Duration, Instant};

use uzkbd_spike::{CLSID_SPIKE, LANGID_UZ_LATN, PROFILE_SPIKE};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

const DESC: &str = "Uzbek 2026 letters (spike)";

fn g(x: &GUID) -> String {
    format!("{{{x:?}}}")
}
fn reg(args: &[&str]) -> bool {
    let out = std::process::Command::new("reg.exe").args(args).output().expect("reg.exe");
    if !out.status.success() {
        eprintln!("reg {:?}: {}", args, String::from_utf8_lossy(&out.stderr).trim());
    }
    out.status.success()
}
fn hkcu_tip_key() -> String {
    format!(r"HKCU\Software\Microsoft\CTF\TIP\{}", g(&CLSID_SPIKE))
}
fn com_key() -> String {
    format!(r"HKCU\Software\Classes\CLSID\{}", g(&CLSID_SPIKE))
}

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
        "reg-com" => {
            let k = com_key();
            let ok = reg(&["add", &k, "/ve", "/d", DESC, "/f"])
                && reg(&["add", &format!(r"{k}\InprocServer32"), "/ve", "/d", a(1), "/f"])
                && reg(&["add", &format!(r"{k}\InprocServer32"), "/v", "ThreadingModel", "/d", "Apartment", "/f"]);
            println!("reg-com {}", if ok { "ok" } else { "FAILED" });
        }
        "api-register" => unsafe {
            let pm = profiles()?;
            let r = pm.RegisterProfile(&CLSID_SPIKE, LANGID_UZ_LATN, &PROFILE_SPIKE, &w(DESC), &[], 0, HKL::default(), 0, true, 0);
            println!("RegisterProfile (system) -> {:?}", r.map_err(|e| e.code()));
            let cm: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
            let r = cm.RegisterCategory(&CLSID_SPIKE, &GUID_TFCAT_TIP_KEYBOARD, &CLSID_SPIKE);
            println!("RegisterCategory keyboard -> {:?}", r.map_err(|e| e.code()));
        },
        "hkcu-tip" => {
            // Same shape as the per-machine registration under HKLM\SOFTWARE\Microsoft\CTF\TIP.
            let k = hkcu_tip_key();
            let lp = format!(r"{k}\LanguageProfile\0x{:08x}\{}", LANGID_UZ_LATN, g(&PROFILE_SPIKE));
            let cat = g(&GUID_TFCAT_TIP_KEYBOARD);
            let ok = reg(&["add", &lp, "/v", "Description", "/d", DESC, "/f"])
                && reg(&["add", &lp, "/v", "Enable", "/t", "REG_DWORD", "/d", "1", "/f"])
                && reg(&["add", &lp, "/v", "IconFile", "/d", "", "/f"])
                && reg(&["add", &lp, "/v", "IconIndex", "/t", "REG_DWORD", "/d", "0", "/f"])
                && reg(&["add", &format!(r"{k}\Category\Category\{cat}\{}", g(&CLSID_SPIKE)), "/f"])
                && reg(&["add", &format!(r"{k}\Category\Item\{}\{cat}", g(&CLSID_SPIKE)), "/f"]);
            println!("hkcu-tip {}", if ok { "ok" } else { "FAILED" });
        }
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
            println!("unlocked after {:.0}s (foreground {})", t0.elapsed().as_secs_f64(), fg_exe());
        }
        "cocreate" => unsafe {
            let r: Result<ITfTextInputProcessor> = CoCreateInstance(&CLSID_SPIKE, None, CLSCTX_INPROC_SERVER);
            println!("CoCreateInstance(spike TIP, per-user COM) -> {:?}", r.map(|_| ()).map_err(|e| e.code()));
        },
        "unreg" => {
            for k in [hkcu_tip_key(), com_key()] {
                let exists = std::process::Command::new("reg.exe").args(["query", &k]).output().map(|o| o.status.success()).unwrap_or(false);
                if exists {
                    println!("delete {k}: {}", reg(&["delete", &k, "/f"]));
                } else {
                    println!("absent {k}");
                }
            }
        }
        "host" => host(a(1) == "rich", a(2) != "plain", a(3) == "feed")?,
        "notepad" => notepad(a(1) != "plain")?,
        _ => eprintln!("usage: see the file header"),
    }
    Ok(())
}

unsafe fn enum_profiles() -> Result<()> {
    unsafe {
        let pm = profiles()?;
        for lang in [LANGID_UZ_LATN, 0x0409] {
            let e = pm.EnumProfiles(lang)?;
            let mut p = [TF_INPUTPROCESSORPROFILE::default()];
            let mut n = 0u32;
            let mut count = 0;
            while e.Next(&mut p, &mut n).is_ok() && n == 1 {
                count += 1;
                if lang == LANGID_UZ_LATN {
                    println!("  lang {lang:#06x}: type={} clsid={:?} hkl={:?} flags={:#x}", p[0].dwProfileType, p[0].clsid, p[0].hkl, p[0].dwFlags);
                }
                if p[0].clsid == CLSID_SPIKE {
                    println!("  lang {lang:#06x}: SPIKE profile visible, flags={:#x}", p[0].dwFlags);
                }
            }
            println!("lang {lang:#06x}: {count} profiles");
        }
        let mut p = TF_INPUTPROCESSORPROFILE::default();
        let r = pm.GetProfile(TF_PROFILETYPE_INPUTPROCESSOR, LANGID_UZ_LATN, &CLSID_SPIKE, &PROFILE_SPIKE, HKL::default(), &mut p);
        println!("GetProfile(spike) -> {:?} flags={:#x}", r.map_err(|e| e.code()), p.dwFlags);
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
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let (vk, shift) = match c {
        '\u{8}' => (VK_BACK, false),
        '\n' => (VK_RETURN, false),
        ' ' => (VK_SPACE, false),
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
        SendMessageTimeoutW(hwnd, WM_GETTEXT, WPARAM(buf.len()), LPARAM(buf.as_mut_ptr() as isize), SMTO_ABORTIFHUNG, 2000, Some(&mut res))
    };
    (ok.0 != 0).then(|| String::from_utf16_lossy(&buf[..res]))
}

/// Cases: (typed, expected with the TIP, expected without it).
const CASES: &[(&str, &str, &str)] = &[
    ("shahar ", "şahar ", "shahar "),
    ("Shahar ", "Şahar ", "Shahar "),
    ("SHAHAR ", "ŞAHAR ", "SHAHAR "),
    ("maktab ", "maktab ", "maktab "),
    ("mashq ", "maşq ", "mashq "),
    ("Ish\u{8}oq ", "Ishoq ", "Isoq "),
    ("as\u{8}b ", "ab ", "ab "),
    ("ssh ", "sş ", "ssh "),
    ("tosh\n", "toş\r\n", "tosh\r\n"),
    ("qush\u{8}\u{8}x ", "qusx ", "qux "),
];

struct Lat(Vec<f64>);
impl Lat {
    fn report(&mut self, name: &str) {
        self.0.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = self.0.len();
        if n == 0 {
            return;
        }
        let p = |q: f64| self.0[((n as f64 - 1.0) * q).round() as usize];
        println!("{name}: keys={n} median={:.2}ms p95={:.2}ms max={:.2}ms", p(0.5), p(0.95), self.0[n - 1]);
    }
}

/// Types every case into `edit` (inside top-level `top`), checks the text, returns per-key latencies (ms).
fn drive(top: HWND, edit: HWND, with_tip: bool) -> (usize, usize, Lat) {
    let mut lat = Lat(vec![]);
    let (mut pass, mut fail) = (0, 0);
    for (typed, want_tip, want_plain) in CASES {
        let want = if with_tip { *want_tip } else { *want_plain };
        unsafe {
            SendMessageTimeoutW(edit, WM_SETTEXT, WPARAM(0), LPARAM(w("\0").as_ptr() as isize), SMTO_ABORTIFHUNG, 2000, None);
        }
        for c in typed.chars() {
            if unsafe { GetForegroundWindow() } != top {
                eprintln!("ABORT: our window lost the foreground; no keys sent elsewhere");
                std::process::exit(3);
            }
            let before = get_text(edit).unwrap_or_default();
            let inp = inputs_for(c);
            let t0 = Instant::now();
            unsafe { SendInput(&inp, std::mem::size_of::<INPUT>() as i32) };
            // Latency: until the visible text changes (Shift-only steps do not exist in the script).
            loop {
                let now = get_text(edit).unwrap_or_default();
                if now != before {
                    lat.0.push(t0.elapsed().as_secs_f64() * 1000.0);
                    break;
                }
                if t0.elapsed() > Duration::from_millis(1000) {
                    eprintln!("  no text change after {c:?}");
                    break;
                }
                std::thread::yield_now();
            }
        }
        // Commit any pending composition the way a user would: the TIP commits on the next key; read after a pause.
        std::thread::sleep(Duration::from_millis(50));
        let got = get_text(edit).unwrap_or_default();
        if got == want {
            pass += 1;
            println!("  ok   {typed:?} -> {got:?}");
        } else {
            fail += 1;
            println!("  FAIL {typed:?} -> {got:?} (want {want:?})");
        }
    }
    (pass, fail, lat)
}

fn force_foreground(hwnd: HWND) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
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
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let mut buf = [0u16; 512];
        let mut n = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut n).is_ok();
        let _ = CloseHandle(h);
        if ok { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() }
    }
}

fn us_layout() -> HKL {
    unsafe { LoadKeyboardLayoutW(w!("00000409"), KLF_ACTIVATE).unwrap_or_default() }
}

// ---------- own test host ----------

/// Feeds keys through ITfKeystrokeMgr, as a TSF-aware application's message loop does, with no SendInput and
/// no foreground (usable while the screen is locked). A key the TIP does not eat is delivered as WM_CHAR.
fn feed_drive(edit: HWND, with_tip: bool) -> Result<(usize, usize, Lat)> {
    unsafe {
        let tm: ITfThreadMgr = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
        let km: ITfKeystrokeMgr = tm.cast()?;
        let focus = tm.GetFocus();
        println!("  thread manager focus document: {}", if focus.is_ok() { "yes" } else { "none" });
        let mut lat = Lat(vec![]);
        let (mut pass, mut fail) = (0, 0);
        for (typed, want_tip, want_plain) in CASES {
            let want = if with_tip { *want_tip } else { *want_plain };
            SendMessageW(edit, WM_SETTEXT, None, Some(LPARAM(w("\0").as_ptr() as isize)));
            for c in typed.chars() {
                let (vk, shift, ch) = match c {
                    '\u{8}' => (VK_BACK.0, false, 8u16),
                    '\n' => (VK_RETURN.0, false, 13),
                    ' ' => (VK_SPACE.0, false, 32),
                    'a'..='z' => (c.to_ascii_uppercase() as u16, false, c as u16),
                    _ => (c as u16, true, c as u16),
                };
                let mut ks = [0u8; 256];
                let _ = GetKeyboardState(&mut ks);
                ks[VK_SHIFT.0 as usize] = if shift { 0x80 } else { 0 };
                let _ = SetKeyboardState(&ks);
                let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as isize;
                let lp = LPARAM(1 | (scan << 16));
                let before = get_text(edit).unwrap_or_default();
                let t0 = Instant::now();
                let test = km.TestKeyDown(WPARAM(vk as usize), lp);
                if std::env::var_os("UZKBD_TRACE").is_some() {
                    println!("  TestKeyDown({vk:#04x}) -> {:?} focus={:?}", test.as_ref().map(|b| b.as_bool()).map_err(|e| e.code()), GetFocus());
                }
                let eaten = test.map(|b| b.as_bool()).unwrap_or(false)
                    && km.KeyDown(WPARAM(vk as usize), lp).map(|b| b.as_bool()).unwrap_or(false);
                if !eaten {
                    SendMessageW(edit, WM_CHAR, Some(WPARAM(ch as usize)), Some(lp));
                }
                let _ = km.TestKeyUp(WPARAM(vk as usize), LPARAM(lp.0 | (3 << 30)));
                loop {
                    pump(Duration::ZERO);
                    if get_text(edit).unwrap_or_default() != before {
                        lat.0.push(t0.elapsed().as_secs_f64() * 1000.0);
                        break;
                    }
                    if t0.elapsed() > Duration::from_millis(500) {
                        println!("  no text change after {c:?} (eaten={eaten})");
                        break;
                    }
                    pump(Duration::from_millis(1));
                }
            }
            let got = get_text(edit).unwrap_or_default();
            if got == want {
                pass += 1;
                println!("  ok   {typed:?} -> {got:?}");
            } else {
                fail += 1;
                println!("  FAIL {typed:?} -> {got:?} (want {want:?})");
            }
        }
        Ok((pass, fail, lat))
    }
}

fn host(rich: bool, with_tip: bool, feed: bool) -> Result<()> {
    unsafe {
        let _ = std::fs::remove_file(uzkbd_spike::log_path());
        let hinst = GetModuleHandleW(None)?;
        let class = if rich {
            LoadLibraryW(w!("msftedit.dll"))?;
            w!("RICHEDIT50W")
        } else {
            w!("EDIT")
        };
        let top = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), w!("uz-keyboard spike host"), WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100, 100, 600, 300, None, None, Some(hinst.into()), None,
        )?;
        // A STATIC top-level has no WndProc of interest; the edit is its child.
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE(0), class, w!(""), WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL) as u32),
            10, 10, 560, 230, Some(top), None, Some(hinst.into()), None,
        )?;
        if rich {
            const EM_SETEDITSTYLE: u32 = 0x0400 + 204;
            const SES_USECTF: usize = 0x10000;
            SendMessageW(edit, EM_SETEDITSTYLE, Some(WPARAM(SES_USECTF)), Some(LPARAM(SES_USECTF as isize)));
        }
        let _ = SetFocus(Some(edit));
        us_layout();
        pump(Duration::from_millis(200));
        if with_tip {
            let pm = profiles()?;
            // UZKBD_LANG=0409: register under en-US (a language in the user's list) to separate "language not
            // installed for the user" from "no keyboard category" as the cause of a profile that does not activate.
            let lang = std::env::var("UZKBD_LANG").ok().and_then(|v| u16::from_str_radix(&v, 16).ok()).unwrap_or(LANGID_UZ_LATN);
            let r = pm.RegisterProfile(&CLSID_SPIKE, lang, &PROFILE_SPIKE, &w(DESC), &[], 0, HKL::default(), 0, true, TF_RP_LOCALPROCESS);
            println!("RegisterProfile(TF_RP_LOCALPROCESS) -> {:?}", r.as_ref().map_err(|e| e.code()));
            let r = pm.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR, lang, &CLSID_SPIKE, &PROFILE_SPIKE, HKL::default(),
                TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
            );
            println!("ActivateProfile(FORPROCESS) -> {:?}", r.as_ref().map_err(|e| e.code()));
            pump(Duration::from_millis(300));
            let mut ap = TF_INPUTPROCESSORPROFILE::default();
            let r = pm.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut ap);
            println!("GetActiveProfile(keyboard) -> {:?} lang={:#06x} clsid={:?} hkl={:?}", r.map_err(|e| e.code()), ap.langid, ap.clsid, ap.hkl);
        }
        if feed {
            let _ = SetFocus(Some(edit));
            pump(Duration::from_millis(200));
            if with_tip && std::env::var("UZKBD_MANUAL").is_ok() {
                // Harness only: activate the TIP directly on this thread's manager (profile activation needs the
                // keyboard category, which only HKLM can hold).
                let tm: ITfThreadMgr = CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
                let tid = tm.cast::<ITfClientId>()?.GetClientId(&CLSID_SPIKE)?;
                let tip: ITfTextInputProcessor = CoCreateInstance(&CLSID_SPIKE, None, CLSCTX_INPROC_SERVER)?;
                println!("manual Activate -> {:?}", tip.Activate(&tm, tid).map_err(|e| e.code()));
                let fg = tm.cast::<ITfKeystrokeMgr>()?.GetForeground();
                println!("keystroke foreground clsid -> {:?}", fg.map_err(|e| e.code()));
                std::mem::forget(tip);
            }
            let ms = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() % 100_000).unwrap_or(0);
            println!("  feed start {:05}", ms());
            let (pass, fail, mut lat) = feed_drive(edit, with_tip)?;
            println!("  feed end {:05}", ms());
            println!("host-feed {} {}: pass={pass} fail={fail}", if rich { "richedit" } else { "edit" }, if with_tip { "TIP" } else { "plain" });
            lat.report("  latency KeyDown->text change (same thread)");
            let _ = DestroyWindow(top);
            return Ok(());
        }
        if !force_foreground(top) {
            eprintln!("ABORT: cannot take the foreground (screen locked or another window holds it)");
            std::process::exit(3);
        }
        let _ = SetFocus(Some(edit));
        let (tx, rx) = std::sync::mpsc::channel();
        let (top_i, edit_i) = (top.0 as isize, edit.0 as isize);
        std::thread::spawn(move || {
            let r = drive(HWND(top_i as _), HWND(edit_i as _), with_tip);
            let _ = tx.send(r);
        });
        let (pass, fail, mut lat) = loop {
            pump(Duration::from_millis(5));
            if let Ok(r) = rx.try_recv() {
                break r;
            }
        };
        println!("host {} {}: pass={pass} fail={fail}", if rich { "richedit" } else { "edit" }, if with_tip { "TIP" } else { "plain" });
        lat.report("  latency SendInput->text change");
        let _ = DestroyWindow(top);
        Ok(())
    }
}

fn pump(d: Duration) {
    let t0 = Instant::now();
    unsafe {
        let mut msg = MSG::default();
        while t0.elapsed() < d {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

// ---------- Notepad ----------

fn notepad(with_tip: bool) -> Result<()> {
    unsafe {
        let _ = std::fs::remove_file(uzkbd_spike::log_path());
        let mut child = std::process::Command::new(r"C:\Windows\System32\notepad.exe").spawn().expect("notepad");
        let pid = child.id();
        let mut top = HWND::default();
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            top = find_top(pid);
            if !top.is_invalid() {
                break;
            }
        }
        let edit = FindWindowExW(Some(top), None, w!("Edit"), None)?;
        // US layout in Notepad (the user's list holds en-US); the TIP profile, if any, is switched to below.
        let _ = PostMessageW(Some(top), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(us_layout().0 as isize));
        if with_tip {
            let pm = profiles()?;
            let r = pm.ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR, LANGID_UZ_LATN, &CLSID_SPIKE, &PROFILE_SPIKE, HKL::default(), TF_IPPMF_FORSESSION | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE);
            println!("ActivateProfile(FORSESSION) -> {:?}", r.map_err(|e| e.code()));
        }
        let ok = force_foreground(top);
        if !ok {
            eprintln!("ABORT: cannot take the foreground");
            let _ = child.kill();
            std::process::exit(3);
        }
        let (pass, fail, mut lat) = drive(top, edit, with_tip);
        println!("notepad {}: pass={pass} fail={fail}", if with_tip { "TIP" } else { "plain" });
        lat.report("  latency SendInput->text change");
        let _ = SendMessageTimeoutW(edit, WM_SETTEXT, WPARAM(0), LPARAM(w("\0").as_ptr() as isize), SMTO_ABORTIFHUNG, 2000, None);
        let _ = child.kill();
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
