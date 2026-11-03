//! Plugin #1 (PLAN phase 7 section 6): the transliteration command as a plugin. It holds only what section 6.1 lists: params to
//! `Options`/`LangPolicy`, `lang_converts` per segment, `detect` for `auto`, `convert_runs` per paragraph and the summary. The text
//! map comes with the call, the edits go back as one `textmap.apply`, and the host's own `apply` writes them (tracked changes, undo).
//! The converter is the same `uz-translit` crate the built-in command links, so the same segments give the same output.
use std::borrow::Cow;

use plugin_sdk::plugin_api::textmap::{EDITABLE, FIELD_RESULT, LOCKED_CONTROL, OUTSIDE_SELECTION};
use plugin_sdk::{Apply, Call, CallKind, Reply, TextMap};
use serde_json::{Map, Value};
use uz_translit::{
    Apostrophe, Direction, LangPolicy, LatinAlphabet, Options, Run, Script, lang_converts,
};

/// `key, en, ru, uz-Latn, uz-Cyrl, review` per line; the first line is the header. The status texts are the built-in command's.
const STRINGS: &str = include_str!("../strings.tsv");

fn params(p: &Map<String, Value>) -> Result<(Options, LangPolicy), String> {
    let s = |k: &str| p.get(k).and_then(Value::as_str);
    let apostrophe = match s("apostrophe") {
        None | Some("official") => Apostrophe::Official,
        Some("ascii") => Apostrophe::Ascii,
        Some(o) => return Err(format!("`apostrophe` must be official or ascii, not `{o}`")),
    };
    let latin_alphabet = match s("latinAlphabet") {
        None => LatinAlphabet::default(),
        Some("1995") => LatinAlphabet::Y1995,
        Some("2026") => LatinAlphabet::Y2026,
        Some("auto") => LatinAlphabet::Auto,
        Some(o) => {
            return Err(format!(
                "`latinAlphabet` must be 1995, 2026 or auto, not `{o}`"
            ));
        }
    };
    let d = Options::default();
    let opts = Options {
        apostrophe,
        latin_alphabet,
        foreign_latin_guard: p
            .get("foreignGuard")
            .and_then(Value::as_bool)
            .unwrap_or(d.foreign_latin_guard),
        ..d
    };
    let policy = if p.get("skipLangs").and_then(Value::as_bool) == Some(true) {
        LangPolicy::SkipRuEn
    } else {
        LangPolicy::ConvertRuEn
    };
    Ok((opts, policy))
}

/// The status text in the interface language `lang` (English when the language or the key is missing).
fn text(lang: &str, key: &str) -> String {
    let col = match lang.to_ascii_lowercase().as_str() {
        "ru" => 2,
        "uz-latn" => 3,
        "uz-cyrl" => 4,
        _ => 1,
    };
    let row = STRINGS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|c| c[0] == key);
    row.map(|c| c[col].to_string())
        .unwrap_or_else(|| key.to_string())
}

/// The built-in's status line: "N paragraphs converted" or "Nothing to convert", then the skipped count when there is one.
fn summary(lang: &str, done: usize, skipped: usize) -> String {
    let mut m = if done == 0 {
        text(lang, "nothing")
    } else {
        text(lang, "converted").replace("{n}", &done.to_string())
    };
    if skipped > 0 {
        m += &format!(
            ". {}",
            text(lang, "skipped").replace("{n}", &skipped.to_string())
        );
    }
    m
}

/// The dominant script of the text that is in scope (Latin goes to Cyrillic and back); `None` for a tie or no letters.
fn detect(map: &TextMap) -> Option<Direction> {
    let mut all = String::new();
    for g in map
        .paragraphs
        .iter()
        .flat_map(|p| &p.segments)
        .filter(|g| g.flags & OUTSIDE_SELECTION == 0)
    {
        all.push_str(&g.text);
        all.push('\n');
    }
    match uz_translit::detect(&all)? {
        Script::Latin => Some(Direction::ToCyrillic),
        Script::Cyrillic => Some(Direction::ToLatin),
    }
}

