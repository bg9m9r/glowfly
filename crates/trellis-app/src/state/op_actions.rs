//! The operation in progress (a merge, rebase, cherry-pick, revert, patch series or bisect):
//! the banner's title and its Continue / Skip / Abort commands, with the English labels the
//! jobs are named by.

use super::am_banner::{AM_ABORT, AM_CONTINUE, AM_SKIP};
use git2::RepositoryState;
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;
use trellis_core::{patch_ops, pick_ops};

pub type StateAction = (&'static str, &'static [&'static str]);

const REBASE_ACTIONS: &[StateAction] =
    &[("Continue", &["rebase", "--continue"]), ("Skip commit", &["rebase", "--skip"]), ("Abort rebase", &["rebase", "--abort"])];

/// The in-progress operation's Continue/Skip/Abort commands.
pub fn state_actions(state: RepositoryState) -> Option<&'static [StateAction]> {
    use RepositoryState::*;
    Some(match state {
        Merge => &[("Abort merge", &["merge", "--abort"])],
        Rebase | RebaseMerge | RebaseInteractive => REBASE_ACTIONS,
        CherryPick | CherryPickSequence => &[
            ("Continue", &["cherry-pick", "--continue"]),
            ("Skip commit", pick_ops::CHERRY_PICK_SKIP),
            ("Abort cherry-pick", &["cherry-pick", "--abort"]),
        ],
        Revert | RevertSequence => {
            &[("Continue", &["revert", "--continue"]), ("Skip commit", pick_ops::REVERT_SKIP), ("Abort revert", &["revert", "--abort"])]
        }
        ApplyMailbox => &[(AM_CONTINUE, patch_ops::AM_CONTINUE), (AM_SKIP, patch_ops::AM_SKIP), (AM_ABORT, patch_ops::AM_ABORT)],
        ApplyMailboxOrRebase => &[("Abort", &["am", "--abort"])],
        Bisect => &[("Reset bisect", &["bisect", "reset"])],
        Clean => return None,
    })
}

/// The in-progress banner's title for `snap`'s state: "Interactive rebase in progress · step 2
/// of 4", "Applying patches · patch 2 of 3", "Merge in progress"; None when nothing is in
/// progress. The step forms need a known count.
pub fn banner_title(t: Lang, snap: &Snapshot) -> Option<String> {
    use RepositoryState::*;
    let rebase = snap.rebase.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    let am = snap.am.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    Some(match snap.state {
        Merge => t.tr("details.banner.title.merge").to_owned(),
        Rebase | RebaseMerge => match rebase {
            Some((step, total)) => t.trf("details.banner.title.rebase_step", &[("step", &step), ("total", &total)]),
            None => t.tr("details.banner.title.rebase").to_owned(),
        },
        RebaseInteractive => match rebase {
            Some((step, total)) => t.trf("details.banner.title.interactive_step", &[("step", &step), ("total", &total)]),
            None => t.tr("details.banner.title.interactive").to_owned(),
        },
        CherryPick | CherryPickSequence => t.tr("details.banner.title.cherry_pick").to_owned(),
        Revert | RevertSequence => t.tr("details.banner.title.revert").to_owned(),
        ApplyMailbox => match am {
            Some((step, total)) => t.trf("details.banner.title.am_step", &[("step", &step), ("total", &total)]),
            None => t.tr("details.banner.title.am").to_owned(),
        },
        ApplyMailboxOrRebase => t.tr("details.banner.title.apply_mailbox").to_owned(),
        Bisect => t.tr("details.banner.title.bisect").to_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::patch_ops::AmProgress;
    use trellis_core::rebase::{RebaseProgress, Stop, StopKind};

    #[test]
    fn banner_titles_by_state() {
        use RepositoryState::*;
        let mut snap = crate::state::op_fixture::snapshot("banner-titles");
        let title = |snap: &mut Snapshot, state| {
            snap.state = state;
            banner_title(Lang::En, snap)
        };
        assert_eq!(title(&mut snap, Clean), None);
        for (state, want) in [
            (Merge, "Merge in progress"),
            (Rebase, "Rebase in progress"),
            (RebaseMerge, "Rebase in progress"),
            (RebaseInteractive, "Interactive rebase in progress"),
            (CherryPick, "Cherry-pick in progress"),
            (CherryPickSequence, "Cherry-pick in progress"),
            (Revert, "Revert in progress"),
            (RevertSequence, "Revert in progress"),
            (ApplyMailbox, "Applying patches"),
            (ApplyMailboxOrRebase, "Apply mailbox in progress"),
            (Bisect, "Bisect in progress"),
        ] {
            assert_eq!(title(&mut snap, state).as_deref(), Some(want), "{state:?}");
            assert!(state_actions(state).is_some(), "{state:?}");
        }
        let stop = Stop { kind: StopKind::Other, oid: None };
        let p = RebaseProgress { step: 2, total: 4, stop, summary: String::new(), autostash: false, interactive: true, command: None };
        snap.rebase = Some(p);
        assert_eq!(title(&mut snap, RebaseInteractive).as_deref(), Some("Interactive rebase in progress \u{b7} step 2 of 4"));
        assert_eq!(title(&mut snap, Rebase).as_deref(), Some("Rebase in progress \u{b7} step 2 of 4"));
        snap.am = Some(AmProgress { step: 2, total: 3, subject: "x".into(), conflicted: false });
        assert_eq!(title(&mut snap, ApplyMailbox).as_deref(), Some("Applying patches \u{b7} patch 2 of 3"));
        let en = title(&mut snap, RebaseInteractive).unwrap();
        let de = banner_title(Lang::De, &snap).unwrap();
        assert!(de.contains('2') && de.contains('4') && de != en, "{de}");
    }
}
