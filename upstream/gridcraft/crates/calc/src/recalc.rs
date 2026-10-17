//! Dependency graph and recalculation.
//!
//! The graph maps every formula cell to its static precedents (resolved areas). A reverse index
//! answers "which formulas read this cell?": single-cell precedents in a hash map, ranges as row
//! intervals per column (wide ranges per sheet), each an interval index so a lookup costs
//! O(log n + matches) however many ranges share the column. After an edit, the dirty set is the
//! transitive closure of dependents; it is evaluated in topological order, with on-demand
//! evaluation as a safety net for dynamic references. Cycles produce `#CIRC!` unless iterative
//! calculation is on: then a formula read while it is being evaluated gives its previous value,
//! and the formulas that read one are evaluated again until the loop settles
//! (`PassHost::iterate_loops`).

use std::collections::{BTreeSet, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};

/// A fast, non-cryptographic hasher for cell keys (FxHash-style multiply-rotate).
#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;
type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<FxHasher>>;

use gridcraft_core::{Array, CellError, CellRef, RangeRef, Value};
use gridcraft_formula::Expr;
use gridcraft_model::{CalcMode, Workbook};

use crate::eval::{Area, Evaluator, Host, precedents};

pub type Key = (usize, CellRef);

#[derive(Clone, Debug, Default)]
struct Node {
    areas: Vec<Area>,
    dynamic: bool,
}

/// Ranges wider than this many columns go to the per-sheet wide index instead of per-column ones.
const WIDE: u32 = 64;

/// Insertions an [`Intervals`] keeps aside (scanned by every query) before merging them in.
const PENDING: usize = 32;

/// Formula evaluations iterative calculation may spend on one recalculation (passes × formulas
/// evaluated again), however high Maximum Iterations is.
const MAX_ITERATION_WORK: usize = 10_000_000;

/// Row intervals `[r0, r1]`, each with a payload, answering "which intervals overlap rows
/// `a..=b`?" in O(log n + k).
///
/// The intervals are sorted by start with an implicit segment tree of their largest end, so a query
/// only descends into subtrees that hold a match. Removals mark entries dead (re-inserting one
/// revives it); insertions wait in a small sorted set. [`Intervals::settle`] merges them and drops
/// dead entries once either grows, so mutations cost O(log n) amortised.
#[derive(Clone, Debug)]
struct Intervals<T> {
    /// Sorted, without duplicates.
    items: Vec<(u32, u32, T)>,
    dead: Vec<bool>,
    dead_count: usize,
    /// Segment tree over `items`: node `i` has children `2i` and `2i + 1`, leaves start at
    /// `max_end.len() / 2`. Each node holds the largest end below it (dead entries included).
    max_end: Vec<u32>,
    /// Inserted since the last merge.
    pending: BTreeSet<(u32, u32, T)>,
}

impl<T> Default for Intervals<T> {
    fn default() -> Self {
        Intervals { items: Vec::new(), dead: Vec::new(), dead_count: 0, max_end: Vec::new(), pending: BTreeSet::new() }
    }
}

impl<T: Copy + Ord> Intervals<T> {
    fn is_empty(&self) -> bool {
        self.items.len() == self.dead_count && self.pending.is_empty()
    }

    fn insert(&mut self, r0: u32, r1: u32, t: T) {
        let it = (r0, r1, t);
        if let Ok(i) = self.items.binary_search(&it) {
            if let Some(d) = self.dead.get_mut(i)
                && *d
            {
                *d = false;
                self.dead_count -= 1;
            }
            return;
        }
        self.pending.insert(it);
    }

    fn remove(&mut self, r0: u32, r1: u32, t: T) {
        let it = (r0, r1, t);
        if let Ok(i) = self.items.binary_search(&it) {
            if let Some(d) = self.dead.get_mut(i)
                && !*d
            {
                *d = true;
                self.dead_count += 1;
            }
            return;
        }
        self.pending.remove(&it);
    }

    /// Merges pending insertions and drops dead entries when there are enough of them.
    fn settle(&mut self) {
        if self.pending.len() <= PENDING && self.dead_count <= (self.items.len() / 2).max(PENDING) {
            return;
        }
        let mut all: Vec<(u32, u32, T)> = Vec::with_capacity(self.items.len() - self.dead_count + self.pending.len());
        all.extend(self.items.iter().zip(&self.dead).filter(|(_, d)| !**d).map(|(it, _)| *it));
        all.extend(std::mem::take(&mut self.pending));
        // Two sorted runs: the stable sort merges them in linear time.
        all.sort();
        all.dedup();
        let size = all.len().next_power_of_two();
        let mut max_end = vec![0; 2 * size];
        if let Some(leaves) = max_end.get_mut(size..) {
            for (slot, it) in leaves.iter_mut().zip(&all) {
                *slot = it.1;
            }
        }
        for i in (1..size).rev() {
            let m = max_end.get(2 * i).copied().unwrap_or(0).max(max_end.get(2 * i + 1).copied().unwrap_or(0));
            if let Some(slot) = max_end.get_mut(i) {
                *slot = m;
            }
        }
        self.dead = vec![false; all.len()];
        self.dead_count = 0;
        self.items = all;
        self.max_end = max_end;
    }

    /// Calls `f` for every interval overlapping rows `a..=b` (`r0 <= b && r1 >= a`). Returns the
    /// number of tree nodes visited.
    fn query(&self, a: u32, b: u32, f: &mut dyn FnMut(&T)) -> usize {
        for it in &self.pending {
            if it.0 <= b && it.1 >= a {
                f(&it.2);
            }
        }
        // Entries before `p` start at or before row `b`.
        let p = self.items.partition_point(|it| it.0 <= b);
        if p == 0 {
            return 0;
        }
        self.walk(1, 0, self.max_end.len() / 2, p, a, f)
    }

    fn walk(&self, node: usize, lo: usize, hi: usize, p: usize, a: u32, f: &mut dyn FnMut(&T)) -> usize {
        if lo >= p || self.max_end.get(node).is_none_or(|m| *m < a) {
            return 1;
        }
        if hi - lo <= 1 {
            if let (Some(it), Some(false)) = (self.items.get(lo), self.dead.get(lo))
                && it.1 >= a
            {
                f(&it.2);
            }
            return 1;
        }
        let mid = lo + (hi - lo) / 2;
        1 + self.walk(2 * node, lo, mid, p, a, f) + self.walk(2 * node + 1, mid, hi, p, a, f)
    }
}

/// Consecutive formulas down one column that are shifted copies of one tree, each at the same
/// offset from it (cells `first..=last` of `col`): every one reads the run's precedent areas
/// moved to its own row. The graph keeps one entry per run and per precedent area, instead of
/// one per formula and per precedent cell.
#[derive(Clone, Debug)]
struct Run {
    sheet: usize,
    col: u32,
    first: u32,
    last: u32,
    /// The precedents in the order [`precedents`] lists them for each formula.
    areas: Vec<RunArea>,
}

/// A precedent area of the formulas of a run. For the formula in row `r` its top row is
/// `top + r` when `top_rel`, else `top` (likewise the bottom); its columns are the same for all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RunArea {
    sheet: usize,
    top: i64,
    top_rel: bool,
    bottom: i64,
    bottom_rel: bool,
    left: u32,
    right: u32,
}

impl RunArea {
    fn top_at(&self, r: u32) -> i64 {
        if self.top_rel { self.top + i64::from(r) } else { self.top }
    }
    fn bottom_at(&self, r: u32) -> i64 {
        if self.bottom_rel { self.bottom + i64::from(r) } else { self.bottom }
    }
    fn at(&self, r: u32) -> Area {
        let row = |v: i64| u32::try_from(v).unwrap_or(0);
        Area {
            sheet: self.sheet,
            range: RangeRef::new(CellRef::new(row(self.top_at(r)), self.left), CellRef::new(row(self.bottom_at(r)), self.right)),
        }
    }
    /// The rows of `first..=last` whose formula's area overlaps rows `a..=b`: `top(r) <= b` and
    /// `bottom(r) >= a`, each end being constant or growing with `r`.
    fn readers(&self, first: u32, last: u32, a: u32, b: u32) -> Option<(u32, u32)> {
        let (mut lo, mut hi) = (i64::from(first), i64::from(last));
        if self.top_rel {
            hi = hi.min(i64::from(b) - self.top);
        } else if self.top > i64::from(b) {
            return None;
        }
        if self.bottom_rel {
            lo = lo.max(i64::from(a) - self.bottom);
        } else if self.bottom < i64::from(a) {
            return None;
        }
        (lo <= hi).then(|| (u32::try_from(lo).unwrap_or(0), u32::try_from(hi).unwrap_or(0)))
    }
}

impl Run {
    fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        (self.first..=self.last).map(|r| (self.sheet, CellRef::new(r, self.col)))
    }
    /// Index entries of the run's areas, without duplicates: (area, rows any formula reads).
    fn entries(&self) -> Vec<(u32, u32, u32)> {
        let mut v: Vec<(usize, RunArea)> = self.areas.iter().copied().enumerate().collect();
        v.sort_unstable_by_key(|(_, a)| *a);
        v.dedup_by_key(|(_, a)| *a);
        v.into_iter()
            .map(|(i, a)| {
                let row = |x: i64| u32::try_from(x).unwrap_or(0);
                (u32::try_from(i).unwrap_or(u32::MAX), row(a.top_at(self.first)), row(a.bottom_at(self.last)))
            })
            .collect()
    }
}

/// Builds runs while the formulas of a sheet are visited in row-major order: per column, the
/// run being extended.
struct PendingRun {
    base: std::sync::Arc<Expr>,
    /// Where `base` is exact: every formula of the run is `base` moved by its cell minus this.
    origin: (i64, i64),
    first: u32,
    last: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    nodes: HashMap<Key, Node>,
    /// single cell → dependents
    cell_deps: HashMap<Key, HashSet<Key>>,
    /// (sheet, col) → row intervals of the ranges (up to `WIDE` columns) that cover the column
    col_deps: HashMap<(usize, u32), Intervals<Key>>,
    /// sheet → row intervals of wider ranges, with their columns: (first, last, dependent)
    wide_deps: HashMap<usize, Intervals<(u32, u32, Key)>>,
    dynamic: HashSet<Key>,
    /// Range indexes changed since the last [`Graph::settle`].
    unsettled_cols: HashSet<(usize, u32)>,
    unsettled_wide: HashSet<usize>,
    /// Runs of formulas sharing a tree, by id; never dynamic.
    runs: HashMap<u32, Run>,
    next_run: u32,
    /// Formulas in runs.
    run_members: usize,
    /// (sheet, col) → first row → run id.
    run_at: HashMap<(usize, u32), std::collections::BTreeMap<u32, u32>>,
    /// (sheet, col) → rows that run areas (up to `WIDE` columns) covering the column read, with
    /// (run, area).
    run_cols: HashMap<(usize, u32), Intervals<(u32, u32)>>,
    /// sheet → rows that wider run areas read, with (first column, last column, run, area).
    run_wide: HashMap<usize, Intervals<(u32, u32, u32, u32)>>,
}

