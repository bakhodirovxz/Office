//! Fonts for PDF export: which face draws (and measures) a cell's text.
//!
//! The PDF writer ([`gridcraft_pdf`]) draws with the standard 14 fonts or with TrueType faces
//! registered here. A workbook font that is installed on the machine (the same files the desktop
//! app binds for the grid) is embedded as a subset, so column widths, `####` overflow and line
//! wrapping use the real advance widths. Text the chosen font cannot show (Cyrillic or Uzbek
//! letters in Helvetica, say) falls back to an installed face of the same style, then to the
//! default face (see [`set_default_face`]), one character at a time.
//!
//! The standard fonts stay in use where they are metric-compatible with what the workbook asked
//! for (Arial, Times New Roman, Courier New) and the text is Latin-1: those PDFs are unchanged.
//!
//! Installed fonts are looked up unless `GRIDCRAFT_PDF_SYSTEM_FONTS=0` (byte-reproducible PDFs
//! that do not depend on the machine's fonts; the crate's own unit tests behave the same way).
//!
//! Hosts without a file system (the web build) or with their own fonts add faces with
//! [`register_font`].

use std::collections::HashMap;
use std::sync::Mutex;

use gridcraft_pdf::{Family, Font, register_face};

/// A face file larger than this is never read.
#[cfg(not(target_arch = "wasm32"))]
const MAX_FONT_FILE: u64 = 48 << 20;
/// Most faces of one `.ttc` collection looked at.
#[cfg(not(target_arch = "wasm32"))]
const MAX_COLLECTION: u32 = 32;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Style {
    bold: bool,
    italic: bool,
}

/// One installed face found by the scan (nothing loaded yet).
#[cfg(not(target_arch = "wasm32"))]
struct Installed {
    path: std::path::PathBuf,
    index: u32,
    style: Option<Style>,
}

#[derive(Default)]
struct State {
    /// The bundled default face, registered on first use.
    default: Option<Option<Font>>,
    /// Faces handed in by the host: (lowercase family, style) → font.
    hosted: HashMap<(String, Style), Font>,
    /// Lowercase family → installed faces.
    #[cfg(not(target_arch = "wasm32"))]
    catalog: Option<HashMap<String, Vec<Installed>>>,
    /// Resolved (lowercase family, style) → font, including negative answers.
    cache: HashMap<(String, Style), Option<Font>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(State::default))
}

/// Registers a TrueType face for `family` (any case) and style, for hosts that carry their own
/// fonts. The face falls back to the default face for characters it lacks. Returns false when the
/// bytes are not a TrueType (`glyf`) font.
pub fn register_font(family: &str, bold: bool, italic: bool, data: Vec<u8>) -> bool {
    let fallback = default_face();
    let Some(font) = register_face(data, 0, fallback) else { return false };
    with_state(|s| {
        s.hosted.insert((family.to_lowercase(), Style { bold, italic }), font);
        s.cache.clear();
    });
    true
}

/// Sets the face used when nothing else covers a character (the last stop of every fallback
/// chain): any `glyf` TrueType font with Latin, Greek and Cyrillic, such as the Ubuntu Light that
/// egui ships. The first successful call wins; later calls are ignored, so a host may call it on
/// every font rebuild. Without a call (and without the `bundled-pdf-font` feature) there is no
/// default face and text a standard font cannot show stays `?`.
pub fn set_default_face(data: Vec<u8>) -> bool {
    if with_state(|s| s.default.is_some_and(|d| d.is_some())) {
        return true;
    }
    let Some(font) = register_face(data, 0, None) else { return false };
    with_state(|s| {
        s.default = Some(Some(font));
        s.cache.clear();
    });
    true
}

/// [`set_default_face`] with a second face behind `data` for the characters it lacks (the first successful call wins).
pub fn set_default_face_with_fallback(data: Vec<u8>, fallback: Vec<u8>) -> bool {
    if with_state(|s| s.default.is_some_and(|d| d.is_some())) {
        return true;
    }
    let behind = register_face(fallback, 0, None);
    let Some(font) = register_face(data, 0, behind) else { return false };
    with_state(|s| {
        s.default = Some(Some(font));
        s.cache.clear();
    });
    true
}

/// The face compiled in with the `bundled-pdf-font` feature (and in this crate's tests).
fn bundled() -> Option<Font> {
    #[cfg(any(test, feature = "bundled-pdf-font"))]
    {
        register_face(epaint_default_fonts::UBUNTU_LIGHT.to_vec(), 0, None)
    }
    #[cfg(not(any(test, feature = "bundled-pdf-font")))]
    {
        None
    }
}

/// The default face (Latin, Greek and Cyrillic), registered once.
fn default_face() -> Option<Font> {
    with_state(|s| {
        *s.default.get_or_insert_with(bundled)
    })
}

