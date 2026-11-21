//! Uzbek Latin <-> Cyrillic transliteration of cell text (office overlay, patches/gridcraft/0120; PLAN section 4).
//!
//! Only constant text cells change. Formulas (and their cached results), numbers, dates, booleans and errors, table header
//! cells, sheet names, defined names, number-format literals, comments, charts, headers and footers stay as typed. The model
//! keeps a cell's text as one plain string (rich-text runs are dropped when a file is read), so the unit is the cell. One
//! `Session::execute` is one undo step; `edit` recalculates the formulas that read a converted cell.
use gridcraft_core::{CellRef, RangeRef, Value};
use serde_json::json;
use uz_translit::{Apostrophe, Direction, LatinAlphabet, Options, Script};

use super::{CommandSpec, Json, bad, bool_param, cmd, edit, has_doc, locked_cell_error, str_param, target_ranges, target_sheet};
use crate::{Result, Session};

pub const TO_CYRILLIC: &str = "tools.translitCyrl";
pub const TO_LATIN: &str = "tools.translitLatn";
pub const AUTO: &str = "tools.translitAuto";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "tools.translitCyrl",
            "Latin to Cyrillic",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"sheet\"|\"workbook\", range?, apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool, latinAlphabet?: \"1995\"|\"2026\"|\"auto\"}",
            has_doc,
            |s, p| run(s, p, Some(Direction::ToCyrillic))
        ),
        cmd!(
            "tools.translitLatn",
            "Cyrillic to Latin",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"sheet\"|\"workbook\", range?, apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool, latinAlphabet?: \"1995\"|\"2026\"|\"auto\"}",
            has_doc,
            |s, p| run(s, p, Some(Direction::ToLatin))
        ),
        cmd!(
            "tools.translitAuto",
            "Auto-detect",
            [],
            None,
            "{scope?: \"auto\"|\"selection\"|\"sheet\"|\"workbook\", range?, apostrophe?: \"official\"|\"ascii\", foreignGuard?: bool, latinAlphabet?: \"1995\"|\"2026\"|\"auto\"}",
            has_doc,
            |s, p| run(s, p, None)
        ),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Auto,
    Selection,
    Sheet,
    Workbook,
}

#[derive(Default)]
struct Stats {
    formula: usize,
    formulas_with_text: usize,
    locked: usize,
    header: usize,
}

/// Milliseconds on a monotonic-enough clock; 0 on wasm, which has no clock (reading it traps).
fn now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::disallowed_methods)]
    {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        t.as_secs_f64() * 1e3
    }
    #[cfg(target_arch = "wasm32")]
    0.0
}

/// A formula with a string literal that holds letters (`=IF(A1="ha";..)`, a lookup key): converting the cells it reads can change its result.
fn has_text_literal(f: &str) -> bool {
    let mut inside = false;
    f.chars().any(|c| {
        if c == '"' {
            inside = !inside;
            false
        } else {
            inside && c.is_alphabetic()
        }
    })
}

fn run(s: &mut Session, p: &Json, dir: Option<Direction>) -> Result<Json> {
    let id = "tools.translit";
    let scope = match str_param(p, "scope").or_else(|| str_param(p, "within")) {
        None | Some("auto") => Scope::Auto,
        Some("selection") => Scope::Selection,
        Some("sheet") => Scope::Sheet,
        Some("workbook") => Scope::Workbook,
        Some(o) => return Err(bad(id, format!("`scope` must be auto, selection, sheet or workbook, not `{o}`"))),
    };
    let apostrophe = match str_param(p, "apostrophe") {
        None | Some("official") => Apostrophe::Official,
        Some("ascii") => Apostrophe::Ascii,
        Some(o) => return Err(bad(id, format!("`apostrophe` must be official or ascii, not `{o}`"))),
    };
    let latin_alphabet = match str_param(p, "latinAlphabet") {
        None => LatinAlphabet::default(), // the suite default (O13): writes 2026, reads 1995 or 2026 per paragraph
        Some("1995") => LatinAlphabet::Y1995,
        Some("2026") => LatinAlphabet::Y2026,
        Some("auto") => LatinAlphabet::Auto,
        Some(o) => return Err(bad(id, format!("`latinAlphabet` must be 1995, 2026 or auto, not `{o}`"))),
    };
    let defaults = Options::default();
    let mut opts = Options {
        apostrophe,
        latin_alphabet,
        foreign_latin_guard: bool_param(p, "foreignGuard").unwrap_or(defaults.foreign_latin_guard),
        ..defaults
    };

    let t0 = now_ms();
    let (scope, found, st) = collect(s, p, scope)?;
    let all: String = found.iter().flat_map(|(_, _, t)| [t.as_ref(), "\n"]).collect();
    let detected = uz_translit::detect(&all);
    // Cyrillic to Latin on a Latin-dominant selection (never a whole sheet or workbook) rewrites 1995 Latin in the 2026 letters.
    opts.latin_upgrade = scope == Scope::Selection && dir == Some(Direction::ToLatin) && detected == Some(Script::Latin);
    let dir = dir.or_else(|| detected.map(|sc| if sc == Script::Latin { Direction::ToCyrillic } else { Direction::ToLatin }));
    let mut changes = Vec::new();
    if let Some(dir) = dir {
        for (si, c, t) in &found {
            let new = uz_translit::convert(t, dir, &opts);
            if new != **t {
                changes.push((*si, *c, new));
            }
        }
    }
    let t1 = now_ms();
    let n = changes.len();
    if n > 0 {
        write_texts(s, changes)?;
    }
    let t2 = now_ms();
    Ok(json!({
        "direction": match dir { Some(Direction::ToCyrillic) => "cyrl", Some(Direction::ToLatin) => "latn", None => "none" },
        "scope": match scope { Scope::Selection => "selection", Scope::Sheet => "sheet", _ => "workbook" },
        "tracked": false,
        "cells": n,
        "runs": n,
        "skipped": {"lang": 0, "formula": st.formula, "locked": st.locked, "header": st.header},
        "formulasWithText": st.formulas_with_text,
        "ms": {"convert": t1 - t0, "apply": t2 - t1},
        "rulesVersion": uz_translit::RULES_VERSION,
    }))
}

