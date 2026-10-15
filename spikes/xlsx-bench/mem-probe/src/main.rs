//! Memory probe for gridcraft data structures (read-only use of upstream crates).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use gridcraft_core::{CellRef, RangeRef, Value};
use gridcraft_formula::Expr;
use gridcraft_formula::ast::{Anchor, RefKind, Reference, SheetSel};
use gridcraft_model::{Cell, CellStore, Formula, StyleId};

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
    println!("Value {}", size_of::<Value>());
    println!("Cell {}", size_of::<Cell>());
    println!("Formula {}", size_of::<Formula>());
    println!("Option<Arc<Formula>> {}", size_of::<Option<Arc<Formula>>>());
    println!("Expr {}", size_of::<Expr>());
    println!("Reference {}", size_of::<Reference>());
    println!("SheetSel {}", size_of::<SheetSel>());
    println!("RefKind {}", size_of::<RefKind>());
    println!("Anchor {}", size_of::<Anchor>());
    println!("StyleId {}", size_of::<StyleId>());
    println!("CellRef {} RangeRef {}", size_of::<CellRef>(), size_of::<RangeRef>());
    println!("calc Key {}", size_of::<gridcraft_calc::Key>());
    println!("calc Area {}", size_of::<gridcraft_calc::Area>());
    println!("Option<RangeRef> {}", size_of::<Option<RangeRef>>());
}

fn formula_cost(text: &str) {
    let (b0, a0) = cur();
    let f = Arc::new(Formula::new(text));
    let (b1, a1) = cur();
    let tl = f.text.len();
    let mut nodes = 0;
    if let Some(e) = &f.expr {
        e.walk(&mut |_| nodes += 1);
    }
    println!("formula {text:<45} heap {:>4} B in {:>2} allocs (text {tl} B, expr nodes {nodes})", b1 - b0, a1 - a0);
    drop(f);
}

/// One bench sheet in memory: rows x 9 columns, like gen.py.
fn synthetic(rows: u32) {
    println!("== synthetic sheet, {rows} rows x 9 cols (gen.py layout), Arc<str> per unique text");
    let cats: Vec<Arc<str>> = (0..20).map(|i| Arc::from(format!("K{i:02}"))).collect();
    let words = ["Приход", "Расход", "Оплата", "Склад", "Товар", "Order", "Invoice", "Клиент", "Supplier", "Возврат", "Delivery", "Акт"];
    let (b0, a0) = cur();
    let mut s = CellStore::new();
    for r in 1..=rows {
        s.set(CellRef::new(r, 0), Cell::value(Value::Number(r as f64)));
        s.set(CellRef::new(r, 1), Cell { value: Value::Number(44000.0 + r as f64), formula: None, style: StyleId(1) });
        s.set(CellRef::new(r, 2), Cell::value(Value::Number(123.45)));
        s.set(CellRef::new(r, 3), Cell::value(Value::Number(7.0)));
    }
    let (b1, a1) = cur();
    let n = rows as usize;
    println!("4 numeric cols:  {:>8.1} MB  {:>6.1} B/cell  {:.2} allocs/cell", mb(b1 - b0), (b1 - b0) as f64 / (4 * n) as f64, (a1 - a0) as f64 / (4 * n) as f64);
    for r in 1..=rows {
        let t = format!("{} {} №{}", words[(r % 12) as usize], words[((r / 12) % 12) as usize], 1000 + r * 7);
        s.set(CellRef::new(r, 4), Cell::value(Value::Text(Arc::from(t))));
        s.set(CellRef::new(r, 5), Cell::value(Value::Text(cats[(r % 20) as usize].clone())));
    }
    let (b2, a2) = cur();
    println!("+2 text cols:    {:>8.1} MB  {:>6.1} B/cell  {:.2} allocs/cell (1 unique + 1 shared Arc<str>)", mb(b2 - b1), (b2 - b1) as f64 / (2 * n) as f64, (a2 - a1) as f64 / (2 * n) as f64);
    for r in 1..=rows {
        let rr = r + 1;
        let fs = [format!("IF(C{rr}>500,C{rr}*D{rr},0)"), format!("VLOOKUP(F{rr},Lookup!$A$2:$B$21,2,FALSE)"), format!("S02!C{rr}+D{rr}")];
        for (j, t) in fs.iter().enumerate() {
            s.set(CellRef::new(r, 6 + j as u32), Cell { value: Value::Number(1.0), formula: Some(Arc::new(Formula::new(t))), style: StyleId::DEFAULT });
        }
    }
    let (b3, a3) = cur();
    println!("+3 formula cols: {:>8.1} MB  {:>6.1} B/cell  {:.2} allocs/cell", mb(b3 - b2), (b3 - b2) as f64 / (3 * n) as f64, (a3 - a2) as f64 / (3 * n) as f64);
    println!("whole sheet:     {:>8.1} MB  {:>6.1} B/cell  {} live allocs; x30 sheets = {:.0} MB requested",
        mb(b3 - b0), (b3 - b0) as f64 / (9 * n) as f64, a3 - a0, mb(b3 - b0) * 30.0);
    // Undo snapshot: clone + one edit copies one band.
    let (c0, _) = cur();
    let mut snap = s.clone();
    snap.set(CellRef::new(5, 2), Cell::value(Value::Number(1.0)));
    let (c1, _) = cur();
    println!("undo snapshot (clone + 1 edit): {:.1} KB", (c1 - c0) as f64 / 1024.0);
    drop(snap);
    drop(s);
}

