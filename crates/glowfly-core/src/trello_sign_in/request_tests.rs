//! Sign in with Trello's pure parts: reading requests, routing them by nonce and host, and
//! reading the fragment Trello returns to.

use super::request::{MAX_REQUEST, Parsed, Request, Route, parse, route};
use super::{Fragment, read_fragment};

const NONCE: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "ATTA0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd";

fn req<'a>(method: &'a str, target: &'a str, host: &'a str, body: &'a [u8]) -> Request<'a> {
    Request { method, target, host: Some(host), origin: None, body }
}

#[test]
fn reads_a_whole_request() {
    let raw = b"POST /trello/x/token HTTP/1.1\r\nHost: localhost:47613\r\nOrigin: http://localhost:47613\r\nContent-Length: 5\r\n\r\nhello";
    let Parsed::Done(r) = parse(raw) else { panic!("{:?}", parse(raw)) };
    assert_eq!(
        (r.method, r.target, r.host, r.origin, r.body),
        ("POST", "/trello/x/token", Some("localhost:47613"), Some("http://localhost:47613"), &b"hello"[..])
    );
    assert_eq!(parse(&raw[..raw.len() - 1]), Parsed::Incomplete, "the body isn't all there");
    assert_eq!(parse(b"GET / HTTP/1.1\r\nHost: a"), Parsed::Incomplete, "the headers aren't all there");
    let get = parse(b"GET /trello/x HTTP/1.1\r\nhost: 127.0.0.1:1\r\n\r\n");
    assert!(matches!(get, Parsed::Done(Request { method: "GET", host: Some("127.0.0.1:1"), body: b"", .. })), "{get:?}");
}

#[test]
fn refuses_what_it_does_not_read() {
    for raw in [
        &b"GET / HTTP/2\r\n\r\n"[..],
        b"GET x HTTP/1.1\r\n\r\n",
        b"GET / HTTP/1.1 extra\r\n\r\n",
        b"GET / HTTP/1.1\r\nno colon\r\n\r\n",
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
        b"POST / HTTP/1.1\r\nContent-Length: 99999\r\n\r\n",
        b"POST / HTTP/1.1\r\nContent-Length: -1\r\n\r\n",
    ] {
        assert_eq!(parse(raw), Parsed::Bad, "{}", String::from_utf8_lossy(raw));
    }
    assert_eq!(parse(&vec![b'a'; MAX_REQUEST]), Parsed::Bad, "too long without an end of headers");
}

#[test]
fn routes_only_the_nonce_from_a_local_host() {
    let page = format!("/trello/{NONCE}");
    let post = format!("/trello/{NONCE}/token");
    let other = format!("{page}/other");
    for host in ["localhost:47613", "127.0.0.1:47613", "[::1]:47613", "LOCALHOST:47613"] {
        assert_eq!(route(&req("GET", &page, host, b""), 47613, NONCE), Route::Page, "{host}");
    }
    assert_eq!(route(&req("GET", &format!("{page}?x=1"), "localhost:47613", b""), 47613, NONCE), Route::Page);
    assert_eq!(route(&req("POST", &post, "localhost:47613", b"#token=a"), 47613, NONCE), Route::Fragment("#token=a"));
    let missing = [
        req("GET", &page, "localhost:47614", b""),
        req("GET", &page, "evil.example:47613", b""),
        req("GET", "/trello/0123456789abcdef0123456789abcdee", "localhost:47613", b""),
        req("GET", "/trello/", "localhost:47613", b""),
        req("GET", "/", "localhost:47613", b""),
        req("POST", &page, "localhost:47613", b""),
        req("GET", &post, "localhost:47613", b""),
        req("POST", &other, "localhost:47613", b""),
        req("POST", &post, "localhost:47613", &[0xff, 0xfe]),
    ];
    for r in &missing {
        assert_eq!(route(r, 47613, NONCE), Route::Missing, "{r:?}");
    }
    let no_host = Request { host: None, ..req("GET", &page, "", b"") };
    assert_eq!(route(&no_host, 47613, NONCE), Route::Missing);
    let foreign = Request { origin: Some("https://evil.example"), ..req("POST", &post, "localhost:47613", b"") };
    assert_eq!(route(&foreign, 47613, NONCE), Route::Missing, "a post from another site");
    let own = Request { origin: Some("http://localhost:47613"), ..req("POST", &post, "localhost:47613", b"") };
    assert_eq!(route(&own, 47613, NONCE), Route::Fragment(""));
}

#[test]
fn reads_the_fragment_tolerantly() {
    for raw in [format!("#token={TOKEN}"), format!("token={TOKEN}"), format!("a=b&token={TOKEN}&c"), TOKEN.to_owned()] {
        assert_eq!(read_fragment(&raw), Fragment::Token(TOKEN.into()), "{raw}");
    }
    for raw in ["#token=", "token=&error=Token%20request%20rejected", "error=denied", &format!("token={TOKEN}&error=x")] {
        assert_eq!(read_fragment(raw), Fragment::Denied, "{raw}");
    }
    for raw in ["", "#", "token=short", "token=has space0123456789abcdef0123456789", "<script>", "a=b"] {
        assert_eq!(read_fragment(raw), Fragment::Invalid, "{raw}");
    }
}
