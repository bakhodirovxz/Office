//! Uzbek Latin <-> Cyrillic transliteration for the wordcraft engine (Phase 2 WP4, PLAN section 3).
//!
//! `install(&mut Session)` appends three commands to the session's registry: `tools.translitCyrl`, `tools.translitLatn` and
//! `tools.translitAuto` (the direction follows the dominant script of the scope). The apps call it through the carried hook
//! `patches/wordcraft/0120`; everything else lives here.
//!
//! One `Session::run` is one undo step. The command rewrites `Paragraph.text` once per paragraph and keeps every `Run.props`
//! (only `Run.len` changes), so formatting, language tags and hyperlink targets are untouched by construction. Field codes and
//! other inline objects (U+FFFC) are never part of a converted slice. The rules come from `uz-translit` (`convert_runs`).
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use serde_json::{Value, json};
use uz_translit::{Apostrophe, Direction, LangPolicy, Options, Script, lang_converts};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{InlineObject, Path, Pos, RevisionKind, StoryRef};
use wordcraft_engine::cmd::{new_revision, selected_paths};
use wordcraft_engine::{CmdError, CmdResult, CommandSpec, Registry, Session};

/// The command ids. `column.rs` (patch 0120) lists the first two so that a column selection converts row by row.
pub const TO_CYRILLIC: &str = "tools.translitCyrl";
pub const TO_LATIN: &str = "tools.translitLatn";
pub const AUTO: &str = "tools.translitAuto";

const PARAMS: &str = r#"{"scope"?: "auto"|"selection"|"document", "skipLangs"?: bool, "fieldResults"?: bool, "apostrophe"?: "official"|"ascii", "foreignGuard"?: bool}"#;

/// The specs, for callers that build a registry themselves.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(TO_CYRILLIC, "To Cyrillic", "Review › Language", |s, v| {
            run(s, v, Some(Direction::ToCyrillic))
        })
        .params(PARAMS),
        CommandSpec::new(TO_LATIN, "To Latin", "Review › Language", |s, v| {
            run(s, v, Some(Direction::ToLatin))
        })
        .params(PARAMS),
        CommandSpec::new(
            AUTO,
            "Transliterate (auto-detect)",
            "Review › Language",
            |s, v| run(s, v, None),
        )
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Auto,
    Selection,
    Document,
}

struct Ctx {
    opts: Options,
    policy: LangPolicy,
    field_results: bool,
    tracked: bool,
}

#[derive(Default)]
struct Stats {
    paragraphs: usize,
    runs: usize,
    lang: usize,
    field: usize,
    locked: usize,
    convert_ms: f64,
    apply_ms: f64,
}

