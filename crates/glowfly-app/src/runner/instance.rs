//! What reaches this window from outside once it runs: the links a second Glowfly process hands
//! to it (`glowfly_core::instance`), reported as `DeepLinkMsg::Open`, and what the OS hands over
//! as events (`platform::open_events`), reported as their command-line form would be.

use super::{Runner, Waker};
use crate::deep_link::DeepLinkMsg;
use crate::msg::Msg;
use glowfly_core::instance;

impl<W: Waker + Clone> Runner<W> {
    /// Listens for links; on failure every link opens its own window, as before.
    pub fn start_instance(&mut self) {
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        let started = instance::serve(move |text| {
            if tx.send(Msg::DeepLink(DeepLinkMsg::Open(text))).is_ok() {
                waker.wake();
            }
        });
        match started {
            Ok(listener) => self.instance = Some(listener),
            Err(e) => tracing::warn!("links won't reach this window: {e}"),
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
