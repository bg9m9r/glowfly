//! The running window's side: a loopback listener that reports each link or folder it is
//! handed, started once this window holds the claim (`claim.rs`).

use super::claim::{self, Claim};
use super::file::{self, InstanceFile};
use super::{Handed, MAX_LINE, REQUEST_TIMEOUT, Request, Response};
use crate::askpass::protocol::{new_token, token_matches};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

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

/// With `claim`, listens on a new loopback port now, writes `file`, and calls `on_open`
/// with each item a connection with the right token sends. Without, a thread waits for the lock next to `file` and does the same once it has it. Each connection is
/// served on its own short-lived thread (up to `MAX_CONNECTIONS` at once) and gets
/// `REQUEST_TIMEOUT` for the whole exchange; `on_open` is called one item at a time.
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

/// How many connections are served at once; one over this is dropped.
const MAX_CONNECTIONS: usize = 8;

/// Listens on a new loopback port and writes `file` naming it.
fn start(file: &Path, claim: Claim, on_open: impl Fn(Handed) + Send + 'static) -> io::Result<Active> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let token = new_token()?;
    file::write(file, &InstanceFile { addr: addr.to_string(), token: token.clone(), pid: std::process::id() })?;
    let stop = Arc::new(AtomicBool::new(false));
    let (stopped, expected) = (stop.clone(), token.clone());
    let on_open = Arc::new(Mutex::new(on_open));
    let live = Arc::new(AtomicUsize::new(0));
    let started = std::thread::Builder::new().name("glowfly-instance".into()).spawn(move || {
        for stream in listener.incoming() {
            if stopped.load(Ordering::SeqCst) {
                return;
            }
            let Ok(stream) = stream else { continue };
            // Over the cap the new connection is dropped; its peer sees it close.
            let Some(slot) = ConnSlot::take(&live) else { continue };
            let (expected, on_open) = (expected.clone(), on_open.clone());
            let spawned = std::thread::Builder::new().name("glowfly-instance-conn".into()).spawn(move || {
                let _slot = slot;
                if let Some(item) = answer(&stream, &expected, Instant::now() + REQUEST_TIMEOUT) {
                    (on_open.lock().unwrap_or_else(PoisonError::into_inner))(item);
                }
            });
            if let Err(e) = spawned {
                tracing::warn!("a connection to this window was dropped: {e}");
            }
        }
    });
    if let Err(e) = started {
        file::remove_if_ours(file, &token);
        return Err(e);
    }
    Ok(Active { file: file.to_path_buf(), token, addr, stop, _claim: claim })
}

/// One of the [`MAX_CONNECTIONS`] places for a connection being served; given back on drop.
struct ConnSlot(Arc<AtomicUsize>);

impl ConnSlot {
    fn take(live: &Arc<AtomicUsize>) -> Option<Self> {
        let taken = live.fetch_add(1, Ordering::SeqCst);
        if taken >= MAX_CONNECTIONS {
            live.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(live.clone()))
    }
}

impl Drop for ConnSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Reads one line of at most [`MAX_LINE`] bytes, giving up once `deadline` passes: each read waits
/// only for the time that is left. None when the line doesn't come whole in time.
fn read_request(mut stream: &TcpStream, deadline: Instant) -> Option<String> {
    let mut line = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
        stream.set_read_timeout(Some(left)).ok()?;
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => {
                line.extend_from_slice(&chunk[..i]);
                break;
            }
            None => line.extend_from_slice(chunk),
        }
        if line.len() as u64 >= MAX_LINE {
            return None;
        }
    }
    if line.len() as u64 >= MAX_LINE {
        return None;
    }
    String::from_utf8(line).ok()
}

/// Reads one request and replies, all before `deadline`; the item when the token matched and the
/// item is well-formed.
fn answer(stream: &TcpStream, token: &str, deadline: Instant) -> Option<Handed> {
    let line = read_request(stream, deadline)?;
    // Malformed requests (a line cut off at the limit among them) get no reply at all.
    let req: Request = serde_json::from_str(line.trim_end()).ok()?;
    let item = if token_matches(&req.token, token) { req.item() } else { None };
    let mut reply = serde_json::to_string(&Response { ok: item.is_some() }).ok()?;
    reply.push('\n');
    let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
    stream.set_write_timeout(Some(left)).ok()?;
    let mut stream = stream;
    let _ = stream.write_all(reply.as_bytes()).and_then(|()| stream.flush());
    item
}
