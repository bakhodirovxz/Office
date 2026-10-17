//! Cells.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use gridcraft_core::{CellRef, RangeRef, Value};
use gridcraft_formula::Expr;
use gridcraft_formula::adjust::{is_shifted_copy, shift_stays_on_sheet, shifted};
use serde::{Deserialize, Serialize};

use crate::style::StyleId;

/// A parsed formula: canonical text (without the leading `=`) and syntax tree.
///
/// A formula either holds its own text and tree, or is a *shifted copy*: another formula's tree
/// with the relative references moved by a row and column offset, as copy and fill move them.
/// The cells of a run of formulas that are the same in R1C1 terms (a filled-down column, an Excel
/// shared formula) hold shifted copies of one shared tree, so the run costs one tree rather than
/// one per cell; a shifted copy makes its text and tree on demand. Either way a formula means the
/// same wherever its cell is stored: the offset is from the shared tree, not from the cell, so
/// moving cells (sort, insert, cut) never changes it.
///
/// A shifted copy is all that most formula cells of a large sheet hold, so it is kept small: 16
/// bytes, the shared tree's pointer and the offset. A formula of its own keeps its text, tree and
/// legacy array range in a box of their own.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "FormulaData", into = "FormulaData")]
pub struct Formula {
    repr: Repr,
}

#[derive(Clone, Debug)]
enum Repr {
    /// Text (canonical when it parses), tree (`None`: it doesn't parse, or it was deserialized)
    /// and legacy array range.
    Own(Box<Own>),
    /// `base` moved by (`dr`, `dc`) as [`gridcraft_formula::adjust::shift_relative`] moves it. A
    /// shifted copy is never a legacy array formula.
    Shifted { base: Arc<Expr>, dr: i32, dc: i32 },
}

#[derive(Clone, Debug)]
struct Own {
    text: Box<str>,
    expr: Option<Arc<Expr>>,
    /// Legacy array formula (Ctrl+Shift+Enter) covering this range; the formula lives in the
    /// top-left cell.
    array: Option<RangeRef>,
}

/// The serialized form (the same as when text, tree and array range were plain fields).
#[derive(Serialize, Deserialize)]
struct FormulaData {
    text: String,
    array: Option<RangeRef>,
}

impl From<FormulaData> for Formula {
    fn from(d: FormulaData) -> Self {
        Formula::own(d.text.into(), None, d.array)
    }
}

impl From<Formula> for FormulaData {
    fn from(f: Formula) -> Self {
        FormulaData { text: f.text().into_owned(), array: f.array() }
    }
}

impl PartialEq for Formula {
    fn eq(&self, other: &Self) -> bool {
        if self.array() != other.array() {
            return false;
        }
        match (&self.repr, &other.repr) {
            (Repr::Shifted { base: a, dr: r1, dc: c1 }, Repr::Shifted { base: b, dr: r2, dc: c2 }) if Arc::ptr_eq(a, b) => r1 == r2 && c1 == c2,
            (Repr::Own(a), Repr::Own(b)) => a.text == b.text && a.expr == b.expr,
            // By meaning: the same text and tree however each one is stored.
            _ => self.text() == other.text() && self.stored_expr() == other.stored_expr(),
        }
    }
}

