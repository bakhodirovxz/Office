//! `cargo xt quarantine check | filter <app> | selector <app> | apply <app>` (PLAN 6.5, WP6b).
//!
//! `ci/quarantine.toml` lists the known failing upstream tests. `filter` prints the nextest filterset of everything
//! that is NOT quarantined (the blocking pass), `selector` the quarantined set, and `apply` runs the quarantined set
//! in a separate pass and reports which entries are still failing, which are stale (they pass) and which no longer
//! exist. Two consecutive stale runs of an entry fail (exit 1); so does an entry that matches no test.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const APPS: [&str; 4] = ["wordcraft", "gridcraft", "deckcraft", "pdfcraft"];
const CLASSES: [&str; 7] = [
    "env",
    "upstream-bug",
    "flaky",
    "flaky-crash",
    "brand-overlay",
    "i18n-incomplete",
    "debug-only",
];
const REQUIRED: [&str; 10] = [
    "app", "crate", "binary", "name", "class", "env", "reason", "evidence", "since", "review",
];

#[derive(Debug, Clone)]
pub struct Entry {
    pub app: String,
    pub krate: String,
    pub binary: String,
    pub name: String,
    pub class: String,
    pub env: String,
    /// "on" / "off": the entry applies only with the overlay on / off (empty = both).
    pub overlay: String,
}

impl Entry {
    /// nextest binary id: `crate` for the lib, `crate::bin/x` for a bin target, `crate::x` for tests/x.rs.
    pub fn binary_id(&self) -> String {
        if self.binary == "lib" {
            self.krate.clone()
        } else {
            format!("{}::{}", self.krate, self.binary)
        }
    }
    /// name = "*": the whole test binary cannot run here (for example the antivirus deletes the .exe), so the blocking
    /// pass skips the binary; it is reported but not run in the quarantined pass.
    fn whole_binary(&self) -> bool {
        self.name == "*"
    }
    fn expr(&self) -> String {
        if self.whole_binary() {
            format!("binary_id(={})", self.binary_id())
        } else {
            format!("(binary_id(={}) & test(={}))", self.binary_id(), self.name)
        }
    }
}

fn load(root: &Path) -> Result<Vec<Entry>, String> {
    let path = root.join("ci/quarantine.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("ci/quarantine.toml: {e}"))?;
    let tests = doc
        .get("test")
        .and_then(|t| t.as_array_of_tables())
        .ok_or("ci/quarantine.toml: no [[test]] tables")?;
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (i, t) in tests.iter().enumerate() {
        let get = |k: &str| {
            t.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let label = format!("entry {} ({})", i + 1, get("name"));
        for k in REQUIRED {
            if get(k).is_empty() {
                problems.push(format!("{label}: empty or missing {k}"));
            }
        }
        if !get("class").is_empty() && !CLASSES.contains(&get("class").as_str()) {
            problems.push(format!("{label}: unknown class {}", get("class")));
        }
        if !get("app").is_empty() && !APPS.contains(&get("app").as_str()) {
            problems.push(format!("{label}: unknown app {}", get("app")));
        }
        if get("name") == "UNIDENTIFIED" {
            problems.push(format!("{label}: placeholder name"));
        }
        out.push(Entry {
            app: get("app"),
            krate: get("crate"),
            binary: get("binary"),
            name: get("name"),
            class: get("class"),
            env: get("env"),
            overlay: get("overlay"),
        });
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems.join("\n"))
    }
}

/// Entries of `app` that apply in this environment. `env = "any"` always applies; any other label applies when it
/// starts with one of the comma-separated tokens in OFFICE_TEST_ENV (default `win10`).
fn active(entries: Vec<Entry>, app: &str) -> Vec<Entry> {
    let tokens = std::env::var("OFFICE_TEST_ENV").unwrap_or_else(|_| "win10".to_string());
    let overlay_off = std::env::var("OFFICE_NO_OVERLAY")
        .map(|v| v == "1")
        .unwrap_or(false);
    let tokens: Vec<&str> = tokens
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    entries
        .into_iter()
        .filter(|e| e.app == app)
        .filter(|e| match e.overlay.as_str() {
            "on" => !overlay_off,
            "off" => overlay_off,
            _ => true,
        })
        .filter(|e| !(overlay_off && (e.class == "brand-overlay" || e.class == "i18n-incomplete")))
        .filter(|e| e.env == "any" || tokens.iter().any(|t| e.env.starts_with(t)))
        .collect()
}

fn selector(es: &[Entry]) -> String {
    es.iter()
        .filter(|e| !e.whole_binary())
        .map(Entry::expr)
        .collect::<Vec<_>>()
        .join(" | ")
}

pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    match args {
        ["check"] => {
            let es = load(root)?;
            Ok(format!("OK, {} entries", es.len()))
        }
        ["filter", app] => {
            let es = active(load(root)?, app);
            Ok(if es.is_empty() {
                "all()".into()
            } else {
                format!(
                    "not ({})",
                    es.iter().map(Entry::expr).collect::<Vec<_>>().join(" | ")
                )
            })
        }
        ["selector", app] => Ok(selector(&active(load(root)?, app))),
        ["apply", app] => apply(root, app),
        _ => Err(
            "usage: cargo xt quarantine check | filter <app> | selector <app> | apply <app>".into(),
        ),
    }
}

