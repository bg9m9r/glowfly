//! The helper's side: sends one prompt to the app and waits for the answer.

use super::protocol::{self, ENV_ADDR, ENV_TOKEN, MAX_LINE, Response, SSH_HINT_ENV};
use super::{PROMPT_TIMEOUT, Secret};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How a prompt ended.
#[derive(Debug)]
pub enum Outcome {
    Answer(Secret),
    Cancelled,
    TimedOut,
}

#[derive(thiserror::Error, Debug)]
pub enum HelperError {
    #[error("{ENV_ADDR} and {ENV_TOKEN} are not set")]
    NotConfigured,
    #[error("could not connect: {0}")]
    Unreachable(io::Error),
    #[error("the connection closed before an answer arrived")]
    Closed,
    #[error("{0}")]
    Protocol(String),
}

/// Asks the app at `addr` to show `prompt`, waiting up to `wait` for the answer.
pub fn ask(addr: &str, token: &str, prompt: &str, hint: Option<&str>, wait: Duration) -> Result<Outcome, HelperError> {
    let addr: SocketAddr = addr.parse().map_err(|_| HelperError::Protocol(format!("{ENV_ADDR} is not an address: {addr}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(HelperError::Unreachable)?;
    stream.set_read_timeout(Some(wait)).map_err(HelperError::Unreachable)?;
    stream
        .write_all(protocol::encode_request(token, prompt, hint).as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|_| HelperError::Closed)?;
    let mut line = String::new();
    match BufReader::new((&stream).take(MAX_LINE)).read_line(&mut line) {
        Ok(0) => return Err(HelperError::Closed),
        Ok(_) => {}
        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => return Ok(Outcome::TimedOut),
        Err(_) => return Err(HelperError::Closed),
    }
    match protocol::decode_response(&line).map_err(|e| HelperError::Protocol(e.to_string()))? {
        Response::Answer(s) => Ok(Outcome::Answer(Secret::new(s))),
        Response::Cancelled => Ok(Outcome::Cancelled),
        Response::TimedOut => Ok(Outcome::TimedOut),
        Response::Rejected => Err(HelperError::Protocol("the app refused the request (wrong token)".into())),
    }
}

/// [`ask`] with the address, token and ssh hint from the environment gitcli set up.
pub fn ask_from_env(prompt: &str) -> Result<Outcome, HelperError> {
    let addr = std::env::var(ENV_ADDR).ok().filter(|v| !v.is_empty());
    let token = std::env::var(ENV_TOKEN).ok().filter(|v| !v.is_empty());
    let (Some(addr), Some(token)) = (addr, token) else { return Err(HelperError::NotConfigured) };
    let hint = std::env::var(SSH_HINT_ENV).ok();
    ask(&addr, &token, prompt, hint.as_deref(), PROMPT_TIMEOUT + Duration::from_secs(30))
}
