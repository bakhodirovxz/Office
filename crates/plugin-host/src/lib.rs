//! `plugin-host`: the native plugin host (PLAN phase 7, WP2; sections 4.2, 4.3 and 5.1).
//!
//! - [`Host`] owns the wasmtime engine and the epoch ticker thread.
//! - [`Plugin`] is a checked, compiled module: imports are exactly `env.memory` (+ optional `plugin_v1.log`), no exported
//!   memory, the four required exports, `api_version() == 1`. Anything else is a [`LoadError`].
//! - [`Session`] is one invocation: a fresh store and instance (memory capped at `memory_mb`), one epoch deadline per round.
//!   [`Session::run`] is the round loop (16 rounds at most).
//!
//! [`net`] is the host side of the `fetch` action (granted origins, address filter, TLS); the adapter that applies actions calls it.
//! No WASI is linked, so a plugin has no other way out of its memory.
#![forbid(unsafe_code)]

pub mod net;

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use plugin_api::action::Answer;
use plugin_api::manifest::MEMORY_CAP_MB;
use plugin_api::wire::{MAX_MESSAGE, MAX_ROUNDS, Message, WireError};
use wasmtime::{
    Caller, Config, Engine, ExternType, Linker, Memory, MemoryType, Module, ResourceLimiter, Store,
    Trap, TypedFunc,
};

/// Epoch tick. A deadline of `n` ticks stops a runaway round after `(n-1)..n` ticks.
const TICK: Duration = Duration::from_millis(50);
/// `plugin_v1.log`: lines kept per session and bytes kept per line.
const LOG_LINES: usize = 64;
const LOG_LINE: usize = 256;
const PAGE: u64 = 65536;
/// Wasm call stack of a guest. Recursion past it is a clean trap.
pub const MAX_WASM_STACK: usize = 256 << 10;
/// Stack of the thread a round runs on: the wasm stack plus the host frames of a debug build, with room to spare.
const ROUND_THREAD_STACK: usize = 1 << 20;

/// Why a module was refused (never run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// `memory_mb` is 0 or above the 64 MB cap of the manifest rules.
    MemoryLimit(u32),
    Compile(String),
    /// Every import that is not allowed, as `module.name`.
    Imports(Vec<String>),
    NoMemoryImport,
    /// The module exports a memory (the host must own it, section 4.2).
    ExportsMemory(String),
    /// The imported memory is shared, 64-bit, or needs more than the cap at start.
    MemoryType(String),
    /// A required export is missing or has the wrong type, or the probe instance failed.
    Exports(String),
    ApiVersion(i32),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::MemoryLimit(m) => {
                write!(f, "memory limit {m} MB is outside 1..={MEMORY_CAP_MB}")
            }
            LoadError::Compile(e) => write!(f, "module does not compile: {e}"),
            LoadError::Imports(v) => write!(f, "imports not allowed: {}", v.join(", ")),
            LoadError::NoMemoryImport => f.write_str("module must import env.memory"),
            LoadError::ExportsMemory(n) => write!(
                f,
                "module exports a memory ({n}); it must import the host's"
            ),
            LoadError::MemoryType(e) => write!(f, "imported memory not allowed: {e}"),
            LoadError::Exports(e) => write!(f, "exports: {e}"),
            LoadError::ApiVersion(v) => write!(f, "api_version() is {v}, this host runs 1"),
        }
    }
}
impl std::error::Error for LoadError {}

/// Why an invocation stopped. The session is unusable after any of these.
#[derive(Debug)]
pub enum RunError {
    /// The round went past its time budget (epoch deadline).
    Timeout,
    /// The plugin hit its memory cap (`memory.grow` was refused) and then trapped.
    Memory,
    Trap(String),
    /// The plugin's answer is not a valid message or not a valid answer header.
    BadOutput(String),
    /// The call could not be encoded (over a limit) or `alloc` returned garbage.
    BadInput(String),
    /// More than 16 rounds were asked for.
    TooManyRounds,
    /// The step callback refused.
    Step(String),
    /// A previous round failed.
    Dead,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Timeout => f.write_str("plugin stopped: time budget exceeded"),
            RunError::Memory => f.write_str("plugin stopped: memory limit reached"),
            RunError::Trap(e) => write!(f, "plugin trapped: {e}"),
            RunError::BadOutput(e) => write!(f, "plugin answer refused: {e}"),
            RunError::BadInput(e) => write!(f, "call refused: {e}"),
            RunError::TooManyRounds => write!(f, "more than {MAX_ROUNDS} rounds"),
            RunError::Step(e) => write!(f, "{e}"),
            RunError::Dead => f.write_str("session already failed"),
        }
    }
}
impl std::error::Error for RunError {}