fn real_load(path: &str) {
    println!("== real load: {path}");
    let bytes = std::fs::read(path).expect("read file");
    let (fb, _) = cur();
    println!("file bytes held: {:.1} MB", mb(fb));
    reset_peak();
    let t = std::time::Instant::now();
    let (mut wb, rep) = gridcraft_xlsx::read_xlsx(&bytes).expect("read_xlsx");
    let (b1, a1) = cur();
    println!("read_xlsx: {:.1} s, peak {:.1} MB (incl. file bytes), live after {:.1} MB in {} allocs, warnings {}",
        t.elapsed().as_secs_f64(), mb(PEAK.load(Relaxed)), mb(b1), a1, rep.warnings.len());
    let (mut n_num, mut n_txt, mut n_f, mut n_other, mut ftext) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut exprs: std::collections::HashSet<*const Expr> = Default::default();
    let mut forms: std::collections::HashSet<*const Formula> = Default::default();
    for sh in &wb.sheets {
        for (_, c) in sh.cells.iter() {
            if let Some(f) = &c.formula {
                n_f += 1;
                ftext += f.text.len();
                forms.insert(Arc::as_ptr(f));
                if let Some(e) = &f.expr {
                    exprs.insert(Arc::as_ptr(e));
                }
            } else {
                match c.value {
                    Value::Number(_) => n_num += 1,
                    Value::Text(_) => n_txt += 1,
                    _ => n_other += 1,
                }
            }
        }
    }
    let cells = n_num + n_txt + n_f + n_other;
    println!("cells {cells}: numbers {n_num}, text {n_txt}, formulas {n_f} (distinct Arc<Formula> {}, distinct Arc<Expr> {}), other {n_other}; formula text {:.1} MB",
        forms.len(), exprs.len(), mb(ftext));
    let model_live = b1 - fb;
    println!("model live: {:.1} MB = {:.0} B/cell", mb(model_live), model_live as f64 / cells as f64);
    drop(exprs);
    drop(forms);
    drop(bytes);
    let (b2, _) = cur();
    reset_peak();
    let mut calc = gridcraft_calc::Calc::new();
    let t = std::time::Instant::now();
    calc.rebuild(&wb);
    let (b3, a3) = cur();
    println!("calc.rebuild: {:.1} s, graph nodes {}, graph live {:.1} MB ({:.0} B/formula, {} allocs), transient peak above model {:.1} MB",
        t.elapsed().as_secs_f64(), calc.graph.len(), mb(b3 - b2), (b3 - b2) as f64 / n_f.max(1) as f64, a3, mb(PEAK.load(Relaxed) - b2));
    reset_peak();
    let t = std::time::Instant::now();
    calc.recalc_all(&mut wb);
    let (b4, _) = cur();
    println!("calc.recalc_all: {:.1} s, peak {:.1} MB (above pre-recalc {:.1} MB), live after {:.1} MB",
        t.elapsed().as_secs_f64(), mb(PEAK.load(Relaxed)), mb(PEAK.load(Relaxed) - b3), mb(b4));
    reset_peak();
    let t = std::time::Instant::now();
    let out = gridcraft_xlsx::write_xlsx(&wb).expect("write");
    println!("write_xlsx: {:.1} s, output {:.1} MB, peak {:.1} MB (above live {:.1} MB)",
        t.elapsed().as_secs_f64(), mb(out.len()), mb(PEAK.load(Relaxed)), mb(PEAK.load(Relaxed) - b4));
    drop(out);
    // Undo snapshot after a one-cell edit on a sheet: Workbook clone + Sheet clone + band copy.
    let (u0, _) = cur();
    let mut wb2 = wb.clone();
    if let Some(sh) = wb2.sheets.get_mut(1) {
        Arc::make_mut(sh).cells.set(CellRef::new(10, 2), Cell::value(Value::Number(5.0)));
    }
    let (u1, _) = cur();
    println!("one undo step (wb clone + edit 1 cell): {:.1} KB", (u1 - u0) as f64 / 1024.0);
    drop(wb2);
    // Wide-impact edit with an undo snapshot alive: Lookup!B2 feeds every VLOOKUP whose key is K00.
    if let Some(li) = wb.sheet_index("Lookup") {
        let snapshot = wb.clone();
        let (w0, _) = cur();
        reset_peak();
        let t = std::time::Instant::now();
        if let Some(sh) = wb.sheets.get_mut(li) {
            Arc::make_mut(sh).cells.set(CellRef::new(1, 1), Cell::value(Value::Number(9.75)));
        }
        calc.cells_changed(&mut wb, &[(li, CellRef::new(1, 1))]);
        let (w1, _) = cur();
        println!("edit Lookup!B2 with undo snapshot alive: {:.1} s, recalculated {} cells, live +{:.1} MB, peak +{:.1} MB",
            t.elapsed().as_secs_f64(), calc.last_recalc_cells, mb(w1 - w0), mb(PEAK.load(Relaxed) - w0));
        drop(snapshot);
        let (w2, _) = cur();
        println!("after dropping the snapshot: live {:.1} MB (was {:.1} MB before the edit)", mb(w2), mb(w0));
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--parse") {
        for t in ["[1]Sheet1!A1", "[2]Data!B2*2", "SUM([1]Sheet1!A1:A5)", "'[source.xlsx]Data'!B2", "[source.xlsx]Data!B2", r"'E:\dir\[source.xlsx]Sheet1'!A1", r"'C:\p\[f.xlsx]My Sheet'!$A$1:$B$9"] {
            match gridcraft_formula::parse(t) {
                Ok(e) => println!("{t:<40} OK  {:?}", e),
                Err(err) => println!("{t:<40} ERR {err}"),
            }
            let f = Formula::new(t);
            println!("{:<40} Formula.expr is_some={} text={}", "", f.expr.is_some(), f.text);
        }
        return;
    }
    sizes();
    println!("== per-formula heap (Arc<Formula> + text + Arc<Expr> + tree)");
    for t in ["IF(C2>500,C2*D2,0)", "VLOOKUP(F2,Lookup!$A$2:$B$21,2,FALSE)", "S02!C2+D2", "C2+D2", "SUM(S01!G2:G19201)", "SUMIFS(S01!C2:C19201,S01!F2:F19201,\"K05\")"] {
        formula_cost(t);
    }
    synthetic(19_200);
    if let Some(p) = std::env::args().nth(1) {
        real_load(&p);
    }
}
