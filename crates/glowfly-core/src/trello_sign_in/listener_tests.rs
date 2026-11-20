//! Sign in with Trello's listener over real loopback connections: only the nonce's page and
//! post are served, and the posted token ends the wait.

use super::SignInEnd;
use super::listener::bind_first;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

const NONCE: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Sends `raw` to the listener and reads the whole answer.
fn send(port: u16, raw: &str) -> String {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

fn post(port: u16, nonce: &str, body: &str) -> String {
    send(port, &format!("POST /trello/{nonce}/token HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: {}\r\n\r\n{body}", body.len()))
}

#[test]
fn hands_back_the_token_posted_with_the_nonce() {
    let bound = bind_first(&[0]).unwrap();
    let port = bound.port;
    let waiting = std::thread::spawn(move || bound.serve(NONCE, Instant::now() + Duration::from_secs(20), &|| false));
    let wrong = send(port, &format!("GET /trello/ffffffffffffffffffffffffffffffff HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"));
    assert!(wrong.starts_with("HTTP/1.1 404"), "{wrong}");
    let page = send(port, &format!("GET /trello/{NONCE} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"));
    assert!(page.starts_with("HTTP/1.1 200") && page.contains("location.hash") && page.contains("default-src 'none'"), "{page}");
    let elsewhere = send(port, &format!("GET /trello/{NONCE} HTTP/1.1\r\nHost: example.com\r\n\r\n"));
    assert!(elsewhere.starts_with("HTTP/1.1 404"), "{elsewhere}");
    assert!(post(port, "ffffffffffffffffffffffffffffffff", &format!("token={TOKEN}")).starts_with("HTTP/1.1 404"));
    assert!(post(port, NONCE, "token=nope").starts_with("HTTP/1.1 400"), "a malformed token keeps it waiting");
    assert!(!waiting.is_finished());
    let ok = post(port, NONCE, &format!("#token={TOKEN}"));
    assert!(ok.starts_with("HTTP/1.1 200") && ok.ends_with("ok"), "{ok}");
    assert_eq!(waiting.join().unwrap(), Ok(TOKEN.to_owned()));
}

#[test]
fn a_refusal_ends_the_wait() {
    let bound = bind_first(&[0]).unwrap();
    let port = bound.port;
    let waiting = std::thread::spawn(move || bound.serve(NONCE, Instant::now() + Duration::from_secs(20), &|| false));
    assert!(post(port, NONCE, "token=&error=denied").ends_with("denied"));
    assert_eq!(waiting.join().unwrap(), Err(SignInEnd::Denied));
}

#[test]
fn stops_when_asked_or_out_of_time() {
    let bound = bind_first(&[0]).unwrap();
    assert_eq!(bound.serve(NONCE, Instant::now() + Duration::from_secs(20), &|| true), Err(SignInEnd::Cancelled));
    let bound = bind_first(&[0]).unwrap();
    assert_eq!(bound.serve(NONCE, Instant::now(), &|| false), Err(SignInEnd::TimedOut));
}

#[test]
fn a_port_in_use_is_skipped() {
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = taken.local_addr().unwrap().port();
    assert_eq!(bind_first(&[port]).err(), Some(SignInEnd::NoPort));
    let bound = bind_first(&[port, 0]).unwrap();
    assert_ne!(bound.port, port);
}