impl Graph {
    /// Formula cells in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len() + self.run_members
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Whether the formula at `k` is in the graph.
    pub fn contains(&self, k: Key) -> bool {
        self.nodes.contains_key(&k) || self.run_of(k).is_some()
    }
    /// Every formula cell in the graph.
    pub fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.nodes.keys().copied().chain(self.runs.values().flat_map(|r| r.keys()))
    }
    /// The run holding the formula at `k`.
    fn run_of(&self, k: Key) -> Option<u32> {
        let (first, id) = self.run_at.get(&(k.0, k.1.col))?.range(..=k.1.row).next_back()?;
        (*first <= k.1.row && self.runs.get(id).is_some_and(|r| r.last >= k.1.row)).then_some(*id)
    }
    fn insert_run(&mut self, run: Run) {
        let id = self.next_run;
        self.next_run = self.next_run.wrapping_add(1);
        for (ai, top, bottom) in run.entries() {
            let Some(a) = run.areas.get(ai as usize) else { continue };
            if a.right - a.left + 1 > WIDE {
                self.run_wide.entry(a.sheet).or_default().insert(top, bottom, (a.left, a.right, id, ai));
                self.unsettled_wide.insert(a.sheet);
            } else {
                for c in a.left..=a.right {
                    self.run_cols.entry((a.sheet, c)).or_default().insert(top, bottom, (id, ai));
                    self.unsettled_cols.insert((a.sheet, c));
                }
            }
        }
        self.run_at.entry((run.sheet, run.col)).or_default().insert(run.first, id);
        self.run_members += (run.last - run.first + 1) as usize;
        self.runs.insert(id, run);
    }
    fn remove_run(&mut self, id: u32) -> Option<Run> {
        let run = self.runs.remove(&id)?;
        for (ai, top, bottom) in run.entries() {
            let Some(a) = run.areas.get(ai as usize) else { continue };
            if a.right - a.left + 1 > WIDE {
                if let Some(v) = self.run_wide.get_mut(&a.sheet) {
                    v.remove(top, bottom, (a.left, a.right, id, ai));
                    self.unsettled_wide.insert(a.sheet);
                }
            } else {
                for c in a.left..=a.right {
                    if let Some(v) = self.run_cols.get_mut(&(a.sheet, c)) {
                        v.remove(top, bottom, (id, ai));
                        self.unsettled_cols.insert((a.sheet, c));
                    }
                }
            }
        }
        if let Some(m) = self.run_at.get_mut(&(run.sheet, run.col)) {
            m.remove(&run.first);
            if m.is_empty() {
                self.run_at.remove(&(run.sheet, run.col));
            }
        }
        self.run_members = self.run_members.saturating_sub((run.last - run.first + 1) as usize);
        Some(run)
    }
    /// Takes the formula at `k` out of its run (the cells above and below stay runs).
    fn split_out(&mut self, k: Key) {
        let Some(run) = self.run_of(k).and_then(|id| self.remove_run(id)) else { return };
        let row = k.1.row;
        if run.first < row {
            self.insert_run(Run { last: row - 1, ..run.clone() });
        }
        if row < run.last {
            self.insert_run(Run { first: row + 1, ..run });
        }
    }

    /// Index entries of a formula's precedents, without duplicates: single cells, column
    /// intervals and wide ranges.
    fn entries(areas: &[Area]) -> (Vec<Key>, Vec<(usize, u32, u32, u32)>, Vec<(usize, RangeRef)>) {
        let (mut cells, mut cols, mut wide) = (Vec::new(), Vec::new(), Vec::new());
        for a in areas {
            if a.range.is_single() {
                cells.push((a.sheet, a.range.start));
            } else if a.range.width() > WIDE {
                wide.push((a.sheet, a.range));
            } else {
                for c in a.range.start.col..=a.range.end.col {
                    cols.push((a.sheet, c, a.range.start.row, a.range.end.row));
                }
            }
        }
        cells.sort_unstable();
        cells.dedup();
        cols.sort_unstable();
        cols.dedup();
        wide.sort_unstable_by_key(|(s, r)| (*s, r.start, r.end));
        wide.dedup();
        (cells, cols, wide)
    }

    fn insert(&mut self, k: Key, node: Node) {
        self.remove(k);
        let (cells, cols, wide) = Self::entries(&node.areas);
        for c in cells {
            self.cell_deps.entry(c).or_default().insert(k);
        }
        for (s, c, r0, r1) in cols {
            self.col_deps.entry((s, c)).or_default().insert(r0, r1, k);
            self.unsettled_cols.insert((s, c));
        }
        for (s, r) in wide {
            self.wide_deps.entry(s).or_default().insert(r.start.row, r.end.row, (r.start.col, r.end.col, k));
            self.unsettled_wide.insert(s);
        }
        if node.dynamic {
            self.dynamic.insert(k);
        }
        self.nodes.insert(k, node);
    }

    fn remove(&mut self, k: Key) {
        self.split_out(k);
        let Some(old) = self.nodes.remove(&k) else { return };
        let (cells, cols, wide) = Self::entries(&old.areas);
        for c in cells {
            if let Some(v) = self.cell_deps.get_mut(&c) {
                v.remove(&k);
                if v.is_empty() {
                    self.cell_deps.remove(&c);
                }
            }
        }
        for (s, c, r0, r1) in cols {
            if let Some(v) = self.col_deps.get_mut(&(s, c)) {
                v.remove(r0, r1, k);
                self.unsettled_cols.insert((s, c));
            }
        }
        for (s, r) in wide {
            if let Some(v) = self.wide_deps.get_mut(&s) {
                v.remove(r.start.row, r.end.row, (r.start.col, r.end.col, k));
                self.unsettled_wide.insert(s);
            }
        }
        self.dynamic.remove(&k);
    }

    /// Tidies the range indexes changed by `insert`/`remove` (queries stay correct without it,
    /// just slower).
    fn settle(&mut self) {
        for key in std::mem::take(&mut self.unsettled_cols) {
            if let Some(v) = self.col_deps.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.col_deps.remove(&key);
                }
            }
            if let Some(v) = self.run_cols.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.run_cols.remove(&key);
                }
            }
        }
        for key in std::mem::take(&mut self.unsettled_wide) {
            if let Some(v) = self.wide_deps.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.wide_deps.remove(&key);
                }
            }
            if let Some(v) = self.run_wide.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.run_wide.remove(&key);
                }
            }
        }
    }

    /// The formulas of run `id` whose area `ai` overlaps rows `a..=b` (and columns `c0..=c1`).
    fn run_readers(&self, id: u32, ai: u32, a: u32, b: u32, (c0, c1): (u32, u32), out: &mut Vec<Key>) {
        let Some(run) = self.runs.get(&id) else { return };
        let Some(area) = run.areas.get(ai as usize) else { return };
        if area.right < c0 || c1 < area.left {
            return;
        }
        if let Some((lo, hi)) = area.readers(run.first, run.last, a, b) {
            out.extend((lo..=hi).map(|r| (run.sheet, CellRef::new(r, run.col))));
        }
    }

    /// Formulas that read cell `c` of `sheet`.
    pub fn dependents(&self, sheet: usize, c: CellRef, out: &mut Vec<Key>) {
        if let Some(v) = self.cell_deps.get(&(sheet, c)) {
            out.extend(v.iter().copied());
        }
        if let Some(v) = self.col_deps.get(&(sheet, c.col)) {
            v.query(c.row, c.row, &mut |k| out.push(*k));
        }
        if let Some(v) = self.wide_deps.get(&sheet) {
            v.query(c.row, c.row, &mut |(c0, c1, k)| {
                if *c0 <= c.col && c.col <= *c1 {
                    out.push(*k);
                }
            });
        }
        if let Some(v) = self.run_cols.get(&(sheet, c.col)) {
            v.query(c.row, c.row, &mut |(id, ai)| self.run_readers(*id, *ai, c.row, c.row, (c.col, c.col), out));
        }
        if let Some(v) = self.run_wide.get(&sheet) {
            v.query(c.row, c.row, &mut |(_, _, id, ai)| self.run_readers(*id, *ai, c.row, c.row, (c.col, c.col), out));
        }
    }

    /// Formulas that read any cell of `range` (a formula can be listed more than once).
    pub fn dependents_of_range(&self, sheet: usize, range: RangeRef, out: &mut Vec<Key>) {
        if range.count() <= 4096 {
            for c in range.iter() {
                if let Some(v) = self.cell_deps.get(&(sheet, c)) {
                    out.extend(v.iter().copied());
                }
            }
        } else {
            for ((s, c), v) in &self.cell_deps {
                if *s == sheet && range.contains(*c) {
                    out.extend(v.iter().copied());
                }
            }
        }
        let (r0, r1) = (range.start.row, range.end.row);
        if (range.width() as usize) <= self.col_deps.len() {
            for col in range.start.col..=range.end.col {
                if let Some(v) = self.col_deps.get(&(sheet, col)) {
                    v.query(r0, r1, &mut |k| out.push(*k));
                }
            }
        } else {
            for ((s, col), v) in &self.col_deps {
                if *s == sheet && range.start.col <= *col && *col <= range.end.col {
                    v.query(r0, r1, &mut |k| out.push(*k));
                }
            }
        }
        if let Some(v) = self.wide_deps.get(&sheet) {
            v.query(r0, r1, &mut |(c0, c1, k)| {
                if *c0 <= range.end.col && range.start.col <= *c1 {
                    out.push(*k);
                }
            });
        }
        let cols = (range.start.col, range.end.col);
        if (range.width() as usize) <= self.run_cols.len() {
            for col in range.start.col..=range.end.col {
                if let Some(v) = self.run_cols.get(&(sheet, col)) {
                    v.query(r0, r1, &mut |(id, ai)| self.run_readers(*id, *ai, r0, r1, cols, out));
                }
            }
        } else {
            for ((s, col), v) in &self.run_cols {
                if *s == sheet && range.start.col <= *col && *col <= range.end.col {
                    v.query(r0, r1, &mut |(id, ai)| self.run_readers(*id, *ai, r0, r1, cols, out));
                }
            }
        }
        if let Some(v) = self.run_wide.get(&sheet) {
            v.query(r0, r1, &mut |(_, _, id, ai)| self.run_readers(*id, *ai, r0, r1, cols, out));
        }
    }

    /// Precedent areas of a formula cell (for Trace Precedents).
    pub fn precedents_of(&self, k: Key) -> Vec<Area> {
        if let Some(run) = self.run_of(k).and_then(|id| self.runs.get(&id)) {
            return run.areas.iter().map(|a| a.at(k.1.row)).collect();
        }
        self.nodes.get(&k).map(|n| n.areas.clone()).unwrap_or_default()
    }

    /// Adds the formulas of a pending run: as one run when they read areas that move with them,
    /// else one by one. `formula` gives the tree of the formula at a row of the run's column.
    fn add_pending(&mut self, wb: &Workbook, si: usize, col: u32, p: PendingRun, formula: &dyn Fn(u32) -> Option<Expr>) {
        if p.last > p.first
            && let Some(areas) = run_areas(wb, si, col, &p)
        {
            self.insert_run(Run { sheet: si, col, first: p.first, last: p.last, areas });
            return;
        }
        for row in p.first..=p.last {
            if let Some(e) = formula(row) {
                let (areas, dynamic) = precedents(wb, si, &e);
                self.insert((si, CellRef::new(row, col)), Node { areas, dynamic });
            }
        }
    }
}

