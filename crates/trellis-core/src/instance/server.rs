//! The running window's side: a loopback listener that reports each link it is handed.

use super::file::{self, InstanceFile};
use super::{MAX_LINE, REQUEST_TIMEOUT, Request, Response};
use crate::askpass::protocol::{new_token, token_matches};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The listener and its instance file. Dropping it stops listening and removes the file, unless
/// another window has written its own there since.
pub struct Listener {
    file: PathBuf,
    token: String,
    addr: SocketAddr,
    closed: Arc<AtomicBool>,
}

impl Listener {
    /// `127.0.0.1:<port>`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        // Wakes the thread waiting in accept, so it sees the flag and ends.
        let _ = TcpStream::connect_timeout(&self.addr, super::CONNECT_TIMEOUT);
        file::remove_if_ours(&self.file, &self.token);
    }
}

/// Listens, with the instance file in the runtime folder.
pub fn serve(on_open: impl Fn(String) + Send + Sync + 'static) -> io::Result<Listener> {
    let file = super::default_file().ok_or_else(|| io::Error::other("there is no folder for the instance file (HOME isn't set)"))?;
    serve_at(&file, on_open)
}

/// Listens on a new loopback port, writes `file`, and calls `on_open` (on the listener's
/// thread) with each link a connection with the right token sends. Connections are served one
/// after another; one that stalls is cut off after `REQUEST_TIMEOUT`.
pub fn serve_at(file: &Path, on_open: impl Fn(String) + Send + Sync + 'static) -> io::Result<Listener> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let token = new_token()?;
    file::write(file, &InstanceFile { addr: addr.to_string(), token: token.clone(), pid: std::process::id() })?;
    let closed = Arc::new(AtomicBool::new(false));
    let (stop, expected) = (closed.clone(), token.clone());
    let started = std::thread::Builder::new().name("trellis-instance".into()).spawn(move || {
        for stream in listener.incoming() {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(stream) = stream
                && let Some(link) = answer(&stream, &expected)
            {
                on_open(link);
            }
        }
    });
    if let Err(e) = started {
        file::remove_if_ours(file, &token);
        return Err(e);
    }
    Ok(Listener { file: file.to_path_buf(), token, addr, closed })
}

/// Reads one request and replies; the link when the token matched.
fn answer(stream: &TcpStream, token: &str) -> Option<String> {
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(stream.take(MAX_LINE)).read_line(&mut line).ok()?;
    // Malformed requests get no reply at all.
    let req: Request = serde_json::from_str(line.trim_end()).ok()?;
    let ok = token_matches(&req.token, token);
    let mut reply = serde_json::to_string(&Response { ok }).ok()?;
    reply.push('\n');
    let mut stream = stream;
    let _ = stream.write_all(reply.as_bytes()).and_then(|()| stream.flush());
    ok.then_some(req.open)
}
