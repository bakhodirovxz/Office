//! Test-only stand-in for a domain CLI (PLAN WP2 step 5). The first argument picks the fault; the integration tests in
//! `tests/fault.rs` run the executor against it.
//!   sleep SECS | exit1 | panic | abort | alloc MB | empty PATH | truncated PATH | ok PATH [TEXT] | warn PATH
//!   translit PATH JSON (prints JSON on stdout) | silent (writes nothing, exit 0)
//! and the shapes the planner really emits, so `convert_file` can run end to end without a domain CLI:
//!   convert IN OUT | run ... (--out/--save OUT, or a file.exportPdf path) | info FILE (fails when the file holds BADINFO)
fn stub(path: &str, extra: &str) {
    let body = if path.ends_with(".pdf") { format!("%PDF-1.4\n1 0 obj<<>>endobj\n{extra}\ntrailer<<>>\n%%EOF\n") } else { format!("stub {extra}\n") };
    std::fs::write(path, body).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| a.get(i).map(String::as_str).unwrap_or("");
    match arg(0) {
        "sleep" => std::thread::sleep(std::time::Duration::from_secs(arg(1).parse().unwrap_or(30))),
        "exit1" => {
            eprintln!("fake-tool: the document is broken");
            std::process::exit(1);
        }
        "panic" => panic!("fake-tool: deliberate panic"),
        "abort" => std::process::abort(),
        "alloc" => {
            // touch every page so the working set really grows; the watchdog kills us long before 4 GB when the cap is low
            let mb: usize = arg(1).parse().unwrap_or(4096);
            let mut hold: Vec<Vec<u8>> = vec![];
            for _ in 0..mb / 64 {
                hold.push(vec![1u8; 64 << 20]);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            std::hint::black_box(&hold);
        }
        "empty" => std::fs::write(arg(1), b"").unwrap(),
        // a zip header and nothing else: right magic, unreadable directory
        "truncated" => std::fs::write(arg(1), b"PK\x03\x04\x14\x00\x00\x00\x08\x00partial").unwrap(),
        "ok" | "warn" => {
            if arg(0) == "warn" {
                eprintln!("Warning: fake-tool dropped the charts");
            }
            stub(arg(1), arg(2));
        }
        "translit" => {
            println!("{}", arg(2));
            stub(arg(1), "");
        }
        "silent" => {}
        "convert" => stub(arg(2), ""),
        "run" => {
            let at = a.iter().position(|x| x == "--out" || x == "--save").map(|i| a[i + 1].clone());
            let pdf = a.iter().find_map(|x| x.strip_prefix("file.exportPdf=")).and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()).and_then(|v| v["path"].as_str().map(String::from));
            stub(&at.or(pdf).expect("run needs an output"), "");
        }
        "info" => {
            let data = std::fs::read(arg(1)).unwrap_or_else(|_| {
                eprintln!("fake-tool: cannot open {}", arg(1));
                std::process::exit(1)
            });
            if data.windows(7).any(|w| w == b"BADINFO") {
                eprintln!("fake-tool: not a valid document");
                std::process::exit(1);
            }
            println!("{{\"pages\": 1}}");
        }
        other => {
            eprintln!("fake-tool: unknown mode '{other}'");
            std::process::exit(2);
        }
    }
}