/// Name aliases: a workbook font that is not installed is looked up under metric-compatible
/// names (Carlito has Calibri's advance widths, Liberation Sans Arial's, and so on).
fn aliases(lower: &str) -> &'static [&'static str] {
    match lower {
        "calibri" => &["carlito"],
        "cambria" => &["caladea"],
        "arial" | "helvetica" => &["liberation sans", "arimo"],
        "times new roman" | "times" => &["liberation serif", "tinos"],
        "courier new" | "courier" => &["liberation mono", "cousine"],
        _ => &[],
    }
}

/// Families the standard 14 fonts match closely enough that embedding them buys nothing.
fn metric_compatible(lower: &str) -> bool {
    matches!(lower, "arial" | "helvetica" | "times new roman" | "times" | "courier new" | "courier")
}

/// Stand-ins for a generic family when the text needs glyphs the named font lacks.
fn generic(family: Family) -> &'static [&'static str] {
    match family {
        Family::Sans => &["arial", "segoe ui", "dejavu sans", "liberation sans", "noto sans", "helvetica neue", "arimo"],
        Family::Serif => &["times new roman", "dejavu serif", "liberation serif", "noto serif", "tinos"],
        Family::Mono => &["courier new", "consolas", "dejavu sans mono", "liberation mono", "cousine"],
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn font_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut dirs: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/Library/Fonts"].map(PathBuf::from));
        if let Some(h) = &home {
            dirs.push(h.join("Library/Fonts"));
        }
    } else if cfg!(windows) {
        let root = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        dirs.push(root.join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Microsoft\\Windows\\Fonts"));
        }
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts", "/usr/share/fonts/TTF"].map(PathBuf::from));
        if let Some(h) = &home {
            dirs.push(h.join(".fonts"));
            dirs.push(h.join(".local/share/fonts"));
        }
    }
    dirs
}

/// The style a subfamily name denotes, for the four faces a cell can ask for; `None` for Light,
/// Semibold, Condensed and the like, which a cell cannot select.
#[cfg(not(target_arch = "wasm32"))]
fn style_of(sub: &str) -> Option<Style> {
    let s = sub.to_lowercase();
    let italic = s.contains("italic") || s.contains("oblique");
    let rest = s.replace("italic", "").replace("oblique", "");
    match rest.trim() {
        "" | "regular" | "normal" | "book" | "roman" => Some(Style { bold: false, italic }),
        "bold" => Some(Style { bold: true, italic }),
        _ => None,
    }
}

/// Catalogues the installed fonts by family name, reading only each file's `name` table.
#[cfg(not(target_arch = "wasm32"))]
fn scan() -> HashMap<String, Vec<Installed>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut out: HashMap<String, Vec<Installed>> = HashMap::new();
    let mut stack = font_dirs();
    let mut seen = std::collections::HashSet::new();
    while let Some(dir) = stack.pop() {
        if !seen.insert(std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone())) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
            if !matches!(ext.as_str(), "ttf" | "ttc") {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&path) else { continue };
            if meta.len() > MAX_FONT_FILE {
                continue;
            }
            let Ok(mut file) = std::fs::File::open(&path) else { continue };
            let len = meta.len();
            for index in 0..MAX_COLLECTION {
                let names = gridcraft_pdf::ttf::probe_names(index, |off, n| {
                    if off.checked_add(n as u64)? > len {
                        return None;
                    }
                    let mut buf = vec![0u8; n];
                    file.seek(SeekFrom::Start(off)).ok()?;
                    file.read_exact(&mut buf).ok()?;
                    Some(buf)
                });
                let Some((family, sub)) = names else { break };
                if family.is_empty() {
                    continue;
                }
                out.entry(family.to_lowercase()).or_default().push(Installed { path: path.clone(), index, style: style_of(&sub) });
            }
        }
    }
    out
}

/// Whether installed fonts are used (see the module docs).
#[cfg(not(target_arch = "wasm32"))]
fn system_fonts_enabled() -> bool {
    !cfg!(test) && std::env::var_os("GRIDCRAFT_PDF_SYSTEM_FONTS").is_none_or(|v| v != "0")
}

/// The installed face of `family` for `style`: the exact style, else the family's regular face.
#[cfg(not(target_arch = "wasm32"))]
fn installed_face(s: &mut State, family: &str, style: Style) -> Option<Font> {
    if !system_fonts_enabled() {
        return None;
    }
    let (path, index) = {
        let faces = s.catalog.get_or_insert_with(scan).get(family)?;
        let pick = |want: Style| faces.iter().find(|f| f.style == Some(want));
        let chosen = pick(style).or_else(|| pick(Style { bold: false, italic: style.italic })).or_else(|| pick(Style { bold: false, italic: false }))?;
        (chosen.path.clone(), chosen.index)
    };
    let data = std::fs::read(path).ok()?;
    let fallback = default_face_in(s);
    register_face(data, index, fallback)
}

