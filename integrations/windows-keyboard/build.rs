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

    let brand = product_name(&std::fs::read_to_string(&brand_toml).expect("brand/brand.toml"));
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

/// `[product] name = "..."`. shortcut: a line scan, not a TOML parser; brand.toml keeps one key per line.
fn product_name(toml: &str) -> String {
    let mut in_product = false;
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_product = line == "[product]";
        } else if in_product && let Some(v) = line.strip_prefix("name") {
            let v = v.trim_start().strip_prefix('=').expect("name =").trim();
            return v.trim_matches('"').to_string();
        }
    }
    panic!("brand.toml: no [product] name");
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
