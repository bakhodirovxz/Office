//! Test-only stand-in for `docs-diff` / `sheets-diff` (tests/history.rs copies it to `<dir>/docs-diff`). The first file's text picks the
//! behaviour: SLEEP (hangs), FAIL (exit 2 with a message), BIG (17 MB of output); anything else prints a small result and exits 1 (0 when
//! the files are equal).
use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let text = std::fs::read(&a[0]).unwrap_or_default();
    let has = |w: &str| text.windows(w.len()).any(|x| x == w.as_bytes());
    if has("SLEEP") {
        std::thread::sleep(std::time::Duration::from_secs(60));
    } else if has("FAIL") {
        eprintln!("fake-diff: not a valid document");
        std::process::exit(2);
    } else if has("BIG") {
        let _ = std::io::stdout().write_all(&vec![b' '; 17 << 20]);
    } else {
        let b = std::fs::read(&a[1]).unwrap_or_default();
        println!(
            "{{\"tool\":\"fake-diff\",\"schema\":1,\"same\":{},\"a_len\":{}}}",
            text == b,
            text.len()
        );
        std::process::exit(if text == b { 0 } else { 1 });
    }
}
