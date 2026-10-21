//! A tiny in-process HTTP server for the service tests. While one runs on a test's thread, calls
//! to `https://<host>/…` go to it over plain HTTP on 127.0.0.1 instead (see `http::request`),
//! with the original host in an `X-Test-Host` header. Without one, tests refuse to make calls,
//! so no test can reach the network.

use crate::error::{Error, Result};
use std::cell::Cell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

thread_local! {
    static PORT: Cell<Option<u16>> = const { Cell::new(None) };
}

/// The mock-server address for an `https://` URL, and the host it named.
pub(super) fn redirect(url: &str) -> Option<(String, String)> {
    let port = PORT.with(Cell::get)?;
    let rest = url.strip_prefix("https://")?;
    let split = rest.find(['/', '?']).unwrap_or(rest.len());
    let (host, path) = rest.split_at(split);
    let path = if path.starts_with('/') { path.to_owned() } else { format!("/{path}") };
    Some((format!("http://127.0.0.1:{port}{path}"), host.to_owned()))
}

/// Fails unless `url` points at the mock server.
pub(super) fn only_local(url: &str) -> Result<()> {
    if url.starts_with("http://127.0.0.1:") { Ok(()) } else { Err(Error::Network(format!("tests may not call {url}"))) }
}

/// A request the server received.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    /// The host the URL named (before it was redirected here).
    pub host: String,
    /// Path and query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_default()
    }

    /// `METHOD host/path`, for matching routes.
    pub fn route(&self) -> String {
        format!("{} {}{}", self.method, self.host, self.path)
    }
}

/// What the server answers.
pub struct Reply {
    status: u16,
    content_type: &'static str,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    pub fn json(body: impl Into<String>) -> Reply {
        Reply { status: 200, content_type: "application/json; charset=utf-8", headers: Vec::new(), body: body.into() }
    }

    pub fn text(body: impl Into<String>) -> Reply {
        Reply { content_type: "text/plain", ..Reply::json(body) }
    }

    pub fn status(status: u16, body: impl Into<String>) -> Reply {
        Reply { status, ..Reply::json(body) }
    }

    pub fn not_found() -> Reply {
        Reply::status(404, r#"{"message":"Not Found"}"#)
    }

    pub fn header(mut self, name: &str, value: &str) -> Reply {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

type Handler = dyn Fn(&Request) -> Reply + Send + Sync;

/// A running server; stops (and stops redirecting) when dropped.
pub struct Mock {
    port: u16,
    seen: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Mock {
    /// Serves `handler`'s replies and redirects this thread's service calls here.
    pub fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen, stop) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicBool::new(false)));
        let handler: Box<Handler> = Box::new(handler);
        let thread = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::spawn(move || {
                for conn in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = conn {
                        serve(stream, &*handler, &seen);
                    }
                }
            })
        };
        PORT.with(|p| p.set(Some(port)));
        Mock { port, seen, stop, thread: Some(thread) }
    }

    /// Every request so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        PORT.with(|p| {
            if p.get() == Some(self.port) {
                p.set(None);
            }
        });
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(stream: TcpStream, handler: &Handler, seen: &Mutex<Vec<Request>>) {
    let Some(req) = read_request(&stream) else { return };
    let reply = handler(&req);
    seen.lock().unwrap().push(req);
    let mut head = format!(
        "HTTP/1.1 {} Mock\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.content_type,
        reply.body.len()
    );
    for (n, v) in &reply.headers {
        head.push_str(&format!("{n}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let mut stream = stream;
    let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(reply.body.as_bytes()));
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        r.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (n, v) = h.split_once(':')?;
        headers.push((n.trim().to_owned(), v.trim().to_owned()));
    }
    let find = |name: &str| headers.iter().find(|(n, _): &&(String, String)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    let len: usize = find("Content-Length").and_then(|l| l.parse().ok()).unwrap_or(0);
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    let host = find("X-Test-Host").unwrap_or_default();
    Some(Request { method, host, path, headers, body: String::from_utf8_lossy(&body).into_owned() })
}
