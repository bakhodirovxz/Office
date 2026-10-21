//! `convert`: the document converter's core (PLAN phase 3, section 2). It links no engine: it plans a job, runs the domain
//! CLIs as processes, validates what they wrote and reports. The CLI is `src/main.rs`; the API (WP7) reuses `job`.
pub mod exec;
pub mod formats;
pub mod job;
pub mod kind;
pub mod losses;
pub mod report;
pub mod scan;
pub mod plan;
