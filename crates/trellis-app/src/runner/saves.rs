//! Saving Trellis's own files (settings, recent repositories, open tabs). Each save runs on its
//! own thread, so each file's saves are ordered: one that was requested before a save that has
//! already been written is dropped, and the newest snapshot always lands last.

use super::{Runner, Waker};
use crate::effect::Effect;
use crate::msg::Msg;
use std::sync::Arc;
use trellis_core::persist::LatestOnly;

#[derive(Default)]
pub(super) struct Saves {
    settings: Arc<LatestOnly>,
    recent: Arc<LatestOnly>,
    tabs: Arc<LatestOnly>,
}

impl Saves {
    fn order(&self, effect: &Effect) -> Option<&Arc<LatestOnly>> {
        match effect {
            Effect::SaveSettings(_) => Some(&self.settings),
            Effect::SaveRecent(_) => Some(&self.recent),
            Effect::SaveTabs { .. } => Some(&self.tabs),
            _ => None,
        }
    }
}

impl<W: Waker + Clone> Runner<W> {
    /// Starts a save effect, or hands any other effect back.
    pub(super) fn save(&self, effect: Effect) -> Option<Effect> {
        let Some(order) = self.saves.order(&effect) else { return Some(effect) };
        let (order, ticket) = (order.clone(), order.ticket());
        let (tx, waker) = (self.tx.clone(), self.waker.clone());
        let name = effect.name();
        #[cfg(test)]
        let busy = self.tally.begin(name);
        let spawned = std::thread::Builder::new().name(format!("trellis-{name}")).spawn(move || {
            if let Some(msg) = order.write(ticket, || save_now(effect)).flatten()
                && tx.send(msg).is_ok()
            {
                waker.wake();
            }
            #[cfg(test)]
            drop(busy);
        });
        if let Err(e) = spawned {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
        None
    }
}

/// Writes one save effect; a failure comes back as a line for the activity log.
pub(super) fn save_now(effect: Effect) -> Option<Msg> {
    let (what, result) = match effect {
        Effect::SaveSettings(settings) => ("settings", settings.save()),
        Effect::SaveRecent(list) => ("the recent repositories", crate::platform::save_recent(&list)),
        Effect::SaveTabs { paths, labels, active } => ("the open tabs", crate::platform::save_tabs(&paths, &labels, active)),
        _ => return None,
    };
    let e = result.err()?;
    tracing::warn!("could not save {what}: {e}");
    Some(Msg::Logged { ok: false, text: format!("Could not save {what}: {e}") })
}
