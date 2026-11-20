//! The loopback listener of Sign in with Trello: binds a fixed port on both loopback addresses,
//! serves the return page and takes the posted fragment, each connection on its own short-lived
//! thread, until a token or refusal arrives, the time runs out or the caller stops it.

use super::request::{self, MAX_REQUEST, Parsed, Request, Route};
use super::{Fragment, PORTS, SignInEnd, read_fragment};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

/// How long one connection may take in all, to send its request and read the answer.
const CONNECTION_TIME: Duration = Duration::from_secs(10);
/// Connections served at once; more are closed straight away.
const MAX_CONNECTIONS: usize = 8;
/// How often the listener looks for connections, the caller's stop and the time.
const POLL: Duration = Duration::from_millis(50);

type Found = Result<String, SignInEnd>;

/// Waits for Trello to hand back a token for `key`: binds a port of [`PORTS`], hands `open`
/// the authorize URL to show in the browser, then serves until the token arrives (`Ok`), the
/// user refuses, `timeout` passes or `stopped` turns true. The listener is closed on return.
pub fn wait_for_token(key: &str, timeout: Duration, stopped: &dyn Fn() -> bool, open: impl FnOnce(String)) -> Found {
    wait_on(&PORTS, key, timeout, stopped, open)
}

/// [`wait_for_token`] on the first free port of `ports`. A stop asked while the port was being
/// bound ends the wait before `open` is called.
pub(super) fn wait_on(ports: &[u16], key: &str, timeout: Duration, stopped: &dyn Fn() -> bool, open: impl FnOnce(String)) -> Found {
    let bound = bind_first(ports)?;
    let nonce = crate::askpass::protocol::new_token().map_err(|e| SignInEnd::Failed(e.to_string()))?;
    let return_url = format!("http://localhost:{}/trello/{nonce}", bound.port);
    let url = crate::forge::trello_sign_in_url(key, &return_url).ok_or_else(|| SignInEnd::Failed("the API key isn't valid".into()))?;
    if stopped() {
        return Err(SignInEnd::Cancelled);
    }
    open(url);
    bound.serve(&nonce, Instant::now() + timeout, stopped)
}

/// A port bound on 127.0.0.1, and on ::1 too where this computer has IPv6.
pub(super) struct Bound {
    pub port: u16,
    listeners: Vec<TcpListener>,
    /// How long one connection may take in all ([`CONNECTION_TIME`]; tests shorten it).
    pub connection_time: Duration,
}

/// Binds the first of `ports` that is free on both loopback addresses (0 picks any).
pub(super) fn bind_first(ports: &[u16]) -> Result<Bound, SignInEnd> {
    bind_first_with(ports, |ip, port| listen(SocketAddr::new(ip, port)))
}

/// [`bind_first`] with `bind` making each listener.
pub(super) fn bind_first_with(ports: &[u16], bind: impl Fn(IpAddr, u16) -> io::Result<TcpListener>) -> Result<Bound, SignInEnd> {
    for &port in ports {
        let Ok(v4) = bind(Ipv4Addr::LOCALHOST.into(), port) else { continue };
        let Ok(port) = v4.local_addr().map(|a| a.port()) else { continue };
        let mut listeners = vec![v4];
        match bind(Ipv6Addr::LOCALHOST.into(), port) {
            Ok(v6) => listeners.push(v6),
            // No IPv6 here, so localhost is 127.0.0.1 alone.
            Err(e) if no_ipv6(&e) => {}
            // In use by another program (a browser resolving localhost to ::1 would reach it),
            // or failing otherwise: the next port.
            Err(_) => continue,
        }
        if listeners.iter().all(|l| l.set_nonblocking(true).is_ok()) {
            return Ok(Bound { port, listeners, connection_time: CONNECTION_TIME });
        }
    }
    Err(SignInEnd::NoPort)
}

/// Whether binding ::1 failed because this computer has no IPv6 loopback: the address isn't
/// there, or the address family isn't supported.
pub(super) fn no_ipv6(e: &io::Error) -> bool {
    #[cfg(windows)]
    const EAFNOSUPPORT: i32 = windows::Win32::Networking::WinSock::WSAEAFNOSUPPORT.0;
    #[cfg(unix)]
    const EAFNOSUPPORT: i32 = libc::EAFNOSUPPORT;
    #[cfg(not(any(windows, unix)))]
    const EAFNOSUPPORT: i32 = -1;
    matches!(e.kind(), ErrorKind::AddrNotAvailable | ErrorKind::Unsupported) || e.raw_os_error() == Some(EAFNOSUPPORT)
}

/// A listener on `addr`. On Windows the port is taken exclusively (`SO_EXCLUSIVEADDRUSE`), so no
/// other program can bind it while Glowfly listens, even with `SO_REUSEADDR`.
#[cfg(windows)]
fn listen(addr: SocketAddr) -> io::Result<TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::os::windows::io::AsRawSocket;
    use windows::Win32::Networking::WinSock::{SO_EXCLUSIVEADDRUSE, SOCKET, SOL_SOCKET, setsockopt};
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?;
    let raw = SOCKET(usize::try_from(socket.as_raw_socket()).map_err(io::Error::other)?);
    // SAFETY: `raw` is the open socket owned by `socket`, and the option takes a BOOL.
    if unsafe { setsockopt(raw, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, Some(&1i32.to_ne_bytes())) } != 0 {
        return Err(io::Error::last_os_error());
    }
    socket.bind(&addr.into())?;
    socket.listen(128)?;
    Ok(socket.into())
}

/// A listener on `addr`, as std binds it (without `SO_REUSEPORT`).
#[cfg(not(windows))]
fn listen(addr: SocketAddr) -> io::Result<TcpListener> {
    TcpListener::bind(addr)
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
        let deadline = Instant::now() + self.connection_time;
        let spawned = std::thread::Builder::new().name("glowfly-trello-sign-in".into()).spawn(move || {
            if let Some(found) = handle(&stream, port, &nonce, deadline) {
                let _ = tx.send(found);
            }
            conn_active.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Reads one request from `stream`, answers it and says what it brought, if anything. The
/// whole exchange must end by `deadline`; a slower client is closed without an answer.
fn handle(mut stream: &TcpStream, port: u16, nonce: &str, deadline: Instant) -> Option<Found> {
    stream.set_nonblocking(false).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let answered = loop {
        stream.set_read_timeout(Some(left(deadline)?)).ok()?;
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
    // What was read counts even when no time is left to answer.
    if let Some(time) = left(deadline)
        && stream.set_write_timeout(Some(time)).is_ok()
    {
        let _ = stream.write_all(&reply).and_then(|()| stream.flush());
    }
    found
}

/// The time left until `deadline`; None once it has passed.
fn left(deadline: Instant) -> Option<Duration> {
    Some(deadline.saturating_duration_since(Instant::now())).filter(|d| !d.is_zero())
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