/// (binary_id, test name, passed) from nextest's JUnit file.
fn parse_junit(xml: &str) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<testcase ") {
        rest = &rest[i..];
        let end_tag = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end_tag];
        let attr = |k: &str| -> String {
            let pat = format!(" {k}=\"");
            tag.find(&pat)
                .map(|p| {
                    let s = &tag[p + pat.len()..];
                    s[..s.find('"').unwrap_or(s.len())]
                        .replace("&lt;", "<")
                        .replace("&gt;", ">")
                        .replace("&amp;", "&")
                })
                .unwrap_or_default()
        };
        let (class, name) = (attr("classname"), attr("name"));
        // The body runs to </testcase> unless the tag is self-closing.
        let body = if tag.ends_with('/') {
            ""
        } else {
            let close = rest.find("</testcase>").unwrap_or(rest.len());
            &rest[end_tag..close.max(end_tag)]
        };
        let failed =
            body.contains("<failure") || body.contains("<error") || body.contains("<flakyFailure");
        let skipped = body.contains("<skipped");
        if !skipped {
            out.push((class, name, !failed));
        }
        rest = &rest[end_tag.min(rest.len())..];
    }
    out
}

fn apply(root: &Path, app: &str) -> Result<String, String> {
    let all = active(load(root)?, app);
    let unrunnable = all.iter().filter(|e| e.whole_binary()).count();
    let es: Vec<Entry> = all.into_iter().filter(|e| !e.whole_binary()).collect();
    if es.is_empty() {
        return Ok(format!(
            "{app}: quarantined 0 (still failing 0, stale 0, missing 0)"
        ));
    }
    let target = std::env::var("UP_TARGET")
        .unwrap_or_else(|_| format!("E:/office-target/p1/upstream-{app}"));
    let manifest = root.join("upstream").join(app).join("Cargo.toml");
    let junit = PathBuf::from(&target).join("nextest/quarantine/junit.xml");
    let _ = fs::remove_file(&junit);
    // Same environment as the blocking pass, or cargo rebuilds the upstream crates that read these (up to 70 min).
    let status = Command::new("cargo")
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_BUILD_JOBS", "4")
        .args([
            "nextest",
            "run",
            "--workspace",
            "--profile",
            "quarantine",
            "--config-file",
        ])
        .arg(
            std::env::var("NEXTEST_CFG")
                .map(PathBuf::from)
                .unwrap_or_else(|_| root.join("ci/nextest.toml")),
        )
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target)
        .args([
            "--build-jobs",
            "4",
            "--test-threads",
            "4",
            "--no-fail-fast",
            "--no-tests",
            "pass",
            "-E",
        ])
        .arg(selector(&es))
        .status()
        .map_err(|e| format!("cargo nextest: {e}"))?;
    let xml = fs::read_to_string(&junit).map_err(|e| {
        format!(
            "{}: {e} (nextest exit {:?}: infrastructure error)",
            junit.display(),
            status.code()
        )
    })?;
    let results = parse_junit(&xml);
    let (mut failing, mut flaky_pass, mut stale, mut missing) = (0, 0, Vec::new(), Vec::new());
    for e in &es {
        let id = e.binary_id();
        match results.iter().find(|(c, n, _)| *c == id && *n == e.name) {
            None => missing.push(e.name.clone()),
            // A flaky test is allowed to pass; only a deterministic failure turning green makes an entry stale.
            Some((_, _, true)) if e.class == "flaky" || e.class == "flaky-crash" => flaky_pass += 1,
            Some((_, _, true)) => stale.push(e.name.clone()),
            Some((_, _, false)) => failing += 1,
        }
    }
    // Two consecutive stale runs fail: the previous run's stale set lives next to the CI output.
    let office_target =
        std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into());
    let state_dir = PathBuf::from(office_target).join("ci/quarantine-stale");
    let state = state_dir.join(format!("{app}.txt"));
    let prev: BTreeSet<String> = fs::read_to_string(&state)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let _ = fs::create_dir_all(&state_dir);
    let _ = fs::write(&state, stale.join("\n"));
    let twice: Vec<&String> = stale.iter().filter(|n| prev.contains(*n)).collect();
    let mut msg = format!(
        "{app}: quarantined {} (still failing {failing}, passed-flaky {flaky_pass}, stale {}, missing {}), unrunnable binaries {unrunnable}",
        es.len(),
        stale.len(),
        missing.len()
    );
    for n in &stale {
        msg.push_str(&format!(
            "\n  quarantine stale: {n} passes now; remove the entry"
        ));
    }
    for n in &missing {
        msg.push_str(&format!(
            "\n  quarantine missing: {n} matches no test; fix or remove the entry"
        ));
    }
    if !twice.is_empty() || !missing.is_empty() {
        return Err(format!(
            "{msg}\n  stale twice in a row or missing: CI fails"
        ));
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junit_pass_fail_and_skip() {
        let xml = r#"<testsuite><testcase name="a::b" classname="c" time="1"/>
<testcase name="x" classname="c::t"><failure message="m"/></testcase>
<testcase name="s" classname="c"><skipped/></testcase></testsuite>"#;
        let r = parse_junit(xml);
        assert_eq!(
            r,
            vec![
                ("c".into(), "a::b".into(), true),
                ("c::t".into(), "x".into(), false)
            ]
        );
    }

    #[test]
    fn binary_ids() {
        let e = |b: &str| Entry {
            app: "a".into(),
            krate: "k".into(),
            binary: b.into(),
            name: "n".into(),
            class: "env".into(),
            env: "any".into(),
            overlay: String::new(),
        };
        assert_eq!(e("lib").binary_id(), "k");
        assert_eq!(e("bin/x").binary_id(), "k::bin/x");
        assert_eq!(e("t").binary_id(), "k::t");
    }
}
