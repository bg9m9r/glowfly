//! The app's side: a loopback listener that turns each helper connection into an
//! [`Event::Asked`] and waits for the app to [`Server::answer`] it.

use super::protocol::{self, MAX_LINE, Response};
use super::{PROMPT_TIMEOUT, Prompt, Reply};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// How long a connection may take to send its request.
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections served at once; more are closed straight away, so a local process flooding
/// the port can't make the app start unbounded threads.
pub(super) const MAX_CONNECTIONS: usize = 8;

/// What the listener reports to the app.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A helper is waiting for an answer to `prompt`.
    Asked { id: u64, prompt: Prompt },
    /// Prompt `id` was not answered in time; the helper has been told.
    Expired { id: u64 },
}

type Pending = Mutex<HashMap<u64, Sender<Reply>>>;
type OnEvent = Arc<dyn Fn(Event) + Send + Sync>;

/// The running listener. Dropping the last handle cancels every prompt still waiting.
pub struct Server {
    addr: SocketAddr,
    token: String,
    pending: Arc<Pending>,
}

/// Starts the listener with the standard timeout. `on_event` runs on the listener's threads.
pub fn start(on_event: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Arc<Server>> {
    start_with(PROMPT_TIMEOUT, on_event)
}

/// Like [`start`], with prompts closing after `timeout`.
pub fn start_with(timeout: Duration, on_event: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Arc<Server>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let token = protocol::new_token()?;
    let server = Arc::new(Server { addr, token: token.clone(), pending: Arc::default() });
    let shared = Arc::new(Shared {
        token,
        timeout,
        pending: Arc::downgrade(&server.pending),
        next_id: AtomicU64::new(0),
        active: AtomicUsize::new(0),
        on_event: Arc::new(on_event),
    });
    std::thread::Builder::new().name("trellis-askpass".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            if shared.active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                shared.active.fetch_sub(1, Ordering::SeqCst);
                continue; // dropping the stream closes it
            }
            let conn = shared.clone();
            let spawned = std::thread::Builder::new().name("trellis-askpass-conn".into()).spawn(move || {
                conn.serve(stream);
                conn.active.fetch_sub(1, Ordering::SeqCst);
            });
            if let Err(e) = spawned {
                shared.active.fetch_sub(1, Ordering::SeqCst);
                eprintln!("trellis: could not start a thread for a credential prompt: {e}");
            }
        }
    })?;
    Ok(server)
}

impl Server {
    /// Hands `reply` to the helper waiting on prompt `id`. False when that prompt has expired
    /// or is unknown.
    pub fn answer(&self, id: u64, reply: Reply) -> bool {
        let tx = self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        tx.is_some_and(|tx| tx.send(reply).is_ok())
    }

    /// `127.0.0.1:<port>`.
    pub fn addr(&self) -> String {
        self.addr.to_string()
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

/// What every connection thread needs.
struct Shared {
    token: String,
    timeout: Duration,
    pending: Weak<Pending>,
    next_id: AtomicU64,
    /// Connections being served now.
    active: AtomicUsize,
    on_event: OnEvent,
}

impl Shared {
    fn serve(&self, stream: TcpStream) {
        let Some(response) = self.respond(&stream) else { return };
        let mut stream = stream;
        let mut text = protocol::encode_response(&response);
        let _ = stream.write_all(text.as_bytes()).and_then(|()| stream.flush());
        super::wipe(&mut text);
        if let Response::Answer(mut s) = response {
            super::wipe(&mut s);
        }
    }

    /// The response to send, or None to close the connection without one.
    fn respond(&self, stream: &TcpStream) -> Option<Response> {
        stream.set_read_timeout(Some(REQUEST_TIMEOUT)).ok()?;
        let mut line = String::new();
        BufReader::new(stream.take(MAX_LINE)).read_line(&mut line).ok()?;
        // Malformed requests get no reply at all.
        let req = protocol::decode_request(&line).ok()?;
        if !protocol::token_matches(&req.token, &self.token) {
            return Some(Response::Rejected);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = mpsc::channel();
        let Some(pending) = self.pending.upgrade() else { return Some(Response::Cancelled) };
        pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        drop(pending);
        let prompt = match req.gpg {
            Some(ask) => Prompt::gpg(ask),
            None => Prompt::new(req.prompt, req.hint),
        };
        (self.on_event)(Event::Asked { id, prompt });
        let reply = match rx.recv_timeout(self.timeout) {
            Ok(reply) => reply,
            Err(RecvTimeoutError::Disconnected) => Reply::Cancel,
            Err(RecvTimeoutError::Timeout) => {
                let removed = self.pending.upgrade().and_then(|p| p.lock().unwrap_or_else(|e| e.into_inner()).remove(&id));
                if removed.is_none() {
                    // Answered just as the time ran out.
                    rx.try_recv().unwrap_or(Reply::Cancel)
                } else {
                    (self.on_event)(Event::Expired { id });
                    return Some(Response::TimedOut);
                }
            }
        };
        Some(match reply {
            Reply::Answer(s) => Response::Answer(s.expose().to_owned()),
            Reply::Cancel => Response::Cancelled,
            Reply::UseSystem => Response::UseSystem,
        })
    }
}
