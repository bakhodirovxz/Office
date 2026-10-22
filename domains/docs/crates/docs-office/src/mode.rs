//! The simple/full toolbar choice: `toolbar = "simple" | "full"` in the shared settings (toolbar-design section 2.1).
//!
//! The store is two function pointers so this crate does not depend on where the setting lives: the apps' start-up wires the
//! shared-settings reader and writer in (`set_store`); until then, and in tests, the mode is kept in memory and defaults to simple.
//! The web build has no toggle and no key: it is always simple.

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Simple,
    Full,
}

/// Where the choice is kept. `read` is called once, on first use; `write` on every change.
#[derive(Clone, Copy)]
pub struct Store {
    pub read: fn() -> Mode,
    pub write: fn(Mode),
}

const UNSET: u8 = 0;
static CURRENT: AtomicU8 = AtomicU8::new(UNSET);
static EXPLICIT: AtomicBool = AtomicBool::new(false);
static STORE: RwLock<Store> = RwLock::new(Store { read: || Mode::Simple, write: |_| {} });

fn code(m: Mode) -> u8 {
    if m == Mode::Full { 2 } else { 1 }
}

/// Whether the build offers the toggle at all (desktop only).
pub const fn toggle_available() -> bool {
    !cfg!(target_arch = "wasm32")
}

/// Use `store` from now on and forget the mode read so far.
pub fn set_store(store: Store) {
    EXPLICIT.store(true, Ordering::SeqCst);
    put(store);
}

/// The app's own store (`install` passes the shared settings): kept unless `set_store` chose another.
pub fn default_store(store: Store) {
    if !EXPLICIT.load(Ordering::SeqCst) {
        put(store);
    }
}

fn put(store: Store) {
    *STORE.write().unwrap_or_else(|e| e.into_inner()) = store;
    CURRENT.store(UNSET, Ordering::SeqCst);
}

/// The current mode (read from the store on first use).
pub fn mode() -> Mode {
    if !toggle_available() {
        return Mode::Simple;
    }
    match CURRENT.load(Ordering::SeqCst) {
        2 => Mode::Full,
        1 => Mode::Simple,
        _ => {
            let m = (STORE.read().unwrap_or_else(|e| e.into_inner()).read)();
            CURRENT.store(code(m), Ordering::SeqCst);
            m
        }
    }
}

/// Switch mode and write it to the store.
pub fn set_mode(m: Mode) {
    if !toggle_available() {
        return;
    }
    CURRENT.store(code(m), Ordering::SeqCst);
    (STORE.read().unwrap_or_else(|e| e.into_inner()).write)(m);
}
