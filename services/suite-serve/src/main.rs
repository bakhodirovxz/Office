//! `suite-serve [--config FILE] [--dist DIR] [--shell DIR] [--brand FILE]`: the web host (PLAN phase 4, 3.1). The API flags are
//! those of `convert-api`; local mode (no config) is `auth = "none"` on 127.0.0.1:8787.
use std::path::PathBuf;
use std::process::ExitCode;
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
    };
    let cfg = match layout.shell_config() {
        Ok(c) => c,
        Err(e) => return convert_api::runner::refuse(e),
    };
    convert_api::runner::run("suite-serve", config.as_deref(), move |app| {
        suite_serve::router(app, &layout, cfg)
    })
    .await
}
