//! `plugin-sdk`: what a Rust plugin needs besides its own logic (docs/plugins.md sections 3 and 4).
//!
//! A plugin is a `cdylib` for `wasm32-unknown-unknown` that links this crate, writes one handler and calls [`export!`]:
//!
//! ```ignore
//! fn handle(call: plugin_sdk::Call, blobs: Vec<Vec<u8>>) -> Result<plugin_sdk::Reply, String> {
//!     Ok(plugin_sdk::Reply::status(format!("hello from {:?}", call.command)))
//! }
//! plugin_sdk::export!(handle);
//! ```
//!
//! The module must import its memory (`--import-memory`, see `plugins/uz-translit/build.rs`). The host calls `alloc`, writes the call
//! there, calls `call`, reads the answer at the returned `ptr << 32 | len` and calls `free` on both.
pub use plugin_api;
pub use plugin_api::action::{Action, Call, CallKind, Next, ReadReq, ReadWhat, Scope, Widget};
pub use plugin_api::textmap::{Apply, TextMap};

use plugin_api::action::Answer;
use plugin_api::wire::Message;

/// What a handler answers: actions, optionally another round of reads, and the blobs the actions point to by index.
#[derive(Default)]
pub struct Reply {
    pub actions: Vec<Action>,
    pub next: Option<Next>,
    pub blobs: Vec<Vec<u8>>,
}

impl Reply {
    pub fn status(text: impl Into<String>) -> Reply {
        Reply {
            actions: vec![Action::UiStatus { text: text.into() }],
            ..Default::default()
        }
    }

    /// Add `textmap.apply` for `apply` (its blob is the next free index).
    pub fn textmap_apply(mut self, apply: &Apply) -> Result<Reply, String> {
        let blob = u32::try_from(self.blobs.len()).map_err(|e| e.to_string())?;
        self.blobs.push(apply.encode().map_err(|e| e.to_string())?);
        self.actions.insert(0, Action::TextmapApply { blob });
        Ok(self)
    }
}

/// The blob a host call named under `name` in its `reads` (for example `"textmap"`).
pub fn read<'a>(call: &Call, blobs: &'a [Vec<u8>], name: &str) -> Result<&'a [u8], String> {
    let i = *call
        .reads
        .get(name)
        .ok_or_else(|| format!("the call carries no `{name}` read"))?;
    blobs
        .get(i as usize)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("`{name}` points to blob {i}, which is missing"))
}

/// `alloc(len)` of the ABI: `len` bytes the host may write into; they stay valid until `free`.
pub fn alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// `free(ptr, len)` of the ABI.
///
/// # Safety
/// `ptr` and `len` must be exactly what [`alloc`] (or the answer of [`dispatch`]) returned, and not freed before.
pub unsafe fn free(ptr: *mut u8, len: usize) {
    // SAFETY: the caller upholds the contract above; the box was made from a slice of this length.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

/// `call(ptr, len)` of the ABI: decode the call, run `handler`, encode the answer. An error of the handler or of the codec becomes a
/// `ui.status` answer, so the user sees why and the document is untouched. Returns `ptr << 32 | len` of the answer.
///
/// # Safety
/// `ptr..ptr + len` must be readable (the host wrote them after `alloc`).
pub unsafe fn dispatch(
    ptr: *const u8,
    len: usize,
    handler: fn(Call, Vec<Vec<u8>>) -> Result<Reply, String>,
) -> u64 {
    // SAFETY: the caller upholds the contract above.
    let input = unsafe { std::slice::from_raw_parts(ptr, len) };
    let out = run(input, handler)
        .or_else(|e| encode(Reply::status(e)))
        .unwrap_or_default();
    let n = out.len();
    ((alloc_with(out) as u64) << 32) | n as u64
}

fn run(
    input: &[u8],
    handler: fn(Call, Vec<Vec<u8>>) -> Result<Reply, String>,
) -> Result<Vec<u8>, String> {
    let m = Message::decode(input).map_err(|e| e.to_string())?;
    let call: Call = m.header_as().map_err(|e| e.to_string())?;
    encode(handler(call, m.blobs)?)
}

fn encode(r: Reply) -> Result<Vec<u8>, String> {
    Message::new(
        &Answer {
            actions: r.actions,
            next: r.next,
        },
        r.blobs,
    )
    .and_then(|m| m.encode())
    .map_err(|e| e.to_string())
}

/// Leak `v` as an exact-size box; the host frees it with `free(ptr, len)`.
fn alloc_with(v: Vec<u8>) -> u32 {
    Box::into_raw(v.into_boxed_slice()) as *mut u8 as usize as u32
}

/// Export the four functions of the ABI (section 3) with `handler` as the plugin's one entry point. Only on `wasm32`: a native build
/// of the plugin crate (its tests) must not define `free`.
#[macro_export]
macro_rules! export {
    ($handler:ident) => {
        #[cfg(target_arch = "wasm32")]
        mod __plugin_abi {
            #[unsafe(no_mangle)]
            pub extern "C" fn api_version() -> i32 {
                1
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn alloc(len: i32) -> i32 {
                $crate::alloc(len as u32 as usize) as usize as i32
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn free(ptr: i32, len: i32) {
                // SAFETY: the host passes back what `alloc` and `call` returned.
                unsafe { $crate::free(ptr as u32 as usize as *mut u8, len as u32 as usize) }
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn call(ptr: i32, len: i32) -> i64 {
                // SAFETY: the host wrote `len` bytes at `ptr` after `alloc`.
                unsafe {
                    $crate::dispatch(
                        ptr as u32 as usize as *const u8,
                        len as u32 as usize,
                        super::$handler,
                    ) as i64
                }
            }
        }
    };
}
