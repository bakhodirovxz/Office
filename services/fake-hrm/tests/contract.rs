//! Every rule of the harness has an accept and a reject case here. Spike S6 (happy path) and S7 (negative) are reproduced at the
//! end against the same rules. Citations: `[S:n]` = docs/phase0/hrm-current-state.md, `[P:n]` = docs/phase0/hr-backend-prompt.md.
use axum::{Router, extract::Query, response::Redirect, routing::get};
use fake_hrm::{Config, FakeHrm, Kind, Via, jwt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;

const SECRET_FLIP: &str = "x";

// ---- helpers ---------------------------------------------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .proxy(None)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

async fn get_raw(url: String) -> (u16, Vec<u8>) {
    tokio::task::spawn_blocking(move || {
        let mut r = agent().get(&url).call().expect("get");
        (r.status().as_u16(), r.body_mut().read_to_vec().unwrap())
    })
    .await
    .unwrap()
}

async fn get_json(url: String) -> (u16, Value) {
    let (s, b) = get_raw(url).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn post_json(url: String, bearer: Option<String>, body: Value) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let mut req = agent()
            .post(&url)
            .header("Content-Type", "application/json");
        if let Some(b) = bearer {
            req = req.header("Authorization", format!("Bearer {b}"));
        }
        let mut r = req.send(body.to_string().as_str()).expect("post");
        (
            r.status().as_u16(),
            serde_json::from_slice(&r.body_mut().read_to_vec().unwrap()).unwrap_or(Value::Null),
        )
    })
    .await
    .unwrap()
}

const DOC_V1: &[u8] = b"synthetic document v1";
const DOC_V2: &[u8] = b"synthetic document v2, saved by the editor";

/// A stand-in for the document server's file route: `/ds/files/s1/1?t=tok` answers DOC_V2, anything else 404.
async fn stub() -> String {
    async fn file(Query(q): Query<HashMap<String, String>>) -> (axum::http::StatusCode, Vec<u8>) {
        if q.get("t").map(String::as_str) == Some("tok") {
            (axum::http::StatusCode::OK, DOC_V2.to_vec())
        } else {
            (axum::http::StatusCode::NOT_FOUND, vec![])
        }
    }
    let app = Router::new()
        .route("/ds/files/s1/1", get(file))
        .route(
            "/redir",
            get(|| async { Redirect::temporary("/ds/files/s1/1?t=tok") }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "late"
            }),
        );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(l, app).await;
    });
    origin
}

/// Harness with file 7 (`a.docx`); callbacks name `docserver.test:8780`, which the rewrite sends to the stub.
async fn setup(tweak: impl FnOnce(&mut Config)) -> (FakeHrm, String) {
    let origin = stub().await;
    let mut cfg = Config::generated(&origin);
    cfg.callback_allowed_hosts = vec!["docserver.test".into()];
    tweak(&mut cfg);
    let h = FakeHrm::start(cfg).await.unwrap();
    h.add_file(Kind::File, 7, "a.docx", DOC_V1.to_vec());
    (h, origin)
}

const GOOD_URL: &str = "http://docserver.test:8780/ds/files/s1/1?t=tok";

fn body(key: &str, status: u64, url: Option<&str>) -> Value {
    let mut b = json!({"status": status, "key": key, "users": ["emp-1"], "actions": []});
    if let Some(u) = url {
        b["url"] = json!(u);
    }
    b
}

/// the document server's way [S:327-329]: `body.token` = JWT(body), key at the top level
fn with_token(secret: &str, mut b: Value) -> Value {
    let t = jwt::sign(secret.as_bytes(), &b);
    b["token"] = json!(t);
    b
}

fn cb_url(h: &FakeHrm, seg: &str, id: u64) -> String {
    format!("{}/{seg}/{id}/save-callback", h.base_url())
}

async fn key_for(h: &FakeHrm, id: u64) -> String {
    let (_, c) = get_json(format!("{}/files/{id}/editor-config", h.base_url())).await;
    c["document"]["key"].as_str().unwrap().to_string()
}

