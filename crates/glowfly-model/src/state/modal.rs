//! The dialog over the window: one variant per dialog, holding its form (the forms live in the
//! state modules of their areas; the views draw them in `ui/modals.rs`).

use super::pull_request::repos::PrRepoPick;
use super::{
    auto_stash, branch_delete, branch_sync, clone, conflict_check, deep_link, files, forge_filter_form, forks, gitflow, init, issue_form,
    op_preview, patch, pick, publish, rebase_quick, release_notes, remotes, repo_group_picker, repo_groups, stashes, submodules, tabs,
    tag_delete, tags, touched, trust, undo, worktree_manage, worktree_move, worktrees,
};
use crate::msg::Msg;

/// A dialog over the window. `Shortcuts` is the keyboard shortcut sheet, `filter` its filter
/// field. `ConflictReport` is a conflict check the user asked for (branch menus, Check again);
/// `OpPreview` what a merge, rebase, reset, cherry-pick or drop would leave, before it runs.
pub enum Modal {
    NewBranch {
        start: Option<String>,
        name: String,
        checkout: bool,
        /// The remote branch (remote, name) the new branch tracks, starting at its tip.
        track: Option<(String, String)>,
        touched: touched::Touched,
    },
    NewTag(tags::TagForm),
    AnnotateTag(tags::AnnotateForm),
    /// Delete tag: the local tag and, ticked, its copies on remotes (`ui/tag_delete_dialog.rs`).
    DeleteTag(tag_delete::DeleteTagForm),
    NewWorktree(worktrees::WorktreeForm),
    RemoveWorktree(worktree_manage::RemoveForm),
    /// Delete a local branch: what it would lose, then Delete or Delete anyway (`ui/branch_delete_dialog.rs`).
    DeleteBranch(branch_delete::DeleteForm),
    DeleteRemoteBranch(branch_sync::DeleteRemoteForm),
    LockWorktree(worktree_manage::LockForm),
    MoveWorktree(worktree_move::MoveForm),
    AddSubmodule(submodules::add::AddForm),
    EditSubmodule(submodules::edit::EditForm),
    RemoveSubmodule(submodules::remove::RemoveForm),
    Confirm {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
    },
    /// `Confirm` with a collapsed Details under the body (the values behind a plain-words question).
    ConfirmDetails {
        title: String,
        body: String,
        details: String,
        button: String,
        then: Box<Msg>,
    },
    /// `Confirm` for a step that loses work: drawn with a danger button that Enter doesn't press.
    /// With `ack` (its label), the button waits for `acked` (constructors in `confirm.rs`).
    ConfirmDanger {
        title: String,
        body: String,
        button: String,
        ack: Option<String>,
        acked: bool,
        then: Box<Msg>,
    },
    /// A question with two answers besides Cancel: `button` (primary, Enter's default) runs `then`, `alt` (left of it) `alt_then`.
    ConfirmChoice {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
        alt: String,
        alt_then: Box<Msg>,
    },
    Clone(clone::CloneForm),
    Init(init::InitForm),
    CreatePr {
        source: String,
        target: String,
        targets: Vec<String>,
        title: String,
        description: String,
        draft: bool,
        items: String,
        /// The From / Into repositories (shown with two or more PR repositories).
        repos: PrRepoPick,
    },
    Publish(Box<publish::PublishForm>),
    RenameBranch(branch_sync::RenameForm),
    RenameStash(stashes::RenameStashForm),
    StashPush(stashes::StashPushForm),
    SetUpstream {
        branch: String,
        remote: String,
        name: String,
        touched: touched::Touched,
    },
    NewFile(files::PathForm),
    RenameFile {
        old: String,
        form: files::PathForm,
    },
    Shortcuts {
        filter: String,
    },
    GitFlowStart(gitflow::StartForm),
    GitFlowFinish(gitflow::FinishForm),
    Remote(remotes::RemoteForm),
    AddFork(Box<forks::ForkPicker>),
    CreateFork(Box<forks::ForkCreate>),
    Trust(trust::TrustPrompt),
    ConflictReport(conflict_check::ConflictReport),
    /// What an operation would do, before it runs (ui/op_preview_dialog.rs).
    OpPreview(Box<op_preview::PreviewDialog>),
    ForgeFilter(Box<forge_filter_form::FilterForm>),
    /// An invalid link, a linked repository not found, or several copies to choose from.
    DeepLink(deep_link::LinkDialog),
    CreateIssue(Box<issue_form::IssueForm>),
    /// Reword a graph commit, or type the message of commits it squashes (`ui/rebase/reword_dialog.rs`).
    RewordCommit(Box<rebase_quick::RewordForm>),
    /// Help > About Glowfly: version, licence, git and links.
    About,
    /// Help > About > Third-party notices: the bundled fonts' and syntax definitions' licenses.
    Notices,
    /// The release notes, with a newer release on top when one is on offer.
    ReleaseNotes(Box<release_notes::NotesDialog>),
    UndoCreated(undo::UndoCreatedForm),
    /// Apply patch: how to apply a patch file that was read (`ui/patch_apply_dialog.rs`).
    ApplyPatch(Box<patch::apply::ApplyForm>),
    /// Create patch from uncommitted changes (`ui/patch_save_dialog.rs`).
    SavePatch(patch::changes::SaveChangesForm),
    /// Find repositories in the project folders (`ui/repo_finder.rs`); `filter` is its field.
    FindRepos {
        filter: String,
    },
    /// Fetch or pull across repositories (`ui/repo_sync.rs`): `AppState::repo_sync`'s rows.
    SyncRepos,
    /// Rename tab (`ui/tab_dialog.rs`).
    RenameTab(tabs::RenameTabForm),
    /// New group and Rename group (`ui/repo_group_dialog.rs`).
    RepoGroup(repo_groups::GroupForm),
    /// A group's Add repositories picker (`ui/repo_group_picker.rs`).
    AddToGroup(repo_group_picker::AddPicker),
    /// Which parent of a merge commit to cherry-pick or revert against (`ui/mainline_dialog.rs`).
    Mainline(pick::MainlineForm),
    /// Uncommitted changes in the way of an operation: stash them and carry on (`ui/auto_stash_dialog.rs`).
    AutoStash(auto_stash::AutoStashAsk),
    /// The trash on a file's drive can't be used: move it to the home Trash or delete it permanently (`ui/trash_offer_dialog.rs`).
    TrashOffer(files::TrashOffer),
    /// The Generate SSH key dialog (`ui/prefs/ssh/keygen_dialog.rs`). Its fields live in `PrefsState.ssh`, so closing it keeps them.
    SshKeygen,
}
