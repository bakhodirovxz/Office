//! Uzbek Latin <-> Cyrillic transliteration of slide text (office overlay, patches/deckcraft/0120; PLAN section 5).
//!
//! Text frames (shapes, groups, table cells) and speaker notes, at run level: a run keeps its properties (font, size, colour,
//! link target, language tag), only `Run.text` changes. Field, break and equation runs are never part of a converted slice.
//! Layouts, masters, comments, alt text and SmartArt are left alone. One `Session::execute` is one undo step.
use std::sync::Arc;

use deckcraft_model::text::RunKind;
use deckcraft_model::{Paragraph, Run, ShapeId, ShapeKind, Slide, TextBody};
use serde_json::{Value, json};
use uz_translit::{Apostrophe, Direction, Options, Script};

use super::*;
use crate::{Result, Session, Target};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "tools.translitCyrl",
            "To Cyrillic",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"slide\"|\"deck\", apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool}",
            has_doc,
            |s, p| run(s, p, Some(Direction::ToCyrillic))
        ),
        cmd!(
            "tools.translitLatn",
            "To Latin",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"slide\"|\"deck\", apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool}",
            has_doc,
            |s, p| run(s, p, Some(Direction::ToLatin))
        ),
        cmd!(
            "tools.translitAuto",
            "Transliterate (auto-detect)",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"slide\"|\"deck\", apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool}",
            has_doc,
            |s, p| run(s, p, None)
        ),
    ]
}

/// Where a text body lives on a slide (`shape: None` = the speaker notes).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Loc {
    shape: Option<u64>,
    cell: Option<(usize, usize)>,
}

type Pos = (usize, usize);

#[derive(Clone, PartialEq)]
enum Scope {
    /// The selected text: one body, from `.0` to `.1` (paragraph, char).
    Range(Loc, Pos, Pos),
    Shapes(Vec<ShapeId>),
    Slide,
    Deck,
}

#[derive(Default)]
struct Stats {
    paragraphs: usize,
    runs: usize,
    field: usize,
    /// Chars gained (or lost) in the last paragraph of a selected range, to keep the selection on the same text.
    end_delta: isize,
}

/// Visit the text bodies of a slide: the shapes in `only` (and what is inside them; every shape when `None`), then the notes.
fn walk(slide: &mut Slide, only: Option<&[ShapeId]>, notes: bool, f: &mut dyn FnMut(Loc, &mut TextBody)) {
    fn rec(shapes: &mut [deckcraft_model::Shape], only: Option<&[ShapeId]>, inside: bool, depth: usize, f: &mut dyn FnMut(Loc, &mut TextBody)) {
        if depth > 64 {
            return;
        }
        for sh in shapes {
            let hit = inside || only.is_none_or(|o| o.contains(&sh.id));
            let id = Some(u64::from(sh.id.0));
            if hit && let Some(t) = &mut sh.text {
                f(Loc { shape: id, cell: None }, t);
            }
            match &mut sh.kind {
                ShapeKind::Table(t) if hit => {
                    for (r, row) in t.rows.iter_mut().enumerate() {
                        for (c, cell) in row.cells.iter_mut().enumerate() {
                            f(Loc { shape: id, cell: Some((r, c)) }, &mut cell.text);
                        }
                    }
                }
                ShapeKind::Group { children, .. } => rec(children, only, hit, depth + 1, f),
                _ => {}
            }
        }
    }
    rec(&mut slide.shapes, only, false, 0, f);
    if notes {
        f(Loc { shape: None, cell: None }, &mut slide.notes);
    }
}

