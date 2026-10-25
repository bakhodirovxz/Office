//! `docserver`: the document server of Phase 5 (PLAN phase 5, WP2). This step holds the configuration with every refusal to start
//! (`config`) and the route table with its security headers (`routes`); the embed script and the wrapper page (WP5, `static/`)
//! are served by `routes`, the session handlers are in `api` (WP3: `sessions` and `tokens`). Nothing here is mounted unless the config file has a `[docserver]` section.
#![forbid(unsafe_code)]
mod api;
pub mod atrest;
pub mod audit;
pub mod callback;
pub mod config;
pub mod history;
pub mod ooxml;
pub mod outbound;
pub mod routes;
pub mod sessions;
pub mod tokens;
pub mod xdfu;

pub use config::{Allow, Docserver, Internal, Provider};
