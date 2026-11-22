//! Build script: the product name from brand/brand.toml, and the DLL's Win32 resources (icons and the localized string
//! tables from i18n/keyboard.tsv) written as a .res file that MSVC link.exe takes directly. No rc.exe, no extra crates.

use std::path::PathBuf;

/// Resource ids; src/res.rs carries the same numbers.
const ICONS: [(u16, &str); 3] = [(1, "brand.ico"), (2, "mode-on.ico"), (3, "mode-off.ico")];
const STRINGS: [(u16, &str); 4] = [
    (100, "kbd.name"),
    (101, "kbd.mode.on"),
    (102, "kbd.mode.off"),
    (103, "kbd.toggle"),
];
/// (LANGID, catalog column) per string table. No LANG_NEUTRAL table: LoadString asks for neutral first and would always
/// get it (measured); without one the loader follows the thread, user and system UI languages (tests/inproc.rs).
const LANGS: [(u16, &str); 4] = [
    (0x0443, "uz_latn"),
    (0x0843, "uz_cyrl"),
    (0x0419, "ru"),
    (0x0409, "source_en"),
];

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = dir.join("../..");
    let brand_toml = root.join("brand/brand.toml");
    let tsv = root.join("i18n/keyboard.tsv");
    for p in [&brand_toml, &tsv] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    for (_, f) in ICONS {
        println!(
            "cargo:rerun-if-changed={}",
            dir.join("res").join(f).display()
        );
    }

    let toml = std::fs::read_to_string(&brand_toml).expect("brand/brand.toml");
    let brand = product_key(&toml, "name");
    println!("cargo:rustc-env=KBD_BRAND={brand}");

    let table = strings(&std::fs::read_to_string(&tsv).expect("i18n/keyboard.tsv"));
    let name_uz = &table.iter().find(|(l, _)| *l == 0x0443).unwrap().1[0];
    println!("cargo:rustc-env=KBD_NAME_UZ_LATN={name_uz}");

    let mut res = Vec::new();
    entry(&mut res, 0, 0, 0, &[]); // the empty first entry every .res file starts with
    for (id, file) in ICONS {
        icon(
            &mut res,
            id,
            &std::fs::read(dir.join("res").join(file)).expect("icon"),
        );
    }
    let version = product_key(&toml, "version");
    entry(
        &mut res,
        16, // RT_VERSION
        1,
        0x0409,
        &version_info(
            &version,
            &[
                ("CompanyName", product_key(&toml, "vendor")),
                ("FileDescription", format!("{brand} keyboard (Uzbek 2026)")),
                ("FileVersion", version.clone()),
                ("InternalName", "kbd".into()),
                ("LegalCopyright", product_key(&toml, "copyright")),
                ("OriginalFilename", "kbd.dll".into()),
                ("ProductName", format!("{brand} keyboard")),
                ("ProductVersion", version.clone()),
            ],
        ),
    );
    for (lang, values) in &table {
        let mut block = Vec::new(); // ids 96..111 = block 7; ours are 100..103
        for slot in 96..112u16 {
            let s = STRINGS
                .iter()
                .position(|(id, _)| *id == slot)
                .map(|i| values[i].as_str())
                .unwrap_or("");
            let w: Vec<u16> = s.encode_utf16().collect();
            block.extend((w.len() as u16).to_le_bytes());
            w.iter().for_each(|u| block.extend(u.to_le_bytes()));
        }
        entry(&mut res, 6, 7, *lang, &block); // RT_STRING
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("kbd.res");
    std::fs::write(&out, &res).unwrap();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg={}", out.display());
    }
}

/// `[product] <key> = "..."`. shortcut: a line scan, not a TOML parser; brand.toml keeps one key per line.
fn product_key(toml: &str, key: &str) -> String {
    let mut in_product = false;
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_product = line == "[product]";
        } else if in_product
            && let Some(v) = line.strip_prefix(key)
            && let Some(v) = v.trim_start().strip_prefix('=')
        {
            return v.trim().trim_matches('"').to_string();
        }
    }
    panic!("brand.toml: no [product] {key}");
}

