//! Test-only stand-in for a domain CLI. `convert IN OUT` writes a stub output; an input that starts with `SLEEP` keeps the tool
//! busy for two minutes (the cancel test), and one that starts with `FAIL` makes it exit 1 (the UI spec). `--version` and `info` answer like the real ones.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("convert") => {
            if std::fs::read(&a[1]).is_ok_and(|b| b.starts_with(b"SLEEP")) {
                std::thread::sleep(std::time::Duration::from_secs(120));
            }
            if std::fs::read(&a[1]).is_ok_and(|b| b.starts_with(b"FAIL")) {
                eprintln!("fake tool: cannot read this file");
                std::process::exit(1);
            }
            let body = if a[2].ends_with(".pdf") { "%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n" } else { "stub\n" };
            std::fs::write(&a[2], body).unwrap();
        }
        Some("info") => println!("{{\"pages\": 1}}"),
        _ => println!("convert-api-fake-tool"),
    }
}
