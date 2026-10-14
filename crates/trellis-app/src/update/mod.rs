//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod askpass;
mod avatars;
mod branches;
mod commit;
mod conflict;
mod diff;
mod files;
mod forge;
mod graph_filter;
mod highlight;
mod history;
mod ops;
mod palette;
mod prefs;
mod publish;
mod rebase;
mod repo;
mod selection;
mod tabs;
mod undo;
mod watch;
mod work_files;

#[cfg(test)]
mod askpass_tests;
#[cfg(test)]
mod branch_tests;
#[cfg(test)]
mod cherry_pick_tests;
#[cfg(test)]
mod commit_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod diff_nav_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod fold_tests;
#[cfg(test)]
mod graph_filter_tests;
#[cfg(test)]
mod highlight_context_tests;
#[cfg(test)]
mod highlight_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod palette_more_tests;
#[cfg(test)]
mod palette_pick_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod publish_tests;
#[cfg(test)]
mod rebase_e2e_tests;
#[cfg(test)]
mod rebase_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod tabs_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
#[cfg(test)]
mod watch_tests;
#[cfg(test)]
mod whitespace_tests;
#[cfg(test)]
mod whole_file_tests;
#[cfg(test)]
mod work_files_stage_tests;
#[cfg(test)]
mod work_files_tests;
#[cfg(test)]
mod work_files_unstage_tests;

pub use repo::commit_matches;

use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;

