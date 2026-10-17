//! Shared by the domain `build.rs` files (`#[path]` include): reads the brand overlay that
//! `cargo xt overlay <app>` writes, so exe icons and VERSIONINFO come from `brand/brand.toml`.
//! With `OFFICE_OVERLAY_DIR` unset the callers keep upstream's own icon and strings.

use std::collections::HashMap;
use std::path::PathBuf;

pub struct Overlay {
    dir: PathBuf,
    env: HashMap<String, String>,
}

/// The overlay named by `OFFICE_OVERLAY_DIR`, or `None` when it is unset or has no `overlay.env`.
pub fn load() -> Option<Overlay> {
    let dir = PathBuf::from(std::env::var_os("OFFICE_OVERLAY_DIR")?);
    let text = std::fs::read_to_string(dir.join("overlay.env")).ok()?;
    let env = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.to_string()))
        .collect();
    println!("cargo:rerun-if-changed={}", dir.join("overlay.env").display());
    Some(Overlay { dir, env })
}

impl Overlay {
    pub fn get(&self, key: &str) -> &str {
        self.env.get(key).map(String::as_str).unwrap_or("")
    }

    /// Absolute path of an overlay file (forward slashes), registered for rebuilds.
    pub fn file(&self, name: &str) -> String {
        let p = self.dir.join(name);
        println!("cargo:rerun-if-changed={}", p.display());
        p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/")
    }

    /// VERSIONINFO strings: one product version for every app (brand.toml `product.version`).
    pub fn version_info(&self, res: &mut winresource::WindowsResource, description: &str, exe: &str) {
        res.set("ProductName", self.get("APP_NAME"))
            .set("FileDescription", &format!("{} {description}", self.get("APP_NAME")))
            .set("CompanyName", self.get("VENDOR"))
            .set("LegalCopyright", self.get("COPYRIGHT"))
            .set("ProductVersion", self.get("VERSION"))
            .set("FileVersion", self.get("VERSION"))
            .set("OriginalFilename", &format!("{exe}.exe"))
            .set("InternalName", exe);
    }
}
