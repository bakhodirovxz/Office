//! Brand config (`brand/brand.toml`): parsing, validation, overlay rendering and the placeholder icon generator.
//!
//! The working name lives only in `brand/brand.toml`. Nothing in this crate hard-codes it.

mod icons;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

pub use icons::{Pictogram, ico_bytes, render_icon};

/// Locale codes in the order the overlay emits them (`brand.toml` spelling).
pub const LOCALES: [&str; 4] = ["en", "ru", "uz-Latn", "uz-Cyrl"];

#[derive(Debug, Clone)]
pub struct App {
    pub key: String,
    pub upstream: String,
    pub upstream_names: Vec<String>,
    pub bin: String,
    pub cli: String,
    pub app_id: String,
    pub aumid: String,
    pub data_dir: String,
    pub storage_key: String,
    pub pictogram: Pictogram,
    pub names: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Brand {
    pub id: String,
    pub name: String,
    pub version: String,
    pub vendor: String,
    pub copyright: String,
    pub attribution: BTreeMap<String, String>,
    pub apps: Vec<App>,
    pub accent_light: [u8; 3],
    pub accent_dark: [u8; 3],
    pub icon_fg: [u8; 3],
    /// community, share, zotero, update_check, upstream_links, contributors_tab
    pub chrome: [bool; 6],
    pub links: [String; 3],
    pub fallback_sans: String,
    pub fallback_mono: String,
}

fn table<'a>(t: &'a Table, key: &str) -> Result<&'a Table, String> {
    t.get(key).and_then(Item::as_table).ok_or_else(|| format!("missing table [{key}]"))
}

fn only_keys(t: &Table, ctx: &str, allowed: &[&str]) -> Result<(), String> {
    for (k, _) in t.iter() {
        if !allowed.contains(&k) {
            return Err(format!("{ctx}: unknown key '{k}'"));
        }
    }
    Ok(())
}

fn string(t: &Table, ctx: &str, key: &str) -> Result<String, String> {
    t.get(key).and_then(Item::as_str).map(str::to_string).ok_or_else(|| format!("{ctx}: missing string '{key}'"))
}

fn boolean(t: &Table, ctx: &str, key: &str) -> Result<bool, String> {
    t.get(key).and_then(Item::as_bool).ok_or_else(|| format!("{ctx}: missing bool '{key}'"))
}

