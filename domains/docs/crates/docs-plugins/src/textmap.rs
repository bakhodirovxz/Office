//! The text map walker (Phase 7 WP3 step 1; PLAN section 6.1). It was `docs-translit`'s walker (targets, `story_pass`, `classify`,
//! `field_spans`, `apply`, `map_offset`) and moved here with no change in behaviour: the built-in commands call it with their
//! converter, and a plugin's text map is read and applied through the same walk, so a plugin sees the same segments the built-in
//! converts and its edits go through the same `apply` (tracked changes included).
//!
//! One `walk` visits the paragraphs of a scope in document order. For each it classifies the runs into [`Seg`]s (never across a run,
//! an object, or a span that must stay as typed), hands them to the caller's converter, and writes back what the converter changed.
use std::collections::BTreeMap;
use std::ops::Range;

use wordcraft_doc::para::OBJ;
use wordcraft_doc::{InlineObject, Path, Pos, RevisionKind, StoryRef};
use wordcraft_engine::cmd::{new_revision, selected_paths};
use wordcraft_engine::{CmdError, Session};

use plugin_api::textmap as wire;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Auto,
    Selection,
    Document,
}

/// Story and, for a selection, the byte bounds per paragraph path; `None` bounds mean every paragraph of the story, whole.
pub type Targets = Vec<(StoryRef, Option<BTreeMap<Path, (usize, usize)>>)>;

#[derive(Default)]
pub struct Stats {
    pub paragraphs: usize,
    pub runs: usize,
    pub lang: usize,
    pub field: usize,
    pub locked: usize,
    pub convert_ms: f64,
    pub apply_ms: f64,
}

/// A piece of one run: `text[range]`, converted or kept as typed.
pub struct Seg {
    pub range: Range<usize>,
    pub run: usize,
    pub convert: bool,
    /// Why a kept piece stays as typed (`plugin_api::textmap` reason bits; 0 for a piece kept by language or a deletion).
    pub why: u8,
    /// A tracked deletion: left out of the converter's input, so the text around it reads as one word.
    pub deleted: bool,
    pub new: Option<String>,
    /// Tracked mode: the old text becomes a tracked deletion and the new text a tracked insertion after it.
    pub replace: bool,
}

pub struct Opts<'a> {
    pub field_results: bool,
    pub tracked: bool,
    /// Does text in this language get converted? (`classify` counts the runs it says no to.)
    pub lang_ok: &'a dyn Fn(Option<&str>) -> bool,
}

/// What a walk hands the caller per paragraph: the session (read only), the paragraph and its pieces. The converter sets `Seg::new`
/// and returns whether it changed anything.
pub type Convert<'a> = dyn FnMut(&Session, &wordcraft_doc::Paragraph, &mut [Seg]) -> bool + 'a;

/// The scope as targets: the selection if asked for, or if it is not collapsed; otherwise the whole document. The flag says whether
/// the selection is used; `None` targets mean a selection was asked for but the caret is collapsed (nothing to do).
pub fn targets(s: &Session, scope: Scope) -> (bool, Option<Targets>) {
    let use_selection = match scope {
        Scope::Selection => true,
        Scope::Document => false,
        Scope::Auto => !s.sel.is_collapsed(),
    };
    if use_selection && s.sel.is_collapsed() {
        return (use_selection, None);
    }
    let mut targets: Targets = Vec::new();
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
    (use_selection, Some(targets))
}

/// The text of the targets, one paragraph per line (what script detection reads).
pub fn text_of(s: &Session, targets: &Targets) -> String {
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
    text
}

/// Visit every target story (see the module docs).
pub fn walk(
    s: &mut Session,
    targets: &Targets,
    o: &Opts,
    st: &mut Stats,
    f: &mut Convert,
) -> Result<(), CmdError> {
    for (story, bounds) in targets {
        story_pass(s, *story, bounds.as_ref(), o, st, f)?;
    }
    Ok(())
}

/// Convert the paragraphs of one story (document order, so that field results are tracked across paragraphs).
fn story_pass(
    s: &mut Session,
    story: StoryRef,
    bounds: Option<&BTreeMap<Path, (usize, usize)>>,
    o: &Opts,
    st: &mut Stats,
    f: &mut Convert,
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
        if own.is_none() && o.field_results {
            continue;
        }
        let t0 = wordcraft_layout::now_ms();
        let Some(p) = s.doc.para(story, &path) else {
            continue;
        };
        let field_spans = if o.field_results {
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
        let mut keep: Vec<(Range<usize>, u8)> = Vec::new();
        keep.push((0..from, wire::OUTSIDE_SELECTION));
        keep.push((to..p.len(), wire::OUTSIDE_SELECTION));
        for (cs, e) in &locked {
            if path < cs.path || path > e.path {
                continue;
            }
            let a = if path == cs.path { cs.off } else { 0 };
            let b = if path == e.path { e.off } else { p.len() };
            if a < b {
                st.locked += 1;
                keep.push((a..b.min(p.len()), wire::LOCKED_CONTROL));
            }
        }
        st.field += field_spans.len();
        keep.extend(field_spans.into_iter().map(|r| (r, wire::FIELD_RESULT)));
        let mut segs = classify(s, p, &keep, o, st);
        // Convert.
        let changed = f(&*s, p, &mut segs);
        let t1 = wordcraft_layout::now_ms();
        st.convert_ms += t1 - t0;
        if !changed {
            continue;
        }
        // Tracked changes: a changed stretch of someone else's (or original) text is replaced as a deletion plus an insertion.
        if o.tracked {
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
    keep: &[(Range<usize>, u8)],
    o: &Opts,
    st: &mut Stats,
) -> Vec<Seg> {
    let n = p.len();
    let mut skip = vec![0u8; n];
    for (r, why) in keep {
        let (a, b) = (r.start.min(n), r.end.min(n));
        if a < b {
            for x in &mut skip[a..b] {
                *x |= why;
            }
        }
    }
    for (i, c) in p.text.char_indices() {
        if c == OBJ {
            for x in &mut skip[i..i + c.len_utf8()] {
                *x |= wire::OBJECT;
            }
        }
    }
    // Reason bits of a kept piece; asked for kept pieces only, so the converting path pays nothing.
    let why_of = |a: usize, b: usize| skip[a..b].iter().fold(0u8, |m, x| m | x);
    let mut segs = Vec::with_capacity(p.runs.len() + 2);
    for (ri, (range, props)) in p.run_ranges().enumerate() {
        if range.is_empty() {
            continue;
        }
        // A tracked deletion is not text any more; a run in a skipped language stays as typed.
        let lang_skip = props.del.is_none()
            && !(o.lang_ok)(
                s.doc
                    .styles
                    .resolve_char(p.props.style.as_deref(), props)
                    .lang
                    .as_deref(),
            );
        if lang_skip {
            st.lang += 1;
        }
        let whole_skip = props.del.is_some() || lang_skip;
        let mut start = range.start;
        let mut cur = whole_skip || skip[start] != 0;
        for (i, _) in p.text[range.clone()].char_indices() {
            let at = range.start + i;
            let flag = whole_skip || skip[at] != 0;
            if flag != cur {
                segs.push(Seg {
                    range: start..at,
                    run: ri,
                    convert: !cur,
                    why: if cur { why_of(start, at) } else { 0 },
                    deleted: props.del.is_some(),
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
            why: if cur { why_of(start, range.end) } else { 0 },
            deleted: props.del.is_some(),
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