impl Formula {
    fn own(text: Box<str>, expr: Option<Arc<Expr>>, array: Option<RangeRef>) -> Formula {
        Formula { repr: Repr::Own(Box::new(Own { text, expr, array })) }
    }
    /// Parses `text` (with or without `=`). Unparseable formulas keep their text and evaluate to
    /// `#NAME?`.
    pub fn new(text: &str) -> Formula {
        let body = text.strip_prefix('=').unwrap_or(text);
        match gridcraft_formula::parse(body) {
            Ok(e) => Formula::from_expr(e),
            Err(_) => Formula::own(body.into(), None, None),
        }
    }
    pub fn from_expr(e: Expr) -> Formula {
        Formula::from_arc(Arc::new(e))
    }
    /// A formula holding (and sharing) the tree `e`.
    pub fn from_arc(e: Arc<Expr>) -> Formula {
        Formula::own(gridcraft_formula::print(&e).into(), Some(e), None)
    }
    /// `base` with its relative references moved by (`dr`, `dc`), sharing `base`'s tree. The same
    /// formula as `Formula::from_expr(shift_relative((*base).clone(), dr, dc))`.
    pub fn shifted(base: Arc<Expr>, dr: i64, dc: i64) -> Formula {
        match (i32::try_from(dr), i32::try_from(dc)) {
            (Ok(dr), Ok(dc)) => Formula { repr: Repr::Shifted { base, dr, dc } },
            // Further than any sheet reaches: every relative reference is off the sheet.
            _ => Formula::from_expr(shifted(&base, dr, dc)),
        }
    }
    /// This formula copied (or filled) (`dr`, `dc`) away: relative references move, and those
    /// moved off the sheet become `#REF!`. The same formula as
    /// `self.expr().map(|e| Formula::from_expr(shift_relative(e, dr, dc)))`, sharing the tree when
    /// it can (`None` when the formula doesn't parse; the legacy array range is not copied).
    pub fn copied(&self, dr: i64, dc: i64) -> Option<Formula> {
        Some(match &self.repr {
            Repr::Own(own) => match &own.expr {
                Some(e) => Formula::shifted(e.clone(), dr, dc),
                None => Formula::shifted(Arc::new(gridcraft_formula::parse(&own.text).ok()?), dr, dc),
            },
            // Two moves compose into one only when the first kept every reference on the sheet
            // (a `#REF!` doesn't come back).
            Repr::Shifted { base, dr: r, dc: c } if shift_stays_on_sheet(base, i64::from(*r), i64::from(*c)) => {
                Formula::shifted(base.clone(), i64::from(*r).saturating_add(dr), i64::from(*c).saturating_add(dc))
            }
            Repr::Shifted { base, dr: r, dc: c } => Formula::from_expr(shifted(&shifted(base, i64::from(*r), i64::from(*c)), dr, dc)),
        })
    }
    /// The canonical formula text (without `=`).
    pub fn text(&self) -> Cow<'_, str> {
        match &self.repr {
            Repr::Own(own) => Cow::Borrowed(&own.text),
            Repr::Shifted { base, dr: 0, dc: 0 } => Cow::Owned(gridcraft_formula::print(base)),
            Repr::Shifted { base, dr, dc } => Cow::Owned(gridcraft_formula::print(&shifted(base, i64::from(*dr), i64::from(*dc)))),
        }
    }
    /// The legacy array formula (Ctrl+Shift+Enter) range this formula covers; the formula lives in
    /// the range's top-left cell.
    pub fn array(&self) -> Option<RangeRef> {
        match &self.repr {
            Repr::Own(own) => own.array,
            Repr::Shifted { .. } => None,
        }
    }
    /// Makes this a legacy array formula covering `range` (or an ordinary formula: `None`). A
    /// shifted copy made an array formula gets a text and tree of its own.
    pub fn set_array(&mut self, range: Option<RangeRef>) {
        match &mut self.repr {
            Repr::Own(own) => own.array = range,
            Repr::Shifted { .. } if range.is_none() => {}
            Repr::Shifted { base, dr, dc } => {
                let e = Arc::new(shifted(base, i64::from(*dr), i64::from(*dc)));
                *self = Formula::own(gridcraft_formula::print(&e).into(), Some(e), range);
            }
        }
    }
    /// This formula as a legacy array formula covering `range` (see [`Formula::set_array`]).
    pub fn with_array(mut self, range: Option<RangeRef>) -> Formula {
        self.set_array(range);
        self
    }
    /// Whether the formula text parses (one that doesn't evaluates to `#NAME?`).
    pub fn is_parsed(&self) -> bool {
        match &self.repr {
            Repr::Own(own) => own.expr.is_some(),
            Repr::Shifted { .. } => true,
        }
    }
    /// The parsed expression, re-parsing after deserialization.
    pub fn expr(&self) -> Option<Expr> {
        match &self.repr {
            Repr::Own(own) => own.expr.as_deref().cloned().or_else(|| gridcraft_formula::parse(&own.text).ok()),
            Repr::Shifted { base, dr, dc } => Some(shifted(base, i64::from(*dr), i64::from(*dc))),
        }
    }
    /// The parsed expression without copying it when it is stored as it is (re-parses after
    /// deserialization; builds a shifted copy's tree).
    pub fn expr_arc(&self) -> Option<Arc<Expr>> {
        match &self.repr {
            Repr::Own(own) => own.expr.clone().or_else(|| gridcraft_formula::parse(&own.text).ok().map(Arc::new)),
            Repr::Shifted { base, dr: 0, dc: 0 } => Some(base.clone()),
            Repr::Shifted { base, dr, dc } => Some(Arc::new(shifted(base, i64::from(*dr), i64::from(*dc)))),
        }
    }
    /// The shared tree this formula is a shifted copy of, and the offset (`(tree, 0, 0)` for a
    /// parsed formula of its own). Formulas with the same tree (`Arc::ptr_eq`) differ only by
    /// where their relative references point.
    pub fn template(&self) -> Option<(&Arc<Expr>, i64, i64)> {
        match &self.repr {
            Repr::Own(own) => own.expr.as_ref().map(|e| (e, 0, 0)),
            Repr::Shifted { base, dr, dc } => Some((base, i64::from(*dr), i64::from(*dc))),
        }
    }
    /// The tree as stored, for [`PartialEq`]: an own formula's (not re-parsed), a shifted copy's
    /// built.
    fn stored_expr(&self) -> Option<Cow<'_, Expr>> {
        match &self.repr {
            Repr::Own(own) => own.expr.as_deref().map(Cow::Borrowed),
            Repr::Shifted { base, dr, dc } => Some(Cow::Owned(shifted(base, i64::from(*dr), i64::from(*dc)))),
        }
    }
}

