//! `suite.exe`: the desktop host (PLAN phase 4, section 3.4, WP9). No console window: the Windows subsystem.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let args = std::env::args_os().skip(1).map(|a| {
        a.into_string()
            .unwrap_or_else(|o| o.to_string_lossy().into_owned())
    });
    match suite_desktop::parse_args(args) {
        #[cfg(windows)]
        Ok(a) => std::process::ExitCode::from(suite_desktop::run(a)),
        #[cfg(not(windows))]
        Ok(_) => {
            eprintln!(
                "suite: the desktop host is Windows only (a Linux host would link webkit2gtk, PLAN R16)"
            );
            std::process::ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::from(2)
        }
    }
}