/// The constant text cells the commands convert (office overlay, patches/gridcraft/0157; Phase 7 WP13: also the plugin text map of
/// `sheets-plugins`): the selection, the sheet or the workbook as `scope` and `p` (`range`, `cell`, `sheet`) name it, in sheet, row,
/// column order. Formulas and their results, non-text values, table header cells, locked cells of a protected sheet, spill and
/// in-cell picture cells are left out (counted in the stats).
fn collect(s: &Session, p: &Json, scope: Scope) -> Result<(Scope, Vec<(usize, CellRef, std::sync::Arc<str>)>, Stats)> {
    let sel = target_ranges(s, p)?;
    let explicit = p.get("range").is_some() || p.get("cell").is_some();
    let scope = match scope {
        Scope::Auto if explicit || sel.len() > 1 || sel.first().is_some_and(|r| !r.is_single()) => Scope::Selection,
        Scope::Auto => Scope::Sheet,
        o => o,
    };
    let sheet = target_sheet(s, p)?;
    let d = s.doc()?;
    let wb = &d.wb;
    let targets: Vec<(usize, Vec<RangeRef>)> = match scope {
        Scope::Selection => vec![(sheet, sel)],
        Scope::Sheet => vec![(sheet, wb.sheet(sheet).and_then(|sh| sh.cells.used_range()).into_iter().collect())],
        _ => (0..wb.sheets.len()).map(|i| (i, wb.sheet(i).and_then(|sh| sh.cells.used_range()).into_iter().collect())).collect(),
    };
    // Collect the constant text cells in scope.
    let mut st = Stats::default();
    let mut found: Vec<(usize, CellRef, std::sync::Arc<str>)> = Vec::new();
    for (si, ranges) in &targets {
        let Some(sh) = wb.sheet(*si) else { continue };
        let protected = sh.is_protected();
        for r in ranges {
            for (c, cell) in sh.cells.iter_range(*r) {
                if let Some(f) = &cell.formula {
                    st.formula += 1;
                    st.formulas_with_text += has_text_literal(&f.text()) as usize;
                    continue;
                }
                let Value::Text(t) = &cell.value else { continue };
                if sh.tables.iter().any(|t| t.header_row && c.row == t.range.start.row && t.range.contains(c)) {
                    st.header += 1;
                } else if protected && wb.styles.get(cell.style).protection.locked && locked_cell_error(sh, c, &d.unlocked_ranges).is_some() {
                    st.locked += 1;
                } else if !sh.spill.contains_key(&c) && !sh.cell_pictures.contains_key(&c) {
                    found.push((*si, c, t.clone()));
                }
            }
        }
    }
    found.sort_by_key(|(si, c, _)| (*si, c.row, c.col));
    found.dedup_by_key(|(si, c, _)| (*si, c.row, c.col));
    Ok((scope, found, st))
}

/// [`collect`] for a caller outside the engine: the scope used (`selection`, `sheet`, `workbook`) and the cells. `scope` is `auto`,
/// `selection`, `sheet` or `workbook` (`document` = `workbook`).
pub fn text_cells(s: &Session, p: &Json, scope: &str) -> Result<(&'static str, Vec<(usize, CellRef, std::sync::Arc<str>)>)> {
    let scope = match scope {
        "auto" => Scope::Auto,
        "selection" => Scope::Selection,
        "sheet" => Scope::Sheet,
        "workbook" | "document" => Scope::Workbook,
        o => return Err(bad("tools.translit", format!("`scope` must be auto, selection, sheet or workbook, not `{o}`"))),
    };
    let (scope, found, _) = collect(s, p, scope)?;
    Ok((
        match scope {
            Scope::Selection => "selection",
            Scope::Sheet => "sheet",
            _ => "workbook",
        },
        found,
    ))
}

/// Write new texts into constant text cells as one edit (one recalculation of the formulas that read them; the cell keeps its
/// style). Not an undo step by itself: `Session::execute` of the command (or the plugin adapter) makes that.
pub fn write_texts(s: &mut Session, changes: Vec<(usize, CellRef, String)>) -> Result<()> {
    edit(s, |cx| {
        for (si, c, new) in changes {
            let sh = cx.sheet_mut(si)?;
            if let Some(mut cell) = sh.cell(c).cloned() {
                cell.value = Value::text(new);
                sh.set_cell(c, cell);
            }
            cx.touch(si, c);
        }
        Ok(())
    })
}