/// The precedent areas of a pending run as [`RunArea`]s, when every formula of the run reads
/// static areas (no INDIRECT, names...) that keep their size and move with it row by row, and no
/// formula of the run has a reference moved off the sheet. Worked out from the first and last
/// formulas and checked on one in the middle: each end of an area is a constant row or follows
/// the formula's row, so two formulas settle it.
fn run_areas(wb: &Workbook, si: usize, col: u32, p: &PendingRun) -> Option<Vec<RunArea>> {
    let at = |row: u32| {
        let (dr, dc) = (i64::from(row) - p.origin.0, i64::from(col) - p.origin.1);
        gridcraft_formula::adjust::shift_stays_on_sheet(&p.base, dr, dc)
            .then(|| precedents(wb, si, &gridcraft_formula::adjust::shifted(&p.base, dr, dc)))
    };
    let (first, false) = at(p.first)? else { return None };
    let (last, false) = at(p.last)? else { return None };
    if first.len() != last.len() {
        return None;
    }
    let span = i64::from(p.last - p.first);
    let end = |a: u32, b: u32, base_row: u32| -> Option<(i64, bool)> {
        match i64::from(b) - i64::from(a) {
            0 => Some((i64::from(a), false)),
            d if d == span => Some((i64::from(a) - i64::from(base_row), true)),
            _ => None,
        }
    };
    let areas = first
        .iter()
        .zip(&last)
        .map(|(a, b)| {
            if a.sheet != b.sheet || a.range.start.col != b.range.start.col || a.range.end.col != b.range.end.col {
                return None;
            }
            let (top, top_rel) = end(a.range.start.row, b.range.start.row, p.first)?;
            let (bottom, bottom_rel) = end(a.range.end.row, b.range.end.row, p.first)?;
            Some(RunArea { sheet: a.sheet, top, top_rel, bottom, bottom_rel, left: a.range.start.col, right: a.range.end.col })
        })
        .collect::<Option<Vec<RunArea>>>()?;
    let mid = p.first + (p.last - p.first) / 2;
    let (check, false) = at(mid)? else { return None };
    (check == areas.iter().map(|a| a.at(mid)).collect::<Vec<_>>()).then_some(areas)
}

/// Calculation state kept beside a workbook.
#[derive(Clone, Debug)]
pub struct Calc {
    pub graph: Graph,
    rng: u64,
    /// Fixed "now" for deterministic tests; `None` = system clock.
    pub fixed_now: Option<f64>,
    /// Cells whose formulas have errors from cycles.
    pub circular: Vec<Key>,
    pub last_recalc_cells: usize,
}

impl Default for Calc {
    fn default() -> Self {
        Calc { graph: Graph::default(), rng: 0x9E37_79B9_7F4A_7C15, fixed_now: None, circular: vec![], last_recalc_cells: 0 }
    }
}

/// Where a cell stands in a recalculation pass.
#[derive(Clone, Debug, Default)]
enum Slot {
    /// Not a formula of the pass.
    #[default]
    Out,
    /// A formula of the pass, not calculated yet.
    Pending,
    /// A formula of the pass and its result.
    Done(Value),
}

/// The formulas of a recalculation pass and their results. A pass can hold every formula of the
/// workbook, so this is laid out for size: per column, one slot per row from the column's first
/// formula to its last when they are dense (a filled-down column: 24 bytes per formula, nothing
/// hashed), else a map of rows. The formulas' trees are not held: the workbook doesn't change
/// during a pass, so each tree is taken from its cell when the formula is calculated (a shifted
/// copy's is built then, and dropped after).
#[derive(Debug, Default)]
struct PassSlots {
    cols: HashMap<(usize, u32), ColSlots>,
    /// Formulas with a result.
    done: usize,
}

#[derive(Debug)]
enum ColSlots {
    Dense { first: u32, slots: Vec<Slot> },
    Sparse(HashMap<u32, Slot>),
}

impl ColSlots {
    fn get(&self, row: u32) -> Option<&Slot> {
        match self {
            ColSlots::Dense { first, slots } => slots.get(row.checked_sub(*first)? as usize),
            ColSlots::Sparse(m) => m.get(&row),
        }
    }
    fn get_mut(&mut self, row: u32) -> Option<&mut Slot> {
        match self {
            ColSlots::Dense { first, slots } => slots.get_mut(row.checked_sub(*first)? as usize),
            ColSlots::Sparse(m) => m.get_mut(&row),
        }
    }
}

