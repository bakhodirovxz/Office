//! Diff of two workbooks (PLAN phase 5, section 6, WP12). The output schema is in `docs/phase5/PLAN.md` §6.2.
//!
//! Sheets are matched by name; an unmatched pair with the same cells is a rename. In each matched sheet:
//! 1. columns are aligned on a hash of their cells ([`seq_diff::matches`]). The alignment is used only when the column
//!    counts differ and at least half the columns match exactly; otherwise columns pair by position. So an inserted
//!    or deleted column is reported once, not as every cell to its right;
//! 2. rows are aligned the same way on a hash of their cells over the paired columns, so inserted and deleted rows
//!    are found; rows left in a gap pair by position when as many were replaced, else by cell similarity
//!    ([`seq_diff::pair`]); unpaired rows and columns are reported as inserted or deleted runs, not cell by cell;
//! 3. the cells of paired rows that differ are listed with their old and new value, formula and a format flag, then
//!    the cells of the inserted and the deleted rows and columns (with only a new or only an old side).
//!
//! Formulas are compared in R1C1 form, so `=A3*2` moved down a row to `=A4*2` by an inserted row is not a change; a
//! formula's cached result is shown but not compared.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use gridcraft_core::{CellRef, Value, col_to_letters};
use gridcraft_model::{Cell, Formula, Sheet, Workbook};
use serde_json::{Value as Json, json};

/// Output schema version.
pub const SCHEMA: u32 = 1;
/// Most cells listed per sheet; the rest are counted in `"truncated"`.
pub const MAX_CELLS: usize = 10_000;
/// Largest gap (rows or columns, deleted × inserted) paired by similarity; a larger one pairs by position.
pub const PAIR_CAP: usize = 40_000;
/// Least share of equal cells for two rows or columns in a gap to be the same one edited.
pub const PAIR_MIN: f64 = 0.5;

