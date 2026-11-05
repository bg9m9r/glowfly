//! Counting the lines added and removed per file of a commit or the working tree
//! (Effect::LineCounts), with libgit2 only.

use super::execute::{Cx, Execute};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo;
use glowfly_model::effect::LineCountsEffect;
use glowfly_model::msg::{Msg, RepoMsg};
use std::sync::Arc;

pub(super) fn run(e: LineCountsEffect) -> Msg {
    Msg::Repo(match e {
        LineCountsEffect::Commit { workdir, oid } => {
            RepoMsg::CommitLineCountsLoaded { oid, result: repo::commit_line_counts(&workdir, oid).map(Arc::new).map_err(ReadFailed::from) }
        }
        LineCountsEffect::Wip { workdir, generation } => {
            RepoMsg::WipLineCountsLoaded { generation, result: repo::wip_line_counts(&workdir).map(Box::new).map_err(ReadFailed::from) }
        }
    })
}

impl Execute for LineCountsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
