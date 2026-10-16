//! Memory probe r2: where the live heap of an opened workbook goes, for the patched gridcraft in
//! E:/office-work/gridcraft-spike. A counting global allocator records requested bytes. After the
//! open path (`read_xlsx` + `Calc::recalc_all`, as `DocState::new`) the live heap is attributed by
//! taking the model apart piece by piece and measuring each drop:
//!   graph (drop `Calc`) -> text values -> formulas -> cell store -> rest of the workbook.
//! Usage: gc-probe-r2 FILE.xlsx
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, CellStore, Formula};

struct Counting;
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
            LIVE.fetch_add(1, Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
            LIVE.fetch_add(1, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), Relaxed);
        LIVE.fetch_sub(1, Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            if n >= l.size() {
                let c = CUR.fetch_add(n - l.size(), Relaxed) + (n - l.size());
                PEAK.fetch_max(c, Relaxed);
            } else {
                CUR.fetch_sub(l.size() - n, Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

fn cur() -> (usize, usize) {
    (CUR.load(Relaxed), LIVE.load(Relaxed))
}
fn reset_peak() {
    PEAK.store(CUR.load(Relaxed), Relaxed);
}
fn mb(b: usize) -> f64 {
    b as f64 / 1048576.0
}

fn sizes() {
    use std::mem::size_of;
    println!("== size_of");
    println!("Value {} Cell {} Formula {} Option<Arc<Formula>> {}", size_of::<Value>(), size_of::<Cell>(), size_of::<Formula>(), size_of::<Option<Arc<Formula>>>());
    println!("Expr {} CellRef {} calc Key {} Area {}", size_of::<gridcraft_formula::Expr>(), size_of::<CellRef>(), size_of::<gridcraft_calc::Key>(), size_of::<gridcraft_calc::Area>());
}

fn step(name: &str, before: (usize, usize), n: usize) -> (usize, usize) {
    let now = cur();
    let d = before.0.saturating_sub(now.0);
    let a = before.1.saturating_sub(now.1);
    println!("  {name:<34} {:>9.1} MiB  {:>6.1} B/cell  {:>9} allocs", mb(d), d as f64 / n.max(1) as f64, a);
    now
}

fn main() {
    sizes();
    let path = std::env::args().nth(1).expect("FILE");
    println!("== {path}");
    let t = std::time::Instant::now();
    let bytes = std::fs::read(&path).expect("read file");
    let fb = cur().0;
    reset_peak();
    let (mut wb, rep) = gridcraft_xlsx::read_xlsx(&bytes).expect("read_xlsx");
    let after_read = cur();
    println!(
        "read_xlsx {:.1} s: peak {:.1} MiB (file bytes {:.1}), live {:.1} MiB in {} allocs, warnings {}",
        t.elapsed().as_secs_f64(),
        mb(PEAK.load(Relaxed)),
        mb(fb),
        mb(after_read.0),
        after_read.1,
        rep.warnings.len()
    );
    drop(bytes);
    let model0 = cur();
    // Census.
    let (mut cells, mut nf, mut ntext, mut rows) = (0usize, 0usize, 0usize, 0usize);
    let mut text_ptrs: HashSet<*const u8> = HashSet::new();
    let mut text_uniq: HashMap<Arc<str>, ()> = HashMap::new();
    let mut text_bytes = 0usize;
    let mut forms: HashSet<*const Formula> = HashSet::new();
    let mut cached = 0usize;
    for sh in &wb.sheets {
        let mut last_row = u32::MAX;
        for (c, cell) in sh.cells.iter() {
            cells += 1;
            if c.row != last_row {
                rows += 1;
                last_row = c.row;
            }
            if let Some(f) = &cell.formula {
                nf += 1;
                forms.insert(Arc::as_ptr(f));
                if !cell.value.is_empty() {
                    cached += 1;
                }
            }
            if let Value::Text(s) = &cell.value {
                ntext += 1;
                text_bytes += s.len();
                text_ptrs.insert(s.as_ptr());
                text_uniq.entry(s.clone()).or_insert(());
            }
        }
    }
    println!(
        "cells {cells} in {rows} rows; formulas {nf} (distinct Arc<Formula> {}, with a value {cached}); text cells {ntext} ({:.1} MiB of text, {} distinct Arc<str>, {} distinct strings)",
        forms.len(),
        mb(text_bytes),
        text_ptrs.len(),
        text_uniq.len()
    );
    drop((text_ptrs, text_uniq, forms));
    let model_live = cur();
    println!("model live after read (file bytes dropped): {:.1} MiB = {:.1} B/cell", mb(model_live.0), model_live.0 as f64 / cells as f64);
    let _ = model0;
    reset_peak();
    let t = std::time::Instant::now();
    let mut calc = gridcraft_calc::Calc::new();
    calc.recalc_all(&mut wb);
    let after_calc = cur();
    println!(
        "recalc_all {:.1} s: peak {:.1} MiB (+{:.1} over the model), live {:.1} MiB, graph entries {}",
        t.elapsed().as_secs_f64(),
        mb(PEAK.load(Relaxed)),
        mb(PEAK.load(Relaxed) - model_live.0),
        mb(after_calc.0),
        calc.graph.len()
    );
    println!("== live heap after open, by component (drop each and measure)");
    let mut s = after_calc;
    drop(calc);
    s = step("dependency graph (Calc)", s, nf);
    // Text values -> numbers.
    for i in 0..wb.sheets.len() {
        let sh = Arc::make_mut(&mut wb.sheets[i]);
        let keys: Vec<CellRef> = sh.cells.iter().filter(|(_, c)| matches!(c.value, Value::Text(_))).map(|(k, _)| k).collect();
        for k in keys {
            if let Some(c) = sh.cells.get_mut(k) {
                c.value = Value::Number(0.0);
            }
        }
    }
    s = step("text values (Arc<str>)", s, ntext);
    for i in 0..wb.sheets.len() {
        let sh = Arc::make_mut(&mut wb.sheets[i]);
        let keys: Vec<CellRef> = sh.cells.iter().filter(|(_, c)| c.formula.is_some()).map(|(k, _)| k).collect();
        for k in keys {
            if let Some(c) = sh.cells.get_mut(k) {
                c.formula = None;
            }
        }
    }
    s = step("formulas (Arc<Formula> + trees)", s, nf);
    for i in 0..wb.sheets.len() {
        Arc::make_mut(&mut wb.sheets[i]).cells = CellStore::new();
    }
    s = step("cell store (bands, rows, Cell slots)", s, cells);
    drop(wb);
    step("rest of the workbook", s, cells);
    println!("left: {:.1} MiB", mb(cur().0));
}
