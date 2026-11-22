//! Uzbek Latin <-> Cyrillic transliteration for the wordcraft engine (Phase 2 WP4, PLAN section 3).
//!
//! `install(&mut Session)` appends three commands to the session's registry: `tools.translitCyrl`, `tools.translitLatn` and
//! `tools.translitAuto` (the direction follows the dominant script of the scope). The apps call it through the carried hook
//! `patches/wordcraft/0120`; everything else lives here.
//!
//! One `Session::run` is one undo step. The command rewrites `Paragraph.text` once per paragraph and keeps every `Run.props`
//! (only `Run.len` changes), so formatting, language tags and hyperlink targets are untouched by construction. Field codes and
//! other inline objects (U+FFFC) are never part of a converted slice. The rules come from `uz-translit` (`convert_runs`).
use std::borrow::Cow;
use std::sync::Arc;

use docs_plugins::textmap::{self, Opts, Scope, Seg, Stats};
use serde_json::{Value, json};
use uz_translit::{
    Apostrophe, Direction, LangPolicy, LatinAlphabet, Options, Script, lang_converts,
};
use wordcraft_engine::{CmdError, CmdResult, CommandSpec, Registry, Session};

/// The command ids. `column.rs` (patch 0120) lists the first two so that a column selection converts row by row.
pub const TO_CYRILLIC: &str = "tools.translitCyrl";
pub const TO_LATIN: &str = "tools.translitLatn";
pub const AUTO: &str = "tools.translitAuto";

const PARAMS: &str = r#"{"scope"?: "auto"|"selection"|"document", "skipLangs"?: bool, "fieldResults"?: bool, "latinAlphabet"?: "1995"|"2026"|"auto", "apostrophe"?: "official"|"ascii", "foreignGuard"?: bool}"#;

/// The specs, for callers that build a registry themselves.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(
            TO_CYRILLIC,
            "Latin to Cyrillic",
            "Review › Language",
            |s, v| run(s, v, Some(Direction::ToCyrillic)),
        )
        .params(PARAMS),
        CommandSpec::new(
            TO_LATIN,
            "Cyrillic to Latin",
            "Review › Language",
            |s, v| run(s, v, Some(Direction::ToLatin)),
        )
        .params(PARAMS),
        CommandSpec::new(AUTO, "Auto-detect", "Review › Language", |s, v| {
            run(s, v, None)
        })
        .params(PARAMS),
    ]
}

/// Add the commands to `s.registry`. Safe to call twice.
pub fn install(s: &mut Session) {
    if s.registry.get(TO_CYRILLIC).is_some() {
        return;
    }
    let mut all = s.registry.all().to_vec();
    all.extend(specs());
    s.registry = Arc::new(Registry::new(all));
}

struct Ctx {
    opts: Options,
    policy: LangPolicy,
    field_results: bool,
    tracked: bool,
}

fn params(v: &Value) -> Result<(Scope, Ctx), CmdError> {
    let scope = match wordcraft_engine::p::str(v, "scope") {
        None | Some("auto") => Scope::Auto,
        Some("selection") => Scope::Selection,
        Some("document") => Scope::Document,
        Some(o) => {
            return Err(CmdError::Params(format!(
                "`scope` must be auto, selection or document, not `{o}`"
            )));
        }
    };
    let apostrophe = match wordcraft_engine::p::str(v, "apostrophe") {
        None | Some("official") => Apostrophe::Official,
        Some("ascii") => Apostrophe::Ascii,
        Some(o) => {
            return Err(CmdError::Params(format!(
                "`apostrophe` must be official or ascii, not `{o}`"
            )));
        }
    };
    let latin_alphabet = match wordcraft_engine::p::str(v, "latinAlphabet") {
        None => LatinAlphabet::default(),
        Some("1995") => LatinAlphabet::Y1995,
        Some("2026") => LatinAlphabet::Y2026,
        Some("auto") => LatinAlphabet::Auto,
        Some(o) => {
            return Err(CmdError::Params(format!(
                "`latinAlphabet` must be 1995, 2026 or auto, not `{o}`"
            )));
        }
    };
    let defaults = Options::default();
    let opts = Options {
        apostrophe,
        latin_alphabet,
        foreign_latin_guard: wordcraft_engine::p::bool(v, "foreignGuard")
            .unwrap_or(defaults.foreign_latin_guard),
        ..defaults
    };
    let policy = if wordcraft_engine::p::bool(v, "skipLangs") == Some(true) {
        LangPolicy::SkipRuEn
    } else {
        LangPolicy::ConvertRuEn
    };
    let field_results = wordcraft_engine::p::bool(v, "fieldResults").unwrap_or(true);
    Ok((
        scope,
        Ctx {
            opts,
            policy,
            field_results,
            tracked: false,
        },
    ))
}

