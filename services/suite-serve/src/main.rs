//! `suite-serve [--config FILE] [--dist DIR] [--shell DIR] [--brand FILE]`: the web host (PLAN phase 4, 3.1). The API flags are
//! those of `convert-api`; local mode (no config) is `auth = "none"` on 127.0.0.1:8787.
use convert_api::config::Settings;
use docserver::Docserver;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use suite_serve::{Layout, PluginSource};

const USAGE: &str = "usage: suite-serve [--config FILE] [--dist DIR] [--shell DIR] [--brand FILE]\n  --dist   the trunk dists, <domain>-web/ each (default: dist)\n  --shell  the assembled shell (scripts/shell-dist.sh; default: <dist>/shell)\n  --brand  brand/brand.toml (default: brand/brand.toml)";

#[tokio::main]
async fn main() -> ExitCode {
    convert_api::runner::quiet_panics();
    let (mut config, mut dist, mut shell, mut brand) = (
        None,
        PathBuf::from("dist"),
        None,
        PathBuf::from("brand/brand.toml"),
    );
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let Some(v) = args.next() else {
            return convert_api::runner::refuse(USAGE);
        };
        match a.as_str() {
            "--config" => config = Some(v),
            "--dist" => dist = v.into(),
            "--shell" => shell = Some(PathBuf::from(v)),
            "--brand" => brand = v.into(),
            _ => return convert_api::runner::refuse(USAGE),
        }
    }
    let layout = Layout {
        shell: shell.unwrap_or_else(|| dist.join("shell")),
        dist,
        brand,
        local_key: None,
    };
    let cfg = match layout.shell_config() {
        Ok(c) => c,
        Err(e) => return convert_api::runner::refuse(e),
    };
    // the optional [docserver] section: checked and its internal listener bound before anything starts
    let ds = match config.as_deref().map(setup_docserver).transpose() {
        Ok(d) => d.flatten(),
        Err(e) => return convert_api::runner::refuse(e),
    };
    let plugins = match config.as_deref().map(setup_plugins).transpose() {
        Ok(p) => p.flatten().map(Arc::new),
        Err(e) => return convert_api::runner::refuse(e),
    };
    convert_api::runner::run("hoshiya-serve", config.as_deref(), move |app| match ds {
        Some((ds, internal)) => {
            ds.spawn_retry_loop();
            ds.spawn_sweep_loop();
            if let Some(l) = internal {
                suite_serve::spawn_internal(&app, &ds, l);
            }
            suite_serve::router_plugins(app, &layout, cfg, Some(ds), plugins)
        }
        None => suite_serve::router_plugins(app, &layout, cfg, None, plugins),
    })
    .await
}

type Mounted = (Arc<Docserver>, Option<tokio::net::TcpListener>);

fn setup_docserver(path: &str) -> Result<Option<Mounted>, String> {
    let s = Settings::load(path.as_ref())?;
    let Some(t) = s.docserver.clone() else {
        return Ok(None);
    };
    let ds = Arc::new(Docserver::from_table(t, s.listen.parse().ok())?);
    suite_serve::check_secrets_distinct(&s, &ds)?;
    // bound and handed to the runtime here, so a failure on either step stops the start
    let l = ds
        .bind_internal()?
        .map(tokio::net::TcpListener::from_std)
        .transpose()
        .map_err(|e| format!("docserver: internal listener: {e}"))?;
    Ok(Some((ds, l)))
}