impl PassSlots {
    /// The pass over the formulas among `keys` (a formula whose text doesn't parse is not one).
    fn new(wb: &Workbook, keys: &[Key]) -> PassSlots {
        let in_pass = |k: &&Key| {
            wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.as_ref()).is_some_and(|f| f.is_parsed() || f.expr_arc().is_some())
        };
        // Rows spanned and formulas per column.
        let mut extent: HashMap<(usize, u32), (u32, u32, usize)> = HashMap::default();
        for (s, c) in keys.iter().filter(in_pass) {
            let e = extent.entry((*s, c.col)).or_insert((c.row, c.row, 0));
            e.0 = e.0.min(c.row);
            e.1 = e.1.max(c.row);
            e.2 += 1;
        }
        let mut cols: HashMap<(usize, u32), ColSlots> = HashMap::with_capacity_and_hasher(extent.len(), Default::default());
        for (col, (first, last, n)) in extent {
            let span = (last - first) as usize + 1;
            let slots = if span <= n.saturating_mul(2) {
                ColSlots::Dense { first, slots: vec![Slot::Out; span] }
            } else {
                ColSlots::Sparse(HashMap::with_capacity_and_hasher(n, Default::default()))
            };
            cols.insert(col, slots);
        }
        let mut slots = PassSlots { cols, done: 0 };
        for (s, c) in keys.iter().filter(in_pass) {
            match slots.cols.get_mut(&(*s, c.col)) {
                Some(ColSlots::Sparse(m)) => {
                    m.insert(c.row, Slot::Pending);
                }
                Some(col) => {
                    if let Some(slot) = col.get_mut(c.row) {
                        *slot = Slot::Pending;
                    }
                }
                None => {}
            }
        }
        slots
    }
    fn get(&self, (s, c): Key) -> &Slot {
        self.cols.get(&(s, c.col)).and_then(|col| col.get(c.row)).unwrap_or(&Slot::Out)
    }
    /// A formula of the pass, calculated or not.
    fn contains(&self, k: Key) -> bool {
        !matches!(self.get(k), Slot::Out)
    }
    fn result(&self, k: Key) -> Option<&Value> {
        match self.get(k) {
            Slot::Done(v) => Some(v),
            _ => None,
        }
    }
    /// Records the result of a formula of the pass.
    fn set_result(&mut self, (s, c): Key, v: Value) {
        if let Some(slot) = self.cols.get_mut(&(s, c.col)).and_then(|col| col.get_mut(c.row))
            && !matches!(slot, Slot::Out)
        {
            if !matches!(slot, Slot::Done(_)) {
                self.done += 1;
            }
            *slot = Slot::Done(v);
        }
    }
    /// Makes a calculated formula of the pass pending again, returning its result.
    fn recalculate(&mut self, (s, c): Key) -> Option<Value> {
        let slot = self.cols.get_mut(&(s, c.col)).and_then(|col| col.get_mut(c.row))?;
        match slot {
            Slot::Done(_) => {
                self.done -= 1;
                match std::mem::replace(slot, Slot::Pending) {
                    Slot::Done(v) => Some(v),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    /// Formulas with a result.
    fn len(&self) -> usize {
        self.done
    }
    /// The results, by cell.
    fn into_results(self) -> impl Iterator<Item = (Key, Value)> {
        self.cols.into_iter().flat_map(|((s, col), slots)| {
            let rows: Box<dyn Iterator<Item = (u32, Slot)>> = match slots {
                ColSlots::Dense { first, slots } => Box::new(slots.into_iter().enumerate().map(move |(i, v)| (first.saturating_add(i as u32), v))),
                ColSlots::Sparse(m) => Box::new(m.into_iter()),
            };
            rows.filter_map(move |(row, slot)| match slot {
                Slot::Done(v) => Some(((s, CellRef::new(row, col)), v)),
                _ => None,
            })
        })
    }
}

struct PassHost<'a> {
    wb: &'a Workbook,
    /// The formulas of the pass and their results.
    slots: PassSlots,
    in_progress: HashSet<Key>,
    /// New spills found during the pass: anchor → array.
    spills: HashMap<Key, std::sync::Arc<Array>>,
    /// Used range per sheet: the workbook doesn't change during a pass, and working it out walks
    /// every row, so once per sheet rather than once per range read.
    used: HashMap<usize, Option<RangeRef>>,
    rng: &'a mut u64,
    now: f64,
    cycle: bool,
    cycles: Vec<Key>,
    depth: usize,
    /// Iterative calculation: a circular reference reads the cell's previous value instead of
    /// giving `#CIRC!`.
    iterate: bool,
    /// Values from the previous iteration; cells not in it read their stored value.
    prev: HashMap<Key, Value>,
    /// Formulas being evaluated, innermost last (kept only when iterating).
    stack: Vec<Key>,
    /// Formulas found on a circular reference (when iterating).
    members: HashSet<Key>,
    /// Results that read a previous value, directly or through other formulas.
    tainted: HashSet<Key>,
    /// Whether the formula being evaluated has read a previous value.
    taint: bool,
}

impl<'a> PassHost<'a> {
    fn new(wb: &'a Workbook, slots: PassSlots, rng: &'a mut u64, now: f64) -> Self {
        PassHost {
            wb,
            slots,
            in_progress: HashSet::default(),
            spills: HashMap::default(),
            used: HashMap::default(),
            rng,
            now,
            cycle: false,
            cycles: vec![],
            depth: 0,
            iterate: false,
            prev: HashMap::default(),
            stack: Vec::new(),
            members: HashSet::default(),
            tainted: HashSet::default(),
            taint: false,
        }
    }
}

/// Whether an array result at `k` can't spill: it is in a table, or its area runs off the sheet,
/// overlaps merged cells, holds other cells, or another formula's spill (unless that formula is
/// recalculated in this pass, and so may spill elsewhere now).
fn spill_blocked(host: &PassHost<'_>, (sheet, anchor): Key, a: &Array) -> bool {
    let Some(sh) = host.wb.sheet(sheet) else { return false };
    let end =
        CellRef::new(anchor.row.saturating_add((a.rows as u32).saturating_sub(1)), anchor.col.saturating_add((a.cols as u32).saturating_sub(1)));
    let range = RangeRef::new(anchor, end);
    let own = sh.spill_ranges.get(&anchor);
    let spilled_over = |c: CellRef| {
        !own.is_some_and(|o| o.contains(c)) && sh.spill_ranges.iter().any(|(o, r)| *o != anchor && r.contains(c) && !host.slots.contains((sheet, *o)))
    };
    end.row >= gridcraft_core::MAX_ROWS
        || end.col >= gridcraft_core::MAX_COLS
        || sh.merges.iter().any(|m| m.intersects(&range))
        || sh.tables.iter().any(|t| t.range.contains(anchor))
        || sh.cells.iter_range(range).any(|(c, cell)| c != anchor && (!cell.value.is_empty() || cell.formula.is_some()))
        || sh.spill.range(range.start..=range.end).any(|(c, _)| range.contains(*c) && spilled_over(*c))
}

impl PassHost<'_> {
    fn compute(&mut self, k: Key) -> Value {
        match self.slots.get(k) {
            Slot::Done(v) => {
                let v = v.clone();
                if self.iterate && self.tainted.contains(&k) {
                    self.taint = true;
                }
                return v;
            }
            Slot::Pending => {}
            Slot::Out => return self.wb.sheet(k.0).map(|s| s.value(k.1)).unwrap_or_default(),
        }
        // The tree of a formula of the pass (built now for a shifted copy).
        let Some(expr) = self.wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.as_ref()).and_then(|f| f.expr_arc()) else {
            return self.wb.sheet(k.0).map(|s| s.value(k.1)).unwrap_or_default();
        };
        if self.in_progress.contains(&k) || self.depth > 2000 {
            self.cycle = true;
            if self.iterate {
                return self.circular_read(k);
            }
            self.cycles.push(k);
            return Value::Error(CellError::Circ);
        }
        self.in_progress.insert(k);
        self.depth += 1;
        let outer_taint = std::mem::replace(&mut self.taint, false);
        if self.iterate {
            self.stack.push(k);
        }
        let v = {
            let mut ev = Evaluator::new(self, k.0, k.1);
            ev.value(&expr)
        };
        // A legacy array formula fills the range it was entered in.
        let legacy = self.wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.as_ref()).and_then(|f| f.array());
        let v = match legacy {
            Some(range) => fit_to_range(v, range),
            None => v,
        };
        self.depth -= 1;
        self.in_progress.remove(&k);
        if self.iterate {
            self.stack.pop();
            if self.taint {
                self.tainted.insert(k);
            } else {
                self.tainted.remove(&k);
            }
        }
        self.taint |= outer_taint;
        // A blank cell shows as 0 in a formula's result, inside an array as well (`=A1:A3`,
        // FILTER or SORT of a range with blanks). GROUPBY and PIVOTBY lay out blank cells
        // of their own, which stay blank.
        let keep_blanks = matches!(&*expr, Expr::Call(n, _) if matches!(n.as_str(), "GROUPBY" | "PIVOTBY"));
        let v = match v {
            Value::Array(a) => {
                let a = if keep_blanks || !a.data.iter().any(|x| matches!(x, Value::Empty)) {
                    a
                } else {
                    let data = a.data.iter().map(|x| if matches!(x, Value::Empty) { Value::Number(0.0) } else { x.clone() }).collect();
                    Array::new(a.rows, a.cols, data).map(std::sync::Arc::new).unwrap_or(a)
                };
                if a.rows > 1 || a.cols > 1 {
                    self.spills.insert(k, a.clone());
                }
                a.data.first().cloned().unwrap_or_default()
            }
            Value::Empty => Value::Number(0.0),
            v => v,
        };
        // An array that can't spill is #SPILL! for the formulas evaluated after it in this pass too.
        let v = match self.spills.get(&k) {
            Some(a) if spill_blocked(self, k, a) => {
                self.spills.remove(&k);
                Value::Error(CellError::Spill)
            }
            _ => v,
        };
        self.slots.set_result(k, v.clone());
        v
    }

    /// A formula's previous value for iterative calculation: from the last iteration, else the
    /// value it shows. Blank, and the `#CIRC!` of a loop calculated with iteration off, are 0.
    fn previous(&self, k: Key) -> Value {
        let v = match self.prev.get(&k) {
            Some(v) => v.clone(),
            None => self.wb.sheet(k.0).map(|s| s.value(k.1)).unwrap_or_default(),
        };
        if matches!(v, Value::Empty | Value::Error(CellError::Circ)) { Value::Number(0.0) } else { v }
    }

    /// Reading `k` while it is being evaluated, with iterative calculation on: its previous value.
    /// The formulas around the loop are the ones whose changes decide when iteration stops.
    fn circular_read(&mut self, k: Key) -> Value {
        self.taint = true;
        match self.stack.iter().rposition(|s| *s == k) {
            Some(i) => self.members.extend(self.stack.get(i..).unwrap_or_default().iter().copied()),
            // Too deep to follow: the formula itself is watched.
            None => {
                self.members.insert(k);
            }
        }
        self.previous(k)
    }

    /// Iterative calculation, after a first pass over `order` that met circular references. The
    /// formulas that read a previous value (the loops and what depends on them) are evaluated
    /// again in the same order, each pass starting from the last one's values and reading what it
    /// has already produced (Gauss-Seidel), until no formula on a loop changes by `max_change` or
    /// more, or after `max_iterations` passes in all. Returns the number of passes.
    fn iterate_loops(&mut self, order: &[Key], max_iterations: u32, max_change: f64) -> u32 {
        let again: Vec<Key> = order.iter().copied().filter(|k| self.tainted.contains(k)).collect();
        let set: HashSet<Key> = again.iter().copied().collect();
        // Bounds the work whatever Maximum Iterations says.
        let max_passes = u32::try_from(MAX_ITERATION_WORK / again.len().max(1)).unwrap_or(u32::MAX).max(1);
        let mut passes = 1u32;
        while passes < max_iterations.min(max_passes) && !self.converged(max_change) {
            self.prev = again.iter().filter_map(|k| self.slots.recalculate(*k).map(|v| (*k, v))).collect();
            self.spills.retain(|a, _| !set.contains(a));
            for k in &again {
                self.compute(*k);
            }
            passes += 1;
        }
        passes
    }

    /// Whether every formula on a loop changed by less than `max_change` in the last pass (text,
    /// logical and error values: not at all).
    fn converged(&self, max_change: f64) -> bool {
        self.members.iter().all(|k| {
            let Some(new) = self.slots.result(*k) else { return true };
            match (new, &self.previous(*k)) {
                (Value::Number(a), Value::Number(b)) => {
                    let d = (a - b).abs();
                    d < max_change || d == 0.0
                }
                (new, old) => new == old,
            }
        })
    }
}

/// A legacy (Ctrl+Shift+Enter) array formula's result fitted to the range it was entered in, as
/// Excel does: a single row or column repeats across the range, a single value fills it, cells
/// past the result show `#N/A`, and a result larger than the range is cut off.
fn fit_to_range(v: Value, range: RangeRef) -> Value {
    if range.count() > crate::eval::MAX_CELLS {
        return Value::Error(CellError::Num);
    }
    let (h, w) = (range.height() as usize, range.width() as usize);
    let Value::Array(a) = v else {
        return if h * w == 1 { v } else { Array::new(h, w, vec![v; h * w]).map(Value::from).unwrap_or(Value::Error(CellError::Value)) };
    };
    if h * w == 1 {
        return a.data.first().cloned().unwrap_or_default();
    }
    let mut data = Vec::with_capacity(h * w);
    for r in 0..h {
        for c in 0..w {
            let (r, c) = (if a.rows == 1 { 0 } else { r }, if a.cols == 1 { 0 } else { c });
            data.push(a.get(r, c).cloned().unwrap_or(Value::Error(CellError::NA)));
        }
    }
    Array::new(h, w, data).map(Value::from).unwrap_or(Value::Error(CellError::Value))
}

