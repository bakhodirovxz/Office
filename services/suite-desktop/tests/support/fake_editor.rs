use std::io::Write;
fn main() {
    let log = std::env::var_os("FAKE_EDITOR_LOG").expect("FAKE_EDITOR_LOG");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .unwrap();
    for a in std::env::args().skip(1) {
        writeln!(f, "{a}").unwrap();
    }
}