/// A piece of one run: `text[range]`, converted or kept as typed.
struct Seg {
    range: Range<usize>,
    run: usize,
    convert: bool,
    new: Option<String>,
    /// Tracked mode: the old text becomes a tracked deletion and the new text a tracked insertion after it.
    replace: bool,
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
    let defaults = Options::default();
    let opts = Options {
        apostrophe,
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
    let use_selection = match scope {
        Scope::Selection => true,
        Scope::Document => false,
        Scope::Auto => !s.sel.is_collapsed(),
    };
    if use_selection && s.sel.is_collapsed() {
        return Ok(summary(&Stats::default(), dir, true, &ctx));
    }
    // story -> (path -> byte bounds); `None` bounds mean every paragraph of the story, whole.
    let mut targets: Vec<(StoryRef, Option<BTreeMap<Path, (usize, usize)>>)> = Vec::new();
    if use_selection {
        let (a, b) = s.sel.ordered();
        let mut m = BTreeMap::new();
        for path in selected_paths(s) {
            let Some(p) = s.doc.para(a.story, &path) else {
                continue;
            };
            let from = if path == a.path { a.off } else { 0 };
            let to = if path == b.path {
                b.off.min(p.len())
            } else {
                p.len()
            };
            if from < to {
                m.insert(path, (from, to));
            }
        }
        targets.push((a.story, Some(m)));
    } else {
        targets.push((StoryRef::Body, None));
        targets.extend(s.doc.parts.keys().map(|k| (StoryRef::Part(*k), None)));
    }
    let dir = match dir {
        Some(d) => Some(d),
        None => detect(s, &targets),
    };
    let mut st = Stats::default();
    if let Some(dir) = dir {
        for (story, bounds) in &targets {
            story_pass(s, *story, bounds.as_ref(), dir, &ctx, &mut st)?;
        }
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
fn detect(
    s: &Session,
    targets: &[(StoryRef, Option<BTreeMap<Path, (usize, usize)>>)],
) -> Option<Direction> {
    let mut text = String::new();
    for (story, bounds) in targets {
        for path in s.doc.para_paths(*story) {
            let Some(p) = s.doc.para(*story, &path) else {
                continue;
            };
            let (from, to) = match bounds {
                None => (0, p.len()),
                Some(m) => match m.get(&path) {
                    Some(b) => *b,
                    None => continue,
                },
            };
            if let Some(t) = p.text.get(from..to) {
                text.push_str(t);
                text.push('\n');
            }
        }
    }
    match uz_translit::detect(&text)? {
        Script::Latin => Some(Direction::ToCyrillic),
        Script::Cyrillic => Some(Direction::ToLatin),
    }
}

/// Convert the paragraphs of one story (document order, so that field results are tracked across paragraphs).
fn story_pass(
    s: &mut Session,
    story: StoryRef,
    bounds: Option<&BTreeMap<Path, (usize, usize)>>,
    dir: Direction,
    ctx: &Ctx,
    st: &mut Stats,
) -> Result<(), CmdError> {
    // Locked content controls: (content start, end) positions.
    let locked: Vec<(Pos, Pos)> = s
        .doc
        .control_ranges(story)
        .into_iter()
        .filter(|r| !r.control.lock.can_edit())
        .map(|r| (r.content_start(), r.end))
        .collect();
    let mut depth = 0usize; // open FieldStart markers
    let mut revs: Option<(u32, u32)> = None;
    for path in s.doc.para_paths(story) {
        let own = match bounds {
            None => Some((0, usize::MAX)),
            Some(m) => m.get(&path).copied(),
        };
        if own.is_none() && ctx.field_results {
            continue;
        }
        let t0 = wordcraft_layout::now_ms();
        let Some(p) = s.doc.para(story, &path) else {
            continue;
        };
        let field_spans = if ctx.field_results {
            Vec::new()
        } else {
            field_spans(p, &mut depth)
        };
        let Some((from, to)) = own else { continue };
        let to = to.min(p.len());
        if !p.text.is_char_boundary(from)
            || !p.text.is_char_boundary(to)
            || p.runs.iter().map(|r| r.len).sum::<usize>() != p.text.len()
        {
            continue;
        }
        // Spans that must stay as typed.
        let mut keep: Vec<Range<usize>> = Vec::new();
        keep.push(0..from);
        keep.push(to..p.len());
        for (cs, e) in &locked {
            if path < cs.path || path > e.path {
                continue;
            }
            let a = if path == cs.path { cs.off } else { 0 };
            let b = if path == e.path { e.off } else { p.len() };
            if a < b {
                st.locked += 1;
                keep.push(a..b.min(p.len()));
            }
        }
        st.field += field_spans.len();
        keep.extend(field_spans);
        let mut segs = classify(s, p, &keep, ctx, st);
        // Convert.
        let runs: Vec<uz_translit::Run> = segs
            .iter()
            .map(|g| uz_translit::Run {
                text: &p.text[g.range.clone()],
                convert: g.convert,
            })
            .collect();
        let out = uz_translit::convert_runs(&runs, dir, &ctx.opts);
        let mut changed = false;
        let outs: Vec<Option<String>> = out
            .into_iter()
            .map(|c| match c {
                std::borrow::Cow::Owned(o) => {
                    changed = true;
                    Some(o)
                }
                std::borrow::Cow::Borrowed(_) => None,
            })
            .collect();
        for (g, o) in segs.iter_mut().zip(outs) {
            g.new = o;
        }
        let t1 = wordcraft_layout::now_ms();
        st.convert_ms += t1 - t0;
        if !changed {
            continue;
        }
        // Tracked changes: a changed stretch of someone else's (or original) text is replaced as a deletion plus an insertion.
        if ctx.tracked {
            for g in segs.iter_mut().filter(|g| g.new.is_some()) {
                let c = p.props_of_char(g.range.start);
                let own_ins = c
                    .ins
                    .and_then(|i| s.doc.revisions.get(i as usize))
                    .is_some_and(|rv| rv.author == s.author);
                g.replace = !own_ins;
            }
        }
        if segs.iter().any(|g| g.replace) && revs.is_none() {
            revs = Some((
                new_revision(s, RevisionKind::Delete),
                new_revision(s, RevisionKind::Insert),
            ));
        }
        let spans = apply(s, story, &path, &segs, revs)?;
        st.paragraphs += 1;
        st.runs += segs.iter().filter(|g| g.new.is_some()).count();
        // A selection or caret in this paragraph keeps covering the same text.
        for end in [&mut s.sel.anchor, &mut s.sel.focus] {
            if end.story == story && end.path == path {
                end.off = map_offset(&spans, end.off);
            }
        }
        st.apply_ms += wordcraft_layout::now_ms() - t1;
    }
    Ok(())
}

/// Split the paragraph's runs into pieces (never across a run, an object, or a `keep` boundary) and mark which are converted.
fn classify(
    s: &Session,
    p: &wordcraft_doc::Paragraph,
    keep: &[Range<usize>],
    ctx: &Ctx,
    st: &mut Stats,
) -> Vec<Seg> {
    let n = p.len();
    let mut skip = vec![false; n];
    for r in keep {
        let (a, b) = (r.start.min(n), r.end.min(n));
        if a < b {
            skip[a..b].fill(true);
        }
    }
    for (i, c) in p.text.char_indices() {
        if c == OBJ {
            skip[i..i + c.len_utf8()].fill(true);
        }
    }
    let mut segs = Vec::with_capacity(p.runs.len() + 2);
    for (ri, (range, props)) in p.run_ranges().enumerate() {
        if range.is_empty() {
            continue;
        }
        // A tracked deletion is not text any more; a run in a skipped language stays as typed.
        let lang_skip = props.del.is_none()
            && !lang_converts(
                s.doc
                    .styles
                    .resolve_char(p.props.style.as_deref(), props)
                    .lang
                    .as_deref(),
                ctx.policy,
            );
        if lang_skip {
            st.lang += 1;
        }
        let whole_skip = props.del.is_some() || lang_skip;
        let mut start = range.start;
        let mut cur = whole_skip || skip[start];
        for (i, _) in p.text[range.clone()].char_indices() {
            let at = range.start + i;
            let flag = whole_skip || skip[at];
            if flag != cur {
                segs.push(Seg {
                    range: start..at,
                    run: ri,
                    convert: !cur,
                    new: None,
                    replace: false,
                });
                start = at;
                cur = flag;
            }
        }
        segs.push(Seg {
            range: start..range.end,
            run: ri,
            convert: !cur,
            new: None,
            replace: false,
        });
    }
    segs
}

/// Byte spans of field result text in `p`, given the number of fields still open from earlier paragraphs.
fn field_spans(p: &wordcraft_doc::Paragraph, depth: &mut usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut open = (*depth > 0).then_some(0usize);
    let mut k = 0;
    for (i, c) in p.text.char_indices() {
        if c != OBJ {
            continue;
        }
        match p.objects.get(k) {
            Some(InlineObject::FieldStart { .. }) => {
                if *depth == 0 {
                    open = Some(i + c.len_utf8());
                }
                *depth += 1;
            }
            Some(InlineObject::FieldEnd) if *depth > 0 => {
                *depth -= 1;
                if *depth == 0
                    && let Some(a) = open.take()
                {
                    out.push(a..i);
                }
            }
            _ => {}
        }
        k += 1;
    }
    if let Some(a) = open {
        out.push(a..p.len());
    }
    out
}

/// Write the converted pieces back. Returns `(start, end, new_len)` of every changed stretch, in text order.
fn apply(
    s: &mut Session,
    story: StoryRef,
    path: &Path,
    segs: &[Seg],
    revs: Option<(u32, u32)>,
) -> Result<Vec<(usize, usize, usize)>, CmdError> {
    let para = s.doc.para_mut(story, path)?;
    let mut spans = Vec::new();
    if !segs.iter().any(|g| g.replace) {
        // One pass: rebuild the text, keep every run's props, only the run lengths change (Cyrillic is 2 bytes a letter).
        let mut text = String::with_capacity(para.text.len() + para.text.len() / 4);
        let mut lens = vec![0usize; para.runs.len()];
        for g in segs {
            let piece = match &g.new {
                Some(n) => n.as_str(),
                None => &para.text[g.range.clone()],
            };
            if let Some(l) = lens.get_mut(g.run) {
                *l += piece.len();
            }
            if let Some(n) = &g.new {
                spans.push((g.range.start, g.range.end, n.len()));
            }
            text.push_str(piece);
        }
        para.text = text;
        for (r, l) in para.runs.iter_mut().zip(lens) {
            r.len = l;
        }
        para.normalize();
        para.touch();
        return Ok(spans);
    }
    // Tracked: right to left so that earlier offsets hold.
    let (del, ins) =
        revs.ok_or_else(|| CmdError::Failed("tracked conversion without revisions".into()))?;
    let mut rev_spans: Vec<(usize, usize, usize)> = Vec::new();
    for g in segs.iter().rev().filter(|g| g.new.is_some()) {
        let new = g.new.as_deref().unwrap_or_default();
        let (start, end) = (g.range.start, g.range.end);
        if g.replace {
            let mut props = para.props_of_char(start).clone();
            para.format(start, end, &|c| c.del = Some(del))?;
            (props.ins, props.del, props.fmt_change) = (Some(ins), None, None);
            para.insert_text(end, new, &props)?;
            rev_spans.push((start, end, end - start + new.len()));
        } else {
            let props = para.props_of_char(start).clone();
            para.delete(start, end)?;
            para.insert_text(start, new, &props)?;
            rev_spans.push((start, end, new.len()));
        }
    }
    rev_spans.reverse();
    para.touch();
    Ok(rev_spans)
}

/// Where `off` lands after the stretches `(start, end, new_len)` (text order) were replaced; an offset inside a stretch moves to its end.
fn map_offset(spans: &[(usize, usize, usize)], off: usize) -> usize {
    let mut out = off;
    for &(start, end, new_len) in spans {
        if end <= off {
            out = out.saturating_sub(end - start).saturating_add(new_len);
        } else if start < off {
            out = out.saturating_sub(off - start).saturating_add(new_len);
        }
    }
    out
}