fn flip(t: &str) -> String {
    let i = t.len() - 5;
    let c = if &t[i..=i] == "A" { "B" } else { "A" };
    format!("{}{c}{}", &t[..i], &t[i + 1..])
}

fn assert_untouched(h: &FakeHrm) {
    assert!(h.received().is_empty(), "{:?}", h.received());
    assert_eq!(h.object(Kind::File, 7).unwrap(), DOC_V1);
}

// ---- editor config [S:232-269] ----------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn config_shape_without_exp() {
    let (h, _) = setup(|_| {}).await;
    let (s, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    assert_eq!(s, 200);
    let cfg = h.config();
    assert_eq!(c["documentType"], "word");
    assert_eq!(c["document"]["fileType"], "docx");
    assert_eq!(c["document"]["title"], "a.docx");
    let key = c["document"]["key"].as_str().unwrap();
    let (pre, suf) = key.rsplit_once('_').unwrap();
    assert!(pre.starts_with("file_7_v"), "{key}");
    assert!(
        suf.len() == 8 && suf.chars().all(|c| c.is_ascii_hexdigit()),
        "{key}"
    );
    assert_ne!(
        key,
        key_for(&h, 7).await,
        "a fresh suffix on every open [S:254]"
    );
    assert!(
        c["document"]["url"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{}/docs-file/", h.base_url()))
    );
    assert_eq!(
        c["document"]["permissions"],
        json!({"edit": true, "download": true, "print": true, "review": false, "comment": true})
    );
    let e = &c["editorConfig"];
    assert_eq!(e["mode"], "edit");
    assert_eq!(
        e["callbackUrl"],
        format!("{}/files/7/save-callback", h.base_url())
    );
    assert_eq!(e["lang"], "ru");
    assert_eq!(e["user"]["id"], "emp-1");
    assert_eq!(
        e["customization"],
        json!({"forcesave": true, "uiTheme": "theme-classic-light"})
    );
    assert!(c.get("type").is_none());
    // the token covers the whole config, signed before `token` was added; no `exp` as surveyed [S:265, P:829]
    let mut unsigned = c.clone();
    let token = unsigned.as_object_mut().unwrap().remove("token").unwrap();
    assert_eq!(
        jwt::verify(cfg.ds_secret.as_bytes(), token.as_str().unwrap()).unwrap(),
        unsigned
    );
    assert!(
        jwt::verify(cfg.jwt_secret.as_bytes(), token.as_str().unwrap()).is_err(),
        "signed with the config secret only"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn config_with_exp() {
    let (h, _) = setup(|c| c.include_exp = true).await; // PR-3 optional commit [P:513-514]
    let (_, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    assert!(c.get("exp").is_none(), "exp lives inside the token only");
    let p = jwt::verify(
        h.config().ds_secret.as_bytes(),
        c["token"].as_str().unwrap(),
    )
    .unwrap();
    let exp = p["exp"].as_u64().unwrap();
    assert!((jwt::now() + 12 * 3600 - 5..=jwt::now() + 12 * 3600 + 5).contains(&exp));
    assert_eq!(p["document"], c["document"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn config_view_mobile_and_errors() {
    let (h, _) = setup(|_| {}).await;
    let url = |q: &str| format!("{}/files/7/editor-config?{q}", h.base_url());
    // mode=view: no callbackUrl, forcesave false [S:259, S:262]
    let (_, c) = get_json(url("mode=view&theme=dark")).await;
    assert_eq!(c["editorConfig"]["mode"], "view");
    assert!(c["editorConfig"].get("callbackUrl").is_none());
    assert_eq!(
        c["editorConfig"]["customization"],
        json!({"forcesave": false, "uiTheme": "theme-dark"})
    );
    assert_eq!(c["document"]["permissions"]["edit"], false);
    assert_eq!(c["document"]["permissions"]["comment"], false);
    // locked and no edit right -> view [S:258]
    for (can_edit, locked) in [(true, true), (false, false)] {
        h.set_access(Kind::File, 7, can_edit, locked);
        let (_, c) = get_json(url("")).await;
        assert_eq!(c["editorConfig"]["mode"], "view", "{can_edit} {locked}");
    }
    h.set_access(Kind::File, 7, true, false);
    // mobile is forced to view unless BACKEND_MOBILE_EDIT [S:264]
    let (_, c) = get_json(url("device=mobile")).await;
    assert_eq!(
        (c["type"].as_str(), c["editorConfig"]["mode"].as_str()),
        (Some("mobile"), Some("view"))
    );
    h.update_config(|c| c.mobile_edit = true);
    let (_, c) = get_json(url("device=mobile")).await;
    assert_eq!(
        (c["type"].as_str(), c["editorConfig"]["mode"].as_str()),
        (Some("mobile"), Some("edit"))
    );
    // another type, unknown id [S:252]
    h.add_file(Kind::File, 8, "x.txt", vec![1]);
    let (s, c) = get_json(format!("{}/files/8/editor-config", h.base_url())).await;
    assert_eq!(
        (s, c["detail"].as_str()),
        (422, Some("unsupported_document_type"))
    );
    assert_eq!(
        get_raw(format!("{}/files/99/editor-config", h.base_url()))
            .await
            .0,
        404
    );
    // other kinds use their own key prefix [S:254]
    h.add_file(Kind::OrderAct, 3, "Order_Act_1.docx", vec![1]);
    let (_, c) = get_json(format!("{}/order-acts/3/editor-config", h.base_url())).await;
    assert!(
        c["document"]["key"]
            .as_str()
            .unwrap()
            .starts_with("order_act_3_v")
    );
    assert!(
        c["editorConfig"]["callbackUrl"]
            .as_str()
            .unwrap()
            .ends_with("/order-acts/3/save-callback")
    );
}

// ---- file route [S:256] -----------------------------------------------------------------------------------------------------

fn file_url(h: &FakeHrm, secret: &str, claims: Value) -> String {
    format!(
        "{}/docs-file/{}",
        h.base_url(),
        jwt::sign(secret.as_bytes(), &claims)
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn file_route_purpose_bound() {
    let (h, _) = setup(|_| {}).await;
    let cfg = h.config();
    let ok = json!({"purpose": "/docs-file/", "bucket": "docs-test", "object": "files/7/a.docx", "exp": jwt::now() + 60});
    // accept: the token from the editor config
    let (_, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    assert_eq!(
        get_raw(c["document"]["url"].as_str().unwrap().to_string()).await,
        (200, DOC_V1.to_vec())
    );
    assert_eq!(
        get_raw(file_url(&h, &cfg.jwt_secret, ok.clone())).await.0,
        200
    );
    // reject
    let with = |k: &str, v: Value| {
        let mut o = ok.clone();
        o[k] = v;
        o
    };
    assert_eq!(
        get_raw(file_url(
            &h,
            &cfg.jwt_secret,
            with("purpose", json!("session"))
        ))
        .await
        .0,
        403
    );
    assert_eq!(
        get_raw(file_url(&h, &cfg.jwt_secret, with("exp", json!(1))))
            .await
            .0,
        401,
        "expired"
    );
    let mut no_exp = ok.clone();
    no_exp.as_object_mut().unwrap().remove("exp");
    assert_eq!(get_raw(file_url(&h, &cfg.jwt_secret, no_exp)).await.0, 403);
    assert_eq!(
        get_raw(file_url(&h, &cfg.ds_secret, ok.clone())).await.0,
        401,
        "the config secret is not the file secret"
    );
    assert_eq!(
        get_raw(file_url(
            &h,
            &cfg.jwt_secret,
            with("object", json!("files/7/zzz"))
        ))
        .await
        .0,
        404
    );
    assert_eq!(
        get_raw(file_url(
            &h,
            &cfg.jwt_secret,
            with("bucket", json!("other"))
        ))
        .await
        .0,
        404
    );
    assert_eq!(
        get_raw(format!(
            "{}/docs-file/{}",
            h.base_url(),
            flip(&jwt::sign(cfg.jwt_secret.as_bytes(), &ok))
        ))
        .await
        .0,
        401
    );
    // a config token must not open the file route even when both secrets are equal (fallback [S:265]): the purpose binds
    h.update_config(|c| c.jwt_secret = c.ds_secret.clone());
    let (_, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    let cfg_token = c["token"].as_str().unwrap();
    assert_eq!(
        get_raw(format!("{}/docs-file/{cfg_token}", h.base_url()))
            .await
            .0,
        403
    );
}

// ---- save callback: accept [S:271-300] -------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn callback_accepts_status_2_and_6_with_host_rewrite() {
    let (h, _) = setup(|_| {}).await;
    let secret = h.config().ds_secret;
    for status in [6, 2] {
        let key = key_for(&h, 7).await;
        let b = with_token(&secret, body(&key, status, Some(GOOD_URL)));
        let (s, r) = post_json(cb_url(&h, "files", 7), None, b).await;
        assert_eq!((s, r), (200, json!({"error": 0})));
        // the stub only answers when the query `t=tok` arrived: path and query kept, host rewritten [S:289-290]
        assert_eq!(h.object(Kind::File, 7).unwrap(), DOC_V2);
        let got = h.received().pop().unwrap();
        assert_eq!(
            (got.status, got.key.as_str(), got.via, got.users),
            (status, key.as_str(), Via::Body, vec!["emp-1".to_string()])
        );
        h.add_file(Kind::File, 7, "a.docx", DOC_V1.to_vec());
    }
    assert_eq!(h.received().len(), 2);
    // the original also finds the key in `payload.key` [S:283]
    let key = key_for(&h, 7).await;
    let mut b = body(&key, 6, Some(GOOD_URL));
    b["token"] = json!(jwt::sign(
        secret.as_bytes(),
        &json!({"payload": {"key": key}})
    ));
    assert_eq!(
        post_json(cb_url(&h, "files", 7), None, b).await.1,
        json!({"error": 0})
    );
    assert_eq!(h.received().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_other_statuses_are_ignored() {
    let (h, _) = setup(|_| {}).await;
    let secret = h.config().ds_secret;
    let key = key_for(&h, 7).await;
    for status in [1, 3, 4, 7, 0, 99] {
        let b = with_token(&secret, body(&key, status, Some(GOOD_URL))); // [S:284-285]
        assert_eq!(
            post_json(cb_url(&h, "files", 7), None, b).await,
            (200, json!({"error": 0})),
            "status {status}"
        );
    }
    assert_untouched(&h);
    assert!(h.rejected().is_empty());
}

// ---- save callback: token rules [S:278-283] ---------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn callback_token_rules() {
    let (h, _) = setup(|_| {}).await;
    let secret = h.config().ds_secret;
    let key = key_for(&h, 7).await;
    let ok_body = body(&key, 6, Some(GOOD_URL));
    let post = |b: Value| post_json(cb_url(&h, "files", 7), None, b);
    // no token; header-only (not read as surveyed [S:279]); wrong secret; tampered; expired
    assert_eq!(post(ok_body.clone()).await, (401, json!({"error": 1})));
    let hdr = jwt::sign(secret.as_bytes(), &json!({"payload": ok_body}));
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(hdr), ok_body.clone())
            .await
            .0,
        401
    );
    assert_eq!(
        post(with_token("another-secret", ok_body.clone())).await.0,
        401
    );
    let mut t = with_token(&secret, ok_body.clone());
    t["token"] = json!(flip(t["token"].as_str().unwrap()));
    assert_eq!(post(t).await.0, 401);
    let mut exp = ok_body.clone();
    exp["exp"] = json!(1);
    assert_eq!(post(with_token(&secret, exp)).await.0, 401);
    // alg none
    let none = format!("{}.{}.", "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0", "e30");
    let mut b = ok_body.clone();
    b["token"] = json!(none);
    assert_eq!(post(b).await.0, 401);
    // replayed editor tokens: `type`, `editorConfig`, `documentType` [S:281-282]; OPEN-2: any `type`
    for (claim, v) in [
        ("type", json!("access")),
        ("type", json!("refresh")),
        ("type", json!("mobile")),
        ("editorConfig", json!({})),
        ("documentType", json!("word")),
    ] {
        let mut b = ok_body.clone();
        b[claim] = v;
        assert_eq!(post(with_token(&secret, b)).await.0, 403, "{claim}");
    }
    // key prefix [S:283]: another id, another kind, a prefix of the id, none
    for bad in [
        "file_70_v1_abcd1234",
        "file_77_v1_x",
        "order_act_7_v1_x",
        "file_7",
        "x",
    ] {
        assert_eq!(
            post(with_token(&secret, body(bad, 6, Some(GOOD_URL))))
                .await
                .0,
            403,
            "{bad}"
        );
    }
    assert_eq!(
        post(with_token(&secret, json!({"status": 6, "url": GOOD_URL})))
            .await
            .0,
        403,
        "no key at all"
    );
    // OPEN-3: the body key must equal the signed key
    let mut b = ok_body.clone();
    let signed = with_token(&secret, ok_body.clone())["token"].clone();
    b["key"] = json!("file_7_v1_other");
    b["token"] = signed;
    assert_eq!(post(b).await.0, 403);
    // the route kind decides the prefix: a files key on the letters route
    h.add_file(Kind::Letter, 7, "l.docx", vec![1]);
    assert_eq!(
        post_json(
            cb_url(&h, "letters", 7),
            None,
            with_token(&secret, ok_body.clone())
        )
        .await
        .0,
        403
    );
    // unknown segment and a non-numeric id are not routes [S:272-275]
    assert_eq!(
        post_json(cb_url(&h, "reports", 7), None, with_token(&secret, ok_body))
            .await
            .0,
        404
    );
    assert_untouched(&h);
    assert!(h.rejected().len() >= 15);
}

#[tokio::test(flavor = "multi_thread")]
async fn bearer_fallback_is_switchable() {
    let (h, _) = setup(|_| {}).await;
    let secret = h.config().ds_secret;
    let key = key_for(&h, 7).await;
    let b = body(&key, 6, Some(GOOD_URL));
    let hdr = jwt::sign(secret.as_bytes(), &json!({"payload": b})); // [P:401]
    // off: the header is not read [S:279]
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(hdr.clone()), b.clone())
            .await
            .0,
        401
    );
    h.update_config(|c| c.bearer_fallback = true);
    // on, header only: accepted
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(hdr.clone()), b.clone()).await,
        (200, json!({"error": 0}))
    );
    assert_eq!(h.received().pop().unwrap().via, Via::Bearer);
    // on, both present and agreeing (what the docserver sends [S:327-329]): accepted
    let both = with_token(&secret, b.clone());
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(hdr.clone()), both)
            .await
            .0,
        200
    );
    assert_eq!(h.received().pop().unwrap().via, Via::Body);
    // reject: bad signature, replayed editor claim inside `payload`, keys that disagree, wrong prefix
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(flip(&hdr)), b.clone())
            .await
            .0,
        401
    );
    let bad = jwt::sign(
        secret.as_bytes(),
        &json!({"payload": {"key": key, "editorConfig": {}}}),
    );
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(bad), b.clone())
            .await
            .0,
        403
    );
    let other = jwt::sign(
        secret.as_bytes(),
        &json!({"payload": {"key": "file_7_v1_zzz"}}),
    );
    assert_eq!(
        post_json(
            cb_url(&h, "files", 7),
            Some(other),
            with_token(&secret, b.clone())
        )
        .await
        .0,
        403,
        "keys disagree [P:401-402]"
    );
    let wrong = jwt::sign(
        secret.as_bytes(),
        &json!({"payload": {"key": "file_8_v1_zzz"}}),
    );
    let mut nb = b.clone();
    nb["key"] = json!("file_8_v1_zzz");
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(wrong), nb).await.0,
        403
    );
    let none = jwt::sign(secret.as_bytes(), &json!({"x": 1})); // header JWT without `payload`
    assert_eq!(
        post_json(cb_url(&h, "files", 7), Some(none), b).await.0,
        403
    );
    assert_eq!(h.received().len(), 2);
}