struct Inner {
    engine: Engine,
    stop: AtomicBool,
    ticker: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.ticker.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
    }
}

/// The engine plus its epoch ticker. Cheap to clone.
#[derive(Clone)]
pub struct Host(Arc<Inner>);

impl Host {
    pub fn new() -> Result<Host, String> {
        let mut cfg = Config::new();
        cfg.epoch_interruption(true);
        // One memory only: the imported, capped one. A module-defined second memory would escape the cap (F1).
        cfg.wasm_multi_memory(false);
        // The guest runs on the UI thread deep inside a frame; the default 512 KiB overflows small thread stacks and aborts the process (F4).
        cfg.max_wasm_stack(MAX_WASM_STACK);
        let engine = Engine::new(&cfg).map_err(|e| e.to_string())?;
        let inner = Arc::new(Inner {
            engine: engine.clone(),
            stop: AtomicBool::new(false),
            ticker: Mutex::new(None),
        });
        // The thread holds the engine and a Weak, so dropping the last Host stops and joins it.
        let me = Arc::downgrade(&inner);
        let ticker = thread::Builder::new()
            .name("plugin-epoch".into())
            .spawn(move || {
                while let Some(i) = me.upgrade() {
                    if i.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    drop(i);
                    thread::sleep(TICK);
                    engine.increment_epoch();
                }
            })
            .map_err(|e| e.to_string())?;
        *inner.ticker.lock().unwrap() = Some(ticker);
        Ok(Host(inner))
    }

    /// Check and compile a module. `memory_mb` is the manifest's limit (1..=64). Cranelift takes 150-185 ms for a 108 KB
    /// module (spike), so UI code calls [`Host::load_in_background`].
    pub fn load(
        &self,
        wasm: &[u8],
        memory_mb: u32,
        round_budget: Duration,
    ) -> Result<Plugin, LoadError> {
        if memory_mb == 0 || memory_mb > MEMORY_CAP_MB {
            return Err(LoadError::MemoryLimit(memory_mb));
        }
        let module =
            Module::new(&self.0.engine, wasm).map_err(|e| LoadError::Compile(e.to_string()))?;
        let mem = check_module(&module, memory_mb)?;
        let ticks = (round_budget.as_millis() / TICK.as_millis()) as u64 + 1;
        let p = Plugin {
            host: self.clone(),
            module,
            mem,
            memory_mb,
            ticks,
        };
        let v = p
            .start()
            .map_err(|e| LoadError::Exports(e.to_string()))?
            .api_version;
        if v != 1 {
            return Err(LoadError::ApiVersion(v));
        }
        Ok(p)
    }

    pub fn load_in_background(
        &self,
        wasm: Vec<u8>,
        memory_mb: u32,
        round_budget: Duration,
    ) -> JoinHandle<Result<Plugin, LoadError>> {
        let me = self.clone();
        thread::spawn(move || me.load(&wasm, memory_mb, round_budget))
    }
}

/// Imports and exports of section 4.2; returns the type of the memory the host must create.
fn check_module(module: &Module, memory_mb: u32) -> Result<MemoryType, LoadError> {
    let (mut bad, mut mem) = (Vec::new(), None);
    for i in module.imports() {
        match (i.module(), i.name(), i.ty()) {
            ("env", "memory", ExternType::Memory(m)) if mem.is_none() => mem = Some(m),
            ("plugin_v1", "log", ExternType::Func(_)) => {} // the exact signature is checked when linking
            (m, n, _) => bad.push(format!("{m}.{n}")),
        }
    }
    if !bad.is_empty() {
        return Err(LoadError::Imports(bad));
    }
    let m = mem.ok_or(LoadError::NoMemoryImport)?;
    if let Some(e) = module
        .exports()
        .find(|e| matches!(e.ty(), ExternType::Memory(_)))
    {
        return Err(LoadError::ExportsMemory(e.name().to_string()));
    }
    let cap = u64::from(memory_mb) * 1024 * 1024 / PAGE;
    if m.is_64() || m.is_shared() {
        return Err(LoadError::MemoryType("shared or 64-bit".into()));
    }
    if m.minimum() > cap {
        return Err(LoadError::MemoryType(format!(
            "starts at {} pages, the cap is {cap}",
            m.minimum()
        )));
    }
    Ok(MemoryType::new(
        m.minimum() as u32,
        Some(m.maximum().map_or(cap, |x| x.min(cap)) as u32),
    ))
}

