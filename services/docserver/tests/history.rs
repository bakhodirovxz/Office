//! WP11 (PLAN phase 5, section 6): the server version store behind the session routes.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";
const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
/// A minimal docx package: saves pass the S9 package check (WP4).
fn docx(tag: &str) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    for (n, b) in [("[Content_Types].xml", "x"), ("word/document.xml", tag)] {
        w.start_file(n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn build(name: &str) -> (Arc<Docserver>, PathBuf) {
    build_with(name, "", 120)
}

/// `extra` is top-level `[docserver]` TOML; `{d}` stands for the test folder.
fn build_with(name: &str, extra: &str, diff_timeout_s: u64) -> (Arc<Docserver>, PathBuf) {
    let d = std::env::temp_dir().join(format!("docserver-hist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("server.key"), SERVER_SECRET).unwrap();
    std::fs::write(d.join("prov.key"), PROV_SECRET).unwrap();
    std::fs::write(d.join("atrest.key"), "a".repeat(40)).unwrap();
    let mut ds = open_at(&d, extra);
    ds.diff_timeout_s = diff_timeout_s;
    (Arc::new(ds), d)
}

/// The server over the folder `d` as it is (a restart: nothing is removed first).
fn open_at(d: &std::path::Path, extra: &str) -> Docserver {
    open_with(d, extra, "")
}

/// `open_at` with `prov`: TOML lines of the provider entry.
fn open_with(d: &std::path::Path, extra: &str, prov: &str) -> Docserver {
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let extra = extra.replace("{d}", &d.to_string_lossy().replace(char::from(92), "/"));
    let t: toml::Table = format!(
        r#"{extra}
server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19998"
internal_allow = ["127.0.0.0/8"]
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
callback_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
require_exp = true
{prov}
"#,
        p("server.key"),
        p("data"),
        p("prov.key")
    )
    .parse()
    .unwrap();
    Docserver::from_table(t, None).unwrap()
}

fn config(key: &str, edit: bool) -> Value {
    config_perms(key, edit, true, true)
}

fn config_perms(key: &str, edit: bool, download: bool, print: bool) -> Value {
    json!({"documentType": "word",
        "document": {"fileType": "docx", "key": key, "title": "a.docx", "url": "http://api:8000/f/1",
            "permissions": {"edit": edit, "download": download, "print": print}},
        "editorConfig": {"mode": "edit", "callbackUrl": "http://api:8000/cb", "user": {"id": "emp-1", "name": "U"}},
        "exp": now() + 3600})
}

async fn call(
    r: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: &[u8],
) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::from(body.to_vec())).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    (
        s,
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

fn js(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap_or(Value::Null)
}

/// A session for `key` with its document stored as version 1 (what WP4's fetch will do).
async fn session(ds: &Arc<Docserver>, r: &Router, key: &str, edit: bool) -> (String, String) {
    session_with(ds, r, config(key, edit)).await
}

async fn session_with(ds: &Arc<Docserver>, r: &Router, cfg: Value) -> (String, String) {
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &cfg)}});
    let (s, b) = call(r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    assert_eq!(s, StatusCode::OK);
    let v = js(&b);
    let sid = v["sid"].as_str().unwrap().to_string();
    ds.put_version(&sid, &docx("history v1"), "provider", false, now())
        .unwrap();
    (sid, v["token"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn saves_list_restore_and_name_over_the_routes() {
    let (ds, d) = build("flow");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (s, _) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/save"),
        Some(&tok),
        &docx("history v2"),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let h = format!("/ds/session/{sid}/history");
    let (s, b) = call(&r, "GET", &h, Some(&tok), b"").await;
    let vs = js(&b)["versions"].as_array().unwrap().clone();
    assert_eq!(
        (s, vs.len(), vs[1]["author"].as_str()),
        (StatusCode::OK, 2, Some("emp-1"))
    );
    assert_eq!(
        call(&r, "GET", &format!("{h}/1"), Some(&tok), b"").await.1,
        docx("history v1")
    );

    // restore: always a new version, in the store and in the session
    let (s, b) = call(&r, "POST", &format!("{h}/1/restore"), Some(&tok), b"").await;
    let j = js(&b);
    assert_eq!(
        (
            s,
            j["version"]["v"].as_u64(),
            j["version"]["restored_from"].as_u64()
        ),
        (StatusCode::OK, Some(3), Some(1))
    );
    assert_eq!(j["session_version"].as_u64(), Some(3));
    let (_, b) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/document"),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(b, docx("history v1"));
    assert_eq!(
        js(&call(&r, "GET", &h, Some(&tok), b"").await.1)["versions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let (s, _) = call(
        &r,
        "POST",
        &format!("{h}/2/name"),
        Some(&tok),
        br#"{"name":"signed"}"#,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = call(&r, "GET", &h, Some(&tok), b"").await;
    assert_eq!(js(&b)["versions"][1]["name"].as_str(), Some("signed"));
    assert_eq!(
        call(&r, "GET", &format!("{h}/99"), Some(&tok), b"").await.0,
        StatusCode::NOT_FOUND
    );
    let _ = std::fs::remove_dir_all(d);
}

/// Gate review G3 (docs/phase5/gate-security.md): a restore changes the document, so like save, autosave and dirty it waits for the
/// provider's copy (docs/phase5/dropin.md D1). Before, a replayed config token (no `exp` in `compat = "docsapi"`) whose document URL had
/// run out could restore an old version as version 1 of a new session and then save any bytes to the provider (status 6).
#[tokio::test]
async fn restore_waits_for_the_providers_document_like_a_save() {
    let (ds, d) = build("restore-first");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let save = |sid: String, tok: String, b: Vec<u8>| {
        let r = r.clone();
        async move {
            call(
                &r,
                "POST",
                &format!("/ds/session/{sid}/save"),
                Some(&tok),
                &b,
            )
            .await
            .0
        }
    };
    assert_eq!(
        save(sid.clone(), tok.clone(), docx("history v2")).await,
        StatusCode::OK
    );
    // the same key opened again, its document never fetched
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &config("k1", true))}});
    let (s, b) = call(&r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    assert_eq!(s, StatusCode::OK);
    let v = js(&b);
    let (sid2, tok2) = (
        v["sid"].as_str().unwrap().to_string(),
        v["token"].as_str().unwrap().to_string(),
    );
    let (s, b) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid2}/history/1/restore"),
        Some(&tok2),
        b"",
    )
    .await;
    assert_eq!(
        (s, js(&b)["error"]["code"].as_str()),
        (StatusCode::CONFLICT, Some("no_document"))
    );
    assert_eq!(
        save(sid2.clone(), tok2, docx("forged")).await,
        StatusCode::CONFLICT
    );
    assert!(ds.load_session(&sid2).unwrap().versions.is_empty());
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn history_routes_check_the_session_token_and_permissions() {
    let (ds, d) = build("auth");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (sid2, tok2) = session(&ds, &r, "k2", false).await;
    let h = format!("/ds/session/{sid}/history");
    assert_eq!(
        call(&r, "GET", &h, None, b"").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&r, "GET", &h, Some("junk"), b"").await.0,
        StatusCode::UNAUTHORIZED
    );
    // another session's token: the same 404 as a session that does not exist
    let other = call(&r, "GET", &h, Some(&tok2), b"").await;
    let none = call(
        &r,
        "GET",
        &format!("/ds/session/{}/history", "0".repeat(32)),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(
        (other.0, none.0),
        (StatusCode::NOT_FOUND, StatusCode::NOT_FOUND)
    );
    assert_eq!(other.1, none.1);
    // a view session neither reads (gate review L4: `view_history` is off) nor restores or names
    let h2 = format!("/ds/session/{sid2}/history");
    assert_eq!(
        call(&r, "GET", &h2, Some(&tok2), b"").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&r, "POST", &format!("{h2}/1/restore"), Some(&tok2), b"")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &r,
            "POST",
            &format!("{h2}/1/name"),
            Some(&tok2),
            br#"{"name":"x"}"#
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // histories of different documents do not mix
    assert_eq!(
        js(&call(&r, "GET", &h, Some(&tok), b"").await.1)["versions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn a_rewritten_server_history_fails_verification() {
    let (ds, d) = build("chain");
    let r = ds.routes::<()>();
    let (_sid, _tok) = session(&ds, &r, "k1", true).await;
    let idx = std::fs::read_dir(d.join("data/history/docs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&idx).unwrap();
    std::fs::write(&idx, text.replace("provider", "mallory")).unwrap();
    let (sid, tok) = session(&ds, &r, "k1", true).await; // the damaged chain refuses a new append
    let _ = sid;
    let (s, _) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/history"),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(
        s,
        StatusCode::INTERNAL_SERVER_ERROR,
        "a history that does not verify is not served"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn download_false_is_refused_in_an_edit_session_and_audited() {
    let (ds, d) = build("nodl");
    let r = ds.routes::<()>();
    let (sid, tok) = session_with(&ds, &r, config_perms("k1", true, false, false)).await;
    let h = format!("/ds/session/{sid}/history");
    // the version download and the compare (which shows both texts) are refused for an editor with download:false
    for uri in [format!("{h}/1"), format!("{h}/1/compare/1")] {
        assert_eq!(
            call(&r, "GET", &uri, Some(&tok), b"").await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
    // everything the editor itself needs keeps working: the working copy, the list, a save
    let doc = format!("/ds/session/{sid}/document");
    assert_eq!(
        call(&r, "GET", &doc, Some(&tok), b"").await,
        (StatusCode::OK, docx("history v1"))
    );
    assert_eq!(call(&r, "GET", &h, Some(&tok), b"").await.0, StatusCode::OK);
    let save = format!("/ds/session/{sid}/save");
    assert_eq!(
        call(&r, "POST", &save, Some(&tok), &docx("history v2"))
            .await
            .0,
        StatusCode::OK
    );
    let audit = std::fs::read_to_string(d.join("data/audit.jsonl")).unwrap();
    let refused: Vec<_> = audit
        .lines()
        .filter(|l| l.contains("perm.refused"))
        .collect();
    assert_eq!(refused.len(), 2, "{audit}");
    assert!(
        refused
            .iter()
            .all(|l| l.contains("download") && !l.contains(&tok))
    );
    let _ = std::fs::remove_dir_all(d);
}

/// The fake tool installed as `<d>/bin/<app>-diff`.
fn install_fake(d: &std::path::Path, app: &str) {
    let bin = d.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let to = bin.join(format!("{app}-diff{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_fake_diff"), to).unwrap();
}

fn leftovers(d: &std::path::Path, sid: &str) -> usize {
    std::fs::read_dir(d.join("data/sessions").join(sid))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("compare-"))
        .count()
}

async fn compare_of(r: &Router, sid: &str, tok: &str, a: u64, b: u64) -> (StatusCode, Value) {
    let uri = format!("/ds/session/{sid}/history/{a}/compare/{b}");
    let (s, body) = call(r, "GET", &uri, Some(tok), b"").await;
    (s, js(&body))
}

#[tokio::test]
async fn compare_runs_the_diff_tool_on_copies_and_cleans_up() {
    let (ds, d) = build_with("cmp", "diff_bin_dir = \"{d}/bin\"", 2);
    install_fake(&d, "docs");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    // versions 2 to 5: plain text, a tool failure, a huge answer, a hang (the fake tool reads these words)
    for text in [&b"plain two"[..], b"FAIL", b"BIG", b"SLEEP"] {
        ds.put_version(&sid, text, "provider", false, now())
            .unwrap();
    }
    // a result: exit 1 means "different", and the tool's JSON is passed on whole
    let (s, j) = compare_of(&r, &sid, &tok, 2, 1).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(
        (
            j["ok"].as_bool(),
            j["app"].as_str(),
            j["a"].as_u64(),
            j["diff"]["tool"].as_str(),
            j["diff"]["a_len"].as_u64()
        ),
        (
            Some(true),
            Some("docs"),
            Some(2),
            Some("fake-diff"),
            Some(9)
        )
    );
    // the same version twice: exit 0 is a result too
    assert_eq!(
        compare_of(&r, &sid, &tok, 2, 2).await.1["diff"]["same"],
        json!(true)
    );
    // trouble (exit 2) carries the tool's message; too much output is refused; a hang is killed at the deadline
    let (s, j) = compare_of(&r, &sid, &tok, 3, 2).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("compare_failed"))
    );
    assert!(
        j["error"]["message"]
            .as_str()
            .unwrap()
            .contains("could not compare")
    );
    let (s, j) = compare_of(&r, &sid, &tok, 4, 2).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("compare_too_large"))
    );
    let t = std::time::Instant::now();
    let (s, j) = compare_of(&r, &sid, &tok, 5, 2).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::GATEWAY_TIMEOUT, Some("compare_timeout"))
    );
    assert!(
        t.elapsed().as_secs() < 20,
        "the hung tool must be killed at the deadline"
    );
    assert_eq!(
        compare_of(&r, &sid, &tok, 2, 99).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        leftovers(&d, &sid),
        0,
        "the copies are removed, on success and on error"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn compare_answers_501_without_the_tool_and_for_other_kinds_and_403_without_download() {
    // diff_bin_dir unset
    let (ds, d) = build("cmp501");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (s, j) = compare_of(&r, &sid, &tok, 1, 1).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::NOT_IMPLEMENTED, Some("not_configured"))
    );
    let uri = format!("/ds/session/{sid}/history/1/compare/1");
    assert_eq!(
        call(&r, "GET", &uri, None, b"").await.0,
        StatusCode::UNAUTHORIZED
    );
    let _ = std::fs::remove_dir_all(d);

    // set, but the binary of this kind is missing (only docs-diff is there); a slides session; no download right
    let (ds, d) = build_with("cmp501b", "diff_bin_dir = \"{d}/bin\"", 2);
    install_fake(&d, "docs");
    let r = ds.routes::<()>();
    let mut sheet = config("k2", true);
    sheet["documentType"] = json!("cell");
    sheet["document"]["fileType"] = json!("xlsx");
    let (sid, tok) = session_with(&ds, &r, sheet).await;
    let (s, j) = compare_of(&r, &sid, &tok, 1, 1).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::NOT_IMPLEMENTED, Some("not_configured"))
    );
    let mut slides = config("k3", true);
    slides["documentType"] = json!("slide");
    slides["document"]["fileType"] = json!("pptx");
    let (sid, tok) = session_with(&ds, &r, slides).await;
    let (s, j) = compare_of(&r, &sid, &tok, 1, 1).await;
    assert_eq!(
        (s, j["error"]["code"].as_str()),
        (StatusCode::NOT_IMPLEMENTED, Some("not_supported"))
    );
    let (sid, tok) = session_with(&ds, &r, config_perms("k4", false, false, true)).await;
    assert_eq!(
        compare_of(&r, &sid, &tok, 1, 1).await.0,
        StatusCode::FORBIDDEN
    );
    let _ = std::fs::remove_dir_all(d);
}

