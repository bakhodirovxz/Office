//! The real Win32 calls: the SID of this process, the user-only pipe, and the identity of a pipe server as a client sees it.
#![cfg(windows)]
use std::fs::OpenOptions;
use suite_winsec::{Sid, create_user_only_pipe, pipe_server_sid};
use tokio::net::windows::named_pipe::ServerOptions;

#[test]
fn sid_text_round_trips() {
    let (me, session) = Sid::current().unwrap();
    assert!(me.as_str().starts_with("S-1-5-"), "{}", me.as_str());
    assert_eq!(Sid::from_string(me.as_str()).unwrap(), me);
    assert_ne!(Sid::from_string("S-1-5-18").unwrap(), me); // LocalSystem
    assert!(session < 100_000);
    assert!(Sid::from_string("not a sid").is_err());
}

#[tokio::test]
async fn a_client_sees_who_runs_the_server() {
    let (me, _) = Sid::current().unwrap();
    let name = format!(r"\\.\pipe\winsec-test-{}", std::process::id());
    let server =
        create_user_only_pipe(ServerOptions::new().first_pipe_instance(true), &name, &me).unwrap();
    let client = OpenOptions::new().write(true).open(&name).unwrap();
    assert_eq!(pipe_server_sid(&client).unwrap(), me);
    // a second "first instance" of the same name is refused, as the single-instance rule needs
    assert!(
        create_user_only_pipe(ServerOptions::new().first_pipe_instance(true), &name, &me).is_err()
    );
    drop(server);
}

#[test]
fn a_dll_search_restriction_can_be_set() {
    suite_winsec::set_default_dll_directories().unwrap();
}
