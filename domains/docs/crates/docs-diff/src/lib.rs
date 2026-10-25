//! Diff of two Word documents (PLAN phase 5, section 6, WP12). The output schema is in `docs/phase5/PLAN.md` §6.1.
//!
//! The body (and the paragraphs inside its tables, in reading order) is flattened into units, one per paragraph, as
//! the text reads with every tracked change accepted. Then:
//! 1. units are aligned on a hash of that text with [`seq_diff::matches`] (patience anchors, bounded Myers in between);
//! 2. a deleted and an inserted unit with the same non-trivial text anywhere in the document are a **move**;
//! 3. inside each gap the rest are paired by word similarity ([`seq_diff::pair`], Dice ≥ [`PAIR_MIN`]; a gap of one
//!    deleted and one inserted unit is always a pair) as **modified**, with a word-level diff of the pair; what stays unpaired is **inserted** or **deleted**;
//! 4. aligned units with the same text but different paragraph or run formatting are **format-only** changes, and the
//!    equal stretches of moved and modified pairs are checked for run formatting the same way.
//!
//! Blame: the authors of the tracked changes (`w:ins`, `w:del`, `w:rPrChange`, `w:pPrChange`) in the new version's
//! paragraph; for a deletion, the authors of the fully tracked-deleted paragraphs left in that place.
use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::{Map, Value, json};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Block, Blocks, Document, Paragraph};

/// Output schema version.
pub const SCHEMA: u32 = 1;
/// Least word similarity (Dice coefficient) for an edited paragraph to pair with its old version.
pub const PAIR_MIN: f64 = 0.5;
/// Largest gap (deleted × inserted units) paired by similarity; a bigger gap is a rewrite (deleted, then inserted).
pub const PAIR_CAP: usize = 250_000;
/// Fewest characters (trimmed) for equal deleted and inserted text to count as a move.
pub const MOVE_MIN_CHARS: usize = 3;

/// One paragraph as it reads with the tracked changes accepted.
struct Unit {
    /// Block path: `12` (body block 12), `5/r1c2/0` (block 0 of row 1, cell 2 of the table at body block 5).
    path: String,
    text: String,
    /// Run formatting over `text`: (end byte, index into [`Side::fmts`]), adjacent equal formats merged.
    segs: Vec<(usize, usize)>,
    /// Paragraph formatting (JSON of `ParaProps::formatting`).
    para: String,
    key: u64,
    fkey: u64,
    authors: BTreeSet<String>,
}

#[derive(Default)]
struct Side {
    units: Vec<Unit>,
    /// Authors of the fully tracked-deleted paragraphs just before unit `i` (`units.len()` = after the last).
    ghosts: Vec<BTreeSet<String>>,
    fmts: Vec<String>,
    index: HashMap<String, usize>,
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

impl Side {
    fn new(doc: &Document) -> Side {
        let mut s = Side { ghosts: vec![BTreeSet::new()], ..Side::default() };
        s.walk(doc, &doc.body, "");
        s
    }

    fn walk(&mut self, doc: &Document, blocks: &Blocks, prefix: &str) {
        for (bi, b) in blocks.iter().enumerate() {
            match &**b {
                Block::Para(p) => self.para(doc, p, format!("{prefix}{bi}")),
                Block::Table(t) => {
                    for (ri, row) in t.rows.iter().enumerate() {
                        for (ci, cell) in row.cells.iter().enumerate() {
                            self.walk(doc, &cell.blocks, &format!("{prefix}{bi}/r{ri}c{ci}/"));
                        }
                    }
                }
            }
        }
    }

    fn intern(&mut self, f: String) -> usize {
        if let Some(&i) = self.index.get(&f) {
            return i;
        }
        self.fmts.push(f.clone());
        self.index.insert(f, self.fmts.len() - 1);
        self.fmts.len() - 1
    }