fn holds(dir: &std::path::Path, needle: &[u8]) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            holds(&p, needle)
        } else {
            std::fs::read(&p).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle))
        }
    })
}

#[tokio::test]
async fn compare_hands_the_tool_plaintext_while_the_store_stays_sealed() {
    let (ds, d) = build_with(
        "cmpenc",
        "encrypt_at_rest = true\nencrypt_key_file = \"{d}/atrest.key\"\ndiff_bin_dir = \"{d}/bin\"",
        5,
    );
    install_fake(&d, "docs");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    ds.put_version(&sid, b"a secret sentence", "provider", false, now())
        .unwrap();
    let (s, j) = compare_of(&r, &sid, &tok, 2, 1).await;
    // the tool read the plaintext (its first file is 17 bytes long); the disk holds none of it
    assert_eq!(
        (s, j["diff"]["a_len"].as_u64()),
        (StatusCode::OK, Some(17)),
        "{j}"
    );
    assert_eq!(leftovers(&d, &sid), 0);
    assert!(
        !holds(&d.join("data"), b"a secret sentence"),
        "plaintext left on disk"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn compare_copies_a_crash_left_behind_are_removed_at_start() {
    let extra = "encrypt_at_rest = true\nencrypt_key_file = \"{d}/atrest.key\"";
    let (ds, d) = build_with("cmpcrash", extra, 5);
    let r = ds.routes::<()>();
    let (sid, _) = session(&ds, &r, "k1", true).await;
    // what a compare leaves when the process dies before its scratch guard runs: plaintext copies in the session folder
    let scratch = d
        .join("data/sessions")
        .join(&sid)
        .join("compare-0011223344556677");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("a.docx"), b"a secret sentence").unwrap();
    drop((r, ds));
    let _restarted = open_at(&d, extra);
    assert_eq!(leftovers(&d, &sid), 0);
    assert!(
        !holds(&d.join("data"), b"a secret sentence"),
        "plaintext left on disk"
    );
    // the session itself is untouched
    assert!(d.join("data/sessions").join(&sid).join("v1.bin").is_file());
    let _ = std::fs::remove_dir_all(d);
}

/// Gate review L4 (docs/phase5/gate-security.md, rehearsal F5): the history of a `document.key` is shared by its sessions, so a view
/// session of a stable key would see versions others saved. View sessions get none unless the provider has `view_history = true`;
/// edit sessions keep it. The session answer tells the wrapper whether to offer the panel.
#[tokio::test]
async fn a_view_session_sees_no_history_unless_the_provider_allows_it() {
    let (ds, d) = build("view-hist");
    let r = ds.routes::<()>();
    let opened = |r: Router, cfg: Value| async move {
        let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &cfg)}});
        js(
            &call(&r, "POST", "/ds/session", None, body.to_string().as_bytes())
                .await
                .1,
        )
    };
    assert_eq!(
        opened(r.clone(), config("k1", true)).await["history"],
        json!(true)
    );
    assert_eq!(
        opened(r.clone(), config("k1", false)).await["history"],
        json!(false)
    );
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (sid2, tok2) = session(&ds, &r, "k1", false).await;
    let routes = |sid: &str| {
        let h = format!("/ds/session/{sid}/history");
        [h.clone(), format!("{h}/1"), format!("{h}/1/compare/1")]
    };
    for u in routes(&sid2) {
        let (s, b) = call(&r, "GET", &u, Some(&tok2), b"").await;
        assert_eq!(
            (s, js(&b)["error"]["code"].as_str()),
            (StatusCode::FORBIDDEN, Some("forbidden")),
            "{u}"
        );
    }
    // the edit session of the same key: list and download (compare is 501 here: no tool installed, past the permission checks)
    let [list, one, cmp] = routes(&sid);
    assert_eq!(
        call(&r, "GET", &list, Some(&tok), b"").await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&r, "GET", &one, Some(&tok), b"").await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&r, "GET", &cmp, Some(&tok), b"").await.0,
        StatusCode::NOT_IMPLEMENTED
    );
    // the refusals are audited as permission refusals
    let audit = std::fs::read_to_string(d.join("data/audit.jsonl")).unwrap();
    assert_eq!(
        audit
            .lines()
            .filter(|l| l.contains("perm.refused") && l.contains(r#""perm":"history""#))
            .count(),
        3
    );

    // the operator allows it for the provider: the same view session reads (a restart over the same folder)
    let ds = Arc::new(open_with(&d, "", "view_history = true"));
    let r = ds.routes::<()>();
    let [list2, one2, _] = routes(&sid2);
    assert_eq!(
        call(&r, "GET", &list2, Some(&tok2), b"").await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&r, "GET", &one2, Some(&tok2), b"").await.0,
        StatusCode::OK
    );
    assert_eq!(
        opened(r.clone(), config("k1", false)).await["history"],
        json!(true)
    );
    // restore and name still need edit
    assert_eq!(
        call(&r, "POST", &format!("{list2}/1/restore"), Some(&tok2), b"")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let _ = std::fs::remove_dir_all(d);
}

async fn post_doc(r: &Router, sid: &str, tok: &str, route: &str, b: &[u8]) -> (StatusCode, Value) {
    let (s, b) = call(
        r,
        "POST",
        &format!("/ds/session/{sid}/{route}"),
        Some(tok),
        b,
    )
    .await;
    (s, js(&b))
}

/// Gate review L1: a session stores at most `max_versions_per_session` versions. Past it a save and a restore are refused (507,
/// `version_limit`) and nothing changes: the stored versions and the history stay as they were. The close still turns the editor's
/// last copy into a version, so the edits reach the provider (status 2).
#[tokio::test]
async fn a_session_past_its_version_cap_refuses_saves_and_keeps_what_it_has() {
    let (ds, d) = build_with("cap", "max_versions_per_session = 3\nmin_free_mb = 0", 120);
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    for n in 2..=3 {
        let (s, v) = post_doc(&r, &sid, &tok, "save", &docx(&format!("history v{n}"))).await;
        assert_eq!((s, v["version"].as_u64()), (StatusCode::OK, Some(n)), "{v}");
    }
    let (s, v) = post_doc(&r, &sid, &tok, "save", &docx("history v4")).await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::INSUFFICIENT_STORAGE, Some("version_limit")),
        "{v}"
    );
    let (s, v) = post_doc(&r, &sid, &tok, "history/1/restore", b"").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::INSUFFICIENT_STORAGE, Some("version_limit")),
        "{v}"
    );
    let rec = ds.load_session(&sid).unwrap();
    assert_eq!(rec.versions.len(), 3);
    assert_eq!(ds.read_version(&sid, 3).unwrap(), docx("history v3"));
    let h = format!("/ds/session/{sid}/history");
    assert_eq!(
        js(&call(&r, "GET", &h, Some(&tok), b"").await.1)["versions"]
            .as_array()
            .unwrap()
            .len(),
        3,
        "the refused restore added no history version"
    );
    // the recovery copy is still stored and the close makes it the last version
    let (s, _) = post_doc(&r, &sid, &tok, "autosave", &docx("history v4")).await;
    assert_eq!(s, StatusCode::OK);
    let (s, v) = post_doc(&r, &sid, &tok, "close", b"").await;
    assert_eq!((s, v["status"].as_u64()), (StatusCode::OK, Some(2)), "{v}");
    assert_eq!(ds.read_version(&sid, 4).unwrap(), docx("history v4"));
    let _ = std::fs::remove_dir_all(d);
}

