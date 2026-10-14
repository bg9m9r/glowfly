//! HTTP helpers shared by the service implementations.

use super::*;
use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use std::time::Duration;

pub(crate) fn dec(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes one URL path segment or query value. `.` and `..` are encoded too, so a
/// segment can never step up the path.
pub(crate) fn enc(s: &str) -> String {
    if s == "." || s == ".." {
        return s.replace('.', "%2E");
    }
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Encodes each `/`-separated segment of a path such as a GitLab `group/subgroup`.
pub(crate) fn enc_path(s: &str) -> String {
    s.split('/').map(enc).collect::<Vec<_>>().join("/")
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent(concat!("Trellis/", env!("CARGO_PKG_VERSION"))).build()
}

pub(super) fn request(method: &str, auth: &Auth, url: &str) -> ureq::Request {
    agent()
        .request(method, url)
        .set("Authorization", &auth.header)
        .set("Accept", "application/json")
        .set("X-GitHub-Api-Version", "2022-11-28")
}

fn read_error(code: u16, resp: ureq::Response) -> Error {
    let body = resp.into_string().unwrap_or_default();
    let msg = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v["message"]
                .as_str()
                .or_else(|| v["error"]["message"].as_str())
                .or_else(|| v["errorMessages"][0].as_str())
                .or_else(|| v["errors"][0]["message"].as_str())
                .or_else(|| v["error"].as_str())
                .map(|m| match v["errors"][0]["message"].as_str() {
                    // GitHub: "Repository creation failed." + "name already exists on this account".
                    Some(detail) if detail != m => format!("{m} {detail}"),
                    _ => m.to_owned(),
                })
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    match code {
        401 => Error::Auth(if msg.is_empty() { "401".into() } else { msg }),
        403 => Error::Forbidden(msg),
        404 => Error::NotFound(msg),
        _ => Error::Http { status: code, message: msg },
    }
}

pub(super) fn finish<T: DeserializeOwned>(r: std::result::Result<ureq::Response, ureq::Error>) -> Result<T> {
    match r {
        Ok(resp) => {
            // Some servers (Azure DevOps) answer bad credentials with 203 + an HTML sign-in page.
            if resp.status() == 203 || !resp.content_type().contains("json") {
                return Err(Error::Auth("the server answered with a sign-in page".into()));
            }
            Ok(resp.into_json()?)
        }
        Err(ureq::Error::Status(code, resp)) => Err(read_error(code, resp)),
        Err(e) => Err(Error::Network(e.to_string())),
    }
}

pub(crate) fn get<T: DeserializeOwned>(auth: &Auth, url: &str) -> Result<T> {
    finish(request("GET", auth, url).call())
}

pub(crate) fn get_text(auth: &Auth, url: &str) -> Result<String> {
    match request("GET", auth, url).call() {
        Ok(resp) => Ok(resp.into_string()?),
        Err(ureq::Error::Status(code, resp)) => Err(read_error(code, resp)),
        Err(e) => Err(Error::Network(e.to_string())),
    }
}

pub(crate) fn post<T: DeserializeOwned>(auth: &Auth, url: &str, body: serde_json::Value) -> Result<T> {
    finish(request("POST", auth, url).send_json(body))
}