    fn para(&mut self, doc: &Document, p: &Paragraph, path: String) {
        let mut authors = BTreeSet::new();
        let mut blame = |rev: Option<u32>| {
            if let Some(r) = rev.and_then(|i| doc.revisions.get(i as usize))
                && !r.author.is_empty()
            {
                authors.insert(r.author.clone());
            }
        };
        blame(p.mark.ins);
        blame(p.mark.del);
        blame(p.props.fmt_change.as_ref().map(|c| c.rev));
        let (mut text, mut segs, mut k) = (String::new(), Vec::<(usize, usize)>::new(), 0);
        for (range, props) in p.run_ranges() {
            blame(props.ins);
            blame(props.del);
            blame(props.fmt_change.as_ref().map(|c| c.rev));
            let Some(s) = p.text.get(range) else { break };
            if props.del.is_some() {
                k += s.matches(OBJ).count();
                continue;
            }
            for c in s.chars() {
                if c == OBJ {
                    // an object without text (a picture) stays visible as U+FFFC
                    match p.objects.get(k).map(|o| o.plain_text()).filter(|t| !t.is_empty()) {
                        Some(t) => text.push_str(t),
                        None => text.push(OBJ),
                    }
                    k += 1;
                } else {
                    text.push(c);
                }
            }
            let f = self.intern(serde_json::to_string(&props.formatting()).unwrap_or_default());
            match segs.last_mut() {
                Some(last) if last.1 == f => last.0 = text.len(),
                _ if s.is_empty() => {}
                _ => segs.push((text.len(), f)),
            }
        }
        if text.is_empty() && (!p.text.is_empty() || p.mark.del.is_some()) {
            // deleted as a whole with tracking on: invisible once accepted, but it says who deleted here
            if let Some(g) = self.ghosts.last_mut() {
                g.extend(authors);
            }
            return;
        }
        let para = serde_json::to_string(&p.props.formatting()).unwrap_or_default();
        let fkey = hash(&(&para, segs.iter().map(|&(e, f)| (e, &self.fmts[f])).collect::<Vec<_>>()));
        self.units.push(Unit { path, key: hash(&text), text, segs, para, fkey, authors });
        self.ghosts.push(BTreeSet::new());
    }
}

/// Word, space run or single other character: byte ranges over `s`.
fn tokens(s: &str) -> Vec<(usize, usize)> {
    // apostrophes inside words: Uzbek oʻ/gʻ and the tutuq belgisi
    let word = |c: char| c.is_alphanumeric() || matches!(c, '\'' | 'ʻ' | 'ʼ' | '‘' | '’');
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let class = if word(c) {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        };
        let mut end = i + c.len_utf8();
        if class < 2 {
            while let Some(&(j, d)) = it.peek() {
                if (class == 0 && word(d)) || (class == 1 && d.is_whitespace()) {
                    end = j + d.len_utf8();
                    it.next();
                } else {
                    break;
                }
            }
        }
        out.push((i, end));
    }
    out
}

/// Sorted hashes of the words of `s` (for similarity).
fn word_bag(s: &str) -> Vec<u64> {
    let mut v: Vec<u64> = tokens(s).into_iter().map(|(a, b)| &s[a..b]).filter(|t| t.chars().any(char::is_alphanumeric)).map(hash).collect();
    v.sort_unstable();
    v
}

