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

/// A panic message can quote document text (a failed assertion prints its values), so the default hook, which prints the message,
/// is replaced by one that prints the location only (WP7 review m14). `job::guarded` still reports the panic to the client.
fn quiet_panics() {
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map_or("unknown".to_string(), |l| format!("{}:{}", l.file(), l.line()));
        eprintln!("convert-api: internal panic at {at} (message withheld)");
    }));
}

#[tokio::main]
async fn main() -> ExitCode {
    quiet_panics();
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
    let (listen, redirect, limits) = (resolved.listen, resolved.s.redirect_listen.clone().zip(resolved.s.public_url.clone()), tls::ConnLimits::from_settings(&resolved));
    if resolved.upstream_tls {
        eprintln!("convert-api: tls = \"upstream\": plain HTTP, answering the trusted proxies only; TLS must end at the proxy");
    }
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
    let served: Result<(), std::io::Error> = {
        if let (Some(_), Some((addr, public))) = (&tls_cfg, redirect) {
            let addr: SocketAddr = match addr.parse() {
                Ok(a) => a,
                Err(_) => return refuse("redirect_listen is not an ip:port"),
            };
            match tokio::net::TcpListener::bind(addr).await {
                // the same timeouts and caps as the API listener; it only ever answers with a redirect
                Ok(l) => {
                    tokio::spawn(tls::serve(l, None, tls::redirect_router(&public), limits.clone(), std::future::pending()));
                }
                Err(e) => return refuse(format!("cannot listen on {addr}: {e}")),
            }
        }
        let l = match tokio::net::TcpListener::bind(listen).await {
            Ok(l) => l,
            Err(e) => return refuse(format!("cannot listen on {listen}: {e}")),
        };
        let scheme = if tls_cfg.is_some() { "https" } else { "http" };
        println!("convert-api listening on {scheme}://{}", l.local_addr().unwrap_or(listen));
        tls::serve(l, tls_cfg, router, limits, stop).await;
        Ok(())
    };
    app.shutdown();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => refuse(e),
    }
}
