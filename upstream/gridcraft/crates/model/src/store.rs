//! Copy-on-write sparse cell storage.
//!
//! Cells live in bands of 64 rows, each band behind an `Arc`. Cloning a store (an undo snapshot)
//! copies only the band pointers; editing a cell copies just its band. A band keeps its cells in
//! row-major order in one vector (and their columns in another), with an index of where each row
//! starts: a cell costs its own size and its column, with no per-row allocation.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use gridcraft_core::{CellRef, RangeRef};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::cell::Cell;

const BAND: u32 = 64;

/// The cells of the rows of one band, row-major: `cells[i]` is in column `cols[i]` of the row whose
/// entry in `rows` is the last one starting at or before `i`. `rows` lists every row with cells,
/// ascending, with the index of its first cell; a row's columns ascend. No row is empty.
#[derive(Clone, Debug, Default, PartialEq)]
struct Band {
    rows: Vec<(u32, u32)>,
    cols: Vec<u32>,
    cells: Vec<Cell>,
}

impl Band {
    fn with_capacity(rows: usize, cells: usize) -> Band {
        Band { rows: Vec::with_capacity(rows), cols: Vec::with_capacity(cells), cells: Vec::with_capacity(cells) }
    }
    fn find_row(&self, row: u32) -> Result<usize, usize> {
        self.rows.binary_search_by_key(&row, |(r, _)| *r)
    }
    /// Index range of the cells of the `ri`-th row.
    fn span(&self, ri: usize) -> Range<usize> {
        let start = self.rows.get(ri).map_or(self.cells.len(), |(_, s)| *s as usize);
        let end = self.rows.get(ri + 1).map_or(self.cells.len(), |(_, s)| *s as usize);
        start..end
    }
    /// Index of the cell at (`row`, `col`), or where it would go (with whether its row exists).
    fn find(&self, row: u32, col: u32) -> Result<usize, (usize, Option<usize>)> {
        match self.find_row(row) {
            Ok(ri) => {
                let span = self.span(ri);
                let cols = self.cols.get(span.clone()).unwrap_or_default();
                match cols.binary_search(&col) {
                    Ok(j) => Ok(span.start + j),
                    Err(j) => Err((span.start + j, Some(ri))),
                }
            }
            Err(ri) => Err((self.span(ri).start, None)),
        }
    }
    fn get(&self, row: u32, col: u32) -> Option<&Cell> {
        self.find(row, col).ok().and_then(|i| self.cells.get(i))
    }
    fn get_mut(&mut self, row: u32, col: u32) -> Option<&mut Cell> {
        let i = self.find(row, col).ok()?;
        self.cells.get_mut(i)
    }
    fn contains(&self, row: u32, col: u32) -> bool {
        self.find(row, col).is_ok()
    }
    /// Stores `cell` at (`row`, `col`), returning the cell it replaces.
    fn insert(&mut self, row: u32, col: u32, cell: Cell) -> Option<Cell> {
        match self.find(row, col) {
            Ok(i) => self.cells.get_mut(i).map(|slot| std::mem::replace(slot, cell)),
            Err((i, existing)) => {
                let ri = match existing {
                    Some(ri) => ri,
                    None => {
                        let ri = self.find_row(row).unwrap_or_else(|ri| ri);
                        self.rows.insert(ri, (row, i as u32));
                        ri
                    }
                };
                self.cols.insert(i, col);
                self.cells.insert(i, cell);
                for r in self.rows.iter_mut().skip(ri + 1) {
                    r.1 += 1;
                }
                None
            }
        }
    }
    fn remove(&mut self, row: u32, col: u32) -> Option<Cell> {
        let i = self.find(row, col).ok()?;
        let ri = self.find_row(row).ok()?;
        self.cols.remove(i);
        let old = self.cells.remove(i);
        for r in self.rows.iter_mut().skip(ri + 1) {
            r.1 -= 1;
        }
        if self.span(ri).is_empty() {
            self.rows.remove(ri);
        }
        Some(old)
    }
    /// The cells of rows `r0..=r1` in columns `c0..=c1`, row-major.
    fn range(&self, r0: u32, r1: u32, c0: u32, c1: u32) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        let a = self.rows.partition_point(|(r, _)| *r < r0);
        let b = self.rows.partition_point(|(r, _)| *r <= r1).max(a);
        (a..b).flat_map(move |ri| {
            let span = self.span(ri);
            let row = self.rows.get(ri).map_or(0, |(r, _)| *r);
            let cols = self.cols.get(span.clone()).unwrap_or_default();
            let x = span.start + cols.partition_point(|c| *c < c0);
            let y = (span.start + cols.partition_point(|c| *c <= c1)).max(x);
            self.cols.get(x..y).unwrap_or_default().iter().zip(self.cells.get(x..y).unwrap_or_default()).map(move |(c, cell)| (CellRef::new(row, *c), cell))
        })
    }
    fn iter(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        self.range(0, u32::MAX, 0, u32::MAX)
    }
    /// Each row's first and last column.
    fn row_ends(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        (0..self.rows.len()).filter_map(|ri| {
            let span = self.span(ri);
            Some((*self.cols.get(span.start)?, *self.cols.get(span.end.checked_sub(1)?)?))
        })
    }
    /// Rebuilt from cells in row-major order.
    fn from_sorted(cells: impl IntoIterator<Item = (CellRef, Cell)>) -> Band {
        let mut b = Band::default();
        for (c, cell) in cells {
            if b.rows.last().is_none_or(|(r, _)| *r != c.row) {
                b.rows.push((c.row, b.cells.len() as u32));
            }
            b.cols.push(c.col);
            b.cells.push(cell);
        }
        b
    }
    /// Takes out the cells of rows `r0..=r1` in columns `c0..=c1` (row-major), keeping the rest.
    fn take(&mut self, r0: u32, r1: u32, c0: u32, c1: u32) -> Vec<(CellRef, Cell)> {
        let all = std::mem::take(self);
        let mut out = Vec::new();
        let mut kept = Vec::with_capacity(all.cells.len());
        let mut rows = all.rows.iter().peekable();
        let mut row = 0;
        for (i, (col, cell)) in all.cols.into_iter().zip(all.cells).enumerate() {
            while rows.peek().is_some_and(|(_, s)| *s as usize <= i) {
                row = rows.next().map_or(row, |(r, _)| *r);
            }
            let at = CellRef::new(row, col);
            if (r0..=r1).contains(&row) && (c0..=c1).contains(&col) { out.push((at, cell)) } else { kept.push((at, cell)) }
        }
        *self = Band::from_sorted(kept);
        out
    }
    fn shrink_to_fit(&mut self) {
        self.rows.shrink_to_fit();
        self.cols.shrink_to_fit();
        self.cells.shrink_to_fit();
    }
}