// ---- save callback: url and download [S:286-291, S:298] --------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn callback_url_allowlist() {
    let (h, _) = setup(|c| {
        c.callback_allowed_hosts = vec!["docserver.test".into(), "pinned.test:9000".into()]
    })
    .await;
    let secret = h.config().ds_secret;
    let key = key_for(&h, 7).await;
    let send = |u: &str| {
        post_json(
            cb_url(&h, "files", 7),
            None,
            with_token(&secret, body(&key, 6, Some(u))),
        )
    };
    // refused (403, {"error":1}, nothing fetched or stored) [S:287]
    for bad in [
        "http://169.254.169.254/latest", // the spike's S7 (ii) address
        "http://other.test/ds/files/s1/1?t=tok",
        "ftp://docserver.test/x",
        "http://user:pw@docserver.test/ds/files/s1/1?t=tok",
        "http://docserver.test.evil.test/x",
        "http://pinned.test:9001/x", // host:port is exact
        "http://pinned.test/x",      // default port 80 is not 9000
        "docserver.test/x",
        "http://[::1]/x",
        "",
    ] {
        assert_eq!(send(bad).await, (403, json!({"error": 1})), "{bad}");
    }
    assert_untouched(&h);
    // accepted forms: any port for a bare entry; upper-case host; https default port is checked; query-only rest
    for ok in [
        "http://DOCSERVER.test:1234/ds/files/s1/1?t=tok",
        "http://docserver.test/ds/files/s1/1?t=tok#frag",
    ] {
        // these resolve to the stub because the host is rewritten: only the allow rule is under test
        let (s, _) = send(ok).await;
        assert_eq!(s, 200, "{ok}");
    }
    assert_eq!(h.received().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_download_failures_answer_error_1() {
    let (h, _) = setup(|c| c.download_timeout = Duration::from_millis(400)).await;
    let secret = h.config().ds_secret;
    let key = key_for(&h, 7).await;
    let send = |b: Value| post_json(cb_url(&h, "files", 7), None, with_token(&secret, b));
    // missing url [S:298]
    assert_eq!(send(body(&key, 6, None)).await, (200, json!({"error": 1})));
    // 404 (wrong query), a redirect (not followed), a timeout [S:291]
    for path in ["/ds/files/s1/1?t=wrong", "/redir", "/slow", "/nothing-here"] {
        let url = format!("http://docserver.test:8780{path}");
        let t = std::time::Instant::now();
        assert_eq!(
            send(body(&key, 2, Some(&url))).await,
            (200, json!({"error": 1})),
            "{path}"
        );
        assert!(
            t.elapsed() < Duration::from_secs(4),
            "{path} took {:?}",
            t.elapsed()
        );
    }
    // never {"error":0} on a failure; nothing stored; each failure has a reason without the query
    assert_untouched(&h);
    let rj = h.rejected();
    assert_eq!(rj.len(), 5);
    assert!(rj.iter().all(|r| !r.reason.contains("t=wrong")), "{rj:?}");
    // and an unreachable internal base is a failure, not a hang
    h.update_config(|c| c.callback_internal_base = "http://127.0.0.1:1".into());
    assert_eq!(
        send(body(&key, 6, Some(GOOD_URL))).await,
        (200, json!({"error": 1}))
    );
    // unknown record
    h.update_config(|c| c.callback_internal_base = c.callback_internal_base.clone());
    let b = with_token(&secret, body("file_55_v1_x", 6, Some(GOOD_URL)));
    assert_eq!(post_json(cb_url(&h, "files", 55), None, b).await.0, 404);
}

// ---- cross-site host page [S:302-319] ---------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn host_page_serves_under_two_site_names() {
    let (h, _) = setup(|c| c.docserver_public_url = "http://docserver.test:8780/".into()).await;
    let a = h.page_url("127.0.0.1", Kind::File, 7);
    let b = h.page_url("localhost", Kind::File, 7);
    assert_ne!(
        a.split('/').nth(2),
        b.split('/').nth(2),
        "two different sites"
    );
    for u in [a, b] {
        let (s, bytes) = get_raw(u.clone()).await;
        let html = String::from_utf8(bytes).unwrap();
        assert_eq!(s, 200, "{u}");
        assert!(
            html.contains("http://docserver.test:8780/web-apps/apps/api/documents/api.js"),
            "{u}"
        );
        assert!(!html.contains("{{DOCSERVER}}"));
        assert!(html.contains("editor-config") && html.contains("cfg.events"));
    }
}

