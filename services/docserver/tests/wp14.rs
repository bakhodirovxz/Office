//! WP14 (PLAN phase 5, section 4): XDFU stamping of the copies that leave the server, and encryption at rest.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower::ServiceExt;

const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";
const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
const ENC_KEY: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const AUDIT_KEY: &str = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk";
const MARK: &str = "PLAINTEXT-MARKER-4711";

/// A docx whose first paragraph is `first`; parts are stored, so a plaintext leak shows as bytes on disk.
fn docx(first: &str) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    let o =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (n, b) in [
        ("[Content_Types].xml", "x".to_string()),
        (
            "word/document.xml",
            format!(
                "<w:document><w:body><w:p><w:r><w:t>{first}</w:t></w:r></w:p><w:p><w:r><w:t>{MARK}</w:t></w:r></w:p></w:body></w:document>"
            ),
        ),
    ] {
        w.start_file(n, o).unwrap();
        w.write_all(b.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn part(zip: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("docserver-wp14-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    for (f, v) in [
        ("server.key", SERVER_SECRET),
        ("prov.key", PROV_SECRET),
        ("enc.key", ENC_KEY),
        ("audit.key", AUDIT_KEY),
        ("other.key", "oooooooooooooooooooooooooooooooooooooooo"),
    ] {
        std::fs::write(d.join(f), v).unwrap();
    }
    d
}

fn p(d: &Path, f: &str) -> String {
    d.join(f).to_string_lossy().replace('\\', "/")
}

/// `extra` is more top-level TOML (it goes before the first table).
fn build(d: &Path, extra: &str) -> Result<Arc<Docserver>, String> {
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19997"
internal_allow = ["127.0.0.0/8"]
{extra}
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
callback_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
require_exp = true
"#,
        p(d, "server.key"),
        p(d, "data"),
        p(d, "prov.key")
    )
    .parse()
    .unwrap();
    Docserver::from_table(t, None).map(Arc::new)
}

fn on(d: &Path, xdfu: bool) -> Arc<Docserver> {
    let mut extra = format!(
        "encrypt_at_rest = true\nencrypt_key_file = \"{}\"\naudit_key_file = \"{}\"\n",
        p(d, "enc.key"),
        p(d, "audit.key")
    );
    if xdfu {
        extra += "xdfu = \"watermark\"\n";
    }
    build(d, &extra).unwrap()
}

async fn call(r: &Router, uri: &str, bearer: Option<&str>) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::empty()).unwrap();
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

/// A session for `key` with `bytes` stored as version 1; returns (sid, session token).
async fn session(ds: &Arc<Docserver>, r: &Router, key: &str, bytes: &[u8]) -> (String, String) {
    let cfg = json!({"documentType": "word",
        "document": {"fileType": "docx", "key": key, "title": "a.docx", "url": "http://api:8000/f/1",
            "permissions": {"edit": true, "download": true}},
        "editorConfig": {"mode": "edit", "callbackUrl": "http://api:8000/cb", "user": {"id": "emp-1", "name": "Ali Valiyev"}},
        "exp": now() + 3600});
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &cfg)}});
    let mut req = Request::builder()
        .method("POST")
        .uri("/ds/session")
        .body(Body::from(body.to_string()))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = r.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let sid = v["sid"].as_str().unwrap().to_string();
    ds.put_version(&sid, bytes, "provider", false, now())
        .unwrap();
    (sid, v["token"].as_str().unwrap().to_string())
}

/// The saved-file URL a callback hands the provider, as a path.
fn file_url(ds: &Docserver, sid: &str, ver: u64) -> String {
    let rec = ds.load_session(sid).unwrap();
    let v = rec.versions.iter().find(|v| v.n == ver).unwrap();
    format!(
        "/ds/files/{sid}/{ver}?t={}",
        ds.mint_file_token(sid, ver, &v.sha256, now())
    )
}

