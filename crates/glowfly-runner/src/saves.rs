//! Saving Glowfly's own files (settings, recent repositories, open tabs; `SaveEffect`). Each
//! save runs on its own thread, so each file's saves are ordered: one that was requested before a
//! save that has already been written is dropped, and the newest snapshot always lands last. What
//! a save of recent.txt or tabs.txt did besides writing (a copy kept, a late read) or why it was
//! refused comes back as `RecentMsg::StateFileSaved` or `StateFileBlocked` (`report`).

use super::Runner;
use super::execute::{Cx, Execute};
use super::platform::SaveRefused;
use glowfly_core::persist::LatestOnly;
use glowfly_model::effect::{EffectMeta, SaveEffect};
use glowfly_model::msg::{Msg, RecentMsg, SaveNotes, StateFile};
use std::path::Path;
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
            SaveEffect::Recent(_) | SaveEffect::ClearRecent => &self.recent,
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

/// Writes one save effect into the user's config folder (`Seams::REAL`'s `save`); see
/// [`save_in`].
pub(super) fn save_now(effect: SaveEffect) -> Option<Msg> {
    let dir = glowfly_core::settings::config_dir()?;
    save_in(&dir, effect)
}

/// Writes one save effect into `dir` (settings.json, recent.txt or tabs.txt): a failure comes
/// back as a line for the activity log, and the state files' notes and refusals as [`report`]
/// says.
pub(super) fn save_in(dir: &Path, effect: SaveEffect) -> Option<Msg> {
    let (recent, tabs) = (dir.join("recent.txt"), dir.join("tabs.txt"));
    let (file, clearing, result) = match effect {
        SaveEffect::Settings(settings) => return failed("settings", settings.save_to(&dir.join("settings.json")).err()?),
        SaveEffect::Recent(list) => (StateFile::Recent, false, super::platform::save_recent_to(&recent, &list)),
        SaveEffect::ClearRecent => (StateFile::Recent, true, super::platform::clear_recent_to(&recent)),
        SaveEffect::Tabs { paths, labels, active } => {
            (StateFile::Tabs, false, super::platform::save_tabs_to(&tabs, &paths, &labels, active))
        }
    };
    let what = match file {
        StateFile::Recent => "the recent repositories",
        StateFile::Tabs => "the open tabs",
    };
    report(file, clearing, what, result)
}

/// What a save of recent.txt or tabs.txt tells the app: nothing when it only wrote the file, its
/// notes, the refusal the user should hear of, or a failure for the activity log.
fn report(file: StateFile, clearing: bool, what: &str, result: Result<SaveNotes, SaveRefused>) -> Option<Msg> {
    match result {
        Ok(notes) if notes == SaveNotes::default() => None,
        Ok(notes) => Some(Msg::Recent(RecentMsg::StateFileSaved { file, notes })),
        Err(SaveRefused::Unread { detail, report: true }) => Some(Msg::Recent(RecentMsg::StateFileBlocked { file, clearing, detail })),
        Err(SaveRefused::Unread { detail, report: false }) => {
            tracing::debug!("not saving {what}: {detail}");
            None
        }
        Err(SaveRefused::Io(e)) => failed(what, e),
    }
}

/// A save that failed: a line for the activity log.
fn failed(what: &str, e: std::io::Error) -> Option<Msg> {
    tracing::warn!("could not save {what}: {e}");
    Some(Msg::Logged { ok: false, text: format!("Could not save {what}: {e}") })
}

#[cfg(test)]
#[path = "saves_tests.rs"]
mod tests;
