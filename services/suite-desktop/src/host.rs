//! The Windows part: the in-process `suite-serve` on a random loopback port, the single-instance pipe, the WebView2 window and the
//! browser fallback. Desktop actions reach the host only through the WebView IPC (`Ctx::message`), never through the HTTP port.
use crate::{
    Args, Brand, Ctx, Out, Source, Store, check_file, download_target, history_fragment,
    ipc_message, nav_ok, plugin_store, reply_script,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use suite_winsec::{Sid, create_user_only_pipe, pipe_server_sid};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use tokio::io::AsyncReadExt as _;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use wry::{NewWindowResponse, WebContext, WebViewBuilder, WebViewBuilderExtWindows as _};

enum Ev {
    Script(String),
    Focus,
}

/// What a second launch sends to the running instance, one JSON object per line.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum PipeMsg {
    Open {
        path: String,
    },
    /// show the History panel of this file
    History {
        path: String,
    },
    Focus {},
}

const PIPE_MAX: u64 = 64 * 1024;

fn dialog(level: rfd::MessageLevel, title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_level(level)
        .set_title(title)
        .set_description(text)
        .show();
}

/// A failed open is told to the user in a dialog (a launch from Explorer has no console to print to). It runs on its own thread.
fn report(title: &str, v: &Value) {
    if v["ok"] == false && v["guard"].is_null() {
        let (t, e) = (
            title.to_string(),
            v["error"].as_str().unwrap_or("error").to_string(),
        );
        std::thread::spawn(move || dialog(rfd::MessageLevel::Error, &t, &e));
    }
}