/// A VS_VERSIONINFO structure: fixed info from `version` (X.Y.Z) plus one string table (en-US, Unicode).
fn version_info(version: &str, strings: &[(&str, String)]) -> Vec<u8> {
    let v: Vec<u32> = version
        .split('.')
        .map(|p| p.parse().expect("version X.Y.Z"))
        .collect();
    assert_eq!(v.len(), 3, "brand.toml version must be X.Y.Z");
    let (ms, ls) = ((v[0] << 16) | v[1], v[2] << 16);
    let mut fixed = Vec::new();
    for d in [
        0xFEEF04BDu32,
        0x10000,
        ms,
        ls,
        ms,
        ls,
        0x3f,
        0,
        0x40004,
        2,
        0,
        0,
        0,
    ] {
        fixed.extend(d.to_le_bytes()); // signature, struct version, file/product version, mask, flags, NT32, DLL
    }
    let mut table = Vec::new();
    for (k, val) in strings {
        let mut w: Vec<u8> = val
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        let n = (w.len() / 2) as u16;
        table.extend(ver_node(k, 1, n, &std::mem::take(&mut w), &[]));
        pad4(&mut table);
    }
    let mut kids = ver_node(
        "StringFileInfo",
        1,
        0,
        &[],
        &ver_node("040904B0", 1, 0, &[], &table),
    );
    pad4(&mut kids);
    let translation = [0x09, 0x04, 0xB0, 0x04]; // 0x0409, 1200
    kids.extend(ver_node(
        "VarFileInfo",
        1,
        0,
        &[],
        &ver_node("Translation", 0, 4, &translation, &[]),
    ));
    ver_node("VS_VERSION_INFO", 0, 52, &fixed, &kids)
}

/// One version-resource node: length, value length, type, key, padding, value, padding, children.
fn ver_node(key: &str, ty: u16, vlen: u16, value: &[u8], kids: &[u8]) -> Vec<u8> {
    let mut n = vec![0, 0];
    n.extend(vlen.to_le_bytes());
    n.extend(ty.to_le_bytes());
    n.extend(key.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    pad4(&mut n);
    n.extend(value);
    pad4(&mut n);
    n.extend(kids);
    let len = (n.len() as u16).to_le_bytes();
    n[..2].copy_from_slice(&len);
    n
}

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

/// Per string-table language, the values of STRINGS in order. An Uzbek or Russian value is used only when its status is
/// trusted (`reviewed:*`, as the i18n release export); otherwise the English text stands in.
fn strings(tsv: &str) -> Vec<(u16, Vec<String>)> {
    let mut lines = tsv.lines();
    let head: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| {
        head.iter()
            .position(|h| *h == n)
            .unwrap_or_else(|| panic!("keyboard.tsv: no column {n}"))
    };
    let rows: Vec<Vec<&str>> = lines
        .filter(|l| !l.is_empty())
        .map(|l| l.split('\t').collect())
        .collect();
    LANGS
        .iter()
        .map(|(lang, c)| {
            let values = STRINGS
                .iter()
                .map(|(_, ctx)| {
                    let r = rows
                        .iter()
                        .find(|r| r[col("context")] == *ctx)
                        .unwrap_or_else(|| panic!("keyboard.tsv: no {ctx}"));
                    let v = r[col(c)];
                    let trusted = *c == "source_en"
                        || r[col(&format!("{c}_status"))].starts_with("reviewed:");
                    if trusted && !v.is_empty() {
                        v
                    } else {
                        r[col("source_en")]
                    }
                    .to_string()
                })
                .collect();
            (*lang, values)
        })
        .collect()
}

/// An ICO file as RT_ICON entries (ids 16*id + n) plus one RT_GROUP_ICON `id`.
fn icon(res: &mut Vec<u8>, id: u16, ico: &[u8]) {
    let u16_at = |o: usize| u16::from_le_bytes([ico[o], ico[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([ico[o], ico[o + 1], ico[o + 2], ico[o + 3]]);
    let n = u16_at(4) as usize;
    let mut group = vec![0, 0, 1, 0];
    group.extend((n as u16).to_le_bytes());
    for i in 0..n {
        let e = 6 + 16 * i;
        let (size, off) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        let img_id = id * 16 + i as u16;
        entry(res, 3, img_id, 0, &ico[off..off + size]); // RT_ICON
        group.extend(&ico[e..e + 12]); // width, height, colours, reserved, planes, bit count, bytes
        group.extend(img_id.to_le_bytes());
    }
    entry(res, 14, id, 0, &group); // RT_GROUP_ICON
}

/// One .res entry with an ordinal type and name (32-byte header), padded to 4 bytes.
fn entry(res: &mut Vec<u8>, ty: u16, name: u16, lang: u16, data: &[u8]) {
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32u32.to_le_bytes());
    res.extend([0xff, 0xff]);
    res.extend(ty.to_le_bytes());
    res.extend([0xff, 0xff]);
    res.extend(name.to_le_bytes());
    res.extend(0u32.to_le_bytes()); // DataVersion
    res.extend(if ty == 0 { 0u16 } else { 0x1030u16 }.to_le_bytes()); // MOVEABLE | PURE | DISCARDABLE
    res.extend(lang.to_le_bytes());
    res.extend(0u32.to_le_bytes()); // Version
    res.extend(0u32.to_le_bytes()); // Characteristics
    res.extend(data);
    while !res.len().is_multiple_of(4) {
        res.push(0);
    }
}