/// Finds runs of formulas that are the same in R1C1 terms, so that they share one tree. Fed a
/// sheet's formulas cell by cell in row-major order (as a reader or a rewrite visits them), it
/// compares each with the last formula of its column and of its row: a formula that is one of
/// them moved to its own cell becomes a shifted copy of it ([`Formula::shifted`]).
#[derive(Debug, Default)]
pub struct FormulaRuns {
    cols: HashMap<u32, Template>,
    /// The row and its last template.
    row: Option<(u32, Template)>,
}

/// A shared tree and the cell it is exact for (possibly off the sheet).
#[derive(Clone, Debug)]
struct Template {
    base: Arc<Expr>,
    row: i64,
    col: i64,
}

impl FormulaRuns {
    /// The formula for the tree `e` at `pos`: the same formula as `Formula::from_expr(e)`.
    pub fn formula(&mut self, pos: CellRef, e: Expr) -> Formula {
        self.enter_row(pos);
        let moved = |t: &Template| {
            let (dr, dc) = (i64::from(pos.row) - t.row, i64::from(pos.col) - t.col);
            is_shifted_copy(&t.base, dr, dc, &e).then(|| (t.clone(), dr, dc))
        };
        match self.cols.get(&pos.col).and_then(moved).or_else(|| self.row.as_ref().and_then(|(_, t)| moved(t))) {
            Some((t, dr, dc)) => {
                let f = Formula::shifted(t.base.clone(), dr, dc);
                self.remember(pos, t);
                f
            }
            None => {
                let f = Formula::from_expr(e);
                self.keep(pos, &f);
                f
            }
        }
    }
    /// Notes a formula kept as it is at `pos`, for the formulas after it to share.
    pub fn keep(&mut self, pos: CellRef, f: &Formula) {
        self.enter_row(pos);
        if let Some((base, dr, dc)) = f.template() {
            self.remember(pos, Template { base: base.clone(), row: i64::from(pos.row) - dr, col: i64::from(pos.col) - dc });
        }
    }
    fn enter_row(&mut self, pos: CellRef) {
        if self.row.as_ref().is_some_and(|(r, _)| *r != pos.row) {
            self.row = None;
        }
    }
    fn remember(&mut self, pos: CellRef, t: Template) {
        self.cols.insert(pos.col, t.clone());
        self.row = Some((pos.row, t));
    }
}

/// A static picture stored as cell content, independent of floating drawing anchors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellPicture {
    /// Encoded PNG/JPEG bytes, base64 in JSON. Sheet entries share the payload through Arc.
    #[serde(with = "crate::b64")]
    pub data: Vec<u8>,
    pub mime: String,
    pub alt: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// Constant value, or the cached result of the formula.
    pub value: Value,
    pub formula: Option<Arc<Formula>>,
    pub style: StyleId,
}

