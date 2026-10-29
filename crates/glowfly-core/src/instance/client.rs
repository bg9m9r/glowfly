//! The second process's side: hands a link or folder to the running window.

use super::file;
use super::{CONNECT_TIMEOUT, Handed, MAX_LINE, REQUEST_TIMEOUT, Request, Response};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

/// Between tries while the running window is still starting.
const RETRY: Duration = Duration::from_millis(100);

#[derive(thiserror::Error, Debug)]
pub enum HandOffError {
    /// No window is listening: no instance file, or its port is closed.
    #[error("no Glowfly window is running")]
    NotRunning,
    /// The window answered, but the token didn't match (an old file) or it refused the item.
    #[error("the running Glowfly refused it")]
    Rejected,
    #[error("couldn't reach the running Glowfly: {0}")]
    Io(#[from] io::Error),
}

/// Hands `item` to the window the runtime folder's instance file names, trying for up to
/// `wait` while it isn't listening yet.
pub fn hand_off(item: &Handed, wait: Duration) -> Result<(), HandOffError> {
    let file = super::default_file().ok_or(HandOffError::NotRunning)?;
    hand_off_waiting(&file, item, wait)
}

/// [`hand_off_via`], tried again until `wait` has passed while no window answers or one
/// refuses: a window that holds the claim may not have written its file or opened its port
/// yet, and the file may still be an older window's. Other errors end it at once.
pub fn hand_off_waiting(file: &Path, item: &Handed, wait: Duration) -> Result<(), HandOffError> {
    let deadline = Instant::now() + wait;
    loop {
        match hand_off_via(file, item) {
            Err(HandOffError::NotRunning | HandOffError::Rejected) if Instant::now() + RETRY < deadline => std::thread::sleep(RETRY),
            done => return done,
        }
    }
}

fn invalid(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text.to_owned())
}

/// Hands `item` to the window `file` names, waiting for its answer.
pub fn hand_off_via(file: &Path, item: &Handed) -> Result<(), HandOffError> {
    let info = match file::read(file) {
        Ok(info) => info,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(HandOffError::NotRunning),
        Err(e) => return Err(e.into()),
    };
    let mut request = serde_json::to_string(&Request::new(info.token, item)?).map_err(io::Error::other)?;
    request.push('\n');
    if request.len() as u64 > MAX_LINE {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "too long to hand over").into());
    }
    let addr: SocketAddr = info.addr.parse().map_err(|_| invalid("the instance file's address is malformed"))?;
    if !addr.ip().is_loopback() {
        return Err(invalid("the instance file names an address on another machine").into());
    }
    super::foreground::allow(info.pid);
    let mut stream = match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
        Ok(s) => s,
        // Windows retries a refused loopback connection for longer than the timeout.
        Err(e) if matches!(e.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::TimedOut) => {
            return Err(HandOffError::NotRunning);
        }
        Err(e) => return Err(e.into()),
    };
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    let mut line = String::new();
    if BufReader::new((&stream).take(MAX_LINE)).read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the running Glowfly closed the connection").into());
    }
    let response: Response = serde_json::from_str(line.trim_end()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if response.ok { Ok(()) } else { Err(HandOffError::Rejected) }
}
