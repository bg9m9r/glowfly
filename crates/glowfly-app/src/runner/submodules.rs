//! The SUBMODULES section's effects (`Effect::Submodule`): reading one submodule (or all, for a
//! batch update) again before its update or removal is confirmed, checking the Add dialog's folder, and removing one.

use super::execute::{Cx, Execute};
use crate::effect::SubmoduleEffect;
use crate::msg::Msg;
use crate::submodule::SubmoduleMsg;
use glowfly_core::{repo, submodule_ops};

pub(super) fn run(e: SubmoduleEffect) -> Msg {
    Msg::Submodule(match e {
        SubmoduleEffect::Check { workdir, path } => {
            let result = repo::submodules_at(&workdir).map(|all| all.into_iter().find(|m| m.path == path)).map_err(|e| e.to_string());
            SubmoduleMsg::Checked { path, result }
        }
        SubmoduleEffect::CheckAddPath { repo, path, delay } => {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            let result = repo::check_submodule_path(&repo, &path).map_err(|p| p.message(&path));
            SubmoduleMsg::AddPathChecked { repo, path, result }
        }
        SubmoduleEffect::CheckAll { workdir, batch } => {
            SubmoduleMsg::CheckedAll { batch, result: repo::submodules_at(&workdir).map_err(|e| e.to_string()) }
        }
        SubmoduleEffect::CheckRemove { workdir, path } => {
            let result = submodule_ops::removal_facts(&workdir, &path).map(|o| o.map(Box::new)).map_err(|e| e.to_string());
            SubmoduleMsg::RemoveChecked { path, result }
        }
        SubmoduleEffect::Remove(r) => {
            let report = submodule_ops::remove(&r.repo, &r.path, &r.name);
            SubmoduleMsg::Removed { request: r, report }
        }
    })
}

impl Execute for SubmoduleEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
