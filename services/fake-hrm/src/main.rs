#![forbid(unsafe_code)]
//! Manual runs: `fake-hrm [file.docx ...]` registers each file as `files/1`, `files/2`, ... and serves on
//! FAKE_HRM_ADDR (default 127.0.0.1:8790). Env: FAKE_HRM_DOCSERVER (origin of the document server), FAKE_HRM_INTERNAL_BASE
//! (callback download base), FAKE_HRM_ALLOWED_HOSTS (comma list), FAKE_HRM_BEARER=1 (PR-1 fallback), FAKE_HRM_EXP=1,
//! FAKE_HRM_TEST_API=1 (the /__test/ routes of the browser specs).
//! Secrets are generated at start and printed to stderr: this is a test tool, run it only on a developer machine.
use fake_hrm::{Config, FakeHrm, Kind};

fn env(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut cfg = Config::generated(&env("FAKE_HRM_INTERNAL_BASE", "http://127.0.0.1:8780"));
    cfg.docserver_public_url = env("FAKE_HRM_DOCSERVER", "http://localhost:8780");
    cfg.callback_allowed_hosts = env("FAKE_HRM_ALLOWED_HOSTS", "127.0.0.1,localhost")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    cfg.bearer_fallback = env("FAKE_HRM_BEARER", "0") == "1";
    cfg.include_exp = env("FAKE_HRM_EXP", "0") == "1";
    cfg.test_api = env("FAKE_HRM_TEST_API", "0") == "1";
    let hrm = FakeHrm::start_on(&env("FAKE_HRM_ADDR", "127.0.0.1:8790"), cfg).await?;
    for (i, p) in std::env::args().skip(1).enumerate() {
        let name = std::path::Path::new(&p)
            .file_name()
            .map_or("doc.docx".into(), |n| n.to_string_lossy().into_owned());
        hrm.add_file(Kind::File, i as u64 + 1, &name, std::fs::read(&p)?);
        eprintln!("files/{} = {name}", i + 1);
    }
    let c = hrm.config();
    eprintln!(
        "fake-hrm on {} (secret {}, file secret {})",
        hrm.base_url(),
        c.ds_secret,
        c.jwt_secret
    );
    eprintln!("page: {}", hrm.page_url("localhost", Kind::File, 1));
    std::future::pending::<()>().await;
    Ok(())
}