/// The paragraph's runs cut at the chars `from` and `to`, each piece flagged: converted (a plain text run inside the range) or kept.
fn pieces(p: &Paragraph, from: usize, to: usize, st: &mut Stats) -> Vec<(Run, bool)> {
    let mut out = Vec::with_capacity(p.runs.len() + 2);
    let mut off = 0;
    for r in &p.runs {
        let n = r.char_len();
        let (s, e) = (off, off + n);
        off = e;
        if r.kind != RunKind::Text {
            st.field += matches!(r.kind, RunKind::Field { .. } | RunKind::Math { .. }) as usize;
            out.push((r.clone(), false));
            continue;
        }
        let (a, b) = (from.clamp(s, e) - s, to.clamp(s, e) - s);
        let cut = |x: usize| r.text.char_indices().nth(x).map_or(r.text.len(), |(i, _)| i);
        let (ia, ib) = (cut(a), cut(b));
        for (range, convert) in [(0..ia, false), (ia..ib, true), (ib..r.text.len(), false)] {
            if !range.is_empty() {
                out.push((Run { text: r.text[range].to_string(), props: r.props.clone(), kind: RunKind::Text }, convert));
            }
        }
    }
    out
}

/// Convert one paragraph between the chars `from` and `to`. `None` direction = only report the convertible text (script detection).
fn para(p: &mut Paragraph, from: usize, to: usize, dir: Option<Direction>, opts: &Options, st: &mut Stats, sink: &mut String) -> Option<isize> {
    let parts = pieces(p, from, to, st);
    let Some(dir) = dir else {
        parts.iter().filter(|(_, c)| *c).for_each(|(r, _)| {
            sink.push_str(&r.text);
            sink.push('\n');
        });
        return None;
    };
    let runs: Vec<uz_translit::Run> = parts.iter().map(|(r, c)| uz_translit::Run { text: &r.text, convert: *c }).collect();
    let out: Vec<Option<String>> = uz_translit::convert_runs(&runs, dir, opts)
        .into_iter()
        .map(|c| match c {
            std::borrow::Cow::Owned(o) => Some(o),
            std::borrow::Cow::Borrowed(_) => None,
        })
        .collect();
    if out.iter().all(Option::is_none) {
        return None;
    }
    let mut delta = 0isize;
    let mut new = Vec::with_capacity(parts.len());
    for ((mut r, _), o) in parts.into_iter().zip(out) {
        if let Some(o) = o {
            delta += o.chars().count() as isize - r.text.chars().count() as isize;
            st.runs += 1;
            r.text = o;
        }
        new.push(r);
    }
    p.runs = new;
    p.normalize();
    st.paragraphs += 1;
    Some(delta)
}

/// Run `para` over the paragraphs of a body: all of them, or the selected range of it.
fn body(t: &mut TextBody, range: Option<(Pos, Pos)>, dir: Option<Direction>, opts: &Options, st: &mut Stats, sink: &mut String) {
    for (pi, p) in t.paragraphs.iter_mut().enumerate() {
        let (from, to) = match range {
            None => (0, usize::MAX),
            Some((a, b)) if pi >= a.0 && pi <= b.0 => (if pi == a.0 { a.1 } else { 0 }, if pi == b.0 { b.1 } else { usize::MAX }),
            Some(_) => continue,
        };
        let delta = para(p, from, to, dir, opts, st, sink);
        if range.is_some_and(|(_, b)| b.0 == pi) {
            st.end_delta = delta.unwrap_or(0);
        }
    }
}

/// Milliseconds; wasm has no `SystemTime`.
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64() * 1e3
    }
}

