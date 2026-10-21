//! The helper's side: sends one prompt to the app and waits for the answer.

use super::protocol::{self, ENV_ADDR, ENV_SSH_SIGNING_KEY, ENV_TOKEN, GpgAsk, MAX_LINE, Response, SSH_HINT_ENV};
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
    /// The app wants the program to ask in its own way (a signing request: GnuPG's pinentry).
    UseSystem,
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
    exchange(addr, &protocol::encode_request(token, prompt, hint), wait)
}

/// Asks the app at `addr` for an OpenPGP key's signing passphrase, waiting up to `wait`.
pub fn ask_gpg(addr: &str, token: &str, ask: &GpgAsk, wait: Duration) -> Result<Outcome, HelperError> {
    exchange(addr, &protocol::encode_gpg_request(token, ask), wait)
}

/// Sends one request line and reads the response.
fn exchange(addr: &str, request: &str, wait: Duration) -> Result<Outcome, HelperError> {
    let addr: SocketAddr = addr.parse().map_err(|_| HelperError::Protocol(format!("{ENV_ADDR} is not an address: {addr}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(HelperError::Unreachable)?;
    stream.set_read_timeout(Some(wait)).map_err(HelperError::Unreachable)?;
    stream.write_all(request.as_bytes()).and_then(|()| stream.flush()).map_err(|_| HelperError::Closed)?;
    let mut line = String::new();
    match BufReader::new((&stream).take(MAX_LINE)).read_line(&mut line) {
        Ok(0) => return Err(HelperError::Closed),
        Ok(_) => {}
        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => return Ok(Outcome::TimedOut),
        Err(_) => return Err(HelperError::Closed),
    }
    let response = protocol::decode_response(&line).map_err(|e| HelperError::Protocol(e.to_string()));
    super::wipe(&mut line);
    match response? {
        Response::Answer(s) => Ok(Outcome::Answer(Secret::new(s))),
        Response::Cancelled => Ok(Outcome::Cancelled),
        Response::TimedOut => Ok(Outcome::TimedOut),
        Response::UseSystem => Ok(Outcome::UseSystem),
        Response::Rejected => Err(HelperError::Protocol("the app refused the request (wrong token)".into())),
    }
}

/// The listener's address and token from the environment gitcli set up.
fn from_env() -> Result<(String, String), HelperError> {
    let addr = std::env::var(ENV_ADDR).ok().filter(|v| !v.is_empty());
    let token = std::env::var(ENV_TOKEN).ok().filter(|v| !v.is_empty());
    match (addr, token) {
        (Some(addr), Some(token)) => Ok((addr, token)),
        _ => Err(HelperError::NotConfigured),
    }
}

/// How long a helper waits: the app's own timeout, plus time for its answer to arrive.
const HELPER_WAIT: Duration = Duration::from_secs(PROMPT_TIMEOUT.as_secs() + 30);

/// [`ask`] with the address, token and ssh hint from the environment gitcli set up. ssh-keygen's
/// prompt in a command gitcli marked as signing with an SSH key goes as a structured request
/// ([`super::ssh_sign::ask_for`]).
pub fn ask_from_env(prompt: &str) -> Result<Outcome, HelperError> {
    let (addr, token) = from_env()?;
    if let Some(ask) = super::ssh_sign::ask_for(prompt, std::env::var(ENV_SSH_SIGNING_KEY).ok().as_deref()) {
        return exchange(&addr, &protocol::encode_ssh_sign_request(&token, &ask), HELPER_WAIT);
    }
    let hint = std::env::var(SSH_HINT_ENV).ok();
    ask(&addr, &token, prompt, hint.as_deref(), HELPER_WAIT)
}

/// [`ask_gpg`] with the address and token from the environment gitcli set up.
pub fn ask_gpg_from_env(ask: &GpgAsk) -> Result<Outcome, HelperError> {
    let (addr, token) = from_env()?;
    ask_gpg(&addr, &token, ask, HELPER_WAIT)
}
