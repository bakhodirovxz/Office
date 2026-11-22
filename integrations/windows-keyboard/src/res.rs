//! Resource ids of the DLL (build.rs writes the resources with the same numbers).

/// The brand icon (white box, black glyph): the profile icon in Settings and the Input Indicator.
pub const IDI_BRAND: u16 = 1;
/// Mode icon, conversion on ("ş").
pub const IDI_MODE_ON: u16 = 2;
/// Mode icon, conversion off ("s").
pub const IDI_MODE_OFF: u16 = 3;

/// The display name (owner decision O4), localized: i18n/keyboard.tsv `kbd.name`.
pub const IDS_NAME: u32 = 100;
pub const IDS_MODE_ON: u32 = 101;
pub const IDS_MODE_OFF: u32 = 102;
pub const IDS_TOGGLE: u32 = 103;
