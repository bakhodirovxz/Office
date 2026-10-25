//! The process around the router, shared by `convert-api` and `suite-serve`: settings, TLS, the sweeper and the accept loop.
use crate::app::App;
use crate::config::Settings;
use crate::tls;
use axum::Router;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

/// Prints the refusal and gives the exit code of a bad start.
pub fn refuse(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("server: {m}");
    ExitCode::from(2)
}

/// `convert-api` alone must not start on a config that has a `[docserver]` section: it would run without the routes the operator
/// configured and look healthy. `hoshiya-serve` mounts that section.
pub fn refuse_docserver_section(config: Option<&str>) -> Result<(), String> {
    match config.map(|p| Settings::load(p.as_ref())).transpose()? {
        Some(s) if s.docserver.is_some() => {
            Err("the [docserver] section is served by hoshiya-serve, not by convert-api".into())
        }
        _ => Ok(()),
    }
}

/// A panic message can quote document text (a failed assertion prints its values), so the default hook, which prints the message,
/// is replaced by one that prints the location only (WP7 review m14). `job::guarded` still reports the panic to the client.
pub fn quiet_panics() {
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map_or("unknown".to_string(), |l| {
            format!("{}:{}", l.file(), l.line())
        });
        eprintln!("server: internal panic at {at} (message withheld)");
    }));
}

/// Runs the server until Ctrl-C. `config` is the `--config FILE` (none = local mode); `build` makes the router from the app, so
/// a caller can add routes to the API router (`routes::router_with`).
pub async fn run(
    name: &str,
    config: Option<&str>,
    build: impl FnOnce(Arc<App>) -> Router,
) -> ExitCode {
    let settings = match config {
        None => Ok(Settings::default()),
        Some(path) => Settings::load(path.as_ref()),
    };
    let resolved = match settings.and_then(Settings::resolve) {
        Ok(r) => r,
        Err(e) => return refuse(e),
    };
    let tls_cfg = match (&resolved.s.tls_cert, &resolved.s.tls_key) {
        (Some(c), Some(k)) => match tls::server_config(c, k) {
            Ok(t) => Some(t),
            Err(e) => return refuse(e),
        },
        _ => None,
    };
    let (listen, redirect, limits) = (
        resolved.listen,
        resolved
            .s
            .redirect_listen
            .clone()
            .zip(resolved.s.public_url.clone()),
        tls::ConnLimits::from_settings(&resolved),
    );
    if resolved.upstream_tls {
        eprintln!(
            "{name}: tls = \"upstream\": plain HTTP, answering the trusted proxies only; TLS must end at the proxy"
        );
    }
    let app = match App::new(
        resolved,
        convert::exec::Config::from_env(),
        tls_cfg.is_some(),
    ) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    let router = build(app.clone());
    let sweeper = app.clone();
    let tick = Duration::from_secs((app.r.s.retention_s / 4).clamp(1, 60));
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tick).await;
            sweeper.sweep();
        }
    });
    let stop = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let served: Result<(), std::io::Error> = {
        if let (Some(_), Some((addr, public))) = (&tls_cfg, redirect) {
            let addr: SocketAddr = match addr.parse() {
                Ok(a) => a,
                Err(_) => return refuse("redirect_listen is not an ip:port"),
            };
            match tokio::net::TcpListener::bind(addr).await {
                // the same timeouts and caps as the API listener; it only ever answers with a redirect
                Ok(l) => {
                    tokio::spawn(tls::serve(
                        l,
                        None,
                        tls::redirect_router(&public),
                        limits.clone(),
                        std::future::pending(),
                    ));
                }
                Err(e) => return refuse(format!("cannot listen on {addr}: {e}")),
            }
        }
        let l = match tokio::net::TcpListener::bind(listen).await {
            Ok(l) => l,
            Err(e) => return refuse(format!("cannot listen on {listen}: {e}")),
        };
        let scheme = if tls_cfg.is_some() { "https" } else { "http" };
        println!(
            "{name} listening on {scheme}://{}",
            l.local_addr().unwrap_or(listen)
        );
        tls::serve(l, tls_cfg, router, limits, stop).await;
        Ok(())
    };
    app.shutdown();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => refuse(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_api_refuses_a_docserver_section() {
        let d = std::env::temp_dir().join(format!("runner-ds-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (with, without) = (d.join("with.toml"), d.join("without.toml"));
        std::fs::write(&with, "[docserver]
data_dir = \"x\"
").unwrap();
        std::fs::write(&without, "").unwrap();
        let e = refuse_docserver_section(with.to_str()).unwrap_err();
        assert!(e.contains("[docserver]") && e.contains("hoshiya-serve"), "{e}");
        assert!(refuse_docserver_section(without.to_str()).is_ok());
        assert!(refuse_docserver_section(None).is_ok());
    }
}