/// Opens a file for the user (the command line, the second launch, the native dialog) and tells them what went wrong, or what the
/// guard found (`crate::guard`): the wall and the external links, each with an offer to open the file in Excel.
fn open_and_report(ctx: &Arc<Ctx>, path: &str) -> Value {
    let v = ctx.open(path, Source::User);
    report(&ctx.brand.product, &v);
    if let Some(guard) = v["guard"].as_str().map(str::to_string) {
        let (ctx, path, excel) = (ctx.clone(), path.to_string(), v["excel"] == true);
        std::thread::spawn(move || {
            let title = ctx.brand.product.clone();
            let (text, level) = match guard.as_str() {
                "wall" => (crate::guard::WALL_TEXT, rfd::MessageLevel::Error),
                _ => (crate::guard::LINKS_TEXT, rfd::MessageLevel::Warning),
            };
            if !excel {
                dialog(level, &title, text);
                return;
            }
            let ask = rfd::MessageDialog::new()
                .set_level(level)
                .set_title(&title)
                .set_description(format!(
                    "{text}

Open it in Excel?"
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if ask == rfd::MessageDialogResult::Yes {
                let r = ctx.open_in_excel(&path, Source::User);
                if r["ok"] == false {
                    dialog(
                        rfd::MessageLevel::Error,
                        &title,
                        r["error"].as_str().unwrap_or("error"),
                    );
                }
            }
        });
    }
    v
}

/// The `--history` request: the file is checked like one from the command line; the shell is told to show its panel (`show_history`
/// event) and navigates to `#history=<path>` itself. A bad path is told to the user in a dialog.
fn show_history(ctx: &Ctx, path: &str) -> Option<Value> {
    match check_file(&ctx.brand, path, Source::User) {
        Ok(_) => Some(json!({"type": "show_history", "path": path})),
        Err(error) => {
            report(&ctx.brand.product, &json!({"ok": false, "error": error}));
            None
        }
    }
}

/// Test hooks (`SUITE_*`) exist in debug builds only: a release binary ignores these variables (review m10), so an environment
/// variable can neither pick an executable to start nor move the pipe, the data or the browser. The `WEBVIEW2_*` variables are
/// WebView2's own, not ours: a release build refuses to open the window while one that changes the browser is set (see `start`).
#[cfg(debug_assertions)]
fn hook(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}
#[cfg(not(debug_assertions))]
fn hook(_: &str) -> Option<std::ffi::OsString> {
    None
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// A fresh 256-bit key for this launch (review M1), as 64 hex digits. It is never written to disk.
/// The version history of this user. The debug data-dir hook keeps it inside that folder.
fn history_store(data: &Path, settings_dir: &Path) -> Option<suite_core::history::Store> {
    if hook("SUITE_DATA_DIR").is_none() {
        return suite_core::history::Store::from_settings();
    }
    let h = suite_core::settings::load_in(settings_dir).settings.history;
    Some(suite_core::history::Store::new(
        data.join("history"),
        suite_core::history::Limits::from(&h),
    ))
}

fn new_key() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).map_err(|e| format!("no random numbers: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn alnum(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`: whoever holds the other end can learn who we are but cannot act as us (B1).
const SQOS_IDENTIFICATION: u32 = 0x0010_0000 | 0x0001_0000;

/// A server may receive our paths only when its process runs as `ours`. Any failure to find that out counts as "not ours".
fn server_is(ours: &Sid, server: std::io::Result<Sid>) -> bool {
    server.is_ok_and(|s| &s == ours)
}

/// Sends the lines to the running instance. False when nobody answers (it is closing, or the name is held by something else, or by
/// another user: then nothing at all is written).
fn send(pipe: &str, lines: &[String], ours: &Sid) -> bool {
    use std::io::Write as _;
    for _ in 0..20 {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .security_qos_flags(SQOS_IDENTIFICATION)
            .open(pipe);
        if let Ok(mut f) = f {
            if !server_is(ours, pipe_server_sid(&f)) {
                return false;
            }
            return f
                .write_all(
                    lines
                        .join(
                            "
",
                        )
                        .as_bytes(),
                )
                .is_ok();
        }
        std::thread::sleep(Duration::from_millis(100)); // ERROR_PIPE_BUSY while the server makes the next instance
    }
    false
}

/// What a launch does when the pipe already exists.
#[derive(Debug, PartialEq)]
enum HandOver {
    /// the running instance has the request
    Done,
    /// nobody answers and there are files: open them in this process, no window
    OpenHere,
    /// nobody answers and there is nothing to open: run as a full instance without a pipe server
    CarryOn,
}

fn hand_over(files: &[String], history: Option<&str>, pipe: &str, me: &Sid) -> HandOver {
    let mut lines: Vec<String> = files
        .iter()
        .map(|f| json!({"type": "open", "path": f}).to_string())
        .collect();
    if let Some(h) = history {
        lines.push(json!({"type": "history", "path": h}).to_string());
    }
    if lines.is_empty() {
        lines.push(json!({"type": "focus"}).to_string());
    }
    if send(pipe, &lines, me) {
        HandOver::Done
    } else if files.is_empty() && history.is_none() {
        HandOver::CarryOn
    } else {
        HandOver::OpenHere
    }
}

async fn pipe_loop(
    mut server: NamedPipeServer,
    name: String,
    owner: Sid,
    ctx: Arc<Ctx>,
    proxy: Option<EventLoopProxy<Ev>>,
) {
    loop {
        if server.connect().await.is_err() {
            return;
        }
        let Ok(next) = create_user_only_pipe(
            ServerOptions::new().reject_remote_clients(true),
            &name,
            &owner,
        ) else {
            return;
        };
        let conn = std::mem::replace(&mut server, next);
        let (ctx, proxy) = (ctx.clone(), proxy.clone());
        tokio::spawn(async move {
            let mut buf = vec![];
            let read = tokio::time::timeout(
                Duration::from_secs(3),
                conn.take(PIPE_MAX).read_to_end(&mut buf),
            )
            .await;
            if read.is_err() {
                return;
            }
            for line in String::from_utf8_lossy(&buf)
                .lines()
                .filter(|l| !l.trim().is_empty())
            {
                // the same checks as for the page: a pipe message is data too
                if let Ok(PipeMsg::Open { path }) = serde_json::from_str(line) {
                    let v = open_and_report(&ctx, &path);
                    if let Some(p) = &proxy {
                        let _ = p.send_event(Ev::Script(reply_script(&v)));
                    }
                } else if let Ok(PipeMsg::History { path }) = serde_json::from_str(line) {
                    if let Some(p) = &proxy
                        && let Some(v) = show_history(&ctx, &path)
                    {
                        let _ = p.send_event(Ev::Script(reply_script(&v)));
                    }
                } else if serde_json::from_str::<PipeMsg>(line).is_err() {
                    break; // not ours: stop reading this connection
                }
            }
            if let Some(p) = &proxy {
                let _ = p.send_event(Ev::Focus);
            }
        });
    }
}

struct Served {
    port: u16,
}

/// The shell, the editors and the converter on `127.0.0.1:<free port>`, in this process.
fn serve(
    rt: &tokio::runtime::Runtime,
    layout: &suite_serve::Layout,
    data: &Path,
    plugins: Option<Arc<suite_serve::PluginSource>>,
) -> Result<Served, String> {
    let cfg = layout.shell_config()?;
    rt.block_on(async {
        for _ in 0..10 {
            let port = std::net::TcpListener::bind("127.0.0.1:0")
                .and_then(|l| l.local_addr())
                .map_err(|e| e.to_string())?
                .port();
            let s = convert_api::config::Settings {
                listen: format!("127.0.0.1:{port}"),
                work_dir: Some(data.join("work")),
                store_dir: Some(data.join("store")),
                ..Default::default()
            };
            let resolved = s.resolve()?;
            let limits = convert_api::tls::ConnLimits::from_settings(&resolved);
            let addr = resolved.listen;
            let Ok(listener) = tokio::net::TcpListener::bind(addr).await else {
                continue;
            }; // the port was taken meanwhile
            let app =
                convert_api::app::App::new(resolved, convert::exec::Config::from_env(), false)?;
            let router = suite_serve::router_plugins(
                app.clone(),
                layout,
                cfg.clone(),
                None,
                plugins.clone(),
            );
            let sweeper = app.clone();
            let tick = Duration::from_secs((app.r.s.retention_s / 4).clamp(1, 60));
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tick).await;
                    sweeper.sweep();
                }
            });
            tokio::spawn(convert_api::tls::serve(
                listener,
                None,
                router,
                limits,
                std::future::pending(),
            ));
            return Ok(Served { port });
        }
        Err("no free loopback port".to_string())
    })
}

