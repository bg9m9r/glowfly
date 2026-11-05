//! The open dialog's form, copied for the frame (`Modal::form_copy`), compared with what is
//! shown (`same_form`) and taken back (`take_form`, from `EditMsg::Dialog`). Only the variants
//! whose views write are forms; the others (questions, reports, notes) are drawn from the
//! dialog itself and send what they change as messages. `Modal` holds messages (`Box<Msg>`),
//! which can't be cloned, so the copy is made per variant, and every variant is classified in
//! `modal_forms!` below: a new one fails to compile until it is listed as a form or not.

use super::Modal;

/// A dialog form sent back by its view (`EditMsg::Dialog`). Its `Debug` names only the
/// dialog's kind, never what was typed.
pub struct DialogForm(Box<Modal>);

impl DialogForm {
    pub fn new(form: Modal) -> Self {
        Self(Box::new(form))
    }

    pub fn into_inner(self) -> Modal {
        *self.0
    }
}

impl std::fmt::Debug for DialogForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DialogForm({:?})", std::mem::discriminant(&*self.0))
    }
}

/// `form_copy` and `same_form` over the struct-like forms (written out), the one-field forms
/// (`forms`) and the dialogs that aren't forms (`not_forms`).
macro_rules! modal_forms {
    (forms: [$($f:ident),* $(,)?], not_forms: [$($n:pat),* $(,)?]) => {
        impl Modal {
            /// Whether this dialog is a form its view writes.
            fn is_form(&self) -> bool {
                !matches!(self, $($n)|*)
            }

            /// A copy of the form for its view to edit this frame; None for the dialogs whose
            /// views only read (so release notes or a preview are never copied).
            pub fn form_copy(&self) -> Option<Modal> {
                Some(match self {
                    Modal::NewBranch { start, name, checkout, track, touched } => Modal::NewBranch {
                        start: start.clone(),
                        name: name.clone(),
                        checkout: *checkout,
                        track: track.clone(),
                        touched: touched.clone(),
                    },
                    Modal::CreatePr { source, target, targets, title, description, draft, items, repos } => Modal::CreatePr {
                        source: source.clone(),
                        target: target.clone(),
                        targets: targets.clone(),
                        title: title.clone(),
                        description: description.clone(),
                        draft: *draft,
                        items: items.clone(),
                        repos: repos.clone(),
                    },
                    Modal::SetUpstream { branch, remote, name, touched } => Modal::SetUpstream {
                        branch: branch.clone(),
                        remote: remote.clone(),
                        name: name.clone(),
                        touched: touched.clone(),
                    },
                    Modal::RenameFile { old, form } => Modal::RenameFile { old: old.clone(), form: form.clone() },
                    Modal::Shortcuts { filter } => Modal::Shortcuts { filter: filter.clone() },
                    Modal::FindRepos { filter } => Modal::FindRepos { filter: filter.clone() },
                    $(Modal::$f(form) => Modal::$f(form.clone()),)*
                    $($n)|* => return None,
                })
            }

            /// Whether `other` is this form unchanged: the same variant with equal fields.
            /// False for the dialogs that aren't forms. Matched per variant of `self`, with no
            /// catch-all, so a new variant fails to compile until it is compared here too.
            pub fn same_form(&self, other: &Modal) -> bool {
                match self {
                    Modal::NewBranch { start, name, checkout, track, touched } => matches!(
                        other,
                        Modal::NewBranch { start: s, name: n, checkout: c, track: t, touched: to }
                            if start == s && name == n && checkout == c && track == t && touched == to
                    ),
                    Modal::CreatePr { source, target, targets, title, description, draft, items, repos } => matches!(
                        other,
                        Modal::CreatePr { source: s, target: t, targets: ts, title: ti, description: d, draft: dr, items: i, repos: r }
                            if source == s
                                && target == t
                                && targets == ts
                                && title == ti
                                && description == d
                                && draft == dr
                                && items == i
                                && repos == r
                    ),
                    Modal::SetUpstream { branch, remote, name, touched } => matches!(
                        other,
                        Modal::SetUpstream { branch: b, remote: r, name: n, touched: t }
                            if branch == b && remote == r && name == n && touched == t
                    ),
                    Modal::RenameFile { old, form } => matches!(other, Modal::RenameFile { old: o, form: f } if old == o && form == f),
                    Modal::Shortcuts { filter } => matches!(other, Modal::Shortcuts { filter: f } if filter == f),
                    Modal::FindRepos { filter } => matches!(other, Modal::FindRepos { filter: f } if filter == f),
                    $(Modal::$f(a) => matches!(other, Modal::$f(b) if a == b),)*
                    $($n)|* => false,
                }
            }
        }
    };
}

modal_forms! {
    forms: [
        NewTag, AnnotateTag, NewWorktree, RemoveWorktree, DeleteRemoteBranch, LockWorktree, MoveWorktree, AddSubmodule,
        EditSubmodule, RemoveSubmodule, Clone, Init, Publish, RenameBranch, RenameStash, StashPush, NewFile, GitFlowStart,
        GitFlowFinish, Remote, AddFork, CreateFork, ForgeFilter, CreateIssue, RewordCommit, UndoCreated, ApplyPatch, SavePatch,
        RenameTab, RepoGroup, AddToGroup, Mainline,
    ],
    not_forms: [
        Modal::Confirm { .. }, Modal::ConfirmDetails { .. }, Modal::ConfirmDanger { .. }, Modal::ConfirmChoice { .. },
        Modal::DeleteBranch(_), Modal::Trust(_), Modal::ConflictReport(_), Modal::OpPreview(_), Modal::DeepLink(_), Modal::About,
        Modal::Notices, Modal::ReleaseNotes(_), Modal::SyncRepos, Modal::TrashOffer(_), Modal::AutoStash(_), Modal::SshKeygen,
    ]
}

impl Modal {
    /// Replaces this dialog with `form`, its view's edited copy, when both are the same form
    /// variant; whether it did. A copy for another dialog, or for one that isn't a form, is
    /// dropped.
    pub fn take_form(&mut self, form: Modal) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(&form) || !self.is_form() {
            return false;
        }
        *self = form;
        true
    }
}
