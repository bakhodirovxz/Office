//! Win32 security helpers for `suite-desktop`, safe to call. All `unsafe` is in `sys.rs`.
#![cfg(windows)]

#[allow(unsafe_code)]
mod sys;
pub use sys::{Sid, create_user_only_pipe, pipe_server_sid, set_default_dll_directories};