fn run(s: &mut Session, v: &Value, dir: Option<Direction>) -> CmdResult {
    let (scope, mut ctx) = params(v)?;
    ctx.tracked = s.doc.settings.track_changes;
    // The scope: the selection if asked for, or if it is not collapsed; otherwise the whole document.
    let (use_selection, targets) = textmap::targets(s, scope);
    let Some(targets) = targets else {
        return Ok(summary(&Stats::default(), dir, true, &ctx));
    };
    let detected = detect(&textmap::text_of(s, &targets));
    // "Cyrillic to Latin" rewrites the 1995 letters in the 2026 ones: on a Latin-dominant selection, and on a whole document
    // too, where a paragraph that looks English keeps its Latin ("fish" stays "fish").
    ctx.opts.latin_upgrade = dir == Some(Direction::ToLatin)
        && (!use_selection || detected == Some(Direction::ToCyrillic));
    ctx.opts.upgrade_skip_english = !use_selection;
    let dir = dir.or(detected);
    let mut st = Stats::default();
    if let Some(dir) = dir {
        let lang_ok = |l: Option<&str>| lang_converts(l, ctx.policy);
        let o = Opts {
            field_results: ctx.field_results,
            tracked: ctx.tracked,
            lang_ok: &lang_ok,
        };
        textmap::walk(s, &targets, &o, &mut st, &mut |_, p, segs| {
            convert(p, segs, dir, &ctx)
        })?;
    }
    Ok(summary(&st, dir, use_selection, &ctx))
}

fn summary(st: &Stats, dir: Option<Direction>, selection: bool, ctx: &Ctx) -> Value {
    json!({
        "direction": match dir { Some(Direction::ToCyrillic) => "cyrl", Some(Direction::ToLatin) => "latn", None => "none" },
        "scope": if selection { "selection" } else { "document" },
        "tracked": ctx.tracked,
        "paragraphs": st.paragraphs,
        "runs": st.runs,
        "skipped": {"lang": st.lang, "field": st.field, "locked": st.locked},
        "ms": {"convert": st.convert_ms, "apply": st.apply_ms},
        "rulesVersion": uz_translit::RULES_VERSION,
    })
}

/// The dominant script of the target text, as a direction (Latin text goes to Cyrillic and back).
fn detect(text: &str) -> Option<Direction> {
    match uz_translit::detect(text)? {
        Script::Latin => Some(Direction::ToCyrillic),
        Script::Cyrillic => Some(Direction::ToLatin),
    }
}

/// Convert one paragraph's pieces (the walker's converter): the pieces that are not tracked deletions go to the converter as runs.
fn convert(p: &wordcraft_doc::Paragraph, segs: &mut [Seg], dir: Direction, ctx: &Ctx) -> bool {
    let live: Vec<usize> = (0..segs.len()).filter(|&i| !segs[i].deleted).collect();
    let runs: Vec<uz_translit::Run> = live
        .iter()
        .map(|&i| uz_translit::Run {
            text: &p.text[segs[i].range.clone()],
            convert: segs[i].convert,
        })
        .collect();
    let out = uz_translit::convert_runs(&runs, dir, &ctx.opts);
    let mut changed = false;
    let outs: Vec<Option<String>> = out
        .into_iter()
        .map(|c| match c {
            Cow::Owned(o) => {
                changed = true;
                Some(o)
            }
            Cow::Borrowed(_) => None,
        })
        .collect();
    for (i, o) in live.into_iter().zip(outs) {
        segs[i].new = o;
    }
    changed
}
