//! `convert-api [--config FILE]`: the HTTP API (PLAN 5). With no config file it runs in local mode: `auth = "none"` on 127.0.0.1.
use convert_api::runner;
use std::process::ExitCode;

const USAGE: &str = "usage: convert-api [--config FILE]
local mode (default): auth = \"none\", 127.0.0.1:8787. Server mode: auth = \"jwt\" with tls_cert, tls_key, secret_file and key_file. See docs/phase3/api.md.";

#[tokio::main]
async fn main() -> ExitCode {
    runner::quiet_panics();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match args.as_slice() {
        [] => None,
        [f, path] if f == "--config" => Some(path.as_str()),
        _ => return runner::refuse(USAGE),
    };
    runner::run("convert-api", config, convert_api::routes::router).await
}