/// A checked, compiled plugin. Cheap to clone; every invocation takes its own [`Session`].
#[derive(Clone)]
pub struct Plugin {
    host: Host,
    module: Module,
    mem: MemoryType,
    memory_mb: u32,
    ticks: u64,
}

struct State {
    memory: Option<Memory>,
    max_bytes: usize,
    denied: bool,
    log: Vec<String>,
}

impl ResourceLimiter for State {
    fn memory_growing(
        &mut self,
        _cur: usize,
        desired: usize,
        _max: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let ok = desired <= self.max_bytes;
        self.denied |= !ok;
        Ok(ok)
    }
    fn table_growing(
        &mut self,
        _cur: usize,
        desired: usize,
        _max: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= 10_000)
    }
    fn instances(&self) -> usize {
        2
    }
    fn memories(&self) -> usize {
        1
    }
    fn tables(&self) -> usize {
        4
    }
}

impl Plugin {
    /// A fresh store and instance: no state survives between invocations.
    pub fn start(&self) -> Result<Session, RunError> {
        let mut store = Store::new(
            &self.host.0.engine,
            State {
                memory: None,
                max_bytes: self.memory_mb as usize * 1024 * 1024,
                denied: false,
                log: Vec::new(),
            },
        );
        store.limiter(|s| s);
        store.epoch_deadline_trap();
        store.set_epoch_deadline(self.ticks); // covers instantiation (a start function) too
        let trap = |e: wasmtime::Error| RunError::Trap(e.to_string());
        let memory = Memory::new(&mut store, self.mem.clone()).map_err(trap)?;
        store.data_mut().memory = Some(memory);
        let mut linker = Linker::new(&self.host.0.engine);
        linker
            .define(&store, "env", "memory", memory)
            .map_err(trap)?;
        linker
            .func_wrap(
                "plugin_v1",
                "log",
                |mut c: Caller<'_, State>, ptr: i32, len: i32| {
                    let Some(m) = c.data().memory else { return };
                    let n = (len.max(0) as usize).min(LOG_LINE);
                    let line = m
                        .data(&c)
                        .get(ptr as u32 as usize..)
                        .and_then(|d| d.get(..n))
                        .map(|b| String::from_utf8_lossy(b).into_owned());
                    if let Some(l) = line.filter(|_| c.data().log.len() < LOG_LINES) {
                        c.data_mut().log.push(l);
                    }
                },
            )
            .map_err(trap)?;
        let inst = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| map_trap(store.data().denied, e))?;
        let typed = |e: wasmtime::Error| RunError::Trap(format!("export: {e}"));
        let api = inst
            .get_typed_func::<(), i32>(&mut store, "api_version")
            .map_err(typed)?;
        let alloc = inst.get_typed_func(&mut store, "alloc").map_err(typed)?;
        let free = inst.get_typed_func(&mut store, "free").map_err(typed)?;
        let call = inst.get_typed_func(&mut store, "call").map_err(typed)?;
        store.set_epoch_deadline(self.ticks);
        let api_version = api
            .call(&mut store, ())
            .map_err(|e| map_trap(store.data().denied, e))?;
        Ok(Session {
            _host: self.host.clone(),
            store,
            memory,
            alloc,
            free,
            call,
            api_version,
            ticks: self.ticks,
            dead: false,
        })
    }
}

fn map_trap(denied: bool, e: wasmtime::Error) -> RunError {
    if e.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
        RunError::Timeout
    } else if denied {
        RunError::Memory
    } else {
        RunError::Trap(e.to_string())
    }
}

/// One invocation of a plugin (all its rounds).
pub struct Session {
    _host: Host, // keeps the epoch ticker alive as long as the session runs
    store: Store<State>,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    free: TypedFunc<(i32, i32), ()>,
    call: TypedFunc<(i32, i32), i64>,
    api_version: i32,
    ticks: u64,
    dead: bool,
}

