//! The line protocol between the helper process and the app's listener: one JSON request line
//! from the helper, one JSON response line back. JSON escaping keeps multi-line prompts (ssh's
//! host-key question) on one line.

use serde::{Deserialize, Serialize};

/// The listener's address (`127.0.0.1:<port>`); its presence puts the executable in helper mode.
pub const ENV_ADDR: &str = "TRELLIS_ASKPASS";
/// The token the listener expects (32 lowercase hex characters).
pub const ENV_TOKEN: &str = "TRELLIS_ASKPASS_TOKEN";
/// ssh's hint for the kind of prompt: `confirm` (yes/no) or `none` (a notice; no answer read).
pub const SSH_HINT_ENV: &str = "SSH_ASKPASS_PROMPT";

/// The longest line either side reads.
pub const MAX_LINE: u64 = 64 * 1024;

const VERSION: u32 = 1;

/// What the gpg.program wrapper asks for (see [`crate::gpg_prompt`]): the passphrase of the
/// OpenPGP key `key` (its long id, or the spec git passed), owned by `user` when gpg could list
/// it. `retry` when the previous answer didn't unlock it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpgAsk {
    pub key: String,
    pub user: Option<String>,
    pub retry: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    pub token: String,
    pub prompt: String,
    pub hint: Option<String>,
    /// Set by the signing wrapper; absent from askpass requests (and from older helpers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpg: Option<GpgAsk>,
}

/// No `Debug`: an answer is a secret.
#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Answer(String),
    Cancelled,
    TimedOut,
    /// The request's token didn't match; nothing was shown.
    Rejected,
    /// Let the program ask in its own way (GnuPG's own pinentry); nothing was shown.
    UseSystem,
}

#[derive(thiserror::Error, Debug)]
pub enum ProtocolError {
    #[error("malformed message: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported protocol version {0}")]
    Version(u32),
    #[error("message longer than {MAX_LINE} bytes")]
    TooLong,
}

fn line<T: Serialize>(msg: &T) -> String {
    let mut s = serde_json::to_string(msg).expect("protocol messages serialize");
    s.push('\n');
    s
}

fn check_len(s: &str) -> Result<(), ProtocolError> {
    if s.len() as u64 > MAX_LINE { Err(ProtocolError::TooLong) } else { Ok(()) }
}

pub fn encode_request(token: &str, prompt: &str, hint: Option<&str>) -> String {
    line(&Request { v: VERSION, token: token.into(), prompt: prompt.into(), hint: hint.map(str::to_owned), gpg: None })
}

/// A signing-passphrase request; its `prompt` is the text [`super::Prompt::gpg`] gives.
pub fn encode_gpg_request(token: &str, ask: &GpgAsk) -> String {
    let prompt = super::Prompt::gpg(ask.clone()).text;
    line(&Request { v: VERSION, token: token.into(), prompt, hint: None, gpg: Some(ask.clone()) })
}

pub fn decode_request(s: &str) -> Result<Request, ProtocolError> {
    check_len(s)?;
    let req: Request = serde_json::from_str(s.trim_end())?;
    if req.v != VERSION {
        return Err(ProtocolError::Version(req.v));
    }
    Ok(req)
}

pub fn encode_response(r: &Response) -> String {
    line(r)
}

pub fn decode_response(s: &str) -> Result<Response, ProtocolError> {
    check_len(s)?;
    Ok(serde_json::from_str(s.trim_end())?)
}

/// Compares two tokens in time that depends only on their length.
pub fn token_matches(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 128 random bits as 32 lowercase hex characters.
pub fn new_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
