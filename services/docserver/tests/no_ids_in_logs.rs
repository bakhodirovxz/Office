//! S17 (PLAN phase 5, section 10): no log line of the server names a session id, a token or a provider URL. The full e2e run greps
//! stderr (`ci/web-smoke/docserver.spec.ts`), but only the lines a good run prints; this reads every `eprintln!`/`println!` of the
//! crate, so the error paths are held to the same rule. A session appears as `sid_ref`, a URL as `host_only`/`host_path`.
use std::path::Path;

/// The arguments of each log macro call in `src`: the code with its string literals cut out, and the literals (inline `{name}`
/// arguments live there).
fn log_calls(src: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for (at, _) in src.match_indices("println!(") {
        let (mut depth, mut code, mut lit, mut in_str, mut esc) =
            (0i32, String::new(), String::new(), false, false);
        for c in src[at + "println!".len()..].chars() {
            if in_str {
                lit.push(c);
                match (esc, c) {
                    (true, _) => esc = false,
                    (false, '\\') => esc = true,
                    (false, '"') => in_str = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => in_str = true,
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            code.push(c);
            if depth == 0 {
                break;
            }
        }
        out.push((code, lit));
    }
    out
}

#[test]
fn no_session_id_token_or_url_in_log_lines() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (mut hits, mut calls) = (vec![], 0);
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        for (code, lit) in log_calls(&src) {
            calls += 1;
            let code = code.replace(char::is_whitespace, "");
            let bad = [
                ".sid",
                "sealed_name(",
                "document_url",
                "callback_url",
                "token",
            ]
            .iter()
            .any(|n| code.contains(n) && !code.contains("sid_ref("))
                || ["{sid", "{token", "{tok", "{url"]
                    .iter()
                    .any(|n| lit.contains(n));
            if bad {
                hits.push(format!("{}: {code}", p.display()));
            }
        }
    }
    assert!(
        calls > 10,
        "the scan found only {calls} log calls: the parser is broken"
    );
    assert!(hits.is_empty(), "{hits:#?}");
}
