//! The running window's side: a loopback listener that reports each link or folder it is
//! handed, started once this window holds the claim (`claim.rs`).

use super::claim::{self, Claim};
use super::file::{self, InstanceFile};
use super::{Handed, MAX_LINE, REQUEST_TIMEOUT, Request, Response};
use crate::askpass::protocol::{new_token, token_matches};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The listener and its instance file, or the wait for another window to close first. Dropping
/// it stops listening, removes the file unless another window has written its own there since,
/// and gives the claim up.
pub struct Listener {
    slot: Arc<Mutex<Slot>>,
}

#[derive(Default)]
struct Slot {
    /// The listener was dropped: a claim that comes later is given up at once.
    closed: bool,
    active: Option<Active>,
}

struct Active {
    file: PathBuf,
    token: String,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    /// Held until this window stops listening.
    _claim: Claim,
}

fn locked(slot: &Mutex<Slot>) -> MutexGuard<'_, Slot> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Listener {
    /// `127.0.0.1:<port>`, once listening.
    pub fn addr(&self) -> Option<SocketAddr> {
        locked(&self.slot).active.as_ref().map(|a| a.addr)
    }

    /// Whether this window takes links and folders now (false while another window holds the
    /// claim).
    pub fn is_listening(&self) -> bool {
        locked(&self.slot).active.is_some()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let mut slot = locked(&self.slot);
        slot.closed = true;
        if let Some(a) = slot.active.take() {
            a.stop.store(true, Ordering::SeqCst);
            // Wakes the thread waiting in accept, so it sees the flag and ends.
            let _ = TcpStream::connect_timeout(&a.addr, super::CONNECT_TIMEOUT);
            file::remove_if_ours(&a.file, &a.token);
        }
    }
}

/// Listens with the instance file in the runtime folder: now, with the claim
/// [`claim`](super::claim) took, else once the window holding it closes.
pub fn serve(on_open: impl Fn(Handed) + Send + 'static) -> io::Result<Listener> {
    let file = super::default_file().ok_or_else(|| io::Error::other("there is no folder for the instance file (HOME isn't set)"))?;
    serve_at(&file, claim::take_held(), on_open)
}

/// With `claim`, listens on a new loopback port now, writes `file`, and calls `on_open` (on the
/// listener's thread) with each item a connection with the right token sends. Without, a
/// thread waits for the lock next to `file` and does the same once it has it. Connections are
/// served one after another; one that stalls is cut off after `REQUEST_TIMEOUT`.
pub fn serve_at(file: &Path, claim: Option<Claim>, on_open: impl Fn(Handed) + Send + 'static) -> io::Result<Listener> {
    let slot = Arc::new(Mutex::new(Slot::default()));
    match claim {
        Some(claim) => locked(&slot).active = Some(start(file, claim, on_open)?),
        None => {
            // Opened here, so a folder that can't hold the lock fails now rather than on the
            // thread.
            let lock = claim::open_lock(&claim::lock_path(file))?;
            let (waiting, file) = (slot.clone(), file.to_path_buf());
            std::thread::Builder::new().name("glowfly-instance-wait".into()).spawn(move || {
                let claim = match claim::wait_for(lock) {
                    Ok(c) => c,
                    Err(e) => return tracing::warn!("links and folders won't reach this window: {e}"),
                };
                let mut slot = locked(&waiting);
                if slot.closed {
                    return;
                }
                match start(&file, claim, on_open) {
                    Ok(active) => {
                        tracing::debug!("the other Glowfly window closed: links and folders come here now");
                        slot.active = Some(active);
                    }
                    Err(e) => tracing::warn!("links and folders won't reach this window: {e}"),
                }
            })?;
        }
    }
    Ok(Listener { slot })
}

/// Listens on a new loopback port and writes `file` naming it.
fn start(file: &Path, claim: Claim, on_open: impl Fn(Handed) + Send + 'static) -> io::Result<Active> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let token = new_token()?;
    file::write(file, &InstanceFile { addr: addr.to_string(), token: token.clone(), pid: std::process::id() })?;
    let stop = Arc::new(AtomicBool::new(false));
    let (stopped, expected) = (stop.clone(), token.clone());
    let started = std::thread::Builder::new().name("glowfly-instance".into()).spawn(move || {
        for stream in listener.incoming() {
            if stopped.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(stream) = stream
                && let Some(item) = answer(&stream, &expected)
            {
                on_open(item);
            }
        }
    });
    if let Err(e) = started {
        file::remove_if_ours(file, &token);
        return Err(e);
    }
    Ok(Active { file: file.to_path_buf(), token, addr, stop, _claim: claim })
}

/// Reads one request and replies; the item when the token matched and the item is well-formed.
fn answer(stream: &TcpStream, token: &str) -> Option<Handed> {
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(stream.take(MAX_LINE)).read_line(&mut line).ok()?;
    // Malformed requests (a line cut off at the limit among them) get no reply at all.
    let req: Request = serde_json::from_str(line.trim_end()).ok()?;
    let item = if token_matches(&req.token, token) { req.item() } else { None };
    let mut reply = serde_json::to_string(&Response { ok: item.is_some() }).ok()?;
    reply.push('\n');
    let mut stream = stream;
    let _ = stream.write_all(reply.as_bytes()).and_then(|()| stream.flush());
    item
}