pub fn update(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    let mut fx = Vec::new();
    match msg {
        // repository and selection
        Msg::PickRepo
        | Msg::Open(_)
        | Msg::Refresh
        | Msg::WindowFocused
        | Msg::SelectWip
        | Msg::SelectCommit(_)
        | Msg::JumpTo(_)
        | Msg::OpenDiff(_)
        | Msg::CloseDiff
        | Msg::SearchNext
        | Msg::WantStats(_)
        | Msg::RepoLoaded { .. }
        | Msg::StatusLoaded { .. }
        | Msg::DetailLoaded { .. }
        | Msg::DiffLoaded { .. }
        | Msg::StatsLoaded(_) => repo::handle(st, msg, now, &mut fx),
        Msg::DiffHighlighted { .. } => highlight::handle(st, msg),
        Msg::SetDiffLayout(_)
        | Msg::SetDiffMode(_)
        | Msg::SetIgnoreWhitespace(_)
        | Msg::JumpToChange(_)
        | Msg::ExpandUnchanged(_)
        | Msg::CollapseUnchanged(_)
        | Msg::ExpandAllUnchanged
        | Msg::CollapseAllUnchanged
        | Msg::SelectDiffLine { .. }
        | Msg::ClearLineSelection => diff::handle(st, msg, &mut fx),
        Msg::ToggleCommit(_) | Msg::SelectCommitRange(_) | Msg::CollapseSelection | Msg::CherryPick(_) | Msg::CompareLoaded { .. } => {
            selection::handle(st, msg, &mut fx)
        }
        Msg::WantAvatars(_) | Msg::AvatarsLoaded(_) => avatars::handle(st, msg, &mut fx),

        // conflicts
        Msg::OpenConflict(_)
        | Msg::CloseConflict
        | Msg::ConflictToggle { .. }
        | Msg::ConflictTakeAll(_)
        | Msg::ConflictReset
        | Msg::ConflictStep(_)
        | Msg::ConflictSelect(_)
        | Msg::SaveConflict
        | Msg::ResolveWithSide { .. }
        | Msg::ConflictLoaded { .. } => conflict::handle(st, msg, &mut fx),

        // file history
        Msg::OpenFileHistory { .. }
        | Msg::CloseFileHistory
        | Msg::SelectHistoryEntry(_)
        | Msg::MoreFileHistory
        | Msg::ShowInGraph(_)
        | Msg::FileHistoryLoaded { .. } => history::handle(st, msg, now, &mut fx),

        // interactive rebase
        Msg::OpenRebase(_)
        | Msg::CloseRebase
        | Msg::RebaseSetAction { .. }
        | Msg::RebaseMove { .. }
        | Msg::RebaseEditMessage(_)
        | Msg::RebaseSaveMessage
        | Msg::RebaseCancelMessage
        | Msg::RebaseReset
        | Msg::StartRebase { .. }
        | Msg::RebasePlanLoaded { .. }
        | Msg::RebaseFinished { .. } => rebase::handle(st, msg, now, &mut fx),

        // tabs
        Msg::NewTab
        | Msg::SelectTab(_)
        | Msg::CycleTab(_)
        | Msg::CloseTab(_)
        | Msg::ReopenClosedTab
        | Msg::RestoreTabs { .. }
        | Msg::MoveTab { .. }
        | Msg::CloseOtherTabs(_)
        | Msg::CloseTabsToRight(_) => tabs::handle(st, msg, &mut fx),
        Msg::Scoped { workdir, msg } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                fx.extend(update(st, *msg, now));
            } else if let Msg::LocalOpDone(Err(e)) = *msg {
                st.log(false, e);
            } else if let Msg::HunkReverted { path, result: Err(e) } = *msg {
                st.log(false, format!("Could not revert the hunk in {path}: {e}"));
            } else if let Msg::FileRestored { path, result: Err(e), .. } = *msg {
                st.log(false, format!("Could not restore {path}: {e}"));
            } else if let Msg::RestoreWouldOverwrite { path, reason, .. } = *msg {
                st.log(false, format!("Did not restore {path}: {reason}."));
            } else if let Msg::WorkFileDone { op, result } = *msg {
                work_files::inactive_result(st, op, result, now);
            } else {
                tracing::debug!("dropped a result for {}, which is no longer the active tab", workdir.display());
            }
        }

        // working tree and git commands
        Msg::Tick(_)
        | Msg::StageFile(_)
        | Msg::UnstageFile(_)
        | Msg::StageAll
        | Msg::UnstageAll
        | Msg::DiscardFile(..)
        | Msg::Hunk { .. }
        | Msg::ApplyLines(_)
        | Msg::RevertHunk { .. }
        | Msg::Git { .. }
        | Msg::Push { .. }
        | Msg::InitRepo
        | Msg::CloneRepo { .. }
        | Msg::LocalOpDone(_)
        | Msg::HunkReverted { .. }
        | Msg::JobDone { .. } => ops::handle(st, msg, now, &mut fx),
        Msg::Commit | Msg::StageAllAndCommit | Msg::PrefillAmend | Msg::AppendToDescription(_) | Msg::SetPushAfterCommit(_) => {
            commit::handle(st, msg, now, &mut fx)
        }
        Msg::Undo | Msg::Redo => undo::handle(st, msg, now, &mut fx),
        Msg::RestoreFile { .. }
        | Msg::FileRestored { .. }
        | Msg::RestoreWouldOverwrite { .. }
        | Msg::OpenWorkingFile(_)
        | Msg::ShowWorkingFile(_)
        | Msg::NewFileDialog(_)
        | Msg::CreateFile(_)
        | Msg::RenameFileDialog(_)
        | Msg::RenameFile { .. }
        | Msg::SetRenameStages(_)
        | Msg::DeleteFile { .. }
        | Msg::DeleteStagedNewFile(_)
        | Msg::UnstageGoneFile(_)
        | Msg::WorkFileDone { .. } => files::handle(st, msg, now, &mut fx),
        Msg::RenameBranchDialog(_)
        | Msg::RenameBranch { .. }
        | Msg::SetUpstreamDialog(_)
        | Msg::SetUpstream { .. }
        | Msg::UnsetUpstream(_)
        | Msg::FastForwardBranch(_)
        | Msg::DeleteBranchAndRemote(_)
        | Msg::PushBranch { .. } => branches::handle(st, msg, &mut fx),
        Msg::GraphFilter(_) => graph_filter::handle(st, msg, &mut fx),
        Msg::RepoChanged { .. } | Msg::WatchFailed { .. } => watch::handle(st, msg, now, &mut fx),

        // credential prompts
        Msg::CredentialPrompt { .. } | Msg::CredentialPromptExpired(_) | Msg::AnswerCredentials | Msg::CancelCredentials => {
            askpass::handle(st, msg, now, &mut fx)
        }

        // hosting services
        Msg::ForgeRefresh { .. }
        | Msg::Connect { .. }
        | Msg::Disconnect(_)
        | Msg::ForgetTokens { .. }
        | Msg::SetTracker(_)
        | Msg::CreatePrDialog(_)
        | Msg::CreatePr(_)
        | Msg::ForgeReplied(_)
        | Msg::Connected { .. } => forge::handle(st, msg, now, &mut fx),
        Msg::PublishDialog | Msg::PublishAccount(_) | Msg::PublishOwners { .. } | Msg::Publish | Msg::Published { .. } => {
            publish::handle(st, msg, &mut fx)
        }

        // preferences
        Msg::OpenPrefs(_)
        | Msg::ClosePrefs
        | Msg::SelectIntegration(_)
        | Msg::GitConfig { .. }
        | Msg::SettingsEdited
        | Msg::GitConfigLoaded(_)
        | Msg::FolderPicked { .. }
        | Msg::PickFolder(_)
        | Msg::PickFile(_)
        | Msg::FilePicked { .. }
        | Msg::LoadSigningKeys
        | Msg::SigningKeysLoaded(_)
        | Msg::GenerateGpgKey
        | Msg::GpgKeyGenerated(_)
        | Msg::CopyPublicKey
        | Msg::PublicKeyExported(_)
        | Msg::SetTheme(_)
        | Msg::SetToolbarLabels(_) => prefs::handle(st, msg, now, &mut fx),

        // command palette
        Msg::TogglePalette
        | Msg::ClosePalette
        | Msg::PaletteMove(_)
        | Msg::RunPaletteCommand(_)
        | Msg::PaletteBack
        | Msg::TrackedPathsLoaded { .. } => palette::handle(st, msg, now, &mut fx),

        // dialogs and the outside world
        Msg::Confirm { title, body, button, then } => st.modal = Some(crate::state::Modal::Confirm { title, body, button, then }),
        Msg::ConfirmAccepted => {
            if let Some(crate::state::Modal::Confirm { then, .. }) = st.modal.take() {
                fx.extend(update(st, *then, now));
            }
        }
        Msg::CloseModal => st.modal = None,
        Msg::NewBranch { start, name } => st.modal = Some(crate::state::Modal::NewBranch { start, name, checkout: true }),
        Msg::NewTag { at, annotated } => {
            st.modal = Some(crate::state::Modal::NewTag { at, name: String::new(), message: String::new(), annotated })
        }
        Msg::CloneDialog => {
            let parent = st.recent.first().and_then(|p| p.parent()).map(|p| p.display().to_string()).unwrap_or_default();
            st.modal = Some(crate::state::Modal::Clone { url: String::new(), parent });
        }
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::OpenUrl(url)),
        Msg::OpenTerminal => {
            if let Some(w) = st.workdir() {
                fx.push(Effect::OpenTerminal(w));
            }
        }
        Msg::OpenFolder(p) => fx.push(Effect::OpenFolder(p)),
        Msg::Logged { ok, text } => st.log(ok, text),
    }
    fx
}
