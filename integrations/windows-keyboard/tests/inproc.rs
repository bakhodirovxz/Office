//! In-process tests of the DLL's COM surface, with no registration anywhere: the class factory comes from
//! DllGetClassObject, and the TSF activation test registers the class object and a process-local profile only.

use kbd::*;
use windows::Win32::Foundation::*;
use windows::Win32::Globalization::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

fn factory(clsid: &GUID) -> std::result::Result<IClassFactory, HRESULT> {
    let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe { DllGetClassObject(clsid, &IClassFactory::IID, &mut p) };
    if hr.is_err() {
        return Err(hr);
    }
    Ok(unsafe { IClassFactory::from_raw(p) })
}

fn com() {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// One test: the live-object count is process-wide, so these steps must not run in parallel with other COM tests.
#[test]
fn class_factory_and_display_attribute() {
    com();
    assert_eq!(
        factory(&GUID::from_u128(1)).err(),
        Some(CLASS_E_CLASSNOTAVAILABLE)
    );
    {
        let f = factory(&CLSID_TIP).unwrap();
        let tip: ITfTextInputProcessorEx = unsafe { f.CreateInstance(None) }.unwrap();
        let _: ITfTextInputProcessor = tip.cast().unwrap();
        let _: ITfKeyEventSink = tip.cast().unwrap();
        let _: ITfCompositionSink = tip.cast().unwrap();
        let _: ITfCompartmentEventSink = tip.cast().unwrap();
        let dap: ITfDisplayAttributeProvider = tip.cast().unwrap();
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        unsafe {
            let e = dap.EnumDisplayAttributeInfo().unwrap();
            let mut out = [None];
            let mut n = 0;
            e.Next(&mut out, &mut n).unwrap();
            assert_eq!(n, 1);
            let info = out[0].take().unwrap();
            assert_eq!(info.GetGUID().unwrap(), GUID_DISPLAY_ATTR);
            let mut da = TF_DISPLAYATTRIBUTE::default();
            info.GetAttributeInfo(&mut da).unwrap();
            assert_eq!(da.lsStyle, TF_LS_NONE);
            assert_eq!(da.crText.r#type, TF_CT_NONE);
            assert_eq!(da.crBk.r#type, TF_CT_NONE);
            assert_eq!(da.bAttr, TF_ATTR_INPUT);
            e.Next(&mut out, &mut n).unwrap(); // S_FALSE, a success code
            assert_eq!(n, 0);
            e.Reset().unwrap();
            e.Clone().unwrap().Next(&mut out, &mut n).unwrap();
            assert_eq!(n, 1);
            out[0] = None;
            assert_eq!(
                dap.GetDisplayAttributeInfo(&GUID_DISPLAY_ATTR)
                    .unwrap()
                    .GetGUID()
                    .unwrap(),
                GUID_DISPLAY_ATTR
            );
            assert_eq!(
                dap.GetDisplayAttributeInfo(&CLSID_TIP)
                    .err()
                    .map(|e| e.code()),
                Some(E_INVALIDARG)
            );
            // Aggregation is refused.
            let outer: IUnknown = f.cast().unwrap();
            assert_eq!(
                f.CreateInstance::<_, IUnknown>(&outer)
                    .err()
                    .map(|e| e.code()),
                Some(CLASS_E_NOAGGREGATION)
            );
        }
    }
    assert_eq!(DllCanUnloadNow(), S_OK, "live objects: {}", live_objects());
}

/// The string tables follow the thread's UI language (the same loader picks the MUI display name in Settings).
#[test]
fn display_name_follows_the_ui_language() {
    let cases = [
        ("uz-Latn-UZ", "Oʻzbek 2026 (ş ç ö ğ)"),
        ("uz-Cyrl-UZ", "Ўзбек 2026 (ş ç ö ğ)"),
        ("ru-RU", "Узбекская 2026 (ş ç ö ğ)"),
        ("en-US", "Uzbek 2026 (ş ç ö ğ)"),
    ];
    std::thread::spawn(move || {
        for (lang, want) in cases {
            let list: Vec<u16> = lang.encode_utf16().chain([0, 0]).collect();
            unsafe {
                SetThreadPreferredUILanguages(MUI_LANGUAGE_NAME, PCWSTR(list.as_ptr()), None)
                    .unwrap()
            };
            assert_eq!(load_string(res::IDS_NAME), want, "{lang}");
        }
        // Another language falls back along the user's and the system's UI languages (measured: de-DE on a Russian
        // Windows gives the Russian name); it is one of the four, never empty.
        let de: Vec<u16> = "de-DE".encode_utf16().chain([0, 0]).collect();
        unsafe {
            SetThreadPreferredUILanguages(MUI_LANGUAGE_NAME, PCWSTR(de.as_ptr()), None).unwrap()
        };
        assert!(cases.iter().any(|(_, n)| load_string(res::IDS_NAME) == *n));
        let en: Vec<u16> = "en-US".encode_utf16().chain([0, 0]).collect();
        unsafe {
            SetThreadPreferredUILanguages(MUI_LANGUAGE_NAME, PCWSTR(en.as_ptr()), None).unwrap()
        };
        assert_eq!(load_string(res::IDS_MODE_ON), "2026 letters: on");
        assert!(!load_string(res::IDS_TOGGLE).is_empty());
    })
    .join()
    .unwrap();
}

#[test]
fn tip_string_and_settings_key() {
    assert_eq!(
        tip_string(),
        "0443:{E384AABE-7F88-4BB2-BFB5-3AA607601E3E}{BB9F7540-B6BF-43AC-AC2C-0EE5FEC092A6}"
    );
    assert_eq!(settings_key(), format!(r"Software\{BRAND}\Keyboard"));
    assert!(!BRAND.is_empty());
}

/// Activation through TSF itself, still without any registry change: the class object is registered in this process
/// only (CoRegisterClassObject) and the profile with TF_RP_LOCALPROCESS under en-US (a language every test machine
/// has; uz-Latn may not be in the user's list). Checks what Activate sets up and that Deactivate takes it down.
/// Keys are not sent: TSF routes keystrokes only to the foreground input thread (spike finding, PLAN 11.1).
#[test]
#[ignore = "changes this process's input profile through TSF; run alone with --ignored"]
fn activates_inside_tsf() {
    com();
    unsafe {
        let f = factory(&CLSID_TIP).unwrap();
        let cookie =
            CoRegisterClassObject(&CLSID_TIP, &f, CLSCTX_INPROC_SERVER, REGCLS_MULTIPLEUSE)
                .unwrap();
        let tm: ITfThreadMgr =
            CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER).unwrap();
        let _tid = tm.Activate().unwrap();
        let pm: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER).unwrap();
        let desc: Vec<u16> = "test".encode_utf16().collect();
        pm.RegisterProfile(
            &CLSID_TIP,
            0x0409,
            &PROFILE,
            &desc,
            &[],
            0,
            HKL::default(),
            0,
            true,
            TF_RP_LOCALPROCESS,
        )
        .unwrap();
        pm.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            0x0409,
            &CLSID_TIP,
            &PROFILE,
            HKL::default(),
            TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
        )
        .unwrap();
        let km: ITfKeystrokeMgr = tm.cast().unwrap();
        assert_eq!(km.GetForeground().unwrap(), CLSID_TIP, "key event sink");
        assert!(
            km.IsPreservedKey(&GUID_PRESERVED_TOGGLE, &TOGGLE_KEY)
                .unwrap()
                .as_bool(),
            "Ctrl+Shift+` preserved"
        );
        let lb: ITfLangBarItemMgr = tm.cast().unwrap();
        let item = lb.GetItem(&GUID_LBI_INPUTMODE).unwrap();
        let mut info = TF_LANGBARITEMINFO::default();
        item.GetInfo(&mut info).unwrap();
        assert_eq!(info.clsidService, CLSID_TIP);
        let button: ITfLangBarItemButton = item.cast().unwrap();
        let on = button.GetText().unwrap().to_string();
        let icon = button.GetIcon().unwrap();
        assert!(!icon.is_invalid());
        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
        // The toggle goes through the compartment and comes back through OnChange.
        let open = tm
            .cast::<ITfCompartmentMgr>()
            .unwrap()
            .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)
            .unwrap();
        assert_eq!(
            i32::try_from(&open.GetValue().unwrap()).unwrap(),
            1,
            "on by default"
        );
        button
            .OnClick(TF_LBI_CLK_LEFT, POINT::default(), &RECT::default())
            .unwrap();
        assert_eq!(i32::try_from(&open.GetValue().unwrap()).unwrap(), 0);
        assert_ne!(
            button.GetText().unwrap().to_string(),
            on,
            "the button follows the mode"
        );
        drop((button, item));
        // Switching back to the keyboard layout leaves the TIP active (measured); DeactivateProfile ends it.
        pm.DeactivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            0x0409,
            &CLSID_TIP,
            &PROFILE,
            HKL::default(),
            TF_IPPMF_FORPROCESS,
        )
        .unwrap();
        assert_ne!(
            km.GetForeground().ok(),
            Some(CLSID_TIP),
            "key sink unadvised"
        );
        assert!(
            lb.GetItem(&GUID_LBI_INPUTMODE).is_err(),
            "button removed on Deactivate"
        );
        assert!(
            !km.IsPreservedKey(&GUID_PRESERVED_TOGGLE, &TOGGLE_KEY)
                .unwrap()
                .as_bool(),
            "key unpreserved"
        );
        tm.Deactivate().unwrap();
        CoRevokeClassObject(cookie).unwrap();
    }
}