impl Host for PassHost<'_> {
    fn workbook(&self) -> &Workbook {
        self.wb
    }
    fn spill_range(&mut self, sheet: usize, anchor: CellRef) -> Option<RangeRef> {
        let k = (sheet, anchor);
        if !self.slots.contains(k) {
            return self.wb.sheet(sheet)?.spill_ranges.get(&anchor).copied();
        }
        // Recalculated in this pass: its new array, unless something blocks it (#SPILL!).
        self.compute(k);
        let arr = self.spills.get(&k)?;
        let end = CellRef::new(
            anchor.row.saturating_add((arr.rows as u32).saturating_sub(1)),
            anchor.col.saturating_add((arr.cols as u32).saturating_sub(1)),
        );
        let range = RangeRef::new(anchor, end);
        let sh = self.wb.sheet(sheet)?;
        let blocked = end.row >= gridcraft_core::MAX_ROWS
            || end.col >= gridcraft_core::MAX_COLS
            || sh.cells.iter_range(range).any(|(c, cell)| c != anchor && (cell.formula.is_some() || !cell.value.is_empty()))
            || sh.merges.iter().any(|m| m.intersects(&range));
        (!blocked).then_some(range)
    }
    fn cell_value(&mut self, sheet: usize, c: CellRef) -> Value {
        let k = (sheet, c);
        if self.slots.contains(k) {
            return self.compute(k);
        }
        // A cell covered by a spill computed in this pass.
        for (anchor, arr) in &self.spills {
            if anchor.0 == sheet && c.row >= anchor.1.row && c.col >= anchor.1.col {
                let (dr, dc) = ((c.row - anchor.1.row) as usize, (c.col - anchor.1.col) as usize);
                if dr < arr.rows && dc < arr.cols {
                    if self.iterate && self.tainted.contains(anchor) {
                        self.taint = true;
                    }
                    return arr.get(dr, dc).cloned().unwrap_or_default();
                }
            }
        }
        self.wb.sheet(sheet).map(|s| s.value(c)).unwrap_or_default()
    }
    fn range_values(&mut self, sheet: usize, range: RangeRef) -> Vec<Value> {
        let (h, w) = (range.height() as usize, range.width() as usize);
        let mut out = vec![Value::Empty; h * w];
        let Some(sh) = self.wb.sheet(sheet) else { return out };
        // Sparse walk: only stored cells; formula cells that are (or were) dirty go through
        // `compute`, constants are copied straight from the store.
        let mut formulas: Vec<(usize, CellRef)> = Vec::new();
        for (c, cell) in sh.cells.iter_range(range) {
            let i = (c.row - range.start.row) as usize * w + (c.col - range.start.col) as usize;
            if cell.formula.is_some() {
                formulas.push((i, c));
            } else if let Some(slot) = out.get_mut(i) {
                *slot = cell.value.clone();
            }
        }
        for (c, v) in sh.spill.range(range.start..=range.end) {
            if range.contains(*c) {
                let i = (c.row - range.start.row) as usize * w + (c.col - range.start.col) as usize;
                if let Some(slot) = out.get_mut(i)
                    && slot.is_empty()
                {
                    *slot = v.clone();
                }
            }
        }
        for (i, c) in formulas {
            let v = self.cell_value(sheet, c);
            if let Some(slot) = out.get_mut(i) {
                *slot = v;
            }
        }
        if !self.spills.is_empty() {
            let new: Vec<(usize, Value)> = self
                .spills
                .iter()
                .filter(|(a, _)| a.0 == sheet)
                .flat_map(|(a, arr)| {
                    let mut v = Vec::new();
                    for r in 0..arr.rows {
                        for cc in 0..arr.cols {
                            let c = CellRef::new(a.1.row.saturating_add(r as u32), a.1.col.saturating_add(cc as u32));
                            if c != a.1 && range.contains(c) {
                                v.push((
                                    (c.row - range.start.row) as usize * w + (c.col - range.start.col) as usize,
                                    arr.get(r, cc).cloned().unwrap_or_default(),
                                ));
                            }
                        }
                    }
                    v
                })
                .collect();
            // Iterating: an array spilled by a formula that read a previous value.
            if self.iterate
                && self.spills.iter().any(|(a, arr)| {
                    let end = CellRef::new(
                        a.1.row.saturating_add((arr.rows as u32).saturating_sub(1)),
                        a.1.col.saturating_add((arr.cols as u32).saturating_sub(1)),
                    );
                    a.0 == sheet && self.tainted.contains(a) && RangeRef::new(a.1, end).intersects(&range)
                })
            {
                self.taint = true;
            }
            for (i, v) in new {
                if let Some(slot) = out.get_mut(i) {
                    *slot = v;
                }
            }
        }
        out
    }
    fn visit_range(&mut self, sheet: usize, range: RangeRef, f: &mut dyn FnMut(&Value) -> bool) -> bool {
        let wb = self.wb;
        let Some(sh) = wb.sheet(sheet) else { return false };
        // Formula cells first, in row-major order like `range_values`: reading one calculates it
        // and the pass remembers the value, so the walk below gets the same value again without
        // this method holding any.
        for (c, cell) in sh.cells.iter_range(range) {
            if cell.formula.is_some() {
                let _ = self.cell_value(sheet, c);
            }
        }
        // Spilled values sit between the stored cells (and the formulas just calculated may have
        // produced some): leave such blocks to `range_values`.
        if self.spills.keys().any(|a| a.0 == sheet) || sh.spill.range(range.start..=range.end).any(|(c, _)| range.contains(*c)) {
            return false;
        }
        for (c, cell) in sh.cells.iter_range(range) {
            let go = if cell.formula.is_some() { f(&self.cell_value(sheet, c)) } else { f(&cell.value) };
            if !go {
                break;
            }
        }
        true
    }
    fn used_range(&mut self, sheet: usize) -> Option<RangeRef> {
        let wb = self.wb;
        *self.used.entry(sheet).or_insert_with(|| wb.sheet(sheet).and_then(|s| s.used_range()))
    }
    fn now_serial(&self) -> f64 {
        self.now
    }
    fn random(&mut self) -> f64 {
        // xorshift64*
        let mut x = *self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

impl Calc {
    pub fn new() -> Self {
        Self::default()
    }

    fn now(&self) -> f64 {
        if let Some(n) = self.fixed_now {
            return n;
        }
        now_serial()
    }

    /// Rebuilds the graph from scratch. Consecutive formulas down a column that are shifted
    /// copies of one tree become one run in the graph (see [`Run`]).
    pub fn rebuild(&mut self, wb: &Workbook) {
        self.build(wb, true);
    }

    fn build(&mut self, wb: &Workbook, runs: bool) {
        self.graph = Graph::default();
        for (si, sheet) in wb.sheets.iter().enumerate() {
            let formula = |row: u32, col: u32| sheet.cell(CellRef::new(row, col)).and_then(|c| c.formula.as_ref()).and_then(|f| f.expr());
            let mut open: HashMap<u32, PendingRun> = HashMap::default();
            for (c, cell) in sheet.cells.iter() {
                let Some(f) = &cell.formula else { continue };
                let template = f.template().filter(|_| runs);
                if let Some((base, dr, dc)) = template {
                    let origin = (i64::from(c.row) - dr, i64::from(c.col) - dc);
                    if let Some(p) = open.get_mut(&c.col)
                        && std::sync::Arc::ptr_eq(&p.base, base)
                        && p.origin == origin
                        && p.last.checked_add(1) == Some(c.row)
                    {
                        p.last = c.row;
                        continue;
                    }
                    let next = PendingRun { base: base.clone(), origin, first: c.row, last: c.row };
                    if let Some(p) = open.insert(c.col, next) {
                        self.graph.add_pending(wb, si, c.col, p, &|row| formula(row, c.col));
                    }
                    continue;
                }
                if let Some(p) = open.remove(&c.col) {
                    self.graph.add_pending(wb, si, c.col, p, &|row| formula(row, c.col));
                }
                if let Some(e) = f.expr() {
                    let (areas, dynamic) = precedents(wb, si, &e);
                    self.graph.insert((si, c), Node { areas, dynamic });
                }
            }
            for (col, p) in open {
                self.graph.add_pending(wb, si, col, p, &|row| formula(row, col));
            }
        }
        self.graph.settle();
    }

    /// Recalculates every formula (F9 / Ctrl+Alt+F9, and after loading).
    pub fn recalc_all(&mut self, wb: &mut Workbook) {
        self.rebuild(wb);
        self.calculate_built(wb);
    }

    /// Prepares a workbook just read from a file. When `calculated` (the file's values come from
    /// a full calculation and it asks for none on open, as a workbook Excel saved), its values are
    /// kept, the way Excel opens such a file: only the graph is built. They are calculated anyway
    /// (as [`Calc::recalc_all`] does) when a formula has no value, or is volatile or has
    /// references only known while calculating (the graph's dynamic formulas), whose values
    /// could be different now. A formula whose value is `#NAME?` (the program that saved the
    /// file didn't have one of its functions, as Excel 2019 hasn't XLOOKUP) is calculated, with
    /// what depends on it. Returns whether the file's values were kept.
    pub fn open(&mut self, wb: &mut Workbook, calculated: bool) -> bool {
        self.rebuild(wb);
        if calculated && self.graph.dynamic.is_empty() && values_are_current(wb) {
            let unknown: Vec<Key> =
                self.graph.keys().filter(|k| wb.sheet(k.0).and_then(|s| s.value_ref(k.1)) == Some(&Value::Error(CellError::Name))).collect();
            self.last_recalc_cells = 0;
            if !unknown.is_empty() {
                let dirty = self.dirty_closure(wb, &unknown);
                self.run(wb, dirty, false);
            }
            return true;
        }
        self.calculate_built(wb);
        false
    }

    /// Every formula of a just-built graph calculated.
    fn calculate_built(&mut self, wb: &mut Workbook) {
        // Spills are laid out again from the formulas where they are now (a structural edit moves
        // or deletes the formulas, not the values they spilled).
        for i in 0..wb.sheets.len() {
            if wb.sheet(i).is_some_and(|s| !s.spill_ranges.is_empty() || !s.spill.is_empty())
                && let Some(sh) = wb.sheet_mut(i)
            {
                sh.spill.clear();
                sh.spill_ranges.clear();
            }
        }
        let all: Vec<Key> = self.graph.keys().collect();
        self.run(wb, all, true);
    }

    /// Updates the graph for cells whose content changed and recalculates what depends on them.
    pub fn cells_changed(&mut self, wb: &mut Workbook, changed: &[Key]) {
        let mut cleared: Vec<(usize, RangeRef)> = Vec::new();
        for &k in changed {
            let formula = wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.clone());
            match formula.and_then(|f| f.expr()) {
                Some(e) => {
                    let (areas, dynamic) = precedents(wb, k.0, &e);
                    self.graph.insert(k, Node { areas, dynamic });
                }
                None => {
                    self.graph.remove(k);
                    // A formula replaced by a constant (or cleared) takes its spilled values with it.
                    if let Some(sh) = wb.sheet_mut(k.0)
                        && let Some(r) = sh.spill_ranges.remove(&k.1)
                    {
                        for c in r.iter() {
                            sh.spill.remove(&c);
                        }
                        cleared.push((k.0, r));
                    }
                    // A formula that doesn't parse evaluates to #NAME?.
                    if let Some(cell) = wb.sheet_mut(k.0).and_then(|s| s.cells.get_mut(k.1))
                        && cell.formula.is_some()
                    {
                        cell.value = Value::Error(CellError::Name);
                    }
                }
            }
        }
        self.graph.settle();
        if wb.calc.mode == CalcMode::Manual {
            // Only the edited formulas themselves are evaluated.
            let own: Vec<Key> = changed.iter().copied().filter(|k| self.graph.contains(*k)).collect();
            self.run(wb, own, false);
            return;
        }
        let mut seeds: Vec<Key> = changed.to_vec();
        let mut emptied: Vec<Key> = Vec::new();
        // Typing into (or clearing) a cell a dynamic array spills over (or would spill over)
        // re-evaluates the anchor, which may now be blocked or unblocked.
        for &k in changed {
            if let Some(sh) = wb.sheet(k.0) {
                for (anchor, r) in &sh.spill_ranges {
                    if *anchor != k.1 && r.contains(k.1) {
                        seeds.push((k.0, *anchor));
                    }
                }
                for (c, cell) in sh.cells.iter_range(RangeRef::new(CellRef::new(k.1.row.saturating_sub(64), k.1.col.saturating_sub(64)), k.1)) {
                    if c != k.1 && cell.value == Value::Error(CellError::Spill) {
                        seeds.push((k.0, c));
                    }
                }
                if !sh.cells.has(k.1) {
                    match emptied.iter_mut().find(|x| x.0 == k.0) {
                        Some(x) => x.1 = CellRef::new(x.1.row.max(k.1.row), x.1.col.max(k.1.col)),
                        None => emptied.push(k),
                    }
                }
            }
        }
        // An emptied cell can unblock an anchor any distance above or to the left of it.
        for (si, end) in emptied {
            for (s, c) in self.graph.keys() {
                if s == si && c.row <= end.row && c.col <= end.col && wb.sheet(s).is_some_and(|sh| sh.value(c) == Value::Error(CellError::Spill)) {
                    seeds.push((s, c));
                }
            }
        }
        // Spill areas of changed anchors also change.
        for &k in changed {
            if let Some(r) = wb.sheet(k.0).and_then(|s| s.spill_ranges.get(&k.1)) {
                seeds.extend(r.iter().map(|c| (k.0, c)));
            }
        }
        for (si, r) in &cleared {
            seeds.extend(r.iter().map(|c| (*si, c)));
        }
        let t0 = prof_now();
        let dirty = self.dirty_closure(wb, &seeds);
        let t1 = prof_now();
        let n = dirty.len();
        self.run(wb, dirty, false);
        if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
            eprintln!("recalc: {} seeds, closure {} cells {:.1} ms, run {:.1} ms", seeds.len(), n, t1 - t0, prof_now() - t1);
        }
    }

    fn dirty_closure(&self, _wb: &Workbook, seeds: &[Key]) -> Vec<Key> {
        let mut seen: HashSet<Key> = HashSet::default();
        let mut queue: VecDeque<Key> = VecDeque::new();
        for k in seeds {
            if self.graph.contains(*k) && seen.insert(*k) {
                queue.push_back(*k);
            }
            let mut deps = Vec::new();
            self.graph.dependents(k.0, k.1, &mut deps);
            for d in deps {
                if seen.insert(d) {
                    queue.push_back(d);
                }
            }
        }
        for k in &self.graph.dynamic {
            if seen.insert(*k) {
                queue.push_back(*k);
            }
        }
        let mut deps = Vec::new();
        while let Some(k) = queue.pop_front() {
            deps.clear();
            self.graph.dependents(k.0, k.1, &mut deps);
            for d in deps.drain(..) {
                if seen.insert(d) {
                    queue.push_back(d);
                }
            }
        }
        seen.into_iter().collect()
    }

    /// Evaluates the dirty set and writes results (and spills) back.
    fn run(&mut self, wb: &mut Workbook, mut dirty: Vec<Key>, full: bool) {
        let mut dynamic_again = !self.graph.dynamic.is_empty();
        for _round in 0..8 {
            if dirty.is_empty() {
                break;
            }
            // Row-major order is a good topological guess; on-demand evaluation fixes the rest.
            dirty.sort_by_key(|(s, c)| (*s, c.row, c.col));
            let slots = PassSlots::new(wb, &dirty);
            let now = self.now();
            let mut rng = self.rng;
            let tp = prof_now();
            let (results, spills, cycles) = {
                let mut host = PassHost::new(wb, slots, &mut rng, now);
                host.iterate = wb.calc.iterative;
                for k in &dirty {
                    host.compute(*k);
                }
                if host.iterate && !host.members.is_empty() {
                    let passes = host.iterate_loops(&dirty, wb.calc.max_iterations, wb.calc.max_change);
                    if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                        eprintln!("  iterative calculation: {} passes over {} formulas", passes, host.tainted.len());
                    }
                }
                (host.slots, host.spills, host.cycles)
            };
            self.rng = rng;
            if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                eprintln!("  eval {} formulas {:.1} ms", results.len(), prof_now() - tp);
            }
            let tw = prof_now();
            self.last_recalc_cells = results.len();
            self.circular = cycles;
            // Write back.
            let mut spill_changes: Vec<Key> = Vec::new();
            let mut freed: Vec<(usize, RangeRef)> = Vec::new();
            // Only values that changed are written: a write copies the cell's band (and the sheet)
            // out of every snapshot that shares them (the undo history, the saved state), so a
            // recalculation that rewrote unchanged values would duplicate every band of formulas.
            for (k, v) in results.into_results() {
                if wb.sheet(k.0).and_then(|s| s.cell(k.1)).is_none_or(|cell| cell.value == v) {
                    continue;
                }
                if let Some(cell) = wb.sheet_mut(k.0).and_then(|s| s.cells.get_mut(k.1)) {
                    cell.value = v;
                }
            }
            for k in &dirty {
                let new = spills.get(k);
                let Some(old) = wb.sheet(k.0).map(|s| s.spill_ranges.get(&k.1).copied()) else { continue };
                if old.is_none() && new.is_none() {
                    continue;
                }
                let Some(sheet) = wb.sheet_mut(k.0) else { continue };
                if let Some(r) = old {
                    for c in r.iter() {
                        sheet.spill.remove(&c);
                    }
                    sheet.spill_ranges.remove(&k.1);
                }
                let mut changed_range = old;
                if let Some(arr) = new {
                    let end = CellRef::new(k.1.row.saturating_add(arr.rows as u32 - 1), k.1.col.saturating_add(arr.cols as u32 - 1));
                    let range = RangeRef::new(k.1, end);
                    let blocked = end.row >= gridcraft_core::MAX_ROWS
                        || end.col >= gridcraft_core::MAX_COLS
                        || range.iter().any(|c| c != k.1 && (sheet.cells.has(c) || sheet.spill.contains_key(&c)))
                        || sheet.merges.iter().any(|m| m.intersects(&range))
                        || sheet.tables.iter().any(|t| t.range.contains(k.1));
                    if blocked {
                        if let Some(cell) = sheet.cells.get_mut(k.1) {
                            cell.value = Value::Error(CellError::Spill);
                        }
                    } else {
                        for (i, c) in range.iter().enumerate() {
                            if c != k.1 {
                                sheet.spill.insert(c, arr.data.get(i).cloned().unwrap_or_default());
                            }
                        }
                        sheet.spill_ranges.insert(k.1, range);
                        changed_range = Some(changed_range.map_or(range, |o| o.union(&range)));
                    }
                }
                if let Some(r) = changed_range {
                    let mut deps = Vec::new();
                    self.graph.dependents_of_range(k.0, r, &mut deps);
                    spill_changes.extend(deps.into_iter().filter(|d| d != k));
                }
                if let Some(o) = old
                    && !sheet.spill_ranges.get(&k.1).is_some_and(|n| n.contains(o.start) && n.contains(o.end))
                {
                    freed.push((k.0, o));
                }
            }
            // Formulas with references only known while evaluating (INDIRECT, OFFSET) may have read
            // a spill area before it was laid out: they are evaluated once more.
            if dynamic_again && !spills.is_empty() {
                dynamic_again = false;
                spill_changes.extend(self.graph.dynamic.iter().copied());
            }
            // An area a formula no longer spills over may unblock another formula's array.
            for (si, o) in freed {
                for (s, c) in self.graph.keys() {
                    if s == si
                        && c.row <= o.end.row
                        && c.col <= o.end.col
                        && wb.sheet(s).is_some_and(|sh| sh.value(c) == Value::Error(CellError::Spill))
                    {
                        spill_changes.push((s, c));
                    }
                }
            }
            if full {
                // Everything was already evaluated once; spill dependents need another pass.
            }
            if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                eprintln!("  write-back {:.1} ms", prof_now() - tw);
            }
            spill_changes.sort_by_key(|(s, c)| (*s, c.row, c.col));
            spill_changes.dedup();
            if spill_changes.is_empty() {
                break;
            }
            dirty = self.dirty_closure(wb, &spill_changes);
        }
    }
}

