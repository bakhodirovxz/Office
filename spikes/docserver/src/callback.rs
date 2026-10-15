//! Build, sign and POST the ONLYOFFICE-shaped callback; await `{"error":0}`.
use crate::fetch::{check_allowed, FetchError};
use crate::jwt;
use crate::session::Session;
use serde_json::{json, Value};
use std::time::Duration;

pub struct CallbackCtx<'a> {
    pub client: &'a reqwest::Client,
    pub secret: &'a [u8],
    pub public: &'a str,
    pub allowed: &'a [String],
}

/// Returns Ok(()) when the receiver answered 2xx with `{"error":0}`, else Err(detail).
pub async fn send(ctx: &CallbackCtx<'_>, sid: &str, s: &Session, ver: u32, status: u32) -> Result<(), String> {
    let url = check_allowed(&s.callback_url, ctx.allowed).map_err(|e| match e {
        FetchError::Blocked(m) | FetchError::Upstream(m) => m,
    })?;
    let file_token = jwt::sign(ctx.secret, &json!({"sid": sid, "ver": ver, "exp": jwt::now() + 600}));
    let mut body = json!({
        "key": s.key,
        "status": status,
        "url": format!("{}/files/{}/{}?t={}", ctx.public, sid, ver, file_token),
        "filetype": s.filetype,
        "users": [s.user_id],
        "forcesavetype": 1,
    });
    // ONLYOFFICE: Authorization: Bearer JWT({"payload": body}); body.token = JWT(body).
    let bearer = jwt::sign(ctx.secret, &json!({ "payload": body.clone() }));
    body["token"] = Value::String(jwt::sign(ctx.secret, &body.clone()));
    let resp = ctx
        .client
        .post(url)
        .timeout(Duration::from_secs(10))
        .bearer_auth(bearer)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("callback receiver answered {status}: {text}"));
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("callback reply not JSON: {text}"))?;
    if v.get("error").and_then(Value::as_i64) == Some(0) {
        Ok(())
    } else {
        Err(format!("callback reply was not error:0: {text}"))
    }
}