impl Cell {
    pub fn value(v: Value) -> Cell {
        Cell { value: v, formula: None, style: StyleId::DEFAULT }
    }
    pub fn formula(f: Formula) -> Cell {
        Cell { value: Value::Empty, formula: Some(Arc::new(f)), style: StyleId::DEFAULT }
    }
    /// No value, no formula and the default style: storing it is pointless.
    pub fn is_blank(&self) -> bool {
        self.value.is_empty() && self.formula.is_none() && self.style == StyleId::DEFAULT
    }
    /// What the formula bar shows.
    pub fn input_text(&self) -> String {
        if let Some(f) = &self.formula {
            return format!("={}", f.text());
        }
        match &self.value {
            Value::Text(t) => {
                // Text that would parse as something else is shown with its apostrophe by Excel
                // only when entered that way; we show it plainly.
                t.to_string()
            }
            v => v.display(),
        }
    }
    /// What the formula bar shows when not editing: [`input_text`](Self::input_text), with a
    /// legacy array formula in braces (`{=A1:A2*2}`).
    pub fn bar_text(&self) -> String {
        match &self.formula {
            Some(f) if f.array().is_some() => format!("{{={}}}", f.text()),
            _ => self.input_text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gridcraft_formula::adjust::shift_relative;

    /// The copy as it was made before shared trees: a whole new formula.
    fn plain(f: &Formula, dr: i64, dc: i64) -> Formula {
        Formula::from_expr(shift_relative(f.expr().unwrap(), dr, dc))
    }

    #[test]
    fn a_shifted_copy_is_the_formula_a_plain_copy_makes() {
        let texts = [
            "IF(C2>500,C2*D2,0)",
            "VLOOKUP(F2,Lookup!$A$2:$B$21,2,FALSE)",
            "S02!C2+D2",
            "SUM($A1:B$3)+SUM(2:3)+SUM(B:C)",
            "A1#+SUM(A1.:.B2)+'My Sheet'!B2",
            "LET(x,A1,x*B$2)",
        ];
        for text in texts {
            let master = Formula::new(text);
            for (dr, dc) in [(0, 0), (1, 0), (4799, 0), (0, 3), (-1, 0), (2, -2), (1_048_575, 0)] {
                let s = master.copied(dr, dc).unwrap();
                let p = plain(&master, dr, dc);
                assert_eq!(s.text(), p.text(), "{text} by ({dr},{dc})");
                assert_eq!(s.expr(), p.expr(), "{text} by ({dr},{dc})");
                assert_eq!(s.expr_arc().as_deref(), p.expr().as_ref());
                assert_eq!(s, p, "a shifted copy equals the same formula held on its own");
                assert!(s.is_parsed());
            }
        }
    }

    #[test]
    fn copies_of_copies_compose_unless_a_reference_fell_off() {
        let master = Formula::new("A2+$B$1");
        let once = master.copied(5, 1).unwrap();
        assert_eq!(once.text(), "B7+$B$1");
        let back = once.copied(-3, 0).unwrap();
        assert_eq!(back.text(), "B4+$B$1");
        assert!(matches!(back.repr, Repr::Shifted { dr: 2, dc: 1, .. }), "shares the master's tree");
        // Off the sheet and back: the `#REF!` stays, as with plain copies.
        let off = master.copied(-2, 0).unwrap();
        assert_eq!(off.text(), "#REF!+$B$1");
        assert_eq!(off.copied(2, 0).unwrap().text(), "#REF!+$B$1");
        assert_eq!(off.copied(2, 0).unwrap(), plain(&off, 2, 0));
    }

    #[test]
    fn copies_share_one_tree() {
        let master = Formula::new("C2*D2");
        let base = master.template().unwrap().0.clone();
        let copies: Vec<Formula> = (1..100).map(|r| master.copied(r, 0).unwrap()).collect();
        assert!(copies.iter().all(|f| f.template().is_some_and(|(b, _, _)| Arc::ptr_eq(b, &base))));
        assert_eq!(copies.last().unwrap().text(), "C101*D101");
        assert_eq!(copies.first().unwrap().expr_arc().as_deref(), Formula::new("C3*D3").expr().as_ref());
        assert_eq!(Arc::strong_count(&base), 101);
    }

    #[test]
    fn unparsed_formulas_are_not_copied() {
        let f = Formula::new("SUM(");
        assert!(!f.is_parsed());
        assert_eq!(f.copied(3, 3), None);
        // Deserialized formulas (text only) parse when copied.
        let d: Formula = serde_json::from_str(r#"{"text":"A1+1","array":null}"#).unwrap();
        assert!(!d.is_parsed());
        assert_eq!(d.copied(1, 0).unwrap().text(), "A2+1");
    }

    #[test]
    fn copies_drop_the_legacy_array_range() {
        let mut f = Formula::new("A1:A2*2");
        f.set_array(Some(RangeRef::parse("B1:B2").unwrap()));
        assert_eq!(f.copied(0, 0).unwrap().array(), None);
        assert_ne!(f.copied(0, 0).unwrap(), f);
    }

    #[test]
    fn shifted_copies_are_small_and_array_ranges_live_in_own_formulas() {
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Formula>(), 16);
        let plain = Formula::new("A3*2");
        let mut s = Formula::new("A1*2").copied(2, 0).unwrap();
        s.set_array(None);
        assert!(matches!(s.repr, Repr::Shifted { .. }), "no array range: still a shifted copy");
        let r = RangeRef::parse("C3:C4");
        s.set_array(r);
        assert_eq!((s.array(), s.text().as_ref()), (r, "A3*2"));
        assert_eq!(s, plain.clone().with_array(r));
        assert_ne!(s, plain);
        s.set_array(None);
        assert_eq!(s, plain);
        let json = serde_json::to_string(&plain.clone().with_array(r)).unwrap();
        assert_eq!(json, r#"{"text":"A3*2","array":{"start":{"row":2,"col":2},"end":{"row":3,"col":2}}}"#);
        assert_eq!(serde_json::from_str::<Formula>(&json).unwrap().array(), r);
    }

    #[test]
    fn runs_share_trees_down_columns_and_across_rows() {
        let mut runs = FormulaRuns::default();
        let parse = |t: &str| gridcraft_formula::parse(t).unwrap();
        let mut got = Vec::new();
        for r in 1..=4u32 {
            let n = r + 1;
            for (c, t) in [(6, format!("IF(C{n}>500,C{n}*D{n},0)")), (7, format!("C{n}+$D$1")), (8, format!("G{n}+H{n}"))] {
                got.push(runs.formula(CellRef::new(r, c), parse(&t)));
            }
        }
        // One tree per column: the first row's.
        for col in 0..3 {
            let first = got[col].template().unwrap().0.clone();
            for f in got.iter().skip(col).step_by(3) {
                assert!(Arc::ptr_eq(f.template().unwrap().0, &first));
            }
        }
        assert_eq!(got[9].text(), "IF(C5>500,C5*D5,0)");
        assert_eq!(got[11].text(), "G5+H5");
        // Filled right: `A1*2` in B1, `B1*2` in C1 share a tree along the row.
        let a = runs.formula(CellRef::new(10, 1), parse("A11*2"));
        let b = runs.formula(CellRef::new(10, 2), parse("B11*2"));
        assert!(Arc::ptr_eq(a.template().unwrap().0, b.template().unwrap().0));
        // A different formula starts a new run, and is never a shifted copy of the old one.
        let other = runs.formula(CellRef::new(5, 6), parse("IF(C6>501,C6*D6,0)"));
        assert_eq!(other.text(), "IF(C6>501,C6*D6,0)");
        assert!(!Arc::ptr_eq(other.template().unwrap().0, got[0].template().unwrap().0));
        // A copy that pushes a reference off the sheet gives `#REF!`, so a `#REF!` there is that copy.
        let r1 = runs.formula(CellRef::new(20, 0), parse("XFD21+1"));
        let r2 = runs.formula(CellRef::new(20, 1), parse("#REF!+1"));
        assert_eq!(r2.text(), "#REF!+1");
        assert!(Arc::ptr_eq(r1.template().unwrap().0, r2.template().unwrap().0));
    }

    #[test]
    fn kept_formulas_seed_runs() {
        let master = Formula::new("A1+1");
        let kept = master.copied(10, 0).unwrap();
        let mut runs = FormulaRuns::default();
        // `kept` sits at row 30 now (say rows were inserted above it): the next cell's formula
        // still joins its run.
        runs.keep(CellRef::new(30, 1), &kept);
        let next = runs.formula(CellRef::new(31, 1), gridcraft_formula::parse("A12+1").unwrap());
        assert!(Arc::ptr_eq(next.template().unwrap().0, master.template().unwrap().0));
        assert_eq!(next.text(), "A12+1");
    }

    #[test]
    fn serializes_as_text() {
        let s = Formula::new("A1*2").copied(1, 1).unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"text":"B2*2","array":null}"#);
        let back: Formula = serde_json::from_str(&json).unwrap();
        assert_eq!(back.text(), "B2*2");
        assert_eq!(back.expr(), s.expr());
    }
}
