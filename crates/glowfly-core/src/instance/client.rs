//! The second process's side: hands a link to the running window.

use super::file;
use super::{CONNECT_TIMEOUT, MAX_LINE, REQUEST_TIMEOUT, Request, Response};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum HandOffError {
    /// No window is listening: no instance file, or its port is closed.
    #[error("no Glowfly window is running")]
    NotRunning,
    /// The window answered, but the token didn't match (an old file).
    #[error("the running Glowfly refused the link")]
    Rejected,
    #[error("couldn't reach the running Glowfly: {0}")]
    Io(#[from] io::Error),
}

/// Hands `link` to the window the runtime folder's instance file names.
pub fn hand_off(link: &str) -> Result<(), HandOffError> {
    let file = super::default_file().ok_or(HandOffError::NotRunning)?;
    hand_off_via(&file, link)
}

fn invalid(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text.to_owned())
}

/// Hands `link` to the window `file` names, waiting for its answer.
pub fn hand_off_via(file: &Path, link: &str) -> Result<(), HandOffError> {
    let info = match file::read(file) {
        Ok(info) => info,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(HandOffError::NotRunning),
        Err(e) => return Err(e.into()),
    };
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
    let mut request = serde_json::to_string(&Request { token: info.token, open: link.to_owned() }).map_err(io::Error::other)?;
    request.push('\n');
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    let mut line = String::new();
    if BufReader::new((&stream).take(MAX_LINE)).read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the running Glowfly closed the connection").into());
    }
    let response: Response = serde_json::from_str(line.trim_end()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if response.ok { Ok(()) } else { Err(HandOffError::Rejected) }
}