// ---- spike S6 / S7 reproduced against the harness rules --------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn spike_s6_happy_path() {
    let (h, _) = setup(|c| c.bearer_fallback = true).await;
    let secret = h.config().ds_secret;
    // 1. the page fetches the config and the editor verifies the token
    let (_, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    jwt::verify(secret.as_bytes(), c["token"].as_str().unwrap()).unwrap();
    // 2. the document server fetches document.url
    assert_eq!(
        get_raw(c["document"]["url"].as_str().unwrap().to_string()).await,
        (200, DOC_V1.to_vec())
    );
    // 3. Ctrl+S: a status-6 callback as the spike sends it, body token plus Authorization header
    let b = body(c["document"]["key"].as_str().unwrap(), 6, Some(GOOD_URL));
    let hdr = jwt::sign(secret.as_bytes(), &json!({"payload": b}));
    let (s, r) = post_json(
        c["editorConfig"]["callbackUrl"]
            .as_str()
            .unwrap()
            .to_string(),
        Some(hdr),
        with_token(&secret, b),
    )
    .await;
    assert_eq!((s, r), (200, json!({"error": 0})));
    // 4. the HRM holds the saved bytes (the spike's received.jsonl line)
    let got = h.received();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].bytes, DOC_V2.len());
    assert_eq!(h.object(Kind::File, 7).unwrap(), DOC_V2);
    // 5. reopening serves the saved version through a fresh config
    let (_, c2) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    assert_eq!(
        get_raw(c2["document"]["url"].as_str().unwrap().to_string()).await,
        (200, DOC_V2.to_vec())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn spike_s7_negative() {
    let (h, _) = setup(|_| {}).await;
    let secret = h.config().ds_secret;
    let (_, c) = get_json(format!("{}/files/7/editor-config", h.base_url())).await;
    let key = c["document"]["key"].as_str().unwrap();
    // (i) one char of config.token flipped: the verifier (the document server's side) refuses it
    assert_eq!(
        jwt::verify(secret.as_bytes(), &flip(c["token"].as_str().unwrap())),
        Err(jwt::JwtError::BadSignature)
    );
    // (ii) a correctly signed callback naming the metadata address: refused, not fetched [S:287]
    let b = with_token(&secret, body(key, 6, Some("http://169.254.169.254/latest")));
    assert_eq!(
        post_json(cb_url(&h, "files", 7), None, b).await,
        (403, json!({"error": 1}))
    );
    // (iii) the HRM verifies with a different secret: 401 {"error":1}, reason BadSignature, nothing stored
    h.set_callback_secret(Some(format!("{SECRET_FLIP}{secret}")));
    let b = with_token(&secret, body(key, 6, Some(GOOD_URL)));
    assert_eq!(
        post_json(cb_url(&h, "files", 7), None, b.clone()).await,
        (401, json!({"error": 1}))
    );
    assert!(h.rejected().last().unwrap().reason.contains("BadSignature"));
    assert_untouched(&h);
    h.set_callback_secret(None);
    assert_eq!(post_json(cb_url(&h, "files", 7), None, b).await.0, 200);
}

#[test]
fn jwt_pins_hs256() {
    let t = jwt::sign(b"k", &json!({"a": 1}));
    assert_eq!(jwt::verify(b"k", &t).unwrap(), json!({"a": 1}));
    assert_eq!(jwt::verify(b"k", "a.b"), Err(jwt::JwtError::Malformed));
    assert_eq!(
        jwt::verify(b"k", &format!("{}.e30.", "eyJhbGciOiJIUzUxMiJ9")),
        Err(jwt::JwtError::BadAlgorithm)
    );
    assert_eq!(
        jwt::verify(b"k", &jwt::sign(b"k", &json!({"exp": 1}))),
        Err(jwt::JwtError::Expired)
    );
}
