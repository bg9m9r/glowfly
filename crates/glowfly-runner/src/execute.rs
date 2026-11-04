//! Executing one effect on a worker thread: each effect area's `Execute` impl (in this folder's
//! `<area>.rs`) and the arms of the flat variants, with the result message each sends back.

use super::{forge_calls, git_ops, history, path_search, revert};
use glowfly_core::{avatars, gitcli, repo};
use glowfly_model::effect::Effect;
use glowfly_model::effect::areas::{effect_areas, effect_match};
use glowfly_model::msg::{AvatarMsg, ForgeMsg, Msg, RecentMsg, RepoMsg};

pub(super) fn err(e: glowfly_core::error::Error) -> String {
    e.to_string()
}

/// What an effect may need from the runner while it runs.
pub(super) struct Cx<'a> {
    /// True once a newer job replaces this one (only `DiffEffect::Highlight` checks it).
    pub cancelled: &'a dyn Fn() -> bool,
    /// The answers repository loads share.
    pub graph_cache: &'a repo::GraphCache,
    /// The stop of an effect that can be cancelled (`Effect::stop_key`).
    pub stop: Option<&'a gitcli::Stop>,
    /// The trust gate a job's git commands go through one by one, with the check it passed
    /// (runner/trust.rs), where hooks follow the working tree.
    pub gate: Option<(&'a super::trust_gate::Gate, &'a glowfly_model::effect::TrustCheck)>,
    /// Sends a message before the effect's result and wakes the UI (an update download's
    /// progress); tests drop them.
    pub send: &'a dyn Fn(Msg),
    /// What the work reaches outside the repositories with (`seams.rs`).
    pub seams: super::seams::Seams,
}

/// Runs one effect area's value to completion (blocking), returning the message to send back.
pub(super) trait Execute {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg>;
}

/// Runs one effect to completion (blocking) with the runner's `Cx` and returns the message to
/// send back, if any. Results that belong to one repository come back as `Msg::Scoped`.
pub(super) fn execute_with(effect: Effect, cx: &Cx<'_>) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect, cx)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(effect: Effect, cx: &Cx<'_>) -> Option<Msg> {
    effect_areas! { effect_match! { effect, |e| Execute::execute(e, cx), {
        Effect::LoadFileHistory { workdir, path, start, limit, folder, generation } => {
            Some(history::walk(&workdir, path, &start, limit, folder, generation))
        }
        Effect::LocalOp { workdir, op } => Some(git_ops::local_done(&workdir, op)),
        Effect::Revert { workdir, req } => Some(revert::run(&workdir, req)),
        Effect::Forge { target, cwd, auth, interactive, connected, call } => {
            Some(Msg::Forge(ForgeMsg::ForgeReplied(forge_calls::call(target, &cwd, auth, interactive, connected, call))))
        }
        Effect::Stats { workdir, oids } => Some(Msg::Repo(RepoMsg::StatsLoaded(repo::commit_stats(&workdir, &oids)))),
        Effect::SearchPaths { workdir, request, terms, oids } => {
            Some(path_search::once(path_search::Request { workdir, request, terms, oids }))
        }
        Effect::FetchAvatars(emails) => Some(Msg::Avatars(AvatarMsg::AvatarsLoaded(
            emails
                .into_iter()
                .map(|e| {
                    let img = avatars::fetch(&e);
                    (e, img)
                })
                .collect(),
        ))),
        save @ (Effect::SaveSettings(_) | Effect::SaveRecent(_) | Effect::SaveTabs { .. }) => (cx.seams.save)(save),
        Effect::CheckRepoFolders(paths) => Some(Msg::Recent(RecentMsg::FoldersChecked(super::platform::check_folders(&paths)))),
        Effect::ReadTabGroups(paths) => {
            let read = paths
                .into_iter()
                .map(|p| {
                    let common = repo::CommonDir::at(&p);
                    (p, common)
                })
                .collect();
            Some(Msg::Tabs(glowfly_model::msg::TabsMsg::GroupsRead(read)))
        }
        // `Runner::run` stops it; there is nothing running here to stop.
        Effect::StopGit(_) => None,
        Effect::Watch(_) | Effect::SweepWatch { .. } | Effect::AnswerPrompt { .. } => {
            tracing::warn!("watch or prompt effect reached a worker; it is handled by Runner::run");
            None
        }
    } } }
}
