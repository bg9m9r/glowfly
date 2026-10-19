//! The operation in progress (a merge, rebase, cherry-pick, revert, patch series or bisect):
//! its name and the banner's Continue / Skip / Abort commands, with the English labels the jobs
//! are named by.

use super::am_banner::{AM_ABORT, AM_CONTINUE, AM_NAME, AM_SKIP};
use git2::RepositoryState;
use trellis_core::i18n::Lang;
use trellis_core::{patch_ops, pick_ops};

pub type StateAction = (&'static str, &'static [&'static str]);

const REBASE_ACTIONS: &[StateAction] =
    &[("Continue", &["rebase", "--continue"]), ("Skip commit", &["rebase", "--skip"]), ("Abort rebase", &["rebase", "--abort"])];

/// The in-progress operation name and its Continue/Skip/Abort commands.
pub fn state_actions(state: RepositoryState) -> Option<(&'static str, &'static [StateAction])> {
    use RepositoryState::*;
    Some(match state {
        Merge => ("Merge", &[("Abort merge", &["merge", "--abort"])]),
        Rebase | RebaseMerge => ("Rebase", REBASE_ACTIONS),
        RebaseInteractive => ("Interactive rebase", REBASE_ACTIONS),
        CherryPick | CherryPickSequence => (
            "Cherry-pick",
            &[
                ("Continue", &["cherry-pick", "--continue"]),
                ("Skip commit", pick_ops::CHERRY_PICK_SKIP),
                ("Abort cherry-pick", &["cherry-pick", "--abort"]),
            ],
        ),
        Revert | RevertSequence => (
            "Revert",
            &[("Continue", &["revert", "--continue"]), ("Skip commit", pick_ops::REVERT_SKIP), ("Abort revert", &["revert", "--abort"])],
        ),
        ApplyMailbox => (AM_NAME, &[(AM_CONTINUE, patch_ops::AM_CONTINUE), (AM_SKIP, patch_ops::AM_SKIP), (AM_ABORT, patch_ops::AM_ABORT)]),
        ApplyMailboxOrRebase => ("Apply mailbox", &[("Abort", &["am", "--abort"])]),
        Bisect => ("Bisect", &[("Reset bisect", &["bisect", "reset"])]),
        Clean => return None,
    })
}

/// A banner button's text in the language; `label` stays the English id the job is named by.
pub fn action_text(t: Lang, label: &'static str) -> &'static str {
    match label {
        "Continue" => t.tr("details.banner.action.continue"),
        "Commit merge" => t.tr("details.banner.action.commit_merge"),
        "Skip commit" => t.tr("details.banner.action.skip_commit"),
        "Skip patch" => t.tr("details.banner.action.skip_patch"),
        "Abort" => t.tr("details.banner.action.abort"),
        "Abort merge" => t.tr("details.banner.action.abort_merge"),
        "Abort rebase" => t.tr("details.banner.action.abort_rebase"),
        "Abort cherry-pick" => t.tr("details.banner.action.abort_cherry_pick"),
        "Abort revert" => t.tr("details.banner.action.abort_revert"),
        "Reset bisect" => t.tr("details.banner.action.reset_bisect"),
        _ => label,
    }
}