/// Whether every formula of `wb` holds a value, and no spill has to be laid out again (spilled
/// values aren't stored). Volatile formulas and those with references only known while
/// calculating are the graph's dynamic ones, which [`Calc::open`] checks.
fn values_are_current(wb: &Workbook) -> bool {
    wb.sheets
        .iter()
        .all(|sheet| sheet.spill_ranges.is_empty() && sheet.cells.iter().all(|(_, cell)| cell.formula.is_none() || !cell.value.is_empty()))
}

/// Current date-time as a serial (1900 system), from the system clock (browser clock on wasm).
/// UTC; the local offset isn't available without platform calls. Falls back to a fixed date only
/// when no clock is available.
pub fn now_serial() -> f64 {
    gridcraft_core::date::unix_now_ms().map_or(46302.5, gridcraft_core::date::serial_from_unix_ms)
}

/// Evaluates a formula in the context of a cell without storing it (conditional formats, data
/// validation, Evaluate Formula, Name Manager previews, the CLI `eval`).
pub fn evaluate(wb: &Workbook, sheet: usize, at: CellRef, formula: &str) -> Value {
    let body = formula.strip_prefix('=').unwrap_or(formula);
    let Ok(expr) = gridcraft_formula::parse(body) else { return Value::Error(CellError::Name) };
    evaluate_expr(wb, sheet, at, &expr)
}

