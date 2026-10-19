//! Counting the lines added and removed per file of a commit or the working tree
//! (Effect::LineCounts), with libgit2 only.

use super::execute::err;
use crate::effect::LineCountsEffect;
use crate::msg::{Msg, RepoMsg};
use std::sync::Arc;
use trellis_core::repo;

pub(super) fn run(e: LineCountsEffect) -> Msg {
    Msg::Repo(match e {
        LineCountsEffect::Commit { workdir, oid } => {
            RepoMsg::CommitLineCountsLoaded { oid, result: repo::commit_line_counts(&workdir, oid).map(Arc::new).map_err(err) }
        }
        LineCountsEffect::Wip { workdir, generation } => {
            RepoMsg::WipLineCountsLoaded { generation, result: repo::wip_line_counts(&workdir).map(Box::new).map_err(err) }
        }
    })
}