impl Session {
    /// Lines the plugin passed to `plugin_v1.log` (for the audit log; never shown to users).
    pub fn log(&self) -> &[String] {
        &self.store.data().log
    }

    /// Did the plugin ask for memory beyond its cap?
    pub fn memory_denied(&self) -> bool {
        self.store.data().denied
    }

    pub fn memory_pages(&self) -> u64 {
        self.memory.size(&self.store)
    }

    /// Shorten the budget of every later round (events get a short one). Never lengthens it.
    pub fn limit_round_budget(&mut self, d: Duration) {
        self.ticks = self
            .ticks
            .min((d.as_millis() / TICK.as_millis()) as u64 + 1);
    }

    /// One round: copy the call in, run `call` under the deadline, validate and copy the answer out.
    pub fn round(&mut self, input: &Message) -> Result<Message, RunError> {
        if self.dead {
            return Err(RunError::Dead);
        }
        // On a thread of its own with a known stack: the caller is the UI thread deep inside a frame, whose spare stack is unknown (F4).
        let r = thread::scope(|sc| {
            thread::Builder::new()
                .name("plugin-round".into())
                .stack_size(ROUND_THREAD_STACK)
                .spawn_scoped(sc, || self.round_inner(input))
                .map_err(|e| RunError::Trap(format!("round thread: {e}")))?
                .join()
                .unwrap_or_else(|_| Err(RunError::Trap("round thread panicked".into())))
        });
        self.dead = r.is_err();
        r
    }

    fn round_inner(&mut self, input: &Message) -> Result<Message, RunError> {
        let bytes = input
            .encode()
            .map_err(|e| RunError::BadInput(e.to_string()))?;
        let len = bytes.len() as i32; // encode() capped it at 32 MiB
        self.store.set_epoch_deadline(self.ticks);
        let den = |s: &Store<State>, e| map_trap(s.data().denied, e);
        let ptr = self
            .alloc
            .call(&mut self.store, len)
            .map_err(|e| den(&self.store, e))?;
        self.memory
            .write(&mut self.store, ptr as u32 as usize, &bytes)
            .map_err(|_| RunError::BadInput("alloc returned a range outside memory".into()))?;
        let packed = self
            .call
            .call(&mut self.store, (ptr, len))
            .map_err(|e| den(&self.store, e))? as u64;
        let (optr, olen) = ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize);
        if olen > MAX_MESSAGE {
            return Err(RunError::BadOutput(WireError::TooLarge.to_string()));
        }
        let mut out = vec![0u8; olen];
        self.memory
            .read(&self.store, optr, &mut out)
            .map_err(|_| RunError::BadOutput("answer is outside memory".into()))?;
        // Freeing is best effort: a plugin that traps here has already given its answer, but the next round will fail.
        let _ = self.free.call(&mut self.store, (optr as i32, olen as i32));
        let _ = self.free.call(&mut self.store, (ptr, len));
        Message::decode(&out).map_err(|e| RunError::BadOutput(e.to_string()))
    }

    /// The round loop (section 4.3). `first` is the `command`/`ui`/`event` call. After each answer, `step` applies it and returns
    /// the `resume` call when the answer asked for one (`next`); `Ok(None)` ends the invocation. More than 16 rounds fail.
    pub fn run(
        &mut self,
        first: Message,
        mut step: impl FnMut(usize, Answer, Vec<Vec<u8>>) -> Result<Option<Message>, String>,
    ) -> Result<(), RunError> {
        let mut call = first;
        for round in 1.. {
            let out = self.round(&call)?;
            let ans: Answer = out
                .header_as()
                .map_err(|e| self.fail(RunError::BadOutput(e.to_string())))?;
            let wants_next = ans.next.is_some();
            match step(round, ans, out.blobs).map_err(|e| self.fail(RunError::Step(e)))? {
                Some(next) if wants_next => {
                    if round == MAX_ROUNDS {
                        return Err(self.fail(RunError::TooManyRounds));
                    }
                    call = next;
                }
                _ => return Ok(()),
            }
        }
        Ok(())
    }

    fn fail(&mut self, e: RunError) -> RunError {
        self.dead = true;
        e
    }
}
