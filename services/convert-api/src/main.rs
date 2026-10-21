//! `convert-api [--config FILE]`: the HTTP API (PLAN 5). With no config file it runs in local mode: `auth = "none"` on 127.0.0.1.
use convert_api::app::App;
use convert_api::config::Settings;
use convert_api::{routes, tls};
use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage: convert-api [--config FILE]\nlocal mode (default): auth = \"none\", 127.0.0.1:8787. Server mode: auth = \"jwt\" with tls_cert, tls_key, secret_file and key_file. See docs/phase3/api.md.";

fn refuse(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("convert-api: {m}");
    ExitCode::from(2)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settings = match args.as_slice() {
        [] => Ok(Settings::default()),
        [f, path] if f == "--config" => Settings::load(path.as_ref()),
        _ => return refuse(USAGE),
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
    let (listen, redirect) = (resolved.listen, resolved.s.redirect_listen.clone().zip(resolved.s.public_url.clone()));
    let app = match App::new(resolved, convert::exec::Config::from_env(), tls_cfg.is_some()) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    let router = routes::router(app.clone());
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
    let served = match tls_cfg {
        Some(cfg) => {
            if let Some((addr, public)) = redirect {
                let addr: SocketAddr = match addr.parse() {
                    Ok(a) => a,
                    Err(_) => return refuse("redirect_listen is not an ip:port"),
                };
                let r = tls::redirect_router(&public);
                match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => {
                        tokio::spawn(async move { axum::serve(l, r).await });
                    }
                    Err(e) => return refuse(format!("cannot listen on {addr}: {e}")),
                }
            }
            let l = match tokio::net::TcpListener::bind(listen).await {
                Ok(l) => l,
                Err(e) => return refuse(format!("cannot listen on {listen}: {e}")),
            };
            println!("convert-api listening on https://{}", l.local_addr().unwrap_or(listen));
            tls::serve(l, cfg, router, stop).await;
            Ok(())
        }
        None => {
            let l = match tokio::net::TcpListener::bind(listen).await {
                Ok(l) => l,
                Err(e) => return refuse(format!("cannot listen on {listen}: {e}")),
            };
            println!("convert-api listening on http://{}", l.local_addr().unwrap_or(listen));
            axum::serve(l, router.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(stop).await
        }
    };
    app.shutdown();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => refuse(e),
    }
}
