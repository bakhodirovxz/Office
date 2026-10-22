//! The `toolbar` key of the shared `settings.toml` (crates/suite-core, WP5) as the mode store. A missing or damaged file reads as simple.

use suite_core::settings::{ToolbarMode, set_toolbar_mode, toolbar_mode};

use crate::mode::{Mode, Store};

pub fn store() -> Store {
    Store {
        read: || if toolbar_mode() == ToolbarMode::Full { Mode::Full } else { Mode::Simple },
        // A failed write keeps the mode for this run; the next start reads what the file says.
        write: |m| {
            let _ = set_toolbar_mode(if m == Mode::Full { ToolbarMode::Full } else { ToolbarMode::Simple });
        },
    }
}
