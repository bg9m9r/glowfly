//! The listener for links that a second Trellis process hands to this window
//! (`glowfly_core::instance`): started once with the app, it reports each link as
//! `DeepLinkMsg::Open`.

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
}
