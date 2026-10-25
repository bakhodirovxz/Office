//! `docserver`: the document server of Phase 5 (PLAN phase 5, WP2). This step holds the configuration with every refusal to start
//! (`config`) and the route table with its security headers (`routes`); the handlers behind the routes are stubs that answer 501
//! until WP3 and later fill them (WP3: `sessions` and `tokens`, the session routes of `routes`). Nothing here is mounted unless the config file has a `[docserver]` section.
#![forbid(unsafe_code)]
mod api;
pub mod config;
pub mod routes;
pub mod sessions;
pub mod tokens;

pub use config::{Allow, Docserver, Internal, Provider};