fn mix(h: u64, x: u64) -> u64 {
    (h.rotate_left(5) ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

struct Keyer<'w> {
    wb: &'w Workbook,
    styles: HashMap<u32, u64>,
    /// R1C1 hash per shared formula tree and the cell it reads from (see [`Keyer::formula`])
    formulas: HashMap<(usize, CellRef), u64>,
}

impl<'w> Keyer<'w> {
    fn new(wb: &'w Workbook) -> Self {
        Keyer { wb, styles: HashMap::new(), formulas: HashMap::new() }
    }
    fn style(&mut self, cell: &Cell) -> u64 {
        let wb = self.wb;
        *self.styles.entry(cell.style.0).or_insert_with(|| hash(wb.styles.get(cell.style)))
    }
    /// A formula in R1C1 terms (position-free). The readers share one tree among the formulas of a run that are equal in
    /// R1C1 terms (`Formula::template`: the tree and the offset from its origin), so the tree is printed once per run at
    /// its origin, not rebuilt and printed per cell (which made a 40 MB workbook take 12 s).
    fn formula(&mut self, f: &Formula, at: CellRef) -> u64 {
        if let Some((base, dr, dc)) = f.template()
            && let (Ok(r), Ok(c)) = (u32::try_from(i64::from(at.row) - dr), u32::try_from(i64::from(at.col) - dc))
        {
            let origin = CellRef::new(r, c);
            return *self.formulas.entry((Arc::as_ptr(base) as usize, origin)).or_insert_with(|| hash(&gridcraft_formula::print_r1c1(base, origin)));
        }
        match f.expr_arc() {
            Some(e) => hash(&gridcraft_formula::print_r1c1(&e, at)),
            None => hash(&*f.text()),
        }
    }
    /// What a cell holds: its formula (R1C1) or else its value, and its resolved style. A formula's cached result is not
    /// part of it: a workbook saved without results (or by another app) would show every formula as changed.
    fn key(&mut self, at: CellRef, cell: &Cell) -> u64 {
        let mut h = DefaultHasher::new();
        match (&cell.formula, &cell.value) {
            (Some(f), _) => (5u8, self.formula(f, at)).hash(&mut h),
            (None, Value::Empty) => 0u8.hash(&mut h),
            (None, Value::Number(n)) => (1u8, n.to_bits()).hash(&mut h),
            (None, Value::Text(t)) => (2u8, &**t).hash(&mut h),
            (None, Value::Bool(b)) => (3u8, b).hash(&mut h),
            (None, v) => (4u8, v.display()).hash(&mut h),
        }
        self.style(cell).hash(&mut h);
        h.finish()
    }
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// A sheet's cells as (row, col, key), row-major, and its size.
struct Grid {
    cells: Vec<(u32, u32, u64)>,
    rows: usize,
    cols: usize,
}

fn grid(k: &mut Keyer, s: &Sheet) -> Grid {
    let cells: Vec<(u32, u32, u64)> = s.cells.iter().map(|(at, c)| (at.row, at.col, k.key(at, c))).collect();
    let rows = cells.last().map_or(0, |c| c.0 as usize + 1);
    let cols = cells.iter().map(|c| c.1 as usize + 1).max().unwrap_or(0);
    Grid { cells, rows, cols }
}

/// Pairs of `0..na` and `0..nb`: the exact `matches` on `ka`/`kb`, then in each gap the items by position when both
/// sides have as many (edited in place), else the similar ones (`sim`), or by position in a gap over [`PAIR_CAP`].
fn align(ka: &[u64], kb: &[u64], mut sim: impl FnMut(usize, usize) -> f64) -> Vec<(usize, usize)> {
    let matched = seq_diff::matches(ka, kb);
    let mut out = Vec::with_capacity(matched.len());
    let mut mi = matched.iter().peekable();
    for (ga, gb) in seq_diff::gaps(&matched, ka.len(), kb.len()) {
        while let Some(&&m) = mi.peek().filter(|m| m.0 < ga.start) {
            out.push(m);
            mi.next();
        }
        let (n, m) = (ga.len(), gb.len());
        if n == 0 || m == 0 {
            continue;
        }
        // as many replaced as there were: edited in place; otherwise by similarity, or by position when large
        let pairs = if n != m && n * m <= PAIR_CAP {
            seq_diff::pair(n, m, PAIR_MIN, |x, y| sim(ga.start + x, gb.start + y))
        } else {
            (0..n.min(m)).map(|x| (x, x)).collect()
        };
        out.extend(pairs.into_iter().map(|(x, y)| (ga.start + x, gb.start + y)));
    }
    out.extend(mi.copied());
    out
}

/// The indices of `0..n` on one side (`b`: the second of each pair) that no pair holds.
fn unpaired(n: usize, pairs: &[(usize, usize)], b: bool) -> Vec<usize> {
    let mut held = vec![false; n];
    for &(x, y) in pairs {
        held[if b { y } else { x }] = true;
    }
    (0..n).filter(|&i| !held[i]).collect()
}

/// Share of equal entries of two sorted lists (cells as (position, key)).
fn overlap(a: &[(u32, u64)], b: &[(u32, u64)]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let set: HashSet<&(u32, u64)> = a.iter().collect();
    b.iter().filter(|x| set.contains(x)).count() as f64 / a.len().max(b.len()) as f64
}

fn letters(c: usize) -> String {
    col_to_letters(c as u32)
}

/// Runs of consecutive indices as `[first, count]`, rows 1-based, columns as letters.
fn runs(idx: impl Iterator<Item = usize>, col: bool) -> Json {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in idx {
        match out.last_mut() {
            Some(r) if r.0 + r.1 == i => r.1 += 1,
            _ => out.push((i, 1)),
        }
    }
    Json::Array(out.into_iter().map(|(i, n)| if col { json!([letters(i), n]) } else { json!([i + 1, n]) }).collect())
}

fn cell_json(cell: Option<&Cell>) -> Json {
    let Some(c) = cell.filter(|c| !c.is_blank()) else { return Json::Null };
    let mut o = serde_json::Map::new();
    if let Some(f) = &c.formula {
        o.insert("formula".into(), json!(format!("={}", f.text())));
    }
    if !c.value.is_empty() || c.formula.is_none() {
        o.insert("value".into(), json!(c.value.display()));
    }
    Json::Object(o)
}

#[derive(Default)]
struct Stats {
    cells: usize,
    rows_ins: usize,
    rows_del: usize,
    cols_ins: usize,
    cols_del: usize,
}

fn diff_sheet(ka: &mut Keyer, a: &Sheet, kb: &mut Keyer, b: &Sheet, st: &mut Stats) -> Option<Json> {
    let (ga, gb) = (grid(ka, a), grid(kb, b));
    // 1. columns
    let col_keys = |g: &Grid| {
        let mut k = vec![0u64; g.cols];
        for &(r, c, x) in &g.cells {
            k[c as usize] = mix(k[c as usize], mix(r as u64, x));
        }
        k
    };
    let (cka, ckb) = (col_keys(&ga), col_keys(&gb));
    let exact = seq_diff::matches(&cka, &ckb).len();
    let cols: Vec<(usize, usize)> = if ga.cols != gb.cols && exact * 2 >= ga.cols.min(gb.cols) && exact > 0 {
        let per_col = |g: &Grid| {
            let mut v: Vec<Vec<(u32, u64)>> = vec![Vec::new(); g.cols];
            for &(r, c, x) in &g.cells {
                v[c as usize].push((r, x));
            }
            v
        };
        let (pa, pb) = (per_col(&ga), per_col(&gb));
        align(&cka, &ckb, |x, y| overlap(&pa[x], &pb[y]))
    } else {
        (0..ga.cols.min(gb.cols)).map(|c| (c, c)).collect()
    };
    // col -> paired-column index, per side
    let (mut ca, mut cb) = (vec![usize::MAX; ga.cols.max(gb.cols)], vec![usize::MAX; ga.cols.max(gb.cols)]);
    for (p, &(x, y)) in cols.iter().enumerate() {
        ca[x] = p;
        cb[y] = p;
    }
    // 2. rows, over the paired columns
    let per_row = |g: &Grid, map: &[usize]| {
        let mut v: Vec<Vec<(u32, u64)>> = vec![Vec::new(); g.rows];
        for &(r, c, x) in &g.cells {
            if let Some(&p) = map.get(c as usize).filter(|p| **p != usize::MAX) {
                v[r as usize].push((p as u32, x));
            }
        }
        v
    };
    let (ra, rb) = (per_row(&ga, &ca), per_row(&gb, &cb));
    let row_key = |cells: &[(u32, u64)]| cells.iter().fold(0u64, |h, &(p, x)| mix(h, mix(p as u64, x)));
    let (rka, rkb): (Vec<u64>, Vec<u64>) = (ra.iter().map(|r| row_key(r)).collect(), rb.iter().map(|r| row_key(r)).collect());
    let rows = align(&rka, &rkb, |x, y| overlap(&ra[x], &rb[y]));

    let (rows_del, rows_ins) = (unpaired(ga.rows, &rows, false), unpaired(gb.rows, &rows, true));
    let (cols_del, cols_ins) = (unpaired(ga.cols, &cols, false), unpaired(gb.cols, &cols, true));

    // 3. the cells of paired rows that differ
    let mut cells = Vec::new();
    let mut count = 0usize;
    for &(x, y) in rows.iter().filter(|&&(x, y)| rka[x] != rkb[y]) {
        let used: BTreeSet<u32> = ra[x].iter().chain(&rb[y]).map(|&(p, _)| p).collect();
        for p in used {
            let (c0, c1) = cols[p as usize];
            let (at_a, at_b) = (CellRef::new(x as u32, c0 as u32), CellRef::new(y as u32, c1 as u32));
            let (cx, cy) = (a.cell(at_a), b.cell(at_b));
            if cx.map(|c| ka.key(at_a, c)) == cy.map(|c| kb.key(at_b, c)) {
                continue;
            }
            count += 1;
            if cells.len() < MAX_CELLS {
                let mut o = json!({"a": at_a.a1(), "b": at_b.a1(), "old": cell_json(cx), "new": cell_json(cy)});
                if let (Some(cx), Some(cy)) = (cx, cy)
                    && ka.style(cx) != kb.style(cy)
                {
                    o["format"] = json!(true);
                }
                cells.push(o);
            }
        }
    }
    // the content of inserted (new side) and deleted (old side) rows and columns, after the changes
    for (g, sheet, rws, cls, new) in [(&gb, b, &rows_ins, &cols_ins, true), (&ga, a, &rows_del, &cols_del, false)] {
        let (mut in_r, mut in_c) = (vec![false; g.rows], vec![false; g.cols]);
        rws.iter().for_each(|&r| in_r[r] = true);
        cls.iter().for_each(|&c| in_c[c] = true);
        for &(r, c, _) in g.cells.iter().filter(|&&(r, c, _)| in_r[r as usize] || in_c[c as usize]) {
            count += 1;
            if cells.len() < MAX_CELLS {
                let at = CellRef::new(r, c);
                let x = cell_json(sheet.cell(at));
                cells.push(if new { json!({"b": at.a1(), "old": null, "new": x}) } else { json!({"a": at.a1(), "old": x, "new": null}) });
            }
        }
    }
    if count == 0 && rows_del.is_empty() && rows_ins.is_empty() && cols_del.is_empty() && cols_ins.is_empty() {
        return None;
    }
    st.cells += count;
    st.rows_ins += rows_ins.len();
    st.rows_del += rows_del.len();
    st.cols_ins += cols_ins.len();
    st.cols_del += cols_del.len();
    let mut o = json!({
        "op": "modify",
        "rows": {"inserted": runs(rows_ins.into_iter(), false), "deleted": runs(rows_del.into_iter(), false)},
        "cols": {"inserted": runs(cols_ins.into_iter(), true), "deleted": runs(cols_del.into_iter(), true)},
        "cells": cells,
    });
    if count > MAX_CELLS {
        o["truncated"] = json!(count - MAX_CELLS);
    }
    Some(o)
}

fn content(k: &mut Keyer, s: &Sheet) -> u64 {
    s.cells.iter().fold(0, |h, (at, c)| mix(h, mix(((at.row as u64) << 32) | at.col as u64, k.key(at, c))))
}

/// The diff of `a` (old) and `b` (new) as the JSON document of PLAN §6.2.
pub fn diff(a: &Workbook, b: &Workbook) -> Json {
    let (mut ka, mut kb) = (Keyer::new(a), Keyer::new(b));
    let mut st = Stats::default();
    let mut sheets = Vec::new();
    let (mut equal, mut inserted, mut deleted, mut renamed, mut modified) = (0, 0, 0, 0, 0);
    let in_a: HashMap<&str, usize> = a.sheets.iter().enumerate().map(|(i, s)| (s.name.as_str(), i)).collect();
    let names_b: HashSet<&str> = b.sheets.iter().map(|s| s.name.as_str()).collect();
    // renames: an old sheet missing by name whose cells equal a new sheet's
    let mut gone: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, s) in a.sheets.iter().enumerate().rev().filter(|(_, s)| !names_b.contains(s.name.as_str())) {
        gone.entry(content(&mut ka, s)).or_default().push(i);
    }
    let mut renamed_from: HashSet<usize> = HashSet::new();
    for s in &b.sheets {
        if let Some(&i) = in_a.get(s.name.as_str()) {
            match diff_sheet(&mut ka, &a.sheets[i], &mut kb, s, &mut st) {
                Some(mut o) => {
                    o["name"] = json!(s.name);
                    modified += 1;
                    sheets.push(o);
                }
                None => equal += 1,
            }
        } else if let Some(i) = gone.get_mut(&content(&mut kb, s)).and_then(Vec::pop) {
            renamed_from.insert(i);
            renamed += 1;
            sheets.push(json!({"op": "rename", "name": s.name, "from": a.sheets[i].name}));
        } else {
            inserted += 1;
            sheets.push(json!({"op": "insert", "name": s.name, "cells": s.cells.len()}));
        }
    }
    for (i, s) in a.sheets.iter().enumerate() {
        if !names_b.contains(s.name.as_str()) && !renamed_from.contains(&i) {
            deleted += 1;
            sheets.push(json!({"op": "delete", "name": s.name, "cells": s.cells.len()}));
        }
    }
    json!({
        "tool": "sheets-diff",
        "schema": SCHEMA,
        "a": {"sheets": a.sheets.len(), "cells": a.sheets.iter().map(|s| s.cells.len()).sum::<usize>()},
        "b": {"sheets": b.sheets.len(), "cells": b.sheets.iter().map(|s| s.cells.len()).sum::<usize>()},
        "stats": {
            "sheets": {"equal": equal, "modified": modified, "inserted": inserted, "deleted": deleted, "renamed": renamed},
            "cells": st.cells,
            "rows": {"inserted": st.rows_ins, "deleted": st.rows_del},
            "cols": {"inserted": st.cols_ins, "deleted": st.cols_del},
        },
        "sheets": sheets,
    })
}

/// Read a workbook (`.xlsx`/`.xlsm`, `.xlsb`, `.ods`, `.xls`), by content.
pub fn read(bytes: &[u8]) -> Result<Workbook, String> {
    use gridcraft_xlsx::{Format, read_ods, read_xls, read_xlsb, read_xlsx, sniff};
    let r = match sniff(bytes) {
        Format::Xlsx => read_xlsx(bytes),
        Format::Xlsb => read_xlsb(bytes),
        Format::Ods => read_ods(bytes),
        Format::Xls => read_xls(bytes),
        Format::Encrypted => return Err("the workbook is password-protected".into()),
        _ => return Err("not a workbook".into()),
    };
    r.map(|(wb, _)| wb).map_err(|e| e.to_string())
}