pub fn evaluate_expr(wb: &Workbook, sheet: usize, at: CellRef, expr: &Expr) -> Value {
    let mut rng = 0x1234_5678_9ABC_DEF0u64;
    let mut host = PassHost::new(wb, PassSlots::default(), &mut rng, now_serial());
    let mut ev = Evaluator::new(&mut host, sheet, at);
    ev.value(expr)
}

#[allow(clippy::disallowed_methods)] // the clock is read only off wasm
fn prof_now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::{Duration, Instant};

    use gridcraft_core::{CellRef, RangeRef, Value};
    use gridcraft_model::{Cell, Formula, Workbook};

    use super::{Calc, Host, Intervals, Key, PassHost, PassSlots};

    /// A workbook whose columns hold runs of shifted copies (as fill or the XLSX reader make them)
    /// reading cells, ranges, wide ranges and another sheet, with breaks in the runs.
    fn run_workbook() -> Workbook {
        let mut wb = Workbook::new();
        wb.sheets.push(std::sync::Arc::new(gridcraft_model::Sheet::new("Other")));
        let fill = |wb: &mut Workbook, sheet: usize, col: u32, rows: std::ops::RangeInclusive<u32>, text: &str| {
            let first = *rows.start();
            let master = Formula::new(text);
            let sh = wb.sheet_mut(sheet).unwrap();
            for r in rows {
                let f = master.copied(i64::from(r - first), 0).unwrap();
                sh.cells.set(CellRef::new(r, col), Cell::formula(f));
            }
        };
        for r in 0..60u32 {
            wb.sheet_mut(0).unwrap().cells.set(CellRef::new(r, 0), Cell::value(Value::Number(f64::from(r))));
            wb.sheet_mut(1).unwrap().cells.set(CellRef::new(r, 1), Cell::value(Value::Number(f64::from(r) * 3.0)));
        }
        fill(&mut wb, 0, 1, 0..=49, "A1*2"); // B1:B50 relative cell
        fill(&mut wb, 0, 2, 0..=49, "SUM($A$1:A1)"); // C running total: top fixed, bottom moves
        fill(&mut wb, 0, 3, 1..=48, "A1+A3+$A$5"); // D2:D49 neighbours and a fixed cell
        fill(&mut wb, 0, 4, 0..=39, "SUM($A$1:$BZ$3)+SUM(A1:BZ1)"); // E wide areas, fixed and moving
        fill(&mut wb, 0, 5, 0..=29, "Other!B1+SUM(Other!B1:B3)"); // F another sheet
        fill(&mut wb, 0, 6, 0..=19, "SUM(A$10:A1)"); // G an area that turns over at row 10: no run
        fill(&mut wb, 0, 7, 0..=19, "INDIRECT(\"A\"&ROW())"); // H dynamic: no run
        fill(&mut wb, 0, 8, 1..=20, "A1"); // I2:I21
        fill(&mut wb, 0, 8, 22..=30, "A1"); // I23:I31 after a gap
        // Breaks: a value in a run, a different formula in a run.
        wb.sheet_mut(0).unwrap().cells.set(CellRef::new(25, 1), Cell::value(Value::Number(1.0)));
        wb.sheet_mut(0).unwrap().cells.set(CellRef::new(30, 2), Cell::formula(Formula::new("A31*3")));
        wb
    }

    fn deps_of(calc: &Calc, sheet: usize, c: CellRef) -> Vec<Key> {
        let mut v = Vec::new();
        calc.graph.dependents(sheet, c, &mut v);
        v.sort_unstable();
        v.dedup();
        v
    }

    fn deps_of_range(calc: &Calc, sheet: usize, r: RangeRef) -> Vec<Key> {
        let mut v = Vec::new();
        calc.graph.dependents_of_range(sheet, r, &mut v);
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The graph with runs answers every query as the graph of one node per formula does.
    fn assert_same_graph(wb: &Workbook, runs: &Calc, plain: &Calc) {
        let mut a: Vec<Key> = runs.graph.keys().collect();
        let mut b: Vec<Key> = plain.graph.keys().collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "formula cells");
        assert_eq!(runs.graph.len(), plain.graph.len());
        for k in &b {
            let mut pa = runs.graph.precedents_of(*k);
            let mut pb = plain.graph.precedents_of(*k);
            pa.sort_unstable_by_key(|x| (x.sheet, x.range.start, x.range.end));
            pb.sort_unstable_by_key(|x| (x.sheet, x.range.start, x.range.end));
            assert_eq!(pa, pb, "precedents of {k:?}");
            assert!(runs.graph.contains(*k));
        }
        for sheet in 0..wb.sheets.len() {
            for r in 0..64u32 {
                for c in 0..80u32 {
                    let at = CellRef::new(r, c);
                    assert_eq!(deps_of(runs, sheet, at), deps_of(plain, sheet, at), "dependents of {sheet}:{}", at.a1());
                }
            }
            let mut seed = 7u64;
            for _ in 0..300 {
                let r0 = (rng(&mut seed) % 64) as u32;
                let c0 = (rng(&mut seed) % 80) as u32;
                let r1 = r0 + (rng(&mut seed) % 12) as u32;
                let c1 = c0 + (rng(&mut seed) % 70) as u32;
                let range = RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1));
                assert_eq!(deps_of_range(runs, sheet, range), deps_of_range(plain, sheet, range), "dependents of {sheet}:{}", range.a1());
            }
        }
    }

    #[test]
    fn runs_in_the_graph_answer_like_one_node_per_formula() {
        let mut wb = run_workbook();
        let mut wb_plain = wb.clone();
        let mut runs = Calc::new();
        runs.recalc_all(&mut wb);
        let mut plain = Calc::new();
        plain.build(&wb_plain, false);
        assert!(!runs.graph.runs.is_empty() && plain.graph.runs.is_empty());
        // B (two runs around the value), C (two runs around the other formula), D, E, F, I (two):
        // G and H stay single formulas.
        assert_eq!(runs.graph.runs.len(), 9, "{:?}", runs.graph.runs.values().map(|r| (r.col, r.first, r.last)).collect::<Vec<_>>());
        assert!(runs.graph.nodes.len() < 50);
        assert_same_graph(&wb, &runs, &plain);

        // Edits take formulas out of their runs (and put new ones in as nodes).
        let edits = [
            (CellRef::new(10, 1), Some("A11*5")),
            (CellRef::new(0, 1), None),
            (CellRef::new(49, 2), Some("A1")),
            (CellRef::new(20, 3), None),
            (CellRef::new(5, 4), Some("SUM(A1:BZ1)")),
        ];
        for (at, input) in edits {
            let cell = match input {
                Some(t) => Cell::formula(Formula::new(t)),
                None => Cell::value(Value::Number(2.0)),
            };
            wb.sheet_mut(0).unwrap().cells.set(at, cell.clone());
            wb_plain.sheet_mut(0).unwrap().cells.set(at, cell);
            runs.cells_changed(&mut wb, &[(0, at)]);
            plain.cells_changed(&mut wb_plain, &[(0, at)]);
            assert_same_graph(&wb, &runs, &plain);
        }
        // And the values agree with a full recalculation from scratch.
        let mut fresh = wb.clone();
        Calc::new().recalc_all(&mut fresh);
        for (c, cell) in wb.sheet(0).unwrap().cells.iter() {
            assert_eq!(cell.value, fresh.sheet(0).unwrap().value(c), "{}", c.a1());
        }
    }

    #[test]
    fn edits_reach_the_formulas_of_runs() {
        let mut wb = run_workbook();
        let mut calc = Calc::new();
        calc.recalc_all(&mut wb);
        for (at, v) in [(CellRef::new(4, 0), 100.0), (CellRef::new(0, 0), -7.0), (CellRef::new(2, 1), 9.0), (CellRef::new(1, 60), 1000.0)] {
            wb.sheet_mut(0).unwrap().cells.set(at, Cell::value(Value::Number(v)));
            calc.cells_changed(&mut wb, &[(0, at)]);
        }
        wb.sheet_mut(1).unwrap().cells.set(CellRef::new(3, 1), Cell::value(Value::Number(-1.0)));
        calc.cells_changed(&mut wb, &[(1, CellRef::new(3, 1))]);
        let mut fresh = wb.clone();
        Calc::new().recalc_all(&mut fresh);
        for si in 0..2 {
            for (c, cell) in wb.sheet(si).unwrap().cells.iter() {
                assert_eq!(cell.value, fresh.sheet(si).unwrap().value(c), "{si}:{}", c.a1());
            }
        }
    }

    /// xorshift64 for repeatable pseudo-random cases.
    fn rng(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn found(iv: &Intervals<u32>, a: u32, b: u32) -> Vec<u32> {
        let mut v = Vec::new();
        iv.query(a, b, &mut |t| v.push(*t));
        v.sort_unstable();
        v
    }

    #[test]
    fn intervals_match_a_linear_scan() {
        let mut iv: Intervals<u32> = Intervals::default();
        let mut model: Vec<(u32, u32, u32)> = Vec::new();
        let mut s = 0x9E37_79B9_7F4A_7C15;
        for step in 0..20_000u32 {
            let roll = rng(&mut s) % 10;
            if roll < 6 || model.is_empty() {
                let r0 = (rng(&mut s) % 500) as u32;
                let r1 = r0 + (rng(&mut s) % 40) as u32;
                if !model.contains(&(r0, r1, step)) {
                    iv.insert(r0, r1, step);
                    model.push((r0, r1, step));
                }
            } else {
                let i = (rng(&mut s) % model.len() as u64) as usize;
                let (r0, r1, t) = model.swap_remove(i);
                iv.remove(r0, r1, t);
                if roll == 9 {
                    // Removed and put back before the next merge.
                    iv.insert(r0, r1, t);
                    model.push((r0, r1, t));
                }
            }
            if rng(&mut s).is_multiple_of(50) {
                iv.settle();
            }
            if step.is_multiple_of(97) {
                let a = (rng(&mut s) % 560) as u32;
                let b = a + (rng(&mut s) % 30) as u32;
                let mut want: Vec<u32> = model.iter().filter(|(r0, r1, _)| *r0 <= b && *r1 >= a).map(|(_, _, t)| *t).collect();
                want.sort_unstable();
                assert_eq!(found(&iv, a, b), want, "rows {a}..={b} at step {step}");
            }
        }
        iv.settle();
        for row in 0..560 {
            let mut want: Vec<u32> = model.iter().filter(|(r0, r1, _)| *r0 <= row && row <= *r1).map(|(_, _, t)| *t).collect();
            want.sort_unstable();
            assert_eq!(found(&iv, row, row), want, "row {row}");
        }
        for (r0, r1, t) in std::mem::take(&mut model) {
            iv.remove(r0, r1, t);
        }
        iv.settle();
        assert!(iv.is_empty());
    }

    #[test]
    fn interval_queries_cost_log_n_plus_matches() {
        // 40k one-row ranges (like =SUM(E1:J1) down a column) and 1k ranges over everything:
        // a point query visits O(log n) tree nodes per match, not every range in the column.
        let n = 40_000u32;
        let mut iv: Intervals<u32> = Intervals::default();
        for r in 0..n {
            iv.insert(r, r, r);
        }
        for t in 0..1_000 {
            iv.insert(0, n, n + t);
        }
        iv.settle();
        let mut worst = 0;
        for row in (0..n).step_by(7) {
            let mut k = 0;
            let visited = iv.query(row, row, &mut |_| k += 1);
            assert_eq!(k, 1_001);
            worst = worst.max(visited);
        }
        // 1,001 matches in a tree of depth 16: well under 4 nodes per match plus the path.
        assert!(worst < 4 * 1_001 + 64, "visited {worst} nodes");
        let mut k = 0;
        let visited = iv.query(n + 1, n + 5, &mut |_| k += 1);
        assert_eq!(k, 0);
        assert!(visited < 64, "visited {visited} nodes for no match");
    }

    /// `rows` rows of six numbers (E:J) and `=SUM(E{r}:J{r})` in K.
    fn sum_rows(rows: u32) -> Workbook {
        let mut wb = Workbook::new();
        if let Some(sh) = wb.sheet_mut(0) {
            for r in 0..rows {
                for c in 4..10 {
                    sh.cells.set(CellRef::new(r, c), Cell::value(Value::Number(f64::from(r % 97 + c))));
                }
                sh.cells.set(CellRef::new(r, 10), Cell::formula(Formula::new(&format!("=SUM(E{0}:J{0})", r + 1))));
            }
        }
        wb
    }

    /// Opening (full recalc), changing every value of a column, and re-entering every formula.
    #[allow(clippy::disallowed_methods)] // a native-only timing test: the clock is read off wasm
    fn open_and_bulk_edit(rows: u32) -> Duration {
        let mut wb = sum_rows(rows);
        let mut calc = Calc::new();
        let t = Instant::now();
        calc.recalc_all(&mut wb);
        let values: Vec<Key> = (0..rows).map(|r| (0, CellRef::new(r, 4))).collect();
        if let Some(sh) = wb.sheet_mut(0) {
            for (_, c) in &values {
                sh.cells.set(*c, Cell::value(Value::Number(1.0)));
            }
        }
        calc.cells_changed(&mut wb, &values);
        let formulas: Vec<Key> = (0..rows).map(|r| (0, CellRef::new(r, 10))).collect();
        calc.cells_changed(&mut wb, &formulas);
        let elapsed = t.elapsed();
        let last = wb.sheet(0).map(|s| s.value(CellRef::new(rows - 1, 10)));
        let r = rows - 1;
        assert_eq!(last, Some(Value::Number(f64::from(1 + (5..10).map(|c| r % 97 + c).sum::<u32>()))));
        elapsed
    }

    #[test]
    fn range_formulas_recalc_and_bulk_edit_correctly() {
        // The values behind the timing test below, checked on every run.
        open_and_bulk_edit(500);
    }

    #[test]
    #[ignore = "timing-sensitive; run with --ignored in release"]
    #[allow(clippy::disallowed_methods)]
    fn recalc_with_range_formulas_scales_linearly() {
        // Each row's SUM reads a range in the same columns. Recalculating on open used to walk
        // every row per range read (the used range), and finding a cell's dependents scanned
        // every range in its column: 4x the rows took 16x the time. Linear is 4x; allow 8x.
        let best = |rows| (0..3).map(|_| open_and_bulk_edit(rows)).min().unwrap_or_default();
        let small = best(5_000);
        let large = best(20_000);
        assert!(large < small * 8 + Duration::from_millis(100), "5k rows {small:?}, 20k rows {large:?}");
    }

    #[test]
    fn range_dependents_after_edits() {
        // Formulas that read ranges, wide ranges and whole columns, found again after edits.
        let mut wb = Workbook::new();
        let mut calc = Calc::new();
        let set = |wb: &mut Workbook, calc: &mut Calc, at: &str, input: &str| {
            let c = CellRef::parse(at).unwrap();
            let cell = if input.starts_with('=') { Cell::formula(Formula::new(input)) } else { Cell::value(Value::Number(input.parse().unwrap())) };
            wb.sheet_mut(0).unwrap().cells.set(c, cell);
            calc.cells_changed(wb, &[(0, c)]);
        };
        let num = |wb: &Workbook, at: &str| {
            let v = wb.sheet(0).unwrap().value(CellRef::parse(at).unwrap());
            v.as_f64().unwrap_or_else(|| panic!("{at} = {v:?}"))
        };
        set(&mut wb, &mut calc, "A1", "1");
        set(&mut wb, &mut calc, "B2", "2");
        set(&mut wb, &mut calc, "CZ3", "3");
        set(&mut wb, &mut calc, "DA5", "=SUM(A1:B2)");
        set(&mut wb, &mut calc, "DA6", "=SUM(A1:CZ3)");
        set(&mut wb, &mut calc, "DA7", "=SUM(B:B)");
        set(&mut wb, &mut calc, "DA8", "=SUM(2:2)");
        assert_eq!((num(&wb, "DA5"), num(&wb, "DA6"), num(&wb, "DA7"), num(&wb, "DA8")), (3.0, 6.0, 2.0, 2.0));
        set(&mut wb, &mut calc, "B2", "20");
        assert_eq!((num(&wb, "DA5"), num(&wb, "DA6"), num(&wb, "DA7"), num(&wb, "DA8")), (21.0, 24.0, 20.0, 20.0));
        // Re-entering a formula with another range drops the old one.
        set(&mut wb, &mut calc, "DA5", "=SUM(A1:A2)");
        set(&mut wb, &mut calc, "B2", "5");
        assert_eq!(num(&wb, "DA5"), 1.0);
        let mut deps = Vec::new();
        calc.graph.dependents(0, CellRef::parse("B2").unwrap(), &mut deps);
        deps.sort_unstable();
        let at = |s: &str| (0, CellRef::parse(s).unwrap());
        assert_eq!(deps, vec![at("DA6"), at("DA7"), at("DA8")]);
        deps.clear();
        calc.graph.dependents_of_range(0, RangeRef::new(CellRef::parse("A1").unwrap(), CellRef::parse("A9").unwrap()), &mut deps);
        deps.sort_unstable();
        deps.dedup();
        assert_eq!(deps, vec![at("DA5"), at("DA6"), at("DA8")]);
        set(&mut wb, &mut calc, "CZ3", "30");
        assert_eq!(num(&wb, "DA6"), 36.0);
        // Ranges on another sheet.
        wb.sheets.push(std::sync::Arc::new(gridcraft_model::Sheet::new("Data")));
        set(&mut wb, &mut calc, "DA9", "=SUM(Data!A1:A3)+SUM(Data!A:CZ)");
        assert_eq!(num(&wb, "DA9"), 0.0);
        wb.sheet_mut(1).unwrap().cells.set(CellRef::parse("A2").unwrap(), Cell::value(Value::Number(4.0)));
        calc.cells_changed(&mut wb, &[(1, CellRef::parse("A2").unwrap())]);
        assert_eq!(num(&wb, "DA9"), 8.0);
        deps.clear();
        calc.graph.dependents(1, CellRef::parse("A2").unwrap(), &mut deps);
        assert_eq!(deps, vec![at("DA9"), at("DA9")]);
    }

    /// For tests of `PassHost::visit_range`, which can't be reached from outside this module: the
    /// values it hands over for `range` while the formulas in `pending` (cell, formula: the
    /// formula the workbook holds there) have not been calculated yet, or `None` when it declines
    /// the block.
    pub(crate) fn probe_visit_range(wb: &Workbook, pending: &[(&str, &str)], range: RangeRef) -> Option<Vec<Value>> {
        let mut keys = Vec::new();
        for (at, formula) in pending {
            let at = CellRef::parse(at)?;
            let held = wb.sheet(0)?.cell(at)?.formula.as_ref()?.text().into_owned();
            assert_eq!(held, formula.strip_prefix('=').unwrap_or(formula));
            keys.push((0, at));
        }
        let slots = PassSlots::new(wb, &keys);

        // A cell in `pending` is calculated the first time something reads it, which is the state
        // of a real pass partway through; `visit_range` has to cope with the formulas it reads
        // being calculated under it.
        let mut rng = 1u64;
        let mut host = PassHost::new(wb, slots, &mut rng, 0.0);

        let mut seen = Vec::new();
        host.visit_range(0, range, &mut |v| {
            seen.push(v.clone());
            true
        })
        .then_some(seen)
    }
}
