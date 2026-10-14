//! HTTP helpers shared by the service implementations.

use super::*;
use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

/// The most of an API response Trellis reads; a larger answer is refused.
const MAX_JSON: u64 = 10 * 1024 * 1024;
/// The most of an error response read, for its message.
const MAX_ERROR: u64 = 64 * 1024;

/// Reads at most `limit` bytes of a response body, failing when there is more.
pub(crate) fn read_capped(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader.take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(Error::Other(format!("the server's response was larger than {} MB", limit / (1024 * 1024))));
    }
    Ok(buf)
}

fn read_json<T: DeserializeOwned>(resp: ureq::Response) -> Result<T> {
    Ok(serde_json::from_slice(&read_capped(resp.into_reader(), MAX_JSON)?)?)
}

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

/// One agent for every service call, so connections (and TLS sessions) are pooled and reused.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent(concat!("Trellis/", env!("CARGO_PKG_VERSION"))).build()
    })
}

/// Headers only one service understands, sent to that service alone.
pub(super) fn service_headers(provider: Provider) -> &'static [(&'static str, &'static str)] {
    match provider {
        Provider::GitHub | Provider::GitHubEnterprise => &[("X-GitHub-Api-Version", "2022-11-28")],
        _ => &[],
    }
}

/// A request carrying the credential and its service's own headers. Only `https://` URLs are
/// accepted, so a credential never travels in the clear.
pub(super) fn request(method: &str, auth: &Auth, url: &str) -> Result<ureq::Request> {
    if links::https_host(url).is_none() {
        return Err(Error::Other("refusing to send credentials to an address that isn't https".into()));
    }
    #[cfg(test)]
    let redirected = super::mock::redirect(url);
    #[cfg(test)]
    let url = redirected.as_ref().map_or(url, |(u, _)| u.as_str());
    let mut req = agent().request(method, url).set("Authorization", &auth.header).set("Accept", "application/json");
    #[cfg(test)]
    if let Some((_, host)) = &redirected {
        req = req.set("X-Test-Host", host);
    }
    for (name, value) in service_headers(auth.provider) {
        req = req.set(name, value);
    }
    Ok(req)
}

/// Sends a request (with a JSON body for writes); a non-success status becomes an error.
fn call(req: ureq::Request, body: Option<serde_json::Value>) -> Result<ureq::Response> {
    #[cfg(test)]
    super::mock::only_local(req.url())?;
    let r = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    match r {
        Ok(resp) => Ok(resp),
        Err(ureq::Error::Status(code, resp)) => Err(read_error(code, resp)),
        Err(e) => Err(Error::Network(e.to_string())),
    }
}

/// The error message in a failed response's body, cleaned of control and bidi characters,
/// shortened, and labelled as the server's words. Empty when the body says nothing.
fn server_message(body: &str) -> String {
    let msg = serde_json::from_str::<serde_json::Value>(body)
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
        .unwrap_or_else(|| body.to_owned());
    let msg = crate::text::printable(&msg);
    if msg.is_empty() {
        return msg;
    }
    let short: String = msg.chars().take(200).collect();
    let more = if short.len() < msg.len() { "…" } else { "" };
    format!("the server said \u{201c}{short}{more}\u{201d}")
}

fn read_error(code: u16, resp: ureq::Response) -> Error {
    let mut body = Vec::new();
    let _ = resp.into_reader().take(MAX_ERROR).read_to_end(&mut body);
    let msg = server_message(&String::from_utf8_lossy(&body));
    match code {
        401 => Error::Auth(if msg.is_empty() { "401".into() } else { msg }),
        403 => Error::Forbidden(msg),
        404 => Error::NotFound(msg),
        _ => Error::Http { status: code, message: msg },
    }
}

fn json<T: DeserializeOwned>(resp: ureq::Response) -> Result<T> {
    // Some servers (Azure DevOps) answer bad credentials with 203 + an HTML sign-in page.
    if resp.status() == 203 || !resp.content_type().contains("json") {
        return Err(Error::Auth("the server answered with a sign-in page".into()));
    }
    read_json(resp)
}

pub(crate) fn get<T: DeserializeOwned>(auth: &Auth, url: &str) -> Result<T> {
    json(call(request("GET", auth, url)?, None)?)
}

pub(crate) fn get_text(auth: &Auth, url: &str) -> Result<String> {
    let resp = call(request("GET", auth, url)?, None)?;
    Ok(String::from_utf8_lossy(&read_capped(resp.into_reader(), MAX_JSON)?).into_owned())
}

pub(crate) fn post<T: DeserializeOwned>(auth: &Auth, url: &str, body: serde_json::Value) -> Result<T> {
    json(call(request("POST", auth, url)?, Some(body))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_bodies_are_capped() {
        assert_eq!(read_capped(&b"12345"[..], 5).unwrap(), b"12345");
        let err = read_capped(std::io::repeat(b'x'), 2 * 1024 * 1024).unwrap_err();
        assert!(err.to_string().contains("larger than 2 MB"), "{err}");
    }

    #[test]
    fn service_headers_go_to_their_own_service_only() {
        for p in Provider::ALL {
            let headers = service_headers(p);
            let github = matches!(p, Provider::GitHub | Provider::GitHubEnterprise);
            assert_eq!(headers.iter().any(|(n, _)| n.starts_with("X-GitHub")), github, "{p:?}");
        }
        let auth = Auth::from_token(Provider::GitLab, None, "t", "test");
        let req = request("GET", &auth, "https://gitlab.com/api/v4/user").unwrap();
        assert_eq!(req.header("X-GitHub-Api-Version"), None);
        let auth = Auth::from_token(Provider::GitHubEnterprise, None, "t", "test");
        let req = request("GET", &auth, "https://git.acme.io/api/v3/user").unwrap();
        assert_eq!(req.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    }

    #[test]
    fn credentials_go_over_https_only() {
        let auth = Auth::from_token(Provider::GitHub, None, "t", "test");
        for bad in ["http://api.github.com/user", "ftp://x/y", "https://user@api.github.com/user", "api.github.com/user"] {
            assert!(request("GET", &auth, bad).is_err(), "{bad}");
        }
        // And tests never reach the network without a mock server.
        let err = get::<serde_json::Value>(&auth, "https://api.github.com/user").unwrap_err();
        assert!(err.to_string().contains("tests may not call"), "{err}");
    }

    #[test]
    fn every_request_shares_one_agent() {
        assert!(std::ptr::eq(agent(), agent()));
    }

    #[test]
    fn server_messages_are_cleaned_and_labelled() {
        assert_eq!(server_message(r#"{"message":"Bad credentials"}"#), "the server said \u{201c}Bad credentials\u{201d}");
        let sneaky = "{\"message\":\"ok\\u001b[2J\\u202e evil\\nline\"}";
        assert_eq!(server_message(sneaky), "the server said \u{201c}ok[2J evil line\u{201d}");
        let long = server_message(&"x".repeat(500));
        assert!(long.contains(&"x".repeat(200)) && !long.contains(&"x".repeat(201)) && long.contains('…'), "{long}");
        assert_eq!(server_message("  \r\n "), "");
    }
}