pub fn handle(call: Call, blobs: Vec<Vec<u8>>) -> Result<Reply, String> {
    if call.kind != CallKind::Command {
        return Ok(Reply::default());
    }
    let dir = match call.command.as_deref() {
        Some("toCyrillic") => Some(Direction::ToCyrillic),
        Some("toLatin") => Some(Direction::ToLatin),
        Some("auto") => None,
        o => return Err(format!("unknown command {o:?}")),
    };
    let (opts, policy) = params(&call.params)?;
    let map =
        TextMap::decode(plugin_sdk::read(&call, &blobs, "textmap")?).map_err(|e| e.to_string())?;
    let Some(dir) = dir.or_else(|| detect(&map)) else {
        return Ok(Reply::status(summary(&call.lang, 0, 0)));
    };
    let (mut apply, mut done, mut skipped) = (Apply::default(), 0, 0);
    for p in &map.paragraphs {
        let ok = |i: usize| {
            lang_converts(
                Some(map.langs[p.segments[i].lang as usize].as_str()),
                policy,
            )
        };
        let runs: Vec<Run> = (0..p.segments.len())
            .map(|i| Run {
                text: &p.segments[i].text,
                convert: p.segments[i].flags == EDITABLE && ok(i),
            })
            .collect();
        skipped += p
            .segments
            .iter()
            .enumerate()
            .filter(|(i, g)| {
                (g.flags == EDITABLE && !ok(*i)) || g.flags & (FIELD_RESULT | LOCKED_CONTROL) != 0
            })
            .count();
        let mut changed = false;
        let out: Vec<Option<String>> = uz_translit::convert_runs(&runs, dir, &opts)
            .into_iter()
            .map(|c| match c {
                Cow::Owned(o) => {
                    changed = true;
                    Some(o)
                }
                Cow::Borrowed(_) => None,
            })
            .collect();
        done += usize::from(changed);
        apply.paragraphs.push(out);
    }
    let reply = Reply::status(summary(&call.lang, done, skipped));
    if done == 0 {
        Ok(reply)
    } else {
        reply.textmap_apply(&apply)
    }
}

plugin_sdk::export!(handle);

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_sdk::Action;
    use plugin_sdk::plugin_api::textmap::{Paragraph, Segment};

    fn call(cmd: &str, lang: &str, params: Value) -> Call {
        let mut c: Call = serde_json::from_value(serde_json::json!({
            "kind": "command", "api": 1, "api_minor": 0, "app": "docs", "lang": lang, "command": cmd, "reads": {"textmap": 0}
        }))
        .unwrap();
        c.params = params.as_object().cloned().unwrap_or_default();
        c
    }

    fn map(segs: &[(u8, &str, &str)]) -> Vec<Vec<u8>> {
        let mut m = TextMap::default();
        let segments = segs
            .iter()
            .map(|(f, l, t)| Segment {
                flags: *f,
                lang: m.lang_index(l).unwrap(),
                text: t.to_string(),
            })
            .collect();
        m.paragraphs.push(Paragraph { index: 0, segments });
        vec![m.encode().unwrap()]
    }

    #[test]
    fn converts_editable_and_leaves_the_rest() {
        let b = map(&[
            (EDITABLE, "uz-Latn", "shahar "),
            (4, "", "\u{fffc}"),
            (EDITABLE, "de-DE", "stadt"),
            (EDITABLE, "ru-RU", "mir"),
        ]);
        let r = handle(
            call("toCyrillic", "en", serde_json::json!({"skipLangs": true})),
            b,
        )
        .unwrap();
        let Action::TextmapApply { blob } = r.actions[0] else {
            panic!()
        };
        let a = Apply::decode(&r.blobs[blob as usize]).unwrap();
        assert_eq!(
            a.paragraphs,
            vec![vec![Some("шаҳар ".to_string()), None, None, None]]
        );
        let Action::UiStatus { text } = &r.actions[1] else {
            panic!()
        };
        assert_eq!(
            text,
            "1 paragraphs converted. 2 items skipped (other language, locked or field)"
        );
    }

    #[test]
    fn auto_follows_the_dominant_script_and_a_tie_does_nothing() {
        let b = map(&[(EDITABLE, "", "Салом дунё")]);
        let r = handle(call("auto", "uz-Latn", serde_json::json!({})), b).unwrap();
        assert!(matches!(r.actions[0], Action::TextmapApply { .. }));
        let r = handle(
            call("auto", "ru", serde_json::json!({})),
            map(&[(EDITABLE, "", "ab аб")]),
        )
        .unwrap();
        assert_eq!(r.actions.len(), 1);
        let Action::UiStatus { text } = &r.actions[0] else {
            panic!()
        };
        assert_eq!(text, "Нечего преобразовывать");
    }

    #[test]
    fn bad_params_and_unknown_commands_are_errors() {
        assert!(
            handle(
                call("toLatin", "en", serde_json::json!({"apostrophe": "x"})),
                map(&[])
            )
            .is_err()
        );
        assert!(handle(call("nope", "en", serde_json::json!({})), map(&[])).is_err());
    }
}
