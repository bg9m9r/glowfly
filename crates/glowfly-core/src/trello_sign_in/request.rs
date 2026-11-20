//! The listener's HTTP, as pure functions: reading one request from the bytes received so far,
//! deciding what it asks for, and the responses (including the page that posts the fragment
//! back).

use crate::askpass::protocol::token_matches;

/// The most bytes one request may take, headers and body; more closes the connection.
pub(crate) const MAX_REQUEST: usize = 8 * 1024;

/// One request, borrowed from the bytes received.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request<'a> {
    pub method: &'a str,
    /// The request target as sent, query included.
    pub target: &'a str,
    pub host: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub body: &'a [u8],
}

/// How far the bytes received make a request.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Parsed<'a> {
    /// More bytes are needed.
    Incomplete,
    /// Not a request this listener reads (too big, chunked, not HTTP/1).
    Bad,
    Done(Request<'a>),
}

/// Reads one HTTP/1 request from `buf`.
pub(crate) fn parse(buf: &[u8]) -> Parsed<'_> {
    let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return if buf.len() >= MAX_REQUEST { Parsed::Bad } else { Parsed::Incomplete };
    };
    let Ok(head) = std::str::from_utf8(&buf[..end]) else { return Parsed::Bad };
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version), None) = (first.next(), first.next(), first.next(), first.next()) else {
        return Parsed::Bad;
    };
    if !version.starts_with("HTTP/1.") || method.is_empty() || !target.starts_with('/') {
        return Parsed::Bad;
    }
    let (mut host, mut origin, mut length) = (None, None, 0usize);
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { return Parsed::Bad };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => host = Some(value),
            "origin" => origin = Some(value),
            "content-length" => match value.parse::<usize>() {
                Ok(n) if n <= MAX_REQUEST => length = n,
                _ => return Parsed::Bad,
            },
            "transfer-encoding" => return Parsed::Bad,
            _ => {}
        }
    }
    let start = end + 4;
    if start + length > MAX_REQUEST {
        return Parsed::Bad;
    }
    match buf.get(start..start + length) {
        Some(body) => Parsed::Done(Request { method, target, host, origin, body }),
        None => Parsed::Incomplete,
    }
}

/// What a request asks for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Route<'a> {
    /// The page Trello returns the browser to.
    Page,
    /// The page posting the fragment back.
    Fragment(&'a str),
    /// Anything else, from anywhere else or without the nonce: 404.
    Missing,
}

/// Decides what `req` asks for on the listener at `port` with `nonce`. Only a local `Host`
/// (and `Origin`, when sent) for this port is served.
pub(crate) fn route<'a>(req: &Request<'a>, port: u16, nonce: &str) -> Route<'a> {
    let local = |h: &str| ["localhost", "127.0.0.1", "[::1]"].iter().any(|n| h.eq_ignore_ascii_case(&format!("{n}:{port}")));
    if !req.host.is_some_and(local) {
        return Route::Missing;
    }
    if req.origin.is_some_and(|o| !o.strip_prefix("http://").is_some_and(local)) {
        return Route::Missing;
    }
    let path = req.target.split_once('?').map_or(req.target, |(p, _)| p);
    let Some(rest) = path.strip_prefix("/trello/") else { return Route::Missing };
    let (id, tail) = rest.split_once('/').map_or((rest, None), |(id, t)| (id, Some(t)));
    if !token_matches(id, nonce) {
        return Route::Missing;
    }
    match (req.method, tail) {
        ("GET", None) => Route::Page,
        ("POST", Some("token")) => std::str::from_utf8(req.body).map_or(Route::Missing, Route::Fragment),
        _ => Route::Missing,
    }
}

/// A whole response that closes the connection; pages may run only their own inline script
/// and talk only to this listener.
pub(crate) fn response(status: &str, content_type: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\
         Referrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; \
         script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

pub(crate) fn not_found() -> Vec<u8> {
    response("404 Not Found", "text/plain; charset=utf-8", "Not found")
}

/// The page Trello returns to: it takes the fragment off the address bar, posts it back and
/// says what happened. Nothing is loaded from elsewhere.
pub(crate) const PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Glowfly</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:32em;margin:4em auto;padding:0 1em;color:#222;background:#fafafa}
@media (prefers-color-scheme:dark){body{color:#ddd;background:#1c1c1e}}</style></head>
<body><h1>Glowfly</h1><p id="m">Finishing sign-in…</p>
<script>
(function () {
  var hash = location.hash.slice(1), m = document.getElementById("m");
  history.replaceState(null, "", location.pathname);
  fetch(location.pathname + "/token", { method: "POST", headers: { "Content-Type": "text/plain" }, body: hash })
    .then(function (r) { return r.text(); })
    .then(function (t) {
      m.textContent = t === "ok" ? "You can close this tab and return to Glowfly."
        : t === "denied" ? "Trello didn't allow Glowfly. You can close this tab and try again in Glowfly."
        : "Trello's answer wasn't understood. You can close this tab and try again in Glowfly.";
    })
    .catch(function () { m.textContent = "Glowfly stopped waiting. You can close this tab and try again in Glowfly."; });
})();
</script></body></html>
"#;