fn files_under(p: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
        let q = e.path();
        if q.is_dir() {
            files_under(&q, out);
        } else {
            out.push(q);
        }
    }
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[tokio::test]
async fn xdfu_off_registers_nothing() {
    let d = dir("off");
    let ds = build(&d, "").unwrap();
    assert!(ds.xdfu.is_none() && ds.at_rest.is_none());
    let (r, ri) = (ds.routes::<()>(), ds.internal_router());
    let marked = docx("ДСП");
    let (sid, tok) = session(&ds, &r, "k1", &marked).await;
    // a download copy and a callback copy of a document that carries the grif: untouched
    let (s, b) = call(&r, &format!("/ds/session/{sid}/history/1"), Some(&tok)).await;
    assert_eq!((s, b), (StatusCode::OK, marked.clone()));
    let (s, b) = call(&ri, &file_url(&ds, &sid, 1), None).await;
    assert_eq!((s, b), (StatusCode::OK, marked));
    // nothing was registered: no copy log, no encryption marker
    assert!(!d.join("data/xdfu-audit.jsonl").exists());
    assert!(!d.join("data/at-rest.check").exists());
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn download_and_callback_copies_of_a_grif_document_carry_the_stamp() {
    let d = dir("stamp");
    let ds = on(&d, true);
    let (r, ri) = (ds.routes::<()>(), ds.internal_router());
    let (sid, tok) = session(&ds, &r, "k1", &docx("Х.Д.Ф.У. buyruq")).await;
    let (sid2, tok2) = session(&ds, &r, "k2", &docx("ordinary text")).await;

    let (s, down) = call(&r, &format!("/ds/session/{sid}/history/1"), Some(&tok)).await;
    assert_eq!(s, StatusCode::OK);
    let xml = part(&down, "word/document.xml");
    // copy number 1, the user, the time; the document's own text stays
    assert!(
        xml.contains("XDFU copy 1 | ") && xml.contains("| Ali Valiyev | ") && xml.contains(" UTC"),
        "{xml}"
    );
    assert!(xml.contains("Х.Д.Ф.У. buyruq") && xml.contains(MARK));
    let (s, cb) = call(&ri, &file_url(&ds, &sid, 1), None).await;
    assert_eq!(s, StatusCode::OK);
    // the callback copy is the next copy, with the peer's address
    let xml = part(&cb, "word/document.xml");
    assert!(
        xml.contains("XDFU copy 2 | 127.0.0.1 | Ali Valiyev | "),
        "{xml}"
    );

    // a document without the grif is served as it is and nothing is logged
    let plain = docx("ordinary text");
    let (s, b) = call(&r, &format!("/ds/session/{sid2}/history/1"), Some(&tok2)).await;
    assert_eq!((s, b), (StatusCode::OK, plain.clone()));
    let (s, b) = call(&ri, &file_url(&ds, &sid2, 1), None).await;
    assert_eq!((s, b), (StatusCode::OK, plain));

    // the copy log is the keyed chain: two lines, and it does not verify without the key
    let log = d.join("data/xdfu-audit.jsonl");
    let key = convert::audit::load_key(&d.join("audit.key")).unwrap();
    assert_eq!(
        convert::audit::verify_keyed(&log, Some(&key))
            .unwrap()
            .lines,
        2
    );
    assert!(convert::audit::verify_keyed(&log, None).is_err());
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("\"kind\":\"download\"") && text.contains("\"kind\":\"callback\""));
    assert!(
        !text.contains(MARK) && !text.contains("Ali Valiyev"),
        "no content, no name"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn a_copy_that_cannot_be_checked_is_refused_when_xdfu_is_on() {
    let d = dir("unchecked");
    let ds = on(&d, true);
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", b"not a package").await;
    let (s, b) = call(&r, &format!("/ds/session/{sid}/history/1"), Some(&tok)).await;
    assert_ne!(s, StatusCode::OK);
    assert!(!contains(&b, "not a package"));
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn encrypted_files_round_trip_hold_no_plaintext_and_survive_a_restart() {
    let d = dir("enc");
    let ds = on(&d, false);
    let r = ds.routes::<()>();
    let bytes = docx("secret decree");
    let (sid, tok) = session(&ds, &r, "k1", &bytes).await;
    let v2 = docx("second version");
    ds.put_version(&sid, &v2, "emp-1", true, now()).unwrap();
    ds.put_autosave(&sid, &docx("autosaved"), now()).unwrap();
    // round trip through the routes (document = newest, history = version 1)
    assert_eq!(
        call(&r, &format!("/ds/session/{sid}/document"), Some(&tok))
            .await
            .1,
        v2
    );
    assert_eq!(
        call(&r, &format!("/ds/session/{sid}/history/1"), Some(&tok))
            .await
            .1,
        bytes
    );

    // no file under data_dir (sessions, versions, autosave, history objects) holds the marker, the title, the URL or the text
    // (the audit logs and the history index hold no document bytes and are not sealed, PLAN 4)
    let mut all = vec![];
    files_under(&d.join("data/sessions"), &mut all);
    files_under(&d.join("data/history/objects"), &mut all);
    assert!(
        all.iter().any(|f| f.ends_with("v1.bin"))
            && all.iter().any(|f| f.ends_with("autosave.bin"))
    );
    assert!(all.iter().any(|f| f.to_string_lossy().contains("objects")));
    for f in &all {
        let b = std::fs::read(f).unwrap();
        for needle in [
            MARK,
            "a.docx",
            "api:8000",
            "secret decree",
            "second version",
        ] {
            assert!(!contains(&b, needle), "{} holds {needle}", f.display());
        }
    }
    // a restart with the same key reads the old session
    let again = on(&d, false);
    assert_eq!(again.read_version(&sid, 1).unwrap(), bytes);
    assert_eq!(again.load_session(&sid).unwrap().title, "a.docx");
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn a_damaged_swapped_or_plaintext_file_is_refused_never_served() {
    let d = dir("tamper");
    let ds = on(&d, false);
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", &docx("one")).await;
    ds.put_version(&sid, &docx("two"), "emp-1", true, now())
        .unwrap();
    let dir = d.join("data/sessions").join(&sid);
    let doc = format!("/ds/session/{sid}/document");
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::OK);
    let (v1, v2) = (dir.join("v1.bin"), dir.join("v2.bin"));
    let good = std::fs::read(&v2).unwrap();
    // one flipped byte
    let mut bad = good.clone();
    let n = bad.len() - 20;
    bad[n] ^= 1;
    std::fs::write(&v2, &bad).unwrap();
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::NOT_FOUND);
    // another version's sealed file moved into its place: the name is authenticated
    std::fs::write(&v2, std::fs::read(&v1).unwrap()).unwrap();
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::NOT_FOUND);
    // a plaintext file in an encrypted store
    std::fs::write(&v2, docx("two")).unwrap();
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::NOT_FOUND);
    // cut short
    std::fs::write(&v2, &good[..10]).unwrap();
    assert!(ds.read_version(&sid, 2).is_none());
    std::fs::write(&v2, &good).unwrap();
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::OK);

    // a history object that was changed is not served
    let hist = format!("/ds/session/{sid}/history/1");
    assert_eq!(call(&r, &hist, Some(&tok)).await.0, StatusCode::OK);
    let mut objs = vec![];
    files_under(&d.join("data/history/objects"), &mut objs);
    assert_eq!(objs.len(), 2);
    for o in &objs {
        let mut b = std::fs::read(o).unwrap();
        let n = b.len() - 1;
        b[n] ^= 1;
        std::fs::write(o, b).unwrap();
    }
    let (s, b) = call(&r, &hist, Some(&tok)).await;
    assert_ne!(s, StatusCode::OK);
    assert!(!contains(&b, MARK));

    // a damaged session record is not served either
    let rec_file = dir.join("session.json");
    let mut rec = std::fs::read(&rec_file).unwrap();
    rec[30] ^= 1;
    std::fs::write(&rec_file, rec).unwrap();
    assert!(ds.load_session(&sid).is_none());
    assert_eq!(call(&r, &doc, Some(&tok)).await.0, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_wrong_key_or_a_mixed_data_folder_stops_the_start() {
    let d = dir("keys");
    drop(on(&d, false));
    // another key
    let e = build(
        &d,
        &format!(
            "encrypt_at_rest = true\nencrypt_key_file = \"{}\"\n",
            p(&d, "other.key")
        ),
    )
    .err()
    .expect("refused");
    assert!(
        e.contains("not the key this data_dir was encrypted with"),
        "{e}"
    );
    assert!(!e.contains("oooooooooooooooooooooooooooooooo"));
    // encryption switched off on encrypted data
    let e = build(&d, "").err().expect("refused");
    assert!(e.contains("holds encrypted data"), "{e}");
    // encryption switched on over plaintext sessions
    let d2 = dir("mixed");
    std::fs::create_dir_all(d2.join("data/sessions/abc")).unwrap();
    std::fs::write(d2.join("data/sessions/abc/session.json"), "{}").unwrap();
    let e = build(
        &d2,
        &format!(
            "encrypt_at_rest = true\nencrypt_key_file = \"{}\"\n",
            p(&d2, "enc.key")
        ),
    )
    .err()
    .expect("refused");
    assert!(e.contains("unencrypted"), "{e}");
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}