fn hex(s: &str, ctx: &str) -> Result<[u8; 3], String> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6).ok_or_else(|| format!("{ctx}: '{s}' is not #RRGGBB"))?;
    let v = u32::from_str_radix(h, 16).map_err(|_| format!("{ctx}: '{s}' is not #RRGGBB"))?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn lum(c: [u8; 3]) -> f64 {
    let f = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

/// WCAG contrast ratio.
pub fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn locale_map(t: &Table, ctx: &str) -> Result<BTreeMap<String, String>, String> {
    let mut m = BTreeMap::new();
    for (k, v) in t.iter() {
        if !LOCALES.contains(&k) {
            return Err(format!("{ctx}: unknown locale '{k}'"));
        }
        m.insert(k.to_string(), v.as_str().ok_or_else(|| format!("{ctx}.{k}: not a string"))?.to_string());
    }
    for l in LOCALES {
        if !m.contains_key(l) {
            return Err(format!("{ctx}: missing locale '{l}'"));
        }
    }
    Ok(m)
}

impl Brand {
    pub fn load(path: &Path) -> Result<Brand, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Brand::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Brand, String> {
        let doc: DocumentMut = text.parse().map_err(|e| format!("brand.toml: {e}"))?;
        let root = doc.as_table();
        only_keys(root, "brand.toml", &["schema", "product", "attribution", "apps", "colors", "chrome", "links", "fonts"])?;
        if root.get("schema").and_then(Item::as_integer) != Some(1) {
            return Err("brand.toml: schema must be 1".into());
        }
        let p = table(root, "product")?;
        only_keys(p, "[product]", &["id", "name", "version", "vendor", "copyright"])?;
        let c = table(root, "colors")?;
        only_keys(c, "[colors]", &["accent_light", "accent_dark", "icon_fg"])?;
        let ch = table(root, "chrome")?;
        let keys = ["community", "share", "zotero", "update_check", "upstream_links", "contributors_tab"];
        only_keys(ch, "[chrome]", &keys)?;
        let mut chrome = [false; 6];
        for (i, k) in keys.iter().enumerate() {
            chrome[i] = boolean(ch, "[chrome]", k)?;
        }
        let l = table(root, "links")?;
        only_keys(l, "[links]", &["website", "help", "source"])?;
        let f = table(root, "fonts")?;
        only_keys(f, "[fonts]", &["fallback_sans", "fallback_mono"])?;
        let mut apps = Vec::new();
        for (key, item) in table(root, "apps")?.iter() {
            let t = item.as_table().ok_or_else(|| format!("[apps.{key}] is not a table"))?;
            let ctx = format!("[apps.{key}]");
            only_keys(
                t,
                &ctx,
                &["upstream", "upstream_names", "bin", "cli", "app_id", "aumid", "data_dir", "storage_key", "pictogram", "names"],
            )?;
            let names_item = t.get("names").ok_or_else(|| format!("{ctx}: missing names"))?;
            let nt = names_item.as_table_like().ok_or_else(|| format!("{ctx}.names: not a table"))?;
            let mut names = BTreeMap::new();
            for (k, v) in nt.iter() {
                names.insert(k.to_string(), v.as_str().ok_or_else(|| format!("{ctx}.names.{k}: not a string"))?.to_string());
            }
            for loc in LOCALES {
                if !names.contains_key(loc) {
                    return Err(format!("{ctx}.names: missing locale '{loc}'"));
                }
            }
            let un = t
                .get("upstream_names")
                .and_then(Item::as_array)
                .ok_or_else(|| format!("{ctx}: missing upstream_names"))?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            apps.push(App {
                key: key.to_string(),
                upstream: string(t, &ctx, "upstream")?,
                upstream_names: un,
                bin: string(t, &ctx, "bin")?,
                cli: string(t, &ctx, "cli")?,
                app_id: string(t, &ctx, "app_id")?,
                aumid: string(t, &ctx, "aumid")?,
                data_dir: string(t, &ctx, "data_dir")?,
                storage_key: t.get("storage_key").and_then(Item::as_str).unwrap_or_default().to_string(),
                pictogram: Pictogram::parse(&string(t, &ctx, "pictogram")?)?,
                names,
            });
        }
        let b = Brand {
            id: string(p, "[product]", "id")?,
            name: string(p, "[product]", "name")?,
            version: string(p, "[product]", "version")?,
            vendor: string(p, "[product]", "vendor")?,
            copyright: string(p, "[product]", "copyright")?,
            attribution: locale_map(table(root, "attribution")?, "[attribution]")?,
            apps,
            accent_light: hex(&string(c, "[colors]", "accent_light")?, "[colors].accent_light")?,
            accent_dark: hex(&string(c, "[colors]", "accent_dark")?, "[colors].accent_dark")?,
            icon_fg: hex(&string(c, "[colors]", "icon_fg")?, "[colors].icon_fg")?,
            chrome,
            links: [string(l, "[links]", "website")?, string(l, "[links]", "help")?, string(l, "[links]", "source")?],
            fallback_sans: string(f, "[fonts]", "fallback_sans")?,
            fallback_mono: string(f, "[fonts]", "fallback_mono")?,
        };
        b.validate()?;
        Ok(b)
    }

    /// Validation rules of brand.md section 2 (those that apply to the Phase 1 schema).
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = self.id.len() >= 2
            && self.id.len() <= 32
            && self.id.starts_with(|c: char| c.is_ascii_lowercase())
            && self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(format!("product.id '{}' must match [a-z][a-z0-9-]{{1,31}}", self.id));
        }
        let parts: Vec<&str> = self.version.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
            return Err(format!("product.version '{}' must be MAJOR.MINOR.PATCH", self.version));
        }
        let mut forbidden: Vec<String> = vec!["ArtCraft".into(), "storytold".into()];
        for a in &self.apps {
            forbidden.extend(a.upstream_names.iter().cloned());
        }
        let mut shown = vec![self.name.clone()];
        for a in &self.apps {
            shown.extend(a.names.values().cloned());
        }
        for s in &shown {
            if s.is_empty() || s.chars().count() > 40 || s.chars().any(char::is_control) {
                return Err(format!("name '{s}' must be 1..40 chars without control characters"));
            }
            if let Some(f) = forbidden.iter().find(|f| s.contains(f.as_str())) {
                return Err(format!("name '{s}' contains the upstream token '{f}'"));
            }
        }
        for (loc, text) in &self.attribution {
            if text.contains("http") || text.contains("www.") {
                return Err(format!("attribution.{loc} must be plain text without a URL"));
            }
            for a in &self.apps {
                for t in &a.upstream_names {
                    if !text.contains(t.as_str()) {
                        return Err(format!("attribution.{loc} does not mention {t}"));
                    }
                }
            }
        }
        let r = contrast(self.accent_light, self.icon_fg);
        if r < 4.5 {
            return Err(format!("colors.accent_light: contrast {r:.2} against icon_fg < 4.5"));
        }
        let mut seen = BTreeSet::new();
        for a in &self.apps {
            for v in [&a.bin, &a.app_id, &a.aumid, &a.data_dir] {
                if !seen.insert(v.clone()) {
                    return Err(format!("apps.{}: '{v}' is not unique across apps", a.key));
                }
            }
        }
        Ok(())
    }

    pub fn app(&self, key: &str) -> Result<&App, String> {
        self.apps.iter().find(|a| a.key == key).ok_or_else(|| format!("unknown app '{key}' (docs|sheets|slides|pdf)"))
    }

    /// `overlay.env` content for one app (brand.md section 3.1, Phase 1 key set).
    pub fn overlay_env(&self, app: &App) -> String {
        let mut o = String::new();
        let mut kv = |k: &str, v: &str| {
            assert!(!v.contains('\n'), "overlay values are single-line");
            o.push_str(k);
            o.push('=');
            o.push_str(v);
            o.push('\n');
        };
        let h = |c: [u8; 3]| format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        kv("OVERLAY_SCHEMA", "1");
        kv("PRODUCT_ID", &self.id);
        kv("PRODUCT_NAME", &self.name);
        kv("APP_NAME", &app.names["en"]);
        kv("APP_ID", &app.app_id);
        for l in LOCALES {
            kv(&format!("APP_NAME_{}", l.to_ascii_lowercase()), &app.names[l]);
        }
        kv("UPSTREAM_NAMES", &app.upstream_names.join(","));
        kv("AUMID", &app.aumid);
        kv("DATA_DIR", &app.data_dir);
        kv("STORAGE_KEY", &app.storage_key);
        kv("VERSION", &self.version);
        kv("VENDOR", &self.vendor);
        kv("COPYRIGHT", &self.copyright);
        for l in LOCALES {
            kv(&format!("ATTRIBUTION_{}", l.to_ascii_lowercase()), &self.attribution[l]);
        }
        kv("ACCENT_LIGHT", &h(self.accent_light));
        kv("ACCENT_DARK", &h(self.accent_dark));
        let names = ["COMMUNITY", "SHARE", "ZOTERO", "UPDATE_CHECK", "UPSTREAM_LINKS", "CONTRIBUTORS_TAB"];
        for (n, v) in names.iter().zip(self.chrome) {
            kv(n, if v { "1" } else { "0" });
        }
        kv("LINK_WEBSITE", &self.links[0]);
        kv("LINK_HELP", &self.links[1]);
        kv("LINK_SOURCE", &self.links[2]);
        kv("ICON_PNG", "icon-256.png");
        kv("LOGO_RGBA", "logo-128.rgba");
        kv("FONTS", "fallback-sans.ttf,fallback-mono.ttf");
        o
    }

    /// Write `overlay.env`, the icons and the fallback fonts for one app into `out`.
    pub fn write_overlay(&self, app_key: &str, assets_fonts: &Path, out: &Path) -> Result<String, String> {
        let app = self.app(app_key)?;
        fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
        let w = |name: &str, bytes: &[u8]| fs::write(out.join(name), bytes).map_err(|e| format!("{name}: {e}"));
        let (acc, fg) = (self.accent_light, self.icon_fg);
        w("overlay.env", self.overlay_env(app).as_bytes())?;
        for size in icons::PNG_SIZES {
            w(&format!("icon-{size}.png"), &icons::png_bytes(&render_icon(app.pictogram, size, acc, fg))?)?;
        }
        w("icon.ico", &ico_bytes(app.pictogram, &icons::ICO_SIZES, acc, fg)?)?;
        w("favicon.ico", &ico_bytes(app.pictogram, &[16, 32, 48], acc, fg)?)?;
        w("icon-document.ico", &ico_bytes(Pictogram::Page, &icons::ICO_SIZES, acc, fg)?)?;
        w("logo-128.rgba", &icons::rgba_bytes(&render_icon(app.pictogram, 128, acc, fg)))?;
        w("icon-192.png", &icons::png_bytes(&render_icon(app.pictogram, 192, acc, fg))?)?;
        w("apple-touch-icon.png", &icons::png_bytes(&render_icon(app.pictogram, 180, acc, fg))?)?;
        w("icon-maskable-512.png", &icons::png_bytes(&icons::render_maskable(app.pictogram, 512, acc, fg))?)?;
        for (dst, src) in [("fallback-sans.ttf", &self.fallback_sans), ("fallback-mono.ttf", &self.fallback_mono)] {
            let bytes = fs::read(assets_fonts.join(src)).map_err(|e| format!("{src}: {e}"))?;
            w(dst, &bytes)?;
        }
        Ok(format!("overlay {app_key}: written to {}", out.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_brand() -> Brand {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brand/brand.toml");
        Brand::load(&p).expect("brand.toml is valid")
    }

    #[test]
    fn brand_toml_is_valid_and_has_four_apps() {
        let b = repo_brand();
        assert_eq!(b.apps.len(), 4);
        assert_eq!(b.version, "0.1.0");
    }

    #[test]
    fn overlay_env_has_required_keys() {
        let b = repo_brand();
        let env = b.overlay_env(b.app("pdf").unwrap());
        for k in ["PRODUCT_NAME=", "APP_NAME=", "VERSION=0.1.0", "COMMUNITY=0", "UPDATE_CHECK=0", "ATTRIBUTION_uz-latn=", "UPSTREAM_NAMES=PdfCraft"] {
            assert!(env.contains(k), "missing {k}");
        }
    }

    #[test]
    fn unknown_key_is_rejected() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brand/brand.toml");
        let text = fs::read_to_string(p).unwrap().replace("community = false", "comunity = false");
        assert!(Brand::parse(&text).is_err());
    }

    #[test]
    fn icons_render_at_all_sizes() {
        for p in [Pictogram::Lines, Pictogram::Grid, Pictogram::Slide, Pictogram::Page] {
            for s in icons::PNG_SIZES {
                let pm = render_icon(p, s, [15, 118, 110], [255, 255, 255]);
                assert_eq!(pm.width(), s);
            }
            let ico = ico_bytes(p, &icons::ICO_SIZES, [15, 118, 110], [255, 255, 255]).unwrap();
            assert_eq!(&ico[..4], &[0, 0, 1, 0]);
        }
    }
}
