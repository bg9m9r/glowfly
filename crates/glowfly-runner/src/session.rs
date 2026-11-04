//! The `Execute` impl of `SessionEffect`: which recent repositories' folders are gone
//! (`platform::check_folders`), and the common dir of each restored tab's folder.

use super::execute::{Cx, Execute};
use glowfly_core::repo;
use glowfly_model::effect::SessionEffect;
use glowfly_model::msg::{Msg, RecentMsg, TabsMsg};

impl Execute for SessionEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            SessionEffect::CheckRepoFolders(paths) => Msg::Recent(RecentMsg::FoldersChecked(super::platform::check_folders(&paths))),
            SessionEffect::ReadTabGroups(paths) => {
                let read = paths
                    .into_iter()
                    .map(|p| {
                        let common = repo::CommonDir::at(&p);
                        (p, common)
                    })
                    .collect();
                Msg::Tabs(TabsMsg::GroupsRead(read))
            }
        })
    }
}
