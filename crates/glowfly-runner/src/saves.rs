//! Saving Glowfly's own files (settings, recent repositories, open tabs; `SaveEffect`). Each
//! save runs on its own thread, so each file's saves are ordered: one that was requested before a
//! save that has already been written is dropped, and the newest snapshot always lands last.

use super::Runner;
use super::execute::{Cx, Execute};
use glowfly_core::persist::LatestOnly;
use glowfly_model::effect::{EffectMeta, SaveEffect};
use glowfly_model::msg::Msg;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Saves {
    settings: Arc<LatestOnly>,
    recent: Arc<LatestOnly>,
    tabs: Arc<LatestOnly>,
}

impl Saves {
    fn order(&self, effect: &SaveEffect) -> &Arc<LatestOnly> {
        match effect {
            SaveEffect::Settings(_) => &self.settings,
            SaveEffect::Recent(_) => &self.recent,
            SaveEffect::Tabs { .. } => &self.tabs,
        }
    }
}

/// A save that reaches a worker as any other effect would (`testing::execute`): written at once.
impl Execute for SaveEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        (cx.seams.save)(self)
    }
}

impl Runner {
    /// Starts a save on its own thread, behind the earlier saves of the same file.
    pub(super) fn save(&self, effect: SaveEffect) {
        let order = self.saves.order(&effect);
        let (order, ticket) = (order.clone(), order.ticket());
        let (tx, waker, save) = (self.tx.clone(), self.waker.clone(), self.seams.save);
        let name = effect.name();
        let busy = self.tally.begin(name);
        let spawned = std::thread::Builder::new().name(format!("glowfly-{name}")).spawn(move || {
            if let Some(msg) = order.write(ticket, || save(effect)).flatten()
                && tx.send(msg).is_ok()
            {
                waker.wake();
            }
            drop(busy);
        });
        if let Err(e) = spawned {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
    }
}

/// Writes one save effect into the user's config folder (`Seams::REAL`'s `save`); a failure
/// comes back as a line for the activity log.
pub(super) fn save_now(effect: SaveEffect) -> Option<Msg> {
    let (what, result) = match effect {
        SaveEffect::Settings(settings) => ("settings", settings.save()),
        SaveEffect::Recent(list) => ("the recent repositories", super::platform::save_recent(&list)),
        SaveEffect::Tabs { paths, labels, active } => ("the open tabs", super::platform::save_tabs(&paths, &labels, active)),
    };
    let e = result.err()?;
    tracing::warn!("could not save {what}: {e}");
    Some(Msg::Logged { ok: false, text: format!("Could not save {what}: {e}") })
}