fn run(s: &mut Session, p: &Value, dir: Option<Direction>) -> Result<Value> {
    let id = "tools.translit";
    let asked = str_param(p, "scope").unwrap_or("auto");
    if !["auto", "selection", "slide", "deck"].contains(&asked) {
        return Err(bad(id, format!("`scope` must be auto, selection, slide or deck, not `{asked}`")));
    }
    let apostrophe = match str_param(p, "apostrophe") {
        None | Some("official") => Apostrophe::Official,
        Some("ascii") => Apostrophe::Ascii,
        Some(o) => return Err(bad(id, format!("`apostrophe` must be official or ascii, not `{o}`"))),
    };
    let defaults = Options::default();
    let opts = Options { apostrophe, foreign_latin_guard: bool_or(p, "foreignGuard", defaults.foreign_latin_guard), ..defaults };

    let t0 = now_ms();
    let st = s.doc()?;
    let sel = &st.selection;
    let range = sel.text.as_ref().filter(|t| t.is_range()).map(|t| {
        let (a, b) = t.ordered();
        Scope::Range(Loc { shape: (!t.notes).then(|| u64::from(t.shape.0)), cell: t.cell }, a, b)
    });
    let masters = sel.target != Target::Slides;
    let scope = match asked {
        "slide" => Scope::Slide,
        "deck" => Scope::Deck,
        "selection" => range.or_else(|| (!sel.shapes.is_empty()).then(|| Scope::Shapes(sel.shapes.clone()))).unwrap_or(Scope::Shapes(vec![])),
        _ => range.or_else(|| (!sel.shapes.is_empty()).then(|| Scope::Shapes(sel.shapes.clone()))).unwrap_or(Scope::Deck),
    };
    // A selection inside a master or layout is not on a slide: nothing to do (shortcut: Slide Master view, add when asked).
    let scope = if masters && matches!(scope, Scope::Range(..) | Scope::Shapes(_)) { Scope::Shapes(vec![]) } else { scope };
    let doc = st.doc.clone();
    let slides: Vec<usize> = match scope {
        Scope::Deck => (0..doc.slides.len()).collect(),
        _ => vec![sel.slide].into_iter().filter(|i| *i < doc.slides.len()).collect(),
    };
    let (only, notes, range): (Option<&[ShapeId]>, bool, Option<(Loc, Pos, Pos)>) = match &scope {
        Scope::Range(l, a, b) => (None, true, Some((*l, *a, *b))),
        Scope::Shapes(v) => (Some(v.as_slice()), false, None),
        _ => (None, true, None),
    };
    let visit = |sl: &mut Slide, dir: Option<Direction>, stats: &mut Stats, sink: &mut String| {
        walk(sl, only, notes, &mut |loc, t| match range {
            Some((l, a, b)) if l == loc => body(t, Some((a, b)), dir, &opts, stats, sink),
            Some(_) => {}
            None => body(t, None, dir, &opts, stats, sink),
        });
    };
    let mut stats = Stats::default();
    let dir = dir.or_else(|| {
        let mut text = String::new();
        for &i in &slides {
            visit(&mut (*doc.slides[i]).clone(), None, &mut Stats::default(), &mut text);
        }
        uz_translit::detect(&text).map(|sc| if sc == Script::Latin { Direction::ToCyrillic } else { Direction::ToLatin })
    });
    let mut changed: Vec<(usize, Slide)> = Vec::new();
    if dir.is_some() {
        for &i in &slides {
            let mut sl = (*doc.slides[i]).clone();
            let before = stats.paragraphs;
            visit(&mut sl, dir, &mut stats, &mut String::new());
            if stats.paragraphs > before {
                changed.push((i, sl));
            }
        }
    }
    let t1 = now_ms();
    if !changed.is_empty() {
        let end = range.map(|(_, _, b)| b);
        let delta = stats.end_delta;
        s.edit(|doc, sel| {
            for (i, sl) in changed {
                if let Some(slot) = doc.slides.get_mut(i) {
                    *slot = Arc::new(sl);
                }
            }
            // The selection keeps covering the same text: its later end moves by the chars the last paragraph gained.
            if let (Some(t), Some(end)) = (sel.text.as_mut(), end) {
                for pos in [&mut t.anchor, &mut t.caret] {
                    if *pos == end {
                        pos.1 = pos.1.saturating_add_signed(delta);
                    }
                }
            }
            Ok(())
        })?;
    }
    let t2 = now_ms();
    Ok(json!({
        "direction": match dir { Some(Direction::ToCyrillic) => "cyrl", Some(Direction::ToLatin) => "latn", None => "none" },
        "scope": match scope { Scope::Range(..) | Scope::Shapes(_) => "selection", Scope::Slide => "slide", Scope::Deck => "deck" },
        "tracked": false,
        "paragraphs": stats.paragraphs,
        "runs": stats.runs,
        "skipped": {"lang": 0, "field": stats.field, "locked": 0},
        "ms": {"convert": t1 - t0, "apply": t2 - t1},
        "rulesVersion": uz_translit::RULES_VERSION,
    }))
}
