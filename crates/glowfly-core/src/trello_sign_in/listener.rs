//! The loopback listener of Sign in with Trello: binds a fixed port on both loopback addresses,
//! serves the return page and takes the posted fragment, each connection on its own short-lived
//! thread, until a token or refusal arrives, the time runs out or the caller stops it.

use super::request::{self, MAX_REQUEST, Parsed, Request, Route};
use super::{Fragment, PORTS, SignInEnd, read_fragment};
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

/// How long one connection may take to send its request or read the answer.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
/// Connections served at once; more are closed straight away.
const MAX_CONNECTIONS: usize = 8;
/// How often the listener looks for connections, the caller's stop and the time.
const POLL: Duration = Duration::from_millis(50);

type Found = Result<String, SignInEnd>;

/// Waits for Trello to hand back a token for `key`: binds a port of [`PORTS`], hands `open`
/// the authorize URL to show in the browser, then serves until the token arrives (`Ok`), the
/// user refuses, `timeout` passes or `stopped` turns true. The listener is closed on return.
pub fn wait_for_token(key: &str, timeout: Duration, stopped: &dyn Fn() -> bool, open: impl FnOnce(String)) -> Found {
    let bound = bind_first(&PORTS)?;
    let nonce = crate::askpass::protocol::new_token().map_err(|e| SignInEnd::Failed(e.to_string()))?;
    let return_url = format!("http://localhost:{}/trello/{nonce}", bound.port);
    let url = crate::forge::trello_sign_in_url(key, &return_url).ok_or_else(|| SignInEnd::Failed("the API key isn't valid".into()))?;
    open(url);
    bound.serve(&nonce, Instant::now() + timeout, stopped)
}

/// A port bound on 127.0.0.1, and on ::1 too where this computer has IPv6.
pub(super) struct Bound {
    pub port: u16,
    listeners: Vec<TcpListener>,
}

/// Binds the first of `ports` that is free on both loopback addresses (0 picks any).
pub(super) fn bind_first(ports: &[u16]) -> Result<Bound, SignInEnd> {
    for &port in ports {
        let Ok(v4) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) else { continue };
        let Ok(port) = v4.local_addr().map(|a| a.port()) else { continue };
        let mut listeners = vec![v4];
        match TcpListener::bind((Ipv6Addr::LOCALHOST, port)) {
            Ok(v6) => listeners.push(v6),
            // Another program listens there: a browser resolving localhost to ::1 would reach it.
            Err(e) if e.kind() == ErrorKind::AddrInUse => continue,
            // No IPv6 here, so localhost is 127.0.0.1 alone.
            Err(_) => {}
        }
        if listeners.iter().all(|l| l.set_nonblocking(true).is_ok()) {
            return Ok(Bound { port, listeners });
        }
    }
    Err(SignInEnd::NoPort)
}

impl Bound {
    /// Serves until a token or refusal arrives, `deadline` passes or `stopped` turns true.
    pub(super) fn serve(self, nonce: &str, deadline: Instant, stopped: &dyn Fn() -> bool) -> Found {
        let (tx, rx) = mpsc::channel();
        let active = Arc::new(AtomicUsize::new(0));
        loop {
            if stopped() {
                return Err(SignInEnd::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(SignInEnd::TimedOut);
            }
            for l in &self.listeners {
                while let Ok((stream, _)) = l.accept() {
                    self.spawn(stream, nonce, &tx, &active);
                }
            }
            if let Ok(found) = rx.recv_timeout(POLL) {
                return found;
            }
        }
    }

    fn spawn(&self, stream: TcpStream, nonce: &str, tx: &Sender<Found>, active: &Arc<AtomicUsize>) {
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            return; // dropping the stream closes it
        }
        let (port, nonce, tx, conn_active) = (self.port, nonce.to_owned(), tx.clone(), active.clone());
        let spawned = std::thread::Builder::new().name("glowfly-trello-sign-in".into()).spawn(move || {
            if let Some(found) = handle(&stream, port, &nonce) {
                let _ = tx.send(found);
            }
            conn_active.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Reads one request from `stream`, answers it and says what it brought, if anything.
fn handle(mut stream: &TcpStream, port: u16, nonce: &str) -> Option<Found> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(CONNECTION_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(CONNECTION_TIMEOUT)).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let answered = loop {
        let n = stream.read(&mut chunk).ok().filter(|&n| n > 0)?;
        buf.extend_from_slice(&chunk[..n]);
        match request::parse(&buf) {
            Parsed::Incomplete if buf.len() < MAX_REQUEST => {}
            // Malformed or too big: closed without an answer.
            Parsed::Incomplete | Parsed::Bad => return None,
            Parsed::Done(req) => break answer(&req, port, nonce),
        }
    };
    buf.fill(0);
    let (reply, found) = answered;
    let _ = stream.write_all(&reply).and_then(|()| stream.flush());
    found
}

/// The response to `req`, and the token or refusal it brought.
pub(super) fn answer(req: &Request<'_>, port: u16, nonce: &str) -> (Vec<u8>, Option<Found>) {
    let text = |status: &str, body: &str| request::response(status, "text/plain; charset=utf-8", body);
    match request::route(req, port, nonce) {
        Route::Page => (request::response("200 OK", "text/html; charset=utf-8", request::PAGE), None),
        Route::Fragment(f) => match read_fragment(f) {
            Fragment::Token(t) => (text("200 OK", "ok"), Some(Ok(t))),
            Fragment::Denied => (text("200 OK", "denied"), Some(Err(SignInEnd::Denied))),
            Fragment::Invalid => (text("400 Bad Request", "invalid"), None),
        },
        Route::Missing => (request::not_found(), None),
    }
}
