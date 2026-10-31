//! What reaches this window from outside once it runs: the links and folders a second Glowfly
//! process hands to it (`glowfly_core::instance`), reported as `DeepLinkMsg::Open` and
//! `DeepLinkMsg::OpenFolder`, and what the OS hands over as events (`platform::open_events`),
//! reported as their command-line form would be.

use super::Runner;
use crate::deep_link::DeepLinkMsg;
use crate::msg::Msg;
use glowfly_core::instance::{self, Handed};

impl Runner {
    /// Listens for links and folders (once no other window does); on failure each opens its own
    /// window, as before.
    pub fn start_instance(&mut self) {
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        let started = instance::serve(move |item| {
            if tx.send(handed_msg(item)).is_ok() {
                waker.wake();
            }
        });
        match started {
            Ok(listener) => {
                if !listener.is_listening() {
                    tracing::debug!("another Glowfly window takes links and folders; this one will once it closes");
                }
                self.instance = Some(listener);
            }
            Err(e) => tracing::warn!("links and folders won't reach this window: {e}"),
        }
    }

    /// Folders and links the OS hands this process as events (macOS Apple events), including
    /// those that came before the window: each becomes the message its command-line form would.
    pub fn start_open_events(&mut self) {
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        crate::platform::open_events::attach(Box::new(move |item| {
            if tx.send(item.into_msg()).is_ok() {
                waker.wake();
            }
        }));
    }
}

/// The message for what a second process handed over.
fn handed_msg(item: Handed) -> Msg {
    match item {
        Handed::Link(text) => Msg::DeepLink(DeepLinkMsg::Open(text)),
        Handed::Folder(path) => Msg::DeepLink(DeepLinkMsg::OpenFolder(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_handed_item_becomes_its_open_message() {
        let link = "glowfly://repo?root=abc".to_owned();
        assert!(matches!(handed_msg(Handed::Link(link.clone())), Msg::DeepLink(DeepLinkMsg::Open(t)) if t == link));
        let dir = glowfly_core::paths::temp_dir().join("glowfly-handed");
        assert!(matches!(handed_msg(Handed::Folder(dir.clone())), Msg::DeepLink(DeepLinkMsg::OpenFolder(p)) if p == dir));
    }
}
