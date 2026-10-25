//! `suite-serve [--config FILE] [--dist DIR] [--shell DIR] [--brand FILE]`: the web host (PLAN phase 4, 3.1). The API flags are
//! those of `convert-api`; local mode (no config) is `auth = "none"` on 127.0.0.1:8787.
use convert_api::config::Settings;
use docserver::Docserver;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use suite_serve::Layout;

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
    convert_api::runner::run("hoshiya-serve", config.as_deref(), move |app| match ds {
        Some((ds, internal)) => {
            if let Some(l) = internal {
                suite_serve::spawn_internal(&app, &ds, l);
            }
            suite_serve::router_ds(app, &layout, cfg, Some(ds))
        }
        None => suite_serve::router(app, &layout, cfg),
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
