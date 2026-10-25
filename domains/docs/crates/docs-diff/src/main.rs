//! `docs-diff OLD.docx NEW.docx [--json] [--pretty] [--timings]`: the diff as JSON on stdout (PLAN phase 5 §6.1).
//! Exit status as diff(1): 0 no differences, 1 differences, 2 trouble. `--json` is the default and accepted for the
//! plan's spelling; `--timings` prints read and diff times (ms) to stderr.
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    if files.len() != 2 || args.iter().any(|a| a.starts_with("--") && !["--json", "--pretty", "--timings"].contains(&a.as_str())) {
        eprintln!("usage: docs-diff OLD.docx NEW.docx [--json] [--pretty] [--timings]");
        return ExitCode::from(2);
    }
    let t0 = Instant::now();
    // both files parse at once: reading is most of the time
    let (a, b) = std::thread::scope(|s| {
        let ha = s.spawn(|| load(files[0]));
        let b = load(files[1]);
        (ha.join().unwrap_or_else(|_| Err("reader panicked".into())), b)
    });
    let (a, b) = match (a, b) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("docs-diff: {e}");
            return ExitCode::from(2);
        }
    };
    let t1 = Instant::now();
    let out = docs_diff::diff(&a, &b);
    let t2 = Instant::now();
    let text = if flag("--pretty") { serde_json::to_string_pretty(&out) } else { serde_json::to_string(&out) };
    println!("{}", text.unwrap_or_default());
    if flag("--timings") {
        eprintln!("read_ms={} diff_ms={}", (t1 - t0).as_millis(), (t2 - t1).as_millis());
    }
    // two large models take seconds to free; the process ends now anyway
    std::mem::forget((a, b));
    let same = out["changes"].as_array().is_some_and(Vec::is_empty);
    ExitCode::from(if same { 0 } else { 1 })
}

fn load(p: &str) -> Result<wordcraft_doc::Document, String> {
    std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| docs_diff::read(&b)).map_err(|e| format!("{p}: {e}"))
}
