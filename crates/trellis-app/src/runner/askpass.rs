//! The credential-prompt listener: started once with the app, it reports prompts from the
//! askpass helper as `Msg`s and hands the dialog's answers back to the waiting helper.

use super::{Runner, Waker};
use crate::msg::CredentialMsg;
use crate::msg::Msg;
use trellis_core::askpass::{self, Event, Reply};
use trellis_core::gitcli::{self, AskpassEnv};

impl<W: Waker + Clone> Runner<W> {
    /// Opens the loopback listener and points every later git child at it. On failure prompts
    /// fail as before (git has no terminal to ask on).
    pub fn start_askpass(&mut self) {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => return tracing::warn!("credential prompts are off: the executable's path is unknown: {e}"),
        };
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        let started = askpass::server::start(move |ev| {
            let msg = match ev {
                Event::Asked { id, prompt } => Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt }),
                Event::Expired { id } => Msg::Credentials(CredentialMsg::CredentialPromptExpired(id)),
            };
            if tx.send(msg).is_ok() {
                waker.wake();
            }
        });
        match started {
            Ok(server) => {
                gitcli::enable_askpass(AskpassEnv { exe, addr: server.addr(), token: server.token().into() });
                self.askpass = Some(server);
            }
            Err(e) => tracing::warn!("credential prompts are off: could not open the listener: {e}"),
        }
    }

    /// Hands `reply` to the helper waiting on prompt `id`, if it still is.
    pub(super) fn answer_prompt(&self, id: u64, reply: Reply) {
        if !self.askpass.as_ref().is_some_and(|s| s.answer(id, reply)) {
            tracing::debug!(id, "credential prompt already closed");
        }
    }
}