/// Serialized as `{"rows": {row: {col: cell}}}`, as it was when bands were maps of maps.
impl Serialize for Band {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct Rows<'a>(&'a Band);
        struct Row<'a>(&'a Band, usize);
        impl Serialize for Rows<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_map(self.0.rows.iter().enumerate().map(|(ri, (r, _))| (r, Row(self.0, ri))))
            }
        }
        impl Serialize for Row<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let span = self.0.span(self.1);
                s.collect_map(self.0.cols.get(span.clone()).unwrap_or_default().iter().zip(self.0.cells.get(span).unwrap_or_default()))
            }
        }
        let mut st = s.serialize_struct("Band", 1)?;
        st.serialize_field("rows", &Rows(self))?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for Band {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Band, D::Error> {
        #[derive(Deserialize)]
        struct Data {
            rows: BTreeMap<u32, BTreeMap<u32, Cell>>,
        }
        let data = Data::deserialize(d)?;
        Ok(Band::from_sorted(data.rows.into_iter().flat_map(|(r, cols)| cols.into_iter().map(move |(c, cell)| (CellRef::new(r, c), cell)))))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellStore {
    bands: BTreeMap<u32, Arc<Band>>,
    len: usize,
}

impl CellStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, c: CellRef) -> Option<&Cell> {
        self.bands.get(&(c.row / BAND))?.get(c.row, c.col)
    }
    /// Mutable access (copies the band if shared).
    pub fn get_mut(&mut self, c: CellRef) -> Option<&mut Cell> {
        let band = self.bands.get_mut(&(c.row / BAND))?;
        if !band.contains(c.row, c.col) {
            return None;
        }
        Arc::make_mut(band).get_mut(c.row, c.col)
    }
    /// Stores a cell; a default (empty, unstyled) cell is removed instead.
    pub fn set(&mut self, c: CellRef, cell: Cell) {
        if cell.is_blank() {
            self.remove(c);
            return;
        }
        let key = c.row / BAND;
        if !self.bands.contains_key(&key)
            && let Some(prev) = key.checked_sub(1).and_then(|k| self.bands.get(&k))
        {
            // A band started right after another (a reader filling a sheet) is sized like it.
            // Grown cell by cell, its vectors would be reallocated at every doubling: large
            // blocks, which fragment the heap and made loading a large sheet twice as slow.
            self.bands.insert(key, Arc::new(Band::with_capacity(prev.rows.len(), prev.cells.len())));
        }
        let band = Arc::make_mut(self.bands.entry(key).or_default());
        if band.insert(c.row, c.col, cell).is_none() {
            self.len += 1;
        }
    }
    pub fn remove(&mut self, c: CellRef) -> Option<Cell> {
        let key = c.row / BAND;
        let band = self.bands.get_mut(&key)?;
        if !band.contains(c.row, c.col) {
            return None;
        }
        let b = Arc::make_mut(band);
        let old = b.remove(c.row, c.col);
        if b.cells.is_empty() {
            self.bands.remove(&key);
        }
        if old.is_some() {
            self.len -= 1;
        }
        old
    }
    /// Gives back the room bands keep for cells added later (a band filled cell by cell, as
    /// readers fill it, may keep up to as much again). Bands shared with a snapshot are left as
    /// they are, so this never copies one.
    pub fn shrink_to_fit(&mut self) {
        for band in self.bands.values_mut() {
            if let Some(b) = Arc::get_mut(band) {
                b.shrink_to_fit();
            }
        }
    }
    /// Cells in `r`, row-major. Efficient for sparse data: skips empty bands and rows.
    pub fn iter_range(&self, r: RangeRef) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        let (b0, b1) = (r.start.row / BAND, r.end.row / BAND);
        self.bands.range(b0..=b1).flat_map(move |(_, band)| band.range(r.start.row, r.end.row, r.start.col, r.end.col))
    }
    pub fn iter(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        self.bands.values().flat_map(|band| band.iter())
    }
    /// Cells of one row in `c0..=c1`.
    pub fn row(&self, row: u32, c0: u32, c1: u32) -> impl Iterator<Item = (u32, &Cell)> + '_ {
        self.bands.get(&(row / BAND)).into_iter().flat_map(move |b| b.range(row, row, c0, c1).map(|(c, cell)| (c.col, cell)))
    }
    /// Smallest range containing every stored cell, `None` when empty.
    pub fn used_range(&self) -> Option<RangeRef> {
        let first_band = self.bands.values().next()?;
        let last_band = self.bands.values().next_back()?;
        let r0 = first_band.rows.first()?.0;
        let r1 = last_band.rows.last()?.0;
        let mut c0 = u32::MAX;
        let mut c1 = 0;
        for band in self.bands.values() {
            for (a, b) in band.row_ends() {
                c0 = c0.min(a);
                c1 = c1.max(b);
            }
        }
        if c0 == u32::MAX {
            return None;
        }
        Some(RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
    }
    /// Last stored row in column `col` at or above `below_or_at` (for Ctrl+arrow navigation).
    pub fn has(&self, c: CellRef) -> bool {
        self.get(c).is_some_and(|cell| !cell.value.is_empty() || cell.formula.is_some())
    }
    /// Positions whose cells differ between `self` and `other` (stored on one side only, or not
    /// equal). Bands both stores share are skipped, so comparing an edited copy with the store
    /// it was cloned from only visits the bands the edit touched.
    pub fn diff(&self, other: &CellStore) -> Vec<CellRef> {
        let empty = Band::default();
        let mut keys: Vec<u32> = self.bands.keys().chain(other.bands.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        let mut out = Vec::new();
        for k in keys {
            let (a, b) = (self.bands.get(&k), other.bands.get(&k));
            if let (Some(a), Some(b)) = (a, b)
                && Arc::ptr_eq(a, b)
            {
                continue;
            }
            let (a, b) = (a.map_or(&empty, |x| x), b.map_or(&empty, |x| x));
            for (c, cell) in a.iter() {
                if b.get(c.row, c.col) != Some(cell) {
                    out.push(c);
                }
            }
            for (c, _) in b.iter() {
                if !a.contains(c.row, c.col) {
                    out.push(c);
                }
            }
        }
        out
    }
    /// Takes out every cell of rows `r0..=r1` in columns `c0..=c1`, row-major. Only bands holding
    /// such a cell are touched (copied when shared).
    fn take_block(&mut self, r0: u32, r1: u32, c0: u32, c1: u32) -> Vec<(CellRef, Cell)> {
        let hit: Vec<u32> = self.bands.range(r0 / BAND..=r1 / BAND).filter(|(_, b)| b.range(r0, r1, c0, c1).next().is_some()).map(|(k, _)| *k).collect();
        let mut out = Vec::new();
        for k in hit {
            let Some(band) = self.bands.get_mut(&k) else { continue };
            let b = Arc::make_mut(band);
            out.extend(b.take(r0, r1, c0, c1));
            if b.cells.is_empty() {
                self.bands.remove(&k);
            }
        }
        self.len -= out.len();
        out
    }
    /// Stores `cells` (row-major, each position once) as [`CellStore::set`] stores each one: a
    /// cell replaces the one at its position, a blank cell removes it. Each band receiving cells
    /// is merged once.
    fn put_sorted(&mut self, cells: Vec<(CellRef, Cell)>) {
        let mut cells = cells.into_iter().peekable();
        while let Some(&(first, _)) = cells.peek() {
            let key = first.row / BAND;
            let mut incoming = Vec::new();
            while cells.peek().is_some_and(|(c, _)| c.row / BAND == key) {
                if let Some(x) = cells.next() {
                    incoming.push(x);
                }
            }
            let existing = self.bands.remove(&key).map(|b| Arc::try_unwrap(b).unwrap_or_else(|b| (*b).clone()));
            let mut merged: Vec<(CellRef, Cell)> = Vec::new();
            let mut old = existing.map(|mut b| b.take(0, u32::MAX, 0, u32::MAX)).unwrap_or_default().into_iter().peekable();
            let pos = |c: &CellRef| (c.row, c.col);
            for (c, cell) in incoming {
                while old.peek().is_some_and(|(o, _)| pos(o) < pos(&c)) {
                    merged.extend(old.next());
                }
                let replaced = old.next_if(|(o, _)| pos(o) == pos(&c)).is_some();
                if cell.is_blank() {
                    if replaced {
                        self.len -= 1;
                    }
                } else {
                    if !replaced {
                        self.len += 1;
                    }
                    merged.push((c, cell));
                }
            }
            merged.extend(old);
            if !merged.is_empty() {
                self.bands.insert(key, Arc::new(Band::from_sorted(merged)));
            }
        }
    }
    /// Removes every cell in `r` and returns them.
    pub fn take_range(&mut self, r: RangeRef) -> Vec<(CellRef, Cell)> {
        self.take_block(r.start.row, r.end.row, r.start.col, r.end.col)
    }
    /// Rows strictly greater than or equal to `from` moved by `delta` (positive = down). Rows that
    /// would land before `from`... callers delete the vacated block first.
    pub fn shift_rows(&mut self, from: u32, delta: i64) {
        // As if every cell of rows `from..` were taken out and each stored again `delta` rows
        // away, in row-major order.
        let moved = self.take_block(from, u32::MAX, 0, u32::MAX);
        self.put_sorted(moved.into_iter().filter_map(|(c, cell)| moved_to(c.row, delta, gridcraft_core::MAX_ROWS).map(|r| (CellRef::new(r, c.col), cell))).collect());
    }
    pub fn shift_cols(&mut self, from: u32, delta: i64) {
        let moved = self.take_block(0, u32::MAX, from, u32::MAX);
        self.put_sorted(moved.into_iter().filter_map(|(c, cell)| moved_to(c.col, delta, gridcraft_core::MAX_COLS).map(|col| (CellRef::new(c.row, col), cell))).collect());
    }
    /// Shifts cells within rows `r0..=r1` and columns `>= from` horizontally (Insert/Delete Cells
    /// with shift right/left).
    pub fn shift_cols_in_rows(&mut self, r0: u32, r1: u32, from: u32, delta: i64) {
        let moved = self.take_block(r0, r1, from, gridcraft_core::MAX_COLS - 1);
        self.put_sorted(moved.into_iter().filter_map(|(c, cell)| moved_to(c.col, delta, gridcraft_core::MAX_COLS).map(|col| (CellRef::new(c.row, col), cell))).collect());
    }
    /// Shifts cells within columns `c0..=c1` and rows `>= from` vertically.
    pub fn shift_rows_in_cols(&mut self, c0: u32, c1: u32, from: u32, delta: i64) {
        let moved = self.take_block(from, gridcraft_core::MAX_ROWS - 1, c0, c1);
        self.put_sorted(moved.into_iter().filter_map(|(c, cell)| moved_to(c.row, delta, gridcraft_core::MAX_ROWS).map(|r| (CellRef::new(r, c.col), cell))).collect());
    }
}

/// `at` moved by `delta`, if that stays below `max`.
fn moved_to(at: u32, delta: i64, max: u32) -> Option<u32> {
    let n = i64::from(at) + delta;
    (0..i64::from(max)).contains(&n).then_some(n as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gridcraft_core::Value;

    fn c(r: u32, col: u32) -> CellRef {
        CellRef::new(r, col)
    }

    #[test]
    fn set_get_remove() {
        let mut s = CellStore::new();
        s.set(c(0, 0), Cell::value(1.0.into()));
        s.set(c(100, 5), Cell::value("x".into()));
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(c(100, 5)).map(|x| x.value.clone()), Some(Value::from("x")));
        assert_eq!(s.used_range().unwrap().a1(), "A1:F101");
        s.set(c(0, 0), Cell::default());
        assert_eq!(s.len(), 1);
        assert!(s.remove(c(100, 5)).is_some());
        assert!(s.is_empty());
        assert!(s.used_range().is_none());
    }

    #[test]
    fn cow_snapshots() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        s.set(c(5, 0), Cell::value(99.0.into()));
        assert_eq!(snap.get(c(5, 0)).unwrap().value, Value::Number(5.0));
        assert_eq!(s.get(c(5, 0)).unwrap().value, Value::Number(99.0));
    }

    #[test]
    fn diff_visits_changed_bands() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        assert!(s.diff(&snap).is_empty());
        s.set(c(5, 0), Cell::value(99.0.into()));
        s.set(c(700, 3), Cell::value("new".into()));
        s.remove(c(900, 0));
        assert_eq!(snap.diff(&s), [c(5, 0), c(700, 3), c(900, 0)]);
    }

    #[test]
    fn range_iteration_order() {
        let mut s = CellStore::new();
        s.set(c(70, 2), Cell::value(3.0.into()));
        s.set(c(1, 1), Cell::value(1.0.into()));
        s.set(c(1, 3), Cell::value(2.0.into()));
        s.set(c(500, 0), Cell::value(4.0.into()));
        let v: Vec<f64> = s.iter_range(RangeRef::parse("A1:D100").unwrap()).map(|(_, x)| x.value.as_f64().unwrap()).collect();
        assert_eq!(v, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn shifting() {
        let mut s = CellStore::new();
        s.set(c(2, 0), Cell::value(1.0.into()));
        s.set(c(5, 0), Cell::value(2.0.into()));
        s.shift_rows(3, 2);
        assert!(s.get(c(7, 0)).is_some());
        assert!(s.get(c(2, 0)).is_some());
        s.shift_cols(0, 1);
        assert!(s.get(c(7, 1)).is_some());
    }

    /// The store as it was with a B-tree map per row: the reference the bands are tested
    /// against.
    #[allow(dead_code)]
    mod reference {
        use std::collections::BTreeMap;
        use std::sync::Arc;

        use gridcraft_core::{CellRef, RangeRef};
        use serde::{Deserialize, Serialize};

        use crate::cell::Cell;

        const BAND: u32 = 64;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct OldBand {
            rows: BTreeMap<u32, BTreeMap<u32, Cell>>,
        }

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct OldStore {
            bands: BTreeMap<u32, Arc<OldBand>>,
            len: usize,
        }

        impl OldStore {
            pub fn new() -> Self {
                Self::default()
            }
            pub fn len(&self) -> usize {
                self.len
            }
            pub fn is_empty(&self) -> bool {
                self.len == 0
            }
            pub fn get(&self, c: CellRef) -> Option<&Cell> {
                self.bands.get(&(c.row / BAND))?.rows.get(&c.row)?.get(&c.col)
            }
            pub fn get_mut(&mut self, c: CellRef) -> Option<&mut Cell> {
                let band = self.bands.get_mut(&(c.row / BAND))?;
                if !band.rows.get(&c.row).is_some_and(|r| r.contains_key(&c.col)) {
                    return None;
                }
                Arc::make_mut(band).rows.get_mut(&c.row)?.get_mut(&c.col)
            }
            pub fn set(&mut self, c: CellRef, cell: Cell) {
                if cell.is_blank() {
                    self.remove(c);
                    return;
                }
                let band = Arc::make_mut(self.bands.entry(c.row / BAND).or_default());
                if band.rows.entry(c.row).or_default().insert(c.col, cell).is_none() {
                    self.len += 1;
                }
            }
            pub fn remove(&mut self, c: CellRef) -> Option<Cell> {
                let key = c.row / BAND;
                let band = self.bands.get_mut(&key)?;
                if !band.rows.get(&c.row).is_some_and(|r| r.contains_key(&c.col)) {
                    return None;
                }
                let b = Arc::make_mut(band);
                let row = b.rows.get_mut(&c.row)?;
                let old = row.remove(&c.col);
                if row.is_empty() {
                    b.rows.remove(&c.row);
                }
                if b.rows.is_empty() {
                    self.bands.remove(&key);
                }
                if old.is_some() {
                    self.len -= 1;
                }
                old
            }
            pub fn iter_range(&self, r: RangeRef) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
                let (b0, b1) = (r.start.row / BAND, r.end.row / BAND);
                self.bands.range(b0..=b1).flat_map(move |(_, band)| {
                    band.rows
                        .range(r.start.row..=r.end.row)
                        .flat_map(move |(row, cols)| cols.range(r.start.col..=r.end.col).map(move |(col, cell)| (CellRef::new(*row, *col), cell)))
                })
            }
            pub fn iter(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
                self.bands
                    .values()
                    .flat_map(|band| band.rows.iter().flat_map(|(row, cols)| cols.iter().map(move |(col, cell)| (CellRef::new(*row, *col), cell))))
            }
            pub fn row(&self, row: u32, c0: u32, c1: u32) -> impl Iterator<Item = (u32, &Cell)> + '_ {
                self.bands.get(&(row / BAND)).and_then(|b| b.rows.get(&row)).into_iter().flat_map(move |cols| cols.range(c0..=c1).map(|(c, cell)| (*c, cell)))
            }
            pub fn used_range(&self) -> Option<RangeRef> {
                let first_band = self.bands.values().next()?;
                let last_band = self.bands.values().next_back()?;
                let r0 = *first_band.rows.keys().next()?;
                let r1 = *last_band.rows.keys().next_back()?;
                let mut c0 = u32::MAX;
                let mut c1 = 0;
                for band in self.bands.values() {
                    for cols in band.rows.values() {
                        if let (Some(a), Some(b)) = (cols.keys().next(), cols.keys().next_back()) {
                            c0 = c0.min(*a);
                            c1 = c1.max(*b);
                        }
                    }
                }
                if c0 == u32::MAX {
                    return None;
                }
                Some(RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
            }
            pub fn has(&self, c: CellRef) -> bool {
                self.get(c).is_some_and(|cell| !cell.value.is_empty() || cell.formula.is_some())
            }
            pub fn diff(&self, other: &OldStore) -> Vec<CellRef> {
                let empty = OldBand::default();
                let mut keys: Vec<u32> = self.bands.keys().chain(other.bands.keys()).copied().collect();
                keys.sort_unstable();
                keys.dedup();
                let mut out = Vec::new();
                for k in keys {
                    let (a, b) = (self.bands.get(&k), other.bands.get(&k));
                    if let (Some(a), Some(b)) = (a, b)
                        && Arc::ptr_eq(a, b)
                    {
                        continue;
                    }
                    let (a, b) = (a.map_or(&empty, |x| x), b.map_or(&empty, |x| x));
                    for (row, cols) in &a.rows {
                        for (col, cell) in cols {
                            if b.rows.get(row).and_then(|r| r.get(col)) != Some(cell) {
                                out.push(CellRef::new(*row, *col));
                            }
                        }
                    }
                    for (row, cols) in &b.rows {
                        for col in cols.keys() {
                            if !a.rows.get(row).is_some_and(|r| r.contains_key(col)) {
                                out.push(CellRef::new(*row, *col));
                            }
                        }
                    }
                }
                out
            }
            pub fn take_range(&mut self, r: RangeRef) -> Vec<(CellRef, Cell)> {
                let keys: Vec<CellRef> = self.iter_range(r).map(|(c, _)| c).collect();
                keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
            }
            pub fn shift_rows(&mut self, from: u32, delta: i64) {
                let moved: Vec<(CellRef, Cell)> = {
                    let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.row >= from).map(|(c, _)| c).collect();
                    keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
                };
                for (c, cell) in moved {
                    let nr = c.row as i64 + delta;
                    if (0..gridcraft_core::MAX_ROWS as i64).contains(&nr) {
                        self.set(CellRef::new(nr as u32, c.col), cell);
                    }
                }
            }
            pub fn shift_cols(&mut self, from: u32, delta: i64) {
                let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.col >= from).map(|(c, _)| c).collect();
                let moved: Vec<(CellRef, Cell)> = keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect();
                for (c, cell) in moved {
                    let nc = c.col as i64 + delta;
                    if (0..gridcraft_core::MAX_COLS as i64).contains(&nc) {
                        self.set(CellRef::new(c.row, nc as u32), cell);
                    }
                }
            }
            pub fn shift_cols_in_rows(&mut self, r0: u32, r1: u32, from: u32, delta: i64) {
                let range = RangeRef::new(CellRef::new(r0, from), CellRef::new(r1, gridcraft_core::MAX_COLS - 1));
                let moved = self.take_range(range);
                for (c, cell) in moved {
                    let nc = c.col as i64 + delta;
                    if (0..gridcraft_core::MAX_COLS as i64).contains(&nc) {
                        self.set(CellRef::new(c.row, nc as u32), cell);
                    }
                }
            }
            pub fn shift_rows_in_cols(&mut self, c0: u32, c1: u32, from: u32, delta: i64) {
                let range = RangeRef::new(CellRef::new(from, c0), CellRef::new(gridcraft_core::MAX_ROWS - 1, c1));
                let moved = self.take_range(range);
                for (c, cell) in moved {
                    let nr = c.row as i64 + delta;
                    if (0..gridcraft_core::MAX_ROWS as i64).contains(&nr) {
                        self.set(CellRef::new(nr as u32, c.col), cell);
                    }
                }
            }
        }
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn delta(&mut self) -> i64 {
            self.below(13) as i64 - 6
        }
    }

    fn cells(s: &CellStore) -> Vec<(CellRef, Cell)> {
        s.iter().map(|(c, x)| (c, x.clone())).collect()
    }
    fn old_cells(s: &reference::OldStore) -> Vec<(CellRef, Cell)> {
        s.iter().map(|(c, x)| (c, x.clone())).collect()
    }

    /// Every operation, on random cells (some of them blank cells left by `get_mut`), gives what
    /// the B-tree store gives: the same cells, count, ranges, diffs against snapshots and JSON.
    #[test]
    fn bands_behave_like_the_b_tree_store() {
        let style = crate::StyleId(3);
        for seed in 1..=40u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let (mut new, mut old) = (CellStore::new(), reference::OldStore::new());
            let (mut snap_new, mut snap_old) = (new.clone(), old.clone());
            for step in 0..400 {
                let at = c(rng.below(200) as u32, rng.below(24) as u32);
                match rng.below(14) {
                    0..=4 => {
                        let cell = match rng.below(6) {
                            0 => Cell::default(),
                            1 => Cell { value: Value::Empty, formula: None, style },
                            _ => Cell::value(Value::Number(rng.below(1000) as f64)),
                        };
                        new.set(at, cell.clone());
                        old.set(at, cell);
                    }
                    5 => assert_eq!(new.remove(at), old.remove(at)),
                    6 => {
                        // A cell made blank in place stays stored until it moves.
                        let blank = rng.below(2) == 0;
                        if let Some(x) = new.get_mut(at) {
                            x.value = if blank { Value::Empty } else { Value::Number(7.0) };
                            x.style = crate::StyleId::DEFAULT;
                        }
                        if let Some(x) = old.get_mut(at) {
                            x.value = if blank { Value::Empty } else { Value::Number(7.0) };
                            x.style = crate::StyleId::DEFAULT;
                        }
                    }
                    7 => {
                        let (from, d) = (rng.below(220) as u32, rng.delta());
                        new.shift_rows(from, d);
                        old.shift_rows(from, d);
                    }
                    8 => {
                        let (from, d) = (rng.below(26) as u32, rng.delta());
                        new.shift_cols(from, d);
                        old.shift_cols(from, d);
                    }
                    9 => {
                        let (r0, from, d) = (rng.below(200) as u32, rng.below(26) as u32, rng.delta());
                        let r1 = r0 + rng.below(80) as u32;
                        new.shift_cols_in_rows(r0, r1, from, d);
                        old.shift_cols_in_rows(r0, r1, from, d);
                    }
                    10 => {
                        let (c0, from, d) = (rng.below(24) as u32, rng.below(220) as u32, rng.delta());
                        let c1 = c0 + rng.below(6) as u32;
                        new.shift_rows_in_cols(c0, c1, from, d);
                        old.shift_rows_in_cols(c0, c1, from, d);
                    }
                    11 => {
                        let r = RangeRef::new(at, c(at.row + rng.below(70) as u32, at.col + rng.below(8) as u32));
                        assert_eq!(new.take_range(r), old.take_range(r));
                    }
                    12 => {
                        snap_new = new.clone();
                        snap_old = old.clone();
                    }
                    _ => new.shrink_to_fit(),
                }
                let ctx = format!("seed {seed} step {step}");
                assert_eq!(cells(&new), old_cells(&old), "{ctx}");
                assert_eq!(new.len(), old.len(), "{ctx}");
                assert_eq!(new.used_range(), old.used_range(), "{ctx}");
                assert_eq!(new.diff(&snap_new), old.diff(&snap_old), "{ctx}");
                let r = RangeRef::new(at, c(at.row + rng.below(40) as u32, at.col + rng.below(10) as u32));
                let a: Vec<_> = new.iter_range(r).map(|(c, x)| (c, x.clone())).collect();
                let b: Vec<_> = old.iter_range(r).map(|(c, x)| (c, x.clone())).collect();
                assert_eq!(a, b, "{ctx}");
                let a: Vec<_> = new.row(at.row, at.col.saturating_sub(3), at.col + 3).map(|(c, x)| (c, x.clone())).collect();
                let b: Vec<_> = old.row(at.row, at.col.saturating_sub(3), at.col + 3).map(|(c, x)| (c, x.clone())).collect();
                assert_eq!(a, b, "{ctx}");
                assert_eq!((new.get(at), new.has(at)), (old.get(at), old.has(at)), "{ctx}");
            }
            let json = serde_json::to_string(&new).unwrap();
            assert_eq!(json, serde_json::to_string(&old).unwrap(), "seed {seed}: the same JSON");
            assert_eq!(serde_json::from_str::<CellStore>(&json).unwrap(), new);
        }
    }

    #[test]
    fn bands_filled_in_order_are_sized_like_the_band_before() {
        let mut s = CellStore::new();
        for r in 0..200 {
            for col in 0..9 {
                s.set(c(r, col), Cell::value(Value::Number(f64::from(r))));
            }
        }
        for k in 1..3 {
            let b = &s.bands[&k];
            assert_eq!((b.rows.capacity(), b.cols.capacity(), b.cells.capacity()), (64, 576, 576), "band {k}");
        }
        s.shrink_to_fit();
        assert_eq!(s.bands[&3].cells.capacity(), 72);
        assert_eq!(s.get(c(199, 8)).map(|x| x.value.clone()), Some(Value::Number(199.0)));
    }

    #[test]
    fn bands_give_back_spare_room() {
        let mut s = CellStore::new();
        for col in 0..9 {
            s.set(c(5, col), Cell::value(Value::Number(col as f64)));
        }
        let cap = |s: &CellStore| s.bands.get(&0).map(|b| (b.cells.capacity(), b.cols.capacity()));
        assert!(cap(&s).is_some_and(|(a, b)| a >= 9 && b >= 9));
        s.shrink_to_fit();
        assert_eq!(cap(&s), Some((9, 9)));
        // A band shared with a snapshot is left as it is.
        let snap = s.clone();
        s.shrink_to_fit();
        assert!(Arc::ptr_eq(&s.bands[&0], &snap.bands[&0]));
    }
}
