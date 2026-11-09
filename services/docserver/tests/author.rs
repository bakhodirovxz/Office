//! The editing person becomes the document's creator when the host's blank-file maker left a generator name (PLAN phase 5, author
//! stamping): `dc:creator` at open, plus `cp:lastModifiedBy` on a save. Synthetic packages only.
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::SocketAddr;
use tower::ServiceExt;

const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";

fn core(creator: Option<&str>, modified_by: Option<&str>) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\"?><cp:coreProperties xmlns:cp=\"x\" xmlns:dc=\"y\"><dc:title>T</dc:title>",
    );
    if let Some(c) = creator {
        s += &format!("<dc:creator>{c}</dc:creator>");
    }
    if let Some(m) = modified_by {
        s += &format!("<cp:lastModifiedBy>{m}</cp:lastModifiedBy>");
    }
    s + "</cp:coreProperties>"
}

/// A package of `ft` with the given core.xml; the other parts are stored (not deflated) so a raw copy is visible as identical bytes.
fn pkg(ft: &str, core: &str) -> Vec<u8> {
    let main = match ft {
        "docx" => "word/document.xml",
        "xlsx" => "xl/workbook.xml",
        _ => "ppt/presentation.xml",
    };
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    let o =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (n, b) in [
        ("[Content_Types].xml", "ct"),
        (main, "<main>body</main>"),
        ("docProps/core.xml", core),
        ("media/a.bin", "binary-ish"),
    ] {
        w.start_file(n, o).unwrap();
        w.write_all(b.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn entry(b: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

async fn open(ft: &str, edit: bool, user: &str) -> (Docserver, String, std::path::PathBuf) {
    let d = std::env::temp_dir().join(format!(
        "docserver-author-{ft}-{}-{}",
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("server.key"), "s".repeat(40)).unwrap();
    std::fs::write(d.join("prov.key"), PROV_SECRET).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace(char::from(92), "/");
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19997"
internal_allow = ["127.0.0.0/8"]
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
callback_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
require_exp = true
"#,
        p("server.key"),
        p("data"),
        p("prov.key")
    )
    .parse()
    .unwrap();
    let ds = std::sync::Arc::new(Docserver::from_table(t, None).unwrap());
    let cfg = json!({"documentType": match ft { "docx" => "word", "xlsx" => "cell", _ => "slide" },
        "document": {"fileType": ft, "key": "k", "title": "a", "url": "http://api:8000/f/1",
            "permissions": {"edit": edit}},
        "editorConfig": {"mode": if edit {"edit"} else {"view"}, "callbackUrl": "http://api:8000/cb",
            "user": {"id": "emp-1", "name": user}},
        "exp": now() + 3600});
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &cfg)}});
    let mut req = Request::builder()
        .method("POST")
        .uri("/ds/session")
        .body(Body::from(body.to_string()))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = ds.routes::<()>().oneshot(req).await.unwrap();
    assert_eq!(res.status(), 200);
    let b = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&b).unwrap();
    let sid = v["sid"].as_str().unwrap().to_string();
    let ds = std::sync::Arc::try_unwrap(ds).ok().unwrap();
    (ds, sid, d)
}

#[tokio::test]
async fn generator_creator_becomes_the_user_on_open_and_save_for_each_type() {
    for (ft, gen_) in [
        ("docx", "python-docx"),
        ("xlsx", "OpenPyXL "),
        ("pptx", "python-pptx"),
    ] {
        let (ds, sid, d) = open(ft, true, "Aziz <Karimov> & Co").await;
        let blank = pkg(ft, &core(Some(gen_), Some("python-docx")));
        ds.put_version(&sid, &blank, "one", false, now()).unwrap();
        let v1 = ds.read_version(&sid, 1).unwrap();
        let c = entry(&v1, "docProps/core.xml");
        assert!(
            c.contains("<dc:creator>Aziz &lt;Karimov&gt; &amp; Co</dc:creator>"),
            "{c}"
        );
        // opening changes the creator only
        assert!(
            c.contains("<cp:lastModifiedBy>python-docx</cp:lastModifiedBy>"),
            "{c}"
        );
        for n in ["[Content_Types].xml", "media/a.bin"] {
            assert_eq!(entry(&v1, n), entry(&blank, n));
        }
        // a save sets lastModifiedBy and leaves the creator and the other parts
        ds.put_version(&sid, &blank, "emp-1", true, now()).unwrap();
        let c = entry(&ds.read_version(&sid, 2).unwrap(), "docProps/core.xml");
        assert!(
            c.contains("<cp:lastModifiedBy>Aziz &lt;Karimov&gt; &amp; Co</cp:lastModifiedBy>"),
            "{c}"
        );
        assert!(
            c.contains("<dc:creator>Aziz &lt;Karimov&gt; &amp; Co</dc:creator>")
                && c.contains("<dc:title>T</dc:title>"),
            "{c}"
        );
        let _ = std::fs::remove_dir_all(d);
    }
}

#[tokio::test]
async fn a_real_creator_is_kept_and_a_missing_one_is_added() {
    let (ds, sid, d) = open("docx", true, "Aziz").await;
    ds.put_version(
        &sid,
        &pkg("docx", &core(Some("Jane Roe"), None)),
        "one",
        true,
        now(),
    )
    .unwrap();
    let c = entry(&ds.read_version(&sid, 1).unwrap(), "docProps/core.xml");
    assert!(
        c.contains("<dc:creator>Jane Roe</dc:creator>")
            && c.contains("<cp:lastModifiedBy>Aziz</cp:lastModifiedBy>"),
        "{c}"
    );
    ds.put_version(&sid, &pkg("docx", &core(None, None)), "one", false, now())
        .unwrap();
    let c = entry(&ds.read_version(&sid, 2).unwrap(), "docProps/core.xml");
    assert!(
        c.contains("<dc:creator>Aziz</dc:creator>") && !c.contains("lastModifiedBy"),
        "{c}"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn a_viewer_changes_nothing_and_the_autosave_of_a_saved_document_is_not_a_new_version() {
    let (ds, sid, d) = open("docx", false, "Aziz").await;
    let blank = pkg("docx", &core(Some("python-docx"), None));
    ds.put_version(&sid, &blank, "one", false, now()).unwrap();
    assert_eq!(ds.read_version(&sid, 1).unwrap(), blank);
    let _ = std::fs::remove_dir_all(d);

    let (ds, sid, d) = open("docx", true, "Aziz").await;
    ds.put_version(&sid, &blank, "one", false, now()).unwrap();
    ds.put_version(&sid, &blank, "emp-1", true, now()).unwrap();
    // the recovery copy is the very bytes just saved: closing must not promote a copy of version 2
    ds.put_autosave(&sid, &blank, now()).unwrap();
    ds.close_session(&sid, now()).unwrap();
    assert_eq!(ds.load_session(&sid).unwrap().versions.len(), 2);
    let _ = std::fs::remove_dir_all(d);
}