/// Gate review L1: under the free-space floor of `data_dir` (`min_free_mb`, here more than any disk has) a save, an autosave, a
/// restore and the first fetch are refused (507, `disk_low`); the versions already stored stay readable and unchanged.
#[tokio::test]
async fn under_the_disk_floor_saves_are_refused_and_stored_versions_stay() {
    let (ds, d) = build("floor");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (s, _) = post_doc(&r, &sid, &tok, "save", &docx("history v2")).await;
    assert_eq!(s, StatusCode::OK);
    // a restart with a floor no disk meets
    let ds = Arc::new(open_at(&d, "min_free_mb = 1048576"));
    let r = ds.routes::<()>();
    for (route, body) in [
        ("save", docx("history v3")),
        ("autosave", docx("history v3")),
        ("history/1/restore", vec![]),
    ] {
        let (s, v) = post_doc(&r, &sid, &tok, route, &body).await;
        assert_eq!(
            (s, v["error"]["code"].as_str()),
            (StatusCode::INSUFFICIENT_STORAGE, Some("disk_low")),
            "{route}: {v}"
        );
    }
    let rec = ds.load_session(&sid).unwrap();
    assert!(rec.versions.len() == 2 && rec.autosave.is_none());
    assert_eq!(ds.read_version(&sid, 2).unwrap(), docx("history v2"));
    assert_eq!(
        call(
            &r,
            "GET",
            &format!("/ds/session/{sid}/history/2"),
            Some(&tok),
            b""
        )
        .await
        .1,
        docx("history v2")
    );
    // a new session's first version (the fetch stores it with put_version) is refused the same way
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &config("k2", true))}});
    let sid2 = js(
        &call(&r, "POST", "/ds/session", None, body.to_string().as_bytes())
            .await
            .1,
    )["sid"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(matches!(
        ds.put_version(&sid2, &docx("x"), "provider", false, now()),
        Err(docserver::sessions::SessErr::NoRoom("disk_low", _))
    ));
    let _ = std::fs::remove_dir_all(d);
}
