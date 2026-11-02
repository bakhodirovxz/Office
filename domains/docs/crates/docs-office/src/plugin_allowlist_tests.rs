//! Phase 7 WP1: the plugin command allowlist (`plugin-api` `allow`, PLAN phase 7 section 5.2) against the real Docs registry.
//! The allowlist is data in a crate that cannot link an engine; this is where the two meet.

use plugin_api::allow::{EXCEPTIONS, PREFIXES, READS, check_run};
use serde_json::json;
use wordcraft_engine::cmd::registry;

/// Words in a `params` description that name a file, a folder or a URL.
fn mentions(params: &str) -> Vec<&'static str> {
    const WORDS: &[&str] = &[
        "path", "dir", "file", "url", "uri", "href", "src", "folder", "filename",
    ];
    let lower = params.to_ascii_lowercase();
    let toks: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    WORDS.iter().copied().filter(|w| toks.contains(w)).collect()
}

/// Allowed specs whose description mentions such a word and were read by hand: why each is fine.
const REVIEWED: &[(&str, &str)] = &[
    (
        "document.paragraph",
        "`path` is a node path [n], an array of integers, which the key rule lets through",
    ),
    (
        "insert.quickParts",
        "prose only: building blocks are `kept in its file` (the document's own)",
    ),
];

fn is_allowed(id: &str) -> bool {
    READS.contains(&id)
        || (PREFIXES.iter().any(|p| id.starts_with(p)) && !EXCEPTIONS.iter().any(|(e, _)| *e == id))
}

#[test]
fn every_read_and_exception_exists_in_the_registry() {
    let r = registry();
    for id in READS
        .iter()
        .copied()
        .chain(EXCEPTIONS.iter().map(|(e, _)| *e))
    {
        assert!(
            r.get(id).is_some(),
            "{id} is on the allowlist data but not in the Docs registry (renamed upstream?)"
        );
    }
    for (id, _) in REVIEWED {
        assert!(r.get(id).is_some(), "{id}");
    }
}

#[test]
fn no_allowed_command_mentions_a_path_or_url_unreviewed() {
    let r = registry();
    let mut bad = Vec::new();
    for s in r.all().iter().filter(|s| is_allowed(s.id)) {
        let m = mentions(s.params);
        if !m.is_empty() && !REVIEWED.iter().any(|(id, _)| *id == s.id) {
            bad.push(format!(
                "{} {:?}: {}",
                s.id,
                m,
                s.params.chars().take(120).collect::<String>()
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "allowed commands that take a path, file or URL (add an exception, or review and list them):\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_dangerous_families_are_refused_and_the_counts_are_sane() {
    let r = registry();
    let mut allowed = 0;
    for s in r.all() {
        let denied_family = [
            "file.",
            "edit.",
            "tools.",
            "mailings.",
            "link.",
            "view.",
            "readAloud.",
            "ui.",
            "office.",
            "plugin.",
            "layout.",
            "arrange.",
            "hf.",
            "design.",
        ]
        .iter()
        .any(|p| s.id.starts_with(p));
        let ok = check_run(s.id, &json!({}));
        assert_eq!(ok.is_ok(), is_allowed(s.id), "{}", s.id);
        if denied_family {
            assert!(ok.is_err(), "{} must not be runnable by a plugin", s.id);
        }
        allowed += usize::from(ok.is_ok());
    }
    assert!(
        r.get("document.setText").is_some() && check_run("document.setText", &json!({})).is_err()
    );
    // measured 2026-11-03: 306 of 633; a drop to a handful means the prefixes stopped matching
    assert!(allowed > 150, "only {allowed} commands are allowed");
    println!(
        "plugin allowlist: {allowed} of {} registry commands",
        r.all().len()
    );
}
