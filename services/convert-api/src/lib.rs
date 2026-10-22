//! `convert-api`: the HTTP API over the converter (PLAN phase 3, section 5; WP7). The binary is `src/main.rs`; the tests build
//! the same router through `app::App::new` and `routes::router`.
pub mod app;
pub mod config;
pub mod crypt;
pub mod gate;
pub mod jwt;
pub mod routes;
pub mod runner;
pub mod tls;