fn dice(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (mut i, mut j, mut common) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

/// `{key: [old, new]}` for the keys of two JSON objects (as strings) whose values differ.
fn changed_keys(a: &str, b: &str) -> Map<String, Value> {
    let parse = |s: &str| serde_json::from_str::<Map<String, Value>>(s).unwrap_or_default();
    let (a, b) = (parse(a), parse(b));
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .map(|k| (k.clone(), json!([a.get(k).cloned().unwrap_or(Value::Null), b.get(k).cloned().unwrap_or(Value::Null)])))
        .collect()
}

fn chars(s: &str, byte: usize) -> usize {
    s[..byte].chars().count()
}

/// Paragraph and run formatting changes between `a` and `b` over their equal stretches `(a byte, b byte, len)`.
fn format_changes(sa: &Side, ua: &Unit, sb: &Side, ub: &Unit, equal: &[(usize, usize, usize)]) -> Option<Value> {
    let at = |segs: &[(usize, usize)], pos: usize| segs.get(segs.partition_point(|s| s.0 <= pos)).copied();
    // (b start, b end, a format, b format)
    let mut runs: Vec<(usize, usize, usize, usize)> = Vec::new();
    for &(oa, ob, len) in equal {
        let mut p = 0;
        while p < len {
            let (Some((ea, fa)), Some((eb, fb))) = (at(&ua.segs, oa + p), at(&ub.segs, ob + p)) else { break };
            let step = (len - p).min(ea - (oa + p)).min(eb - (ob + p));
            if sa.fmts[fa] != sb.fmts[fb] {
                match runs.last_mut() {
                    Some(r) if r.1 == ob + p && sa.fmts[r.2] == sa.fmts[fa] && sb.fmts[r.3] == sb.fmts[fb] => r.1 += step,
                    _ => runs.push((ob + p, ob + p + step, fa, fb)),
                }
            }
            p += step;
        }
    }
    let mut out = Map::new();
    if ua.para != ub.para {
        out.insert("para".into(), Value::Object(changed_keys(&ua.para, &ub.para)));
    }
    if !runs.is_empty() {
        let t = &ub.text;
        let runs = runs
            .into_iter()
            .map(|(s, e, fa, fb)| {
                json!({"start": chars(t, s), "end": chars(t, e), "text": &t[s..e], "props": changed_keys(&sa.fmts[fa], &sb.fmts[fb])})
            })
            .collect();
        out.insert("runs".into(), Value::Array(runs));
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// Word-level diff of a modified pair: the spans and the equal stretches `(a byte, b byte, len)`.
fn word_diff(a: &str, b: &str) -> (Vec<Value>, Vec<(usize, usize, usize)>) {
    let (ta, tb) = (tokens(a), tokens(b));
    let ka: Vec<u64> = ta.iter().map(|&(s, e)| hash(&a[s..e])).collect();
    let kb: Vec<u64> = tb.iter().map(|&(s, e)| hash(&b[s..e])).collect();
    let m = seq_diff::matches(&ka, &kb);
    let mut spans: Vec<(&str, String)> = Vec::new();
    let mut equal: Vec<(usize, usize, usize)> = Vec::new();
    let mut push = |op: &'static str, s: &str| match spans.last_mut() {
        Some((o, t)) if *o == op => t.push_str(s),
        _ if s.is_empty() => {}
        _ => spans.push((op, s.to_string())),
    };
    let mut at = (0, 0);
    for &(i, j) in m.iter().chain(std::iter::once(&(ta.len(), tb.len()))) {
        let span = |t: &[(usize, usize)], r: std::ops::Range<usize>| if r.is_empty() { 0..0 } else { t[r.start].0..t[r.end - 1].1 };
        let (da, db) = (span(&ta, at.0..i), span(&tb, at.1..j));
        push("del", &a[da]);
        push("ins", &b[db]);
        if i < ta.len() {
            let ((sa, ea), (sb, _)) = (ta[i], tb[j]);
            push("eq", &a[sa..ea]);
            match equal.last_mut() {
                Some(e) if e.0 + e.2 == sa && e.1 + e.2 == sb => e.2 += ea - sa,
                _ => equal.push((sa, sb, ea - sa)),
            }
        }
        at = (i + 1, j + 1);
    }
    (spans.into_iter().map(|(op, t)| json!({ op: t })).collect(), equal)
}

fn authors(set: &BTreeSet<String>) -> Value {
    Value::Array(set.iter().map(|a| Value::String(a.clone())).collect())
}

/// Add `"authors"` when there are any.
fn with_authors(mut v: Value, set: &BTreeSet<String>) -> Value {
    if !set.is_empty()
        && let Some(o) = v.as_object_mut()
    {
        o.insert("authors".into(), authors(set));
    }
    v
}

/// The diff of `a` (old) and `b` (new) as the JSON document of PLAN §6.1.
pub fn diff(a: &Document, b: &Document) -> Value {
    let (sa, sb) = (Side::new(a), Side::new(b));
    let (ua, ub) = (&sa.units, &sb.units);
    let ka: Vec<u64> = ua.iter().map(|u| u.key).collect();
    let kb: Vec<u64> = ub.iter().map(|u| u.key).collect();
    let matched = seq_diff::matches(&ka, &kb);
    let gaps = seq_diff::gaps(&matched, ua.len(), ub.len());

    // moves: equal non-trivial text deleted in one place and inserted in another, first come first paired (each list holds
    // descending positions, so pop gives the first)
    let mut deleted: HashMap<u64, Vec<usize>> = HashMap::new();
    for (ga, _) in gaps.iter().rev() {
        for i in ga.clone().rev() {
            if ua[i].text.trim().chars().count() >= MOVE_MIN_CHARS {
                deleted.entry(ua[i].key).or_default().push(i);
            }
        }
    }
    let mut moved_to: HashMap<usize, usize> = HashMap::new(); // b -> a
    let mut moved_from: BTreeSet<usize> = BTreeSet::new();
    for (_, gb) in &gaps {
        for j in gb.clone() {
            if let Some(i) = deleted.get_mut(&ub[j].key).and_then(Vec::pop) {
                moved_to.insert(j, i);
                moved_from.insert(i);
            }
        }
    }

    let mut changes = Vec::new();
    let mut stats: [usize; 6] = [0; 6]; // insert, delete, modify, move, format, equal
    let emit_pair = |i: usize, j: usize, changes: &mut Vec<Value>, stats: &mut [usize; 6]| {
        let equal = [(0, 0, ua[i].text.len())];
        if ua[i].fkey == ub[j].fkey {
            stats[5] += 1;
        } else if let Some(f) = format_changes(&sa, &ua[i], &sb, &ub[j], &equal) {
            stats[4] += 1;
            let v = json!({"op": "format", "a": i, "b": j, "path": ub[j].path, "text": ub[j].text, "format": f});
            changes.push(with_authors(v, &ub[j].authors));
        } else {
            stats[5] += 1;
        }
    };
    let mut gi = gaps.iter().peekable();
    let mut mi = matched.iter().peekable();
    loop {
        // the gap before the next match (gaps and matches alternate in order)
        let next = mi.peek().copied();
        let gap = gi.next_if(|(ga, gb)| next.is_none_or(|&(i, j)| ga.end <= i && gb.end <= j));
        if let Some((ga, gb)) = gap {
            let ghost: BTreeSet<String> = sb.ghosts[gb.start..=gb.end].iter().flatten().cloned().collect();
            let dels: Vec<usize> = ga.clone().filter(|i| !moved_from.contains(i)).collect();
            let inss: Vec<usize> = gb.clone().filter(|j| !moved_to.contains_key(j)).collect();
            // one paragraph replaced by one in the same place is an edit, however different (a table cell, a short line)
            let pairs = if dels.len() == 1 && inss.len() == 1 {
                vec![(0, 0)]
            } else if dels.len() * inss.len() <= PAIR_CAP && !dels.is_empty() && !inss.is_empty() {
                let bags_a: Vec<Vec<u64>> = dels.iter().map(|&i| word_bag(&ua[i].text)).collect();
                let bags_b: Vec<Vec<u64>> = inss.iter().map(|&j| word_bag(&ub[j].text)).collect();
                seq_diff::pair(dels.len(), inss.len(), PAIR_MIN, |x, y| dice(&bags_a[x], &bags_b[y]))
            } else {
                Vec::new()
            };
            let mut x = 0;
            let del = |upto: usize, x: &mut usize, changes: &mut Vec<Value>, stats: &mut [usize; 6]| {
                while *x < upto {
                    let i = dels[*x];
                    stats[1] += 1;
                    changes.push(with_authors(json!({"op": "delete", "a": i, "path": ua[i].path, "text": ua[i].text}), &ghost));
                    *x += 1;
                }
            };
            // inserted and moved-here units in B order, before B position `upto`
            let ins = |upto: usize, y: &mut usize, changes: &mut Vec<Value>, stats: &mut [usize; 6]| {
                for (j, u) in ub.iter().enumerate().take(upto).skip(*y) {
                    if let Some(&i) = moved_to.get(&j) {
                        stats[3] += 1;
                        let mut v = json!({"op": "move", "a": i, "b": j, "from": ua[i].path, "path": u.path, "text": u.text});
                        if ua[i].fkey != u.fkey
                            && let Some(f) = format_changes(&sa, &ua[i], &sb, u, &[(0, 0, ua[i].text.len())])
                        {
                            v["format"] = f;
                        }
                        changes.push(with_authors(v, &u.authors));
                    } else {
                        stats[0] += 1;
                        changes.push(with_authors(json!({"op": "insert", "b": j, "path": u.path, "text": u.text}), &u.authors));
                    }
                }
                *y = upto;
            };
            let mut y = gb.start;
            for &(px, py) in &pairs {
                let (i, j) = (dels[px], inss[py]);
                del(px, &mut x, &mut changes, &mut stats);
                ins(j, &mut y, &mut changes, &mut stats);
                let (spans, equal) = word_diff(&ua[i].text, &ub[j].text);
                let mut v = json!({"op": "modify", "a": i, "b": j, "path": ub[j].path, "spans": spans});
                if let Some(f) = format_changes(&sa, &ua[i], &sb, &ub[j], &equal) {
                    v["format"] = f;
                }
                stats[2] += 1;
                changes.push(with_authors(v, &ub[j].authors));
                x = px + 1;
                y = j + 1;
            }
            del(dels.len(), &mut x, &mut changes, &mut stats);
            ins(gb.end, &mut y, &mut changes, &mut stats);
            continue;
        }
        match mi.next() {
            Some(&(i, j)) => emit_pair(i, j, &mut changes, &mut stats),
            None => break,
        }
    }
    json!({
        "tool": "docs-diff",
        "schema": SCHEMA,
        "a": {"paragraphs": ua.len()},
        "b": {"paragraphs": ub.len()},
        "stats": {"inserted": stats[0], "deleted": stats[1], "modified": stats[2], "moved": stats[3], "format": stats[4], "equal": stats[5]},
        "changes": changes,
    })
}

/// Read a `.docx`/`.docm`/`.dotx`/`.dotm` package.
pub fn read(bytes: &[u8]) -> Result<Document, String> {
    wordcraft_docx::read(bytes).map_err(|e| e.to_string())
}