pub fn run(args: Args) -> u8 {
    match start(args) {
        Ok(()) => 0,
        Err(e) => {
            // the title is the exe's own name, so no product name is written here
            let me = std::env::current_exe()
                .ok()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_default();
            eprintln!("{me}: {e}");
            dialog(rfd::MessageLevel::Error, &me, &e);
            1
        }
    }
}

fn start(args: Args) -> Result<(), String> {
    // later LoadLibrary calls search the application folder and System32 only, not the current folder or PATH (review m7)
    let _ = suite_winsec::set_default_dll_directories();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let dist = args.dist.unwrap_or_else(|| exe_dir.join("dist"));
    let layout = suite_serve::Layout {
        shell: args.shell.unwrap_or_else(|| dist.join("shell")),
        dist,
        brand: args.brand.unwrap_or_else(|| exe_dir.join("brand.toml")),
        local_key: Some(new_key()?),
    };
    let brand_text = std::fs::read_to_string(&layout.brand)
        .map_err(|e| format!("{}: {e}", layout.brand.display()))?;
    let brand = Brand::parse(&brand_text)?;
    let data = hook("SUITE_DATA_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| env_dir("LOCALAPPDATA").map(|d| d.join(&brand.product).join("Host")))
        .ok_or("no data directory (LOCALAPPDATA is not set)")?;
    // the shared settings.toml (PLAN 5.1); the debug data-dir hook keeps it inside that folder unless SUITE_SETTINGS_DIR says otherwise
    let settings_dir = match (hook("SUITE_DATA_DIR"), hook("SUITE_SETTINGS_DIR")) {
        (Some(_), None) => data.clone(),
        _ => suite_core::settings::dir().unwrap_or_else(|| data.clone()),
    };
    let bin_dir = args.bin_dir.unwrap_or(exe_dir);
    let editor_cwd = bin_dir.clone();
    let product = brand.product.clone();
    // the name holds the user's SID and session, not USERNAME (which is public and collides between a local and a domain account)
    let (me, session) =
        Sid::current().map_err(|e| format!("cannot read the user identity: {e}"))?;
    let pipe = format!(
        r"\\.\pipe\{}-host-{}-{session}{}",
        alnum(&brand.id),
        me.as_str(),
        alnum(&hook("SUITE_INSTANCE").unwrap_or_default().to_string_lossy())
    );
    let ctx = Arc::new(Ctx {
        brand,
        store: Store::with_settings_dir(data.clone(), settings_dir.clone()),
        bin_dir,
        history: history_store(&data, &settings_dir),
        downloads: env_dir("USERPROFILE")
            .map_or_else(|| data.join("downloads"), |d| d.join("Downloads")),
        plugins: Some(plugin_store(&settings_dir)),
        // the editor starts in the install folder, not in the folder of the document (review m7: a DLL it loads late must not be
        // found next to a downloaded file)
        spawn: Box::new(move |exe, file| {
            std::process::Command::new(exe)
                .arg(file)
                .current_dir(&editor_cwd)
                .spawn()
                .map(drop)
        }),
    });
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    // F14: the first instance owns the pipe. Creating it with `first_pipe_instance` fails when another one exists. Only our user
    // may open it (a protected DACL), so no other user can connect to it, and a name taken by someone else is not trusted (B1).
    let first = {
        let _g = rt.enter();
        create_user_only_pipe(
            ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true),
            &pipe,
            &me,
        )
    };
    let server = match first {
        Ok(s) => Some(s),
        Err(_) => match hand_over(&args.files, args.history.as_deref(), &pipe, &me) {
            HandOver::Done => return Ok(()),
            HandOver::OpenHere => {
                // nobody answers: a launch never fails, so open the files here and run without the pipe
                for f in &args.files {
                    open_and_report(&ctx, f);
                }
                return Ok(());
            }
            // F16: the name is held by something that is not our instance; a plain launch still starts, just without the pipe server
            HandOver::CarryOn => None,
        },
    };

    // the enabled plugins are read from settings.toml on every request, so enabling one needs no restart
    let plugin_src = {
        let s = plugin_store(&settings_dir);
        Arc::new(suite_serve::PluginSource {
            roots: s.roots(),
            now: Box::new(move || (s.state(), s.policy())),
            fixed: None,
        })
    };
    let Served { port } = serve(&rt, &layout, &data, Some(plugin_src))?;
    let _ = std::fs::create_dir_all(&data);
    let host_json = data.join("host.json");
    let _ = std::fs::write(
        &host_json,
        json!({"pid": std::process::id(), "port": port}).to_string(),
    );
    for f in &args.files {
        open_and_report(&ctx, f);
    }
    // the first request sets the cookie that every later request needs (M1); the key stays in this process and the opener's command line
    // `--history FILE` opens the shell on the History panel of that file (a fragment survives the redirect of /_k/)
    let fragment = args
        .history
        .as_deref()
        .and_then(|h| show_history(&ctx, h))
        .map(|_| history_fragment(args.history.as_deref().unwrap_or_default()))
        .unwrap_or_default();
    let start_url = format!(
        "http://127.0.0.1:{port}/_k/{}{fragment}",
        layout.local_key.as_deref().unwrap_or_default()
    );

    // WebView2 missing (or forced missing, for tests) -> the default browser
    let no_webview = args.browser
        || hook("SUITE_FORCE_NO_WEBVIEW2").is_some()
        || wry::webview_version().is_err();
    if no_webview {
        if let Some(s) = server {
            rt.spawn(pipe_loop(s, pipe, me, ctx.clone(), None));
        }
        return browser_fallback(&product, &start_url, &host_json);
    }

    #[cfg(not(debug_assertions))]
    if let Some(k) = crate::webview2_env_override(|k| std::env::var_os(k).is_some()) {
        return Err(format!(
            "{k} is set in the environment; remove it to start the window"
        ));
    }

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    if let Some(s) = server {
        rt.spawn(pipe_loop(s, pipe, me, ctx.clone(), Some(proxy.clone())));
    }
    let window = WindowBuilder::new()
        .with_title(&product)
        .with_inner_size(LogicalSize::new(1100.0, 760.0))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    // Nothing of the web layer is kept (review M2): the WebView runs InPrivate, and what an earlier version left in the profile
    // (IndexedDB copies of saved documents, one origin per launch) is deleted. This is the only instance, so nothing else uses it.
    let _ = std::fs::remove_dir_all(data.join("webview2"));
    // read-only copies of old versions (History > View); a copy an editor still holds stays
    let _ = std::fs::remove_dir_all(data.join("temp"));
    let mut web_context = WebContext::new(Some(data.join("webview2")));
    let ipc_proxy = proxy.clone();
    let ipc_ctx = ctx.clone();
    let (dl_ctx, dl_proxy, done_ctx, done_proxy) =
        (ctx.clone(), proxy.clone(), ctx.clone(), proxy.clone());
    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_url(&start_url)
        .with_incognito(true)
        .with_devtools(false)
        // no browser shortcuts (print, save page, reload) and no context menu: they would bypass our own UI (review m6)
        .with_browser_accelerator_keys(false)
        .with_default_context_menus(false)
        // SmartScreen stays off (wry's default): this WebView loads no remote content, and the downloads are the user's own documents
        // a save is a download: only our file types, never over an existing file, and recorded for Recent (review M2, m2, m6)
        .with_download_started_handler(move |_url, path| match download_target(&dl_ctx.brand, path) {
            Ok(p) => {
                *path = p;
                true
            }
            Err(error) => {
                let v = json!({"type": "saved", "ok": false, "error": error});
                let _ = dl_proxy.send_event(Ev::Script(reply_script(&v)));
                false
            }
        })
        .with_download_completed_handler(move |_url, path, ok| {
            let v = match path {
                Some(p) if ok => done_ctx.saved(&p),
                _ => json!({"type": "saved", "ok": false, "error": "the file could not be saved"}),
            };
            let _ = done_proxy.send_event(Ev::Script(reply_script(&v)));
        })
        .with_navigation_handler(move |u| nav_ok(&u, port))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_ipc_handler(move |req| {
            // IPC only from our own origin; anything else gets no answer at all
            // F11: History reads, hashes and rewrites whole documents (up to the size cap), so it runs on a worker like Pick; the page
            // matches the reply by the path it echoes
            let history = serde_json::from_str::<serde_json::Value>(req.body())
                .is_ok_and(|v| v["type"].as_str().is_some_and(|t| t.starts_with("history_")));
            if history {
                let (ctx, p, uri, body) = (
                    ipc_ctx.clone(),
                    ipc_proxy.clone(),
                    req.uri().to_string(),
                    req.body().clone(),
                );
                std::thread::spawn(move || {
                    if let Some(Out::Reply(v)) = ipc_message(&ctx, &uri, port, &body) {
                        let _ = p.send_event(Ev::Script(reply_script(&v)));
                    }
                });
                return;
            }
            let Some(out) = ipc_message(&ipc_ctx, &req.uri().to_string(), port, req.body()) else {
                return;
            };
            match out {
                Out::Reply(v) => {
                    let _ = ipc_proxy.send_event(Ev::Script(reply_script(&v)));
                }
                Out::Work(work) => {
                    let p = ipc_proxy.clone();
                    std::thread::spawn(move || {
                        let _ = p.send_event(Ev::Script(reply_script(&work())));
                    });
                }
                Out::PickPackage => {
                    let (ctx, p) = (ipc_ctx.clone(), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let v = match rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file() {
                            Some(f) => ctx.plugins_install(&f.to_string_lossy(), Source::User),
                            None => json!({"type": "plugins", "ok": false, "error": "cancelled", "cancelled": true}),
                        };
                        let _ = p.send_event(Ev::Script(reply_script(&v)));
                    });
                }
                Out::Pick(exts) => {
                    let (ctx, p) = (ipc_ctx.clone(), ipc_proxy.clone());
                    // the dialog blocks, so it must not run on the UI thread
                    std::thread::spawn(move || {
                        let v = match rfd::FileDialog::new().add_filter("*", &exts).pick_file() {
                            Some(f) => open_and_report(&ctx, &f.to_string_lossy()),
                            None => json!({"type": "opened", "ok": false, "error": "cancelled", "cancelled": true}),
                        };
                        let _ = p.send_event(Ev::Script(reply_script(&v)));
                    });
                }
            }
        })
        .build(&window)
        .map_err(|e| format!("cannot create the WebView2 window: {e}"))?;

    event_loop.run(move |event, _, flow| {
        let _keep = (&rt, &web_context);
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ev::Script(s)) => {
                let _ = webview.evaluate_script(&s);
            }
            Event::UserEvent(Ev::Focus) => {
                window.set_minimized(false);
                window.set_focus();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let _ = std::fs::remove_file(&host_json);
                *flow = ControlFlow::Exit;
            }
            _ => {}
        }
    })
}