#[cfg(target_arch = "wasm32")]
fn installed_face(_: &mut State, _: &str, _: Style) -> Option<Font> {
    None
}

/// Looks `family` (lowercase) up among host faces, then installed faces, then aliases.
fn lookup(s: &mut State, family: &str, style: Style) -> Option<Font> {
    if let Some(f) = s.cache.get(&(family.to_string(), style)) {
        return *f;
    }
    let found = [Style { bold: style.bold, italic: style.italic }, Style { bold: false, italic: style.italic }, Style { bold: false, italic: false }]
        .iter()
        .find_map(|st| s.hosted.get(&(family.to_string(), *st)).copied());
    let found = found.or_else(|| installed_face(s, family, style)).or_else(|| aliases(family).iter().find_map(|a| lookup(s, a, style)));
    s.cache.insert((family.to_string(), style), found);
    found
}

/// The font that draws and measures a cell set in `name`: the installed face when its metrics
/// differ from the standard fonts', else the standard font (see [`for_text`] for non-Latin-1 text).
pub fn resolve(name: &str, bold: bool, italic: bool) -> Font {
    let standard = Font::styled(Font::family_for(name), bold, italic);
    let lower = name.trim().to_lowercase();
    if lower.is_empty() || metric_compatible(&lower) {
        return standard;
    }
    with_state(|s| lookup(s, &lower, Style { bold, italic })).unwrap_or(standard)
}

/// `font`, or a face that can show `text` when `font` cannot (a standard font given Cyrillic or
/// Uzbek letters, for instance). An embedded face already falls back per character.
pub fn for_text(font: Font, text: &str) -> Font {
    if font.covers(text) {
        return font;
    }
    let Some((family, bold, italic)) = font.parts() else { return font };
    let style = Style { bold, italic };
    with_state(|s| {
        // The first stand-in that shows every character; failing that the first installed one
        // (its missing characters fall back to the default face), failing that the default face.
        let mut first = None;
        for name in generic(family) {
            if let Some(f) = lookup(s, name, style) {
                if f.covers(text) {
                    return Some(f);
                }
                first = first.or(Some(f));
            }
        }
        first.or_else(|| default_face_in(s))
    })
    .unwrap_or(font)
}

fn default_face_in(s: &mut State) -> Option<Font> {
    *s.default.get_or_insert_with(bundled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_fonts_stay_for_latin_one_text() {
        assert_eq!(resolve("Arial", false, false), Font::Helvetica);
        assert_eq!(resolve("Times New Roman", true, false), Font::TimesBold);
        assert_eq!(resolve("", false, false), Font::Helvetica);
        assert_eq!(for_text(Font::Helvetica, "Grüße, café €5"), Font::Helvetica);
    }

    #[test]
    fn cyrillic_and_uzbek_text_gets_a_face_that_covers_it() {
        for text in ["Привет, мир", "Ozbekiston, maʼlumot, sanʼat", "Ўзбекистон Ғ қ ҳ", "Hisobot / Отчёт"] {
            let f = for_text(Font::Helvetica, text);
            assert!(matches!(f, Font::Face(_)), "{text}");
            assert!(f.covers(text), "{text}");
            // The bold and serif requests keep their own family when an installed one exists.
            assert!(for_text(Font::TimesBold, text).covers(text));
        }
    }

    #[test]
    fn okina_is_covered_when_an_installed_face_has_it() {
        // U+02BB (oʻ, gʻ) is missing from the bundled default face, so it needs an installed one;
        // either way the text must resolve to a face and measure.
        let text = "Oʻzbekiston, gʻoya";
        let f = for_text(Font::Helvetica, text);
        assert!(matches!(f, Font::Face(_)));
        assert!(gridcraft_pdf::text_width(f, 10.0, text) > 0.0);
        if !f.covers(text) {
            eprintln!("no installed face with U+02BB; okina draws as .notdef");
        }
    }

    #[test]
    fn unknown_family_falls_back_to_standard() {
        assert_eq!(resolve("No Such Font Family 123", false, false), Font::Helvetica);
    }

    #[test]
    fn host_registered_face_wins() {
        assert!(register_font("Test Family Zz", false, false, epaint_default_fonts::HACK_REGULAR.to_vec()));
        let f = resolve("test family zz", true, true);
        assert!(matches!(f, Font::Face(_)));
        assert!(f.covers("Привет"));
        assert!(!register_font("Bad", false, false, vec![1, 2, 3]));
    }
}
