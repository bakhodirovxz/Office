//! Review B1: tools die with the parent. Windows only (Job Object, kill on close); Linux gets a process group and
//! PR_SET_PDEATHSIG in WP11.
#![cfg(windows)]
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[test]
fn killing_convert_kills_the_tool_it_started() {
    let dir = std::env::temp_dir().join(format!("convert-contain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // the fake tool sleeps when its input says so (the executor clears the environment, so the input carries the order)
    let input = dir.join("slow.txt");
    std::fs::write(&input, "SLEEP").unwrap();
    let mut parent = Command::new(env!("CARGO_BIN_EXE_convert"))
        .arg(&input)
        .args(["--to", "docx", "--work-dir"])
        .arg(dir.join("work"))
        .args(["--out-dir"])
        .arg(dir.join("out"))
        .env("CONVERT_DOCS_CLI", env!("CARGO_BIN_EXE_convert-fake-tool"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let parent_pid = Pid::from_u32(parent.id());
    let mut sys = System::new();
    let t = Instant::now();
    let child = loop {
        sys.refresh_processes(ProcessesToUpdate::All, true);
        if let Some(p) = sys.processes().values().find(|p| p.parent() == Some(parent_pid) && p.name().to_string_lossy().contains("convert-fake-tool")) {
            break p.pid();
        }
        assert!(t.elapsed() < Duration::from_secs(20), "the tool never started");
        std::thread::sleep(Duration::from_millis(100));
    };
    parent.kill().unwrap();
    let _ = parent.wait();
    let t = Instant::now();
    loop {
        sys.refresh_processes(ProcessesToUpdate::All, true);
        if sys.process(child).is_none() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "the tool outlived the parent");
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
