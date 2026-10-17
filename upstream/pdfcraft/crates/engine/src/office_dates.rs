// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Month and weekday names of the overlay's Uzbek languages (`dates-uz-*.tsv`, read by `build.rs`). Empty without the overlay.

#[cfg(office_uz)]
use crate::dates::DateLanguage;

include!(concat!(env!("OUT_DIR"), "/office_dates_gen.rs"));
