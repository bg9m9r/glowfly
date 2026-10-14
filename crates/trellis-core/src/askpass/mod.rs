//! Credential prompts through an askpass helper.
//!
//! git and ssh ask for missing credentials by running the program named in `GIT_ASKPASS` /
//! `SSH_ASKPASS` with the prompt text as its only argument, and read the answer from its
//! standard output. Trellis names its own executable there:
//!
//! 1. At startup the app opens a loopback listener ([`server::start`]) with a random token.
//! 2. `gitcli` passes the executable, the listener's address and the token to every git child
//!    through environment variables ([`protocol::ENV_ADDR`], [`protocol::ENV_TOKEN`]).
//! 3. git or ssh runs `trellis "<prompt>"`. Seeing the address variable, the executable runs as
//!    a helper instead of opening a window: it connects ([`client::ask_from_env`]), sends one
//!    JSON line with the token and the prompt, waits for one JSON line back, prints the answer
//!    and exits 0, or exits non-zero when the prompt was cancelled or timed out.
//! 4. The listener reports each prompt as an [`server::Event`]; the app shows a dialog and
//!    hands the user's [`Reply`] back with [`server::Server::answer`].
//!
//! Classification of the prompt text ([`prompt::classify`]) is pure and decides which dialog
//! is shown.

pub mod client;
pub mod prompt;
pub mod protocol;
pub mod server;

#[cfg(test)]
mod prompt_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod server_tests;

use std::time::Duration;

pub use client::{HelperError, Outcome, ask, ask_from_env};
pub use prompt::{PromptKind, classify, host, user_and_host};
pub use protocol::{ENV_ADDR, ENV_TOKEN, SSH_HINT_ENV};
pub use server::{Event, Server};

/// How long a prompt waits for an answer before it is closed as timed out.
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(300);

/// Something the user typed for git: a password, passphrase or token. Its `Debug` output is
/// redacted, and its bytes are overwritten when it is dropped.
#[derive(Clone, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Empties `s`, overwriting its bytes first (best effort: earlier reallocations are not
/// reached).
pub fn wipe(s: &mut String) {
    let mut bytes = std::mem::take(s).into_bytes();
    bytes.iter_mut().for_each(|b| *b = 0);
    std::hint::black_box(&bytes);
}

/// The app's answer to one prompt.
#[derive(Debug)]
pub enum Reply {
    Answer(Secret),
    Cancel,
}

/// One prompt from git or ssh: its text, ssh's `SSH_ASKPASS_PROMPT` hint, and what it asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub text: String,
    pub hint: Option<String>,
    pub kind: PromptKind,
}

impl Prompt {
    pub fn new(text: impl Into<String>, hint: Option<String>) -> Self {
        let text = text.into();
        let kind = classify(&text, hint.as_deref());
        Prompt { text, hint, kind }
    }
}