/// The optional `[plugins]` section of the web host (PLAN phase 7, section 8): `dir` (the operator's folder of packages),
/// `enabled = [ids]`, `[plugins.grants] <id> = [origins]` and optionally `[plugins.sha256] <id> = "<hex of plugin.wasm>"` (a package
/// whose wasm differs is left out). The operator is the only one who enables or grants here; a package that changed since its record
/// is refused, one without a record is accepted (the folder is the operator's). The state is static, so it is worked out once here.
fn setup_plugins(path: &str) -> Result<Option<PluginSource>, String> {
    let Some(t) = Settings::load(path.as_ref())?.plugins else {
        return Ok(None);
    };
    let strs = |v: Option<&toml::Value>, key: &str| -> Result<Vec<String>, String> {
        v.map_or(Ok(vec![]), |v| {
            v.as_array()
                .and_then(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect())
                .ok_or(format!("plugins: {key} must be a list of strings"))
        })
    };
    if let Some(k) = t
        .keys()
        .find(|k| !["dir", "enabled", "grants", "sha256"].contains(&k.as_str()))
    {
        return Err(format!("plugins: unknown key {k}"));
    }
    let dir = t
        .get("dir")
        .and_then(|d| d.as_str())
        .ok_or("plugins: dir is required")?;
    let enabled = strs(t.get("enabled"), "enabled")?;
    let mut grants = std::collections::BTreeMap::new();
    for (id, v) in t
        .get("grants")
        .and_then(|g| g.as_table())
        .into_iter()
        .flatten()
    {
        grants.insert(id.clone(), strs(Some(v), &format!("grants.{id}"))?);
    }
    let mut pins = std::collections::BTreeMap::new();
    for (id, v) in t
        .get("sha256")
        .and_then(|g| g.as_table())
        .into_iter()
        .flatten()
    {
        let h = v
            .as_str()
            .filter(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
            .ok_or(format!(
                "plugins: sha256.{id} must be 64 lower-case hex digits"
            ))?;
        pins.insert(id.clone(), h.to_string());
    }
    let state = suite_core::plugins::State { enabled, grants };
    let roots = vec![suite_core::plugins::Root {
        dir: dir.into(),
        machine: true,
    }];
    let pol = suite_core::plugins::Policy::default();
    let mut fixed = suite_core::plugins::enabled(&roots, &state, &pol);
    fixed.retain(|e| {
        let ok = pins
            .get(&e.package.manifest.id)
            .is_none_or(|h| *h == e.package.sha256);
        if !ok {
            eprintln!(
                "plugins: {} left out: its wasm differs from the pinned sha256",
                e.package.manifest.id
            );
        }
        ok
    });
    Ok(Some(PluginSource {
        roots,
        now: Box::new(move || (state.clone(), pol)),
        fixed: Some(fixed),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "id = \"alpha\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"Test\"\n[name]\nen = \"Alpha\"\n[permissions]\ndocument = \"read\"\n";

    fn sha(b: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect()
    }

    fn config(name: &str, extra: &str) -> (std::path::PathBuf, String) {
        let d = std::env::temp_dir().join(format!("serve-plg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = d.join("plugins/alpha/1.0.0");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("plugin.toml"), MANIFEST).unwrap();
        std::fs::write(p.join("plugin.wasm"), b"\0asm-alpha").unwrap();
        let dir = d.join("plugins").to_string_lossy().replace('\\', "/");
        let c = d.join("c.toml");
        std::fs::write(
            &c,
            format!("[plugins]\ndir = \"{dir}\"\nenabled = [\"alpha\"]\n{extra}"),
        )
        .unwrap();
        (d, c.to_string_lossy().into_owned())
    }

    #[test]
    fn the_web_hosts_list_is_worked_out_once() {
        let (d, c) = config("once", "");
        let src = setup_plugins(&c).unwrap().unwrap();
        assert_eq!(src.fixed.as_ref().unwrap().len(), 1);
        std::fs::remove_dir_all(d.join("plugins/alpha")).unwrap();
        assert_eq!(src.fixed.as_ref().unwrap().len(), 1, "no rescan");
        assert!(
            plugins_read(&src).is_err(),
            "but the file is read again on serve"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    fn plugins_read(src: &PluginSource) -> Result<Vec<u8>, String> {
        suite_core::plugins::read_wasm(&src.fixed.as_ref().unwrap()[0].package)
    }

    #[test]
    fn a_pinned_hash_must_match() {
        let (d, c) = config(
            "pin",
            &format!("[plugins.sha256]\nalpha = \"{}\"\n", sha(b"\0asm-alpha")),
        );
        assert_eq!(setup_plugins(&c).unwrap().unwrap().fixed.unwrap().len(), 1);
        let (d2, c2) = config(
            "pin2",
            &format!("[plugins.sha256]\nalpha = \"{}\"\n", sha(b"other")),
        );
        assert!(
            setup_plugins(&c2)
                .unwrap()
                .unwrap()
                .fixed
                .unwrap()
                .is_empty()
        );
        let (d3, c3) = config("pin3", "[plugins.sha256]\nalpha = \"ABC\"\n");
        assert!(setup_plugins(&c3).is_err());
        for d in [d, d2, d3] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
