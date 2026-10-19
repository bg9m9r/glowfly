//! What kind of dialog each `Modal` is, and the part of a form the user typed or chose: what a
//! backdrop click and Esc may do to it (`update/dialog.rs`).

use super::Modal;

/// How a dialog treats a backdrop click and Esc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    /// The user types or chooses something: the backdrop does nothing, and Esc asks before
    /// throwing away an edit.
    Form,
    /// Shows something: the backdrop and Esc close it.
    Info,
    /// Asks a yes-or-no question: the backdrop and Esc close it, as its Cancel would.
    Confirm,
}

impl Modal {
    pub fn kind(&self) -> DialogKind {
        use DialogKind::{Confirm, Form, Info};
        match self {
            Modal::NewBranch { .. }
            | Modal::NewTag(_)
            | Modal::AnnotateTag(_)
            | Modal::NewWorktree(_)
            | Modal::LockWorktree(_)
            | Modal::MoveWorktree(_)
            | Modal::AddSubmodule(_)
            | Modal::EditSubmodule(_)
            | Modal::Clone(_)
            | Modal::Init(_)
            | Modal::CreatePr { .. }
            | Modal::Publish(_)
            | Modal::RenameBranch { .. }
            | Modal::RenameStash(_)
            | Modal::StashPush(_)
            | Modal::SetUpstream { .. }
            | Modal::NewFile(_)
            | Modal::RenameFile { .. }
            | Modal::GitFlowStart(_)
            | Modal::GitFlowFinish(_)
            | Modal::Remote(_)
            | Modal::AddFork(_)
            | Modal::ForgeFilter(_)
            | Modal::CreateIssue(_)
            | Modal::RewordCommit(_)
            | Modal::RenameTab(_)
            | Modal::ApplyPatch(_)
            | Modal::SavePatch(_) => Form,
            // Create fork mostly asks "fork it?"; its one field is filled in by the service
            // lookup after it opens.
            Modal::Confirm { .. }
            | Modal::ConfirmDanger { .. }
            | Modal::ConfirmChoice { .. }
            | Modal::Trust(_)
            | Modal::RemoveWorktree(_)
            | Modal::RemoveSubmodule(_)
            | Modal::UndoCreated(_)
            | Modal::Mainline(_)
            | Modal::TrashOffer(_)
            | Modal::AutoStash(_)
            | Modal::CreateFork(_) => Confirm,
            // Their filter fields are cheap to type again.
            Modal::Shortcuts { .. }
            | Modal::About
            | Modal::ReleaseNotes(_)
            | Modal::ConflictReport(_)
            | Modal::DeepLink(_)
            | Modal::FindRepos { .. }
            | Modal::SyncRepos => Info,
        }
    }

    /// What the user typed or chose in a form, to compare with the form as it opened; None for
    /// information and confirm dialogs. Loading flags, errors, disk-check results and lists that
    /// load later are left out. A field a result fills in after opening is left out, or counted
    /// only when it differs from what was filled in.
    pub fn draft(&self) -> Option<String> {
        let d = match self {
            Modal::NewBranch { name, checkout, .. } => format!("{:?}", (name, checkout)),
            Modal::NewTag(f) => format!("{:?}", (&f.name, &f.message, f.annotated, &f.remote)),
            Modal::AnnotateTag(f) => format!("{:?}", f.message),
            Modal::NewWorktree(f) => format!("{:?}", (&f.base, &f.folder, &f.branch, f.open)),
            Modal::LockWorktree(f) => format!("{:?}", f.reason),
            Modal::MoveWorktree(f) => format!("{:?}", f.folder),
            Modal::AddSubmodule(f) => format!("{:?}", (&f.url, &f.path, &f.branch)),
            Modal::EditSubmodule(f) => format!("{:?}", (&f.url, &f.branch)),
            Modal::Clone(f) => {
                let s = &f.service;
                format!(
                    "{:?}",
                    (
                        (&f.source, &f.url, &f.parent, f.shallow, &f.limit, &f.depth, &f.since, &f.branch),
                        (f.sparse, &f.sparse_folders, f.partial),
                        (s.account, &s.filter, s.picked, s.ssh),
                    )
                )
            }
            Modal::Init(f) => {
                let holder = (f.holder != f.holder_suggested).then_some(&f.holder);
                format!("{:?}", (&f.folder, &f.branch, &f.gitignore, f.os_editor, &f.license, holder, f.commit))
            }
            Modal::CreatePr { target, title, description, draft, items, .. } => {
                format!("{:?}", (target, title, description, draft, items))
            }
            Modal::Publish(f) => {
                let owner = (!f.owner.is_empty() && f.owners.first() != Some(&f.owner)).then_some(&f.owner);
                format!("{:?}", (f.account, owner, &f.name, &f.description, f.private))
            }
            Modal::RenameBranch { name, .. } => format!("{name:?}"),
            Modal::RenameStash(f) => format!("{:?}", f.message),
            Modal::StashPush(f) => format!("{:?}", (&f.scope, &f.message, &f.chosen)),
            Modal::SetUpstream { remote, name, .. } => format!("{:?}", (remote, name)),
            Modal::NewFile(f) => format!("{:?}", f.text),
            Modal::RenameFile { form, .. } => format!("{:?}", form.text),
            Modal::GitFlowStart(f) => format!("{:?}", (&f.kind, &f.name)),
            Modal::GitFlowFinish(f) => format!("{:?}", (&f.tag, &f.message, f.keep)),
            Modal::Remote(f) => format!("{:?}", (&f.name, &f.url, &f.push_url, f.fetch_after)),
            Modal::AddFork(p) => format!("{:?}", (&p.filter, p.picked, &p.name, p.fetch_after)),
            Modal::ForgeFilter(f) => format!("{:?}", (&f.name, &f.labels, &f.c)),
            // The type and priority are left out: loading the options fills their defaults.
            Modal::CreateIssue(f) => {
                format!("{:?}", (&f.project_text, &f.title, &f.description, &f.labels, &f.people, &f.epic, f.then_branch))
            }
            Modal::RewordCommit(f) => format!("{:?}", f.draft),
            Modal::RenameTab(f) => format!("{:?}", f.text),
            Modal::ApplyPatch(f) => format!("{:?}", (&f.mode, &f.opts, f.am_three_way)),
            Modal::SavePatch(f) => format!("{:?}", (&f.scope, f.pick, &f.chosen, f.untracked)),
            Modal::Confirm { .. }
            | Modal::ConfirmDanger { .. }
            | Modal::ConfirmChoice { .. }
            | Modal::Trust(_)
            | Modal::RemoveWorktree(_)
            | Modal::RemoveSubmodule(_)
            | Modal::UndoCreated(_)
            | Modal::Mainline(_)
            | Modal::TrashOffer(_)
            | Modal::AutoStash(_)
            | Modal::CreateFork(_)
            | Modal::Shortcuts { .. }
            | Modal::About
            | Modal::ReleaseNotes(_)
            | Modal::ConflictReport(_)
            | Modal::DeepLink(_)
            | Modal::FindRepos { .. }
            | Modal::SyncRepos => return None,
        };
        Some(d)
    }
}

#[cfg(test)]
#[path = "dialog_kind_tests.rs"]
mod tests;
