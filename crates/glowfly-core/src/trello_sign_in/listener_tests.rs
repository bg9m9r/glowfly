//! Sign in with Trello's listener over real loopback connections: only the nonce's page and
//! post are served, the posted token ends the wait, a stop before the browser opens ends it
//! there, slow clients are closed and the ports are taken with care.

use super::SignInEnd;
use super::listener::{bind_first, bind_first_with, no_ipv6, wait_on};
use std::cell::Cell;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

#[test]
fn a_stop_while_binding_opens_no_browser() {
    let opened = Cell::new(false);
    let result = wait_on(&[0], NONCE, Duration::from_secs(20), &|| true, |_| opened.set(true));
    assert_eq!(result, Err(SignInEnd::Cancelled));
    assert!(!opened.get());
}

#[test]
fn a_slow_client_is_closed_at_the_connection_deadline() {
    let mut bound = bind_first(&[0]).unwrap();
    bound.connection_time = Duration::from_millis(500);
    let port = bound.port;
    let done = Arc::new(AtomicBool::new(false));
    let stop = done.clone();
    let waiting = std::thread::spawn(move || bound.serve(NONCE, Instant::now() + Duration::from_secs(20), &|| stop.load(Ordering::SeqCst)));
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    // One byte every 100 ms keeps each read short, but the whole request takes too long.
    for b in format!("GET /trello/{NONCE} HTTP/1.1\r\n").bytes().take(15) {
        if s.write_all(&[b]).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut out = [0u8; 64];
    match s.read(&mut out) {
        Ok(n) => assert_eq!(n, 0, "closed without an answer"),
        Err(e) => assert!(!matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut), "still open: {e}"),
    }
    assert!(!waiting.is_finished(), "the wait goes on");
    done.store(true, Ordering::SeqCst);
    assert_eq!(waiting.join().unwrap(), Err(SignInEnd::Cancelled));
}

#[test]
fn ipv6_errors_other_than_a_missing_loopback_skip_the_port() {
    let v4_binds = Cell::new(0);
    let counted = &v4_binds;
    let fake = |v6_error: ErrorKind| {
        move |ip: IpAddr, _: u16| {
            if ip.is_ipv4() {
                counted.set(counted.get() + 1);
                TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            } else {
                Err(io::Error::from(v6_error))
            }
        }
    };
    assert!(bind_first_with(&[1, 2], fake(ErrorKind::AddrNotAvailable)).is_ok(), "no IPv6: 127.0.0.1 alone");
    assert_eq!(v4_binds.replace(0), 1);
    assert_eq!(bind_first_with(&[1, 2], fake(ErrorKind::PermissionDenied)).err(), Some(SignInEnd::NoPort));
    assert_eq!(v4_binds.replace(0), 2, "each port tried");
    assert!(no_ipv6(&io::Error::from(ErrorKind::Unsupported)));
    assert!(!no_ipv6(&io::Error::from(ErrorKind::AddrInUse)));
}

/// On Windows no other socket can bind the port, not even one asking to reuse the address.
#[cfg(windows)]
#[test]
fn windows_takes_the_port_exclusively() {
    use socket2::{Domain, Socket, Type};
    let bound = bind_first(&[0]).unwrap();
    let other = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
    other.set_reuse_address(true).unwrap();
    let addr = std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, bound.port));
    assert!(other.bind(&addr.into()).is_err());
}