/// No WebView2: open the default browser on the local URL and say so. The server runs as long as the message is open.
fn browser_fallback(product: &str, url: &str, host_json: &Path) -> Result<(), String> {
    // `explorer <url>` opens the default browser; the URL is ours (a fixed loopback address)
    let opener = hook("SUITE_BROWSER").unwrap_or_else(|| "explorer".into());
    std::process::Command::new(opener)
        .arg(url)
        .spawn()
        .map_err(|e| format!("cannot open the browser: {e}"))?;
    if let Some(ms) =
        hook("SUITE_TEST_EXIT_AFTER_MS").and_then(|v| v.to_str().and_then(|v| v.parse().ok()))
    {
        std::thread::sleep(Duration::from_millis(ms)); // test hook: no dialog
    } else {
        let shown = url.split("/_k/").next().unwrap_or(url); // the key stays off the screen
        dialog(
            rfd::MessageLevel::Warning,
            product,
            &format!(
                "The Microsoft Edge WebView2 Runtime is not installed, so {product} opened in your default browser at {shown}\n\n\
                 Keep this message open while you work. Close it to stop the local server.\n\
                 Install the WebView2 Runtime to use the {product} window."
            ),
        );
    }
    let _ = std::fs::remove_file(host_json);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foreign() -> Sid {
        Sid::from_string("S-1-5-18").unwrap() // LocalSystem stands in for another identity
    }

    #[test]
    fn a_server_run_by_another_identity_is_refused() {
        let (me, _) = Sid::current().unwrap();
        assert!(server_is(&me, Ok(me.clone())));
        assert!(!server_is(&me, Ok(foreign())));
        assert!(!server_is(&me, Err(std::io::Error::other("access denied"))));
    }

    /// The real client path against a real pipe server: with a matching identity the line arrives; when the server's identity is
    /// not the expected one, `send` writes nothing and says so (the caller then opens the files itself).
    #[tokio::test(flavor = "multi_thread")]
    async fn send_writes_only_to_a_server_of_our_own_identity() {
        let (me, session) = Sid::current().unwrap();
        for (expected, delivered) in [(me.clone(), true), (foreign(), false)] {
            let name = format!(
                r"\\.\pipe\suite-host-test-{}-{session}-{delivered}",
                std::process::id()
            );
            let server =
                create_user_only_pipe(ServerOptions::new().first_pipe_instance(true), &name, &me)
                    .unwrap();
            let reader = tokio::spawn(async move {
                server.connect().await.unwrap();
                let mut buf = vec![];
                let _ = tokio::time::timeout(
                    Duration::from_secs(3),
                    server.take(1024).read_to_end(&mut buf),
                )
                .await;
                buf
            });
            let (n, ex) = (name.clone(), expected.clone());
            let sent = tokio::task::spawn_blocking(move || send(&n, &["hello".into()], &ex))
                .await
                .unwrap();
            assert_eq!(sent, delivered);
            let got = reader.await.unwrap();
            assert_eq!(got, if delivered { b"hello".to_vec() } else { vec![] });
        }
    }

    /// F16: a pipe name squatted by a foreign server never stops a plain launch; files are still opened in-process.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_squatted_pipe_name_does_not_stop_a_launch() {
        let (me, session) = Sid::current().unwrap();
        let name = format!(
            r"\\.\pipe\suite-host-test-{}-{session}-squat",
            std::process::id()
        );
        let _squatter =
            create_user_only_pipe(ServerOptions::new().first_pipe_instance(true), &name, &me)
                .unwrap();
        let second = {
            let _g = tokio::runtime::Handle::current().enter();
            create_user_only_pipe(ServerOptions::new().first_pipe_instance(true), &name, &me)
        };
        assert!(second.is_err(), "the name is taken");
        // the squatter is not run by the identity we expect, so nothing is sent to it
        let (n, with_file) = (name.clone(), name.clone());
        let plain = tokio::task::spawn_blocking(move || hand_over(&[], None, &n, &foreign()))
            .await
            .unwrap();
        assert_eq!(plain, HandOver::CarryOn);
        let got = tokio::task::spawn_blocking(move || {
            hand_over(&["a.docx".to_string()], None, &with_file, &foreign())
        })
        .await
        .unwrap();
        assert_eq!(got, HandOver::OpenHere);
    }
}
