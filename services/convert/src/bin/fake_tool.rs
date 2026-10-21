//! Test-only stand-in for a domain CLI (PLAN WP2 step 5). The first argument picks the fault; the integration tests in
//! `tests/fault.rs` run the executor against it.
//!   sleep SECS | exit1 | panic | abort | alloc MB | empty PATH | truncated PATH | ok PATH | warn PATH
use std::io::Write;

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
            let mut f = std::fs::File::create(arg(1)).unwrap();
            f.write_all(b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n").unwrap();
        }
        other => {
            eprintln!("fake-tool: unknown mode '{other}'");
            std::process::exit(2);
        }
    }
}
