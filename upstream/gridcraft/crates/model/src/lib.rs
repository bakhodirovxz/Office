//! The GridCraft document model: workbooks, sheets, a copy-on-write cell store, interned
//! styles, defined names, tables, conditional formats, validation, comments, charts and print
//! settings. Pure data; calculation lives in `gridcraft-calc`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod b64;
pub mod cell;
mod cellmap;
pub mod features;
pub mod password;
pub mod preserved;
pub mod sheet;
pub mod store;
pub mod style;
pub mod workbook;

pub use cell::{Cell, CellPicture, Formula, FormulaRuns};
pub use features::*;
pub use password::PasswordHash;
pub use preserved::{Preserved, PreservedPart, PreservedRel, PreservedSheet, PreservedXml};
pub use sheet::{DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, LineIndex, Sheet, SheetViewMode, Visibility, points_to_chars};
pub use store::CellStore;
pub use style::{
    Alignment, BorderLine, BorderStyle, Borders, Color, Fill, Font, HAlign, NumFmt, PatternType, Style, StyleId, StyleTable, Theme, Underline,
    VAlign, VertAlign,
};
pub use workbook::{CalcMode, CalcSettings, DefinedName, DocProps, Workbook};

/// Font used for new workbooks. Only a name: renderers substitute an available open font
/// (e.g. the metric-compatible Carlito) when it isn't installed.
pub const DEFAULT_FONT: &str = "Calibri";
pub const DEFAULT_FONT_SIZE: f32 = 11.0;
/// office overlay (gridcraft/0151): the Normal font of new workbooks (owner decision 2026-10-29). Not bundled: Windows has it;
/// elsewhere the renderers substitute a metric-compatible serif (Liberation Serif, Tinos) when one is installed.
pub const NEW_WORKBOOK_FONT: &str = "Times New Roman";
pub const NEW_WORKBOOK_FONT_SIZE: f32 = 12.0;
/// The default row height Excel gives that font: 15.75 pt (21 px) where Calibri 11 has 15 pt (20 px).
pub const NEW_WORKBOOK_ROW_HEIGHT: f32 = 21.0;
