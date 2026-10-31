//! The strip under the toolbar while an operation is in progress or conflicts are left: its
//! title (the operation, with the step of a rebase or patch series) and how the conflicts stand.

use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

#[derive(Clone, Debug, PartialEq)]
pub struct StripText {
    /// "Interactive rebase in progress, step 2 of 4".
    pub title: String,
    /// "2 conflicted files left", or (with `true`: all resolved) "All conflicts resolved".
    pub detail: Option<(String, bool)>,
    /// Conflicted files remain.
    pub conflicts: bool,
}

/// The strip's text for `snap`, with `resolved` the resolved files; None when no operation is in
/// progress and nothing is conflicted.
pub fn strip_text(t: Lang, snap: &Snapshot, resolved: usize) -> Option<StripText> {
    use RepositoryState::*;
    let conflicted = snap.unstaged.iter().filter(|f| f.status == '!').count();
    let rebase_step = snap.rebase.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    let am_step = snap.am.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    let title = match snap.state {
        Clean if conflicted == 0 => return None,
        Clean => t.tr("op_strip.conflicts").to_owned(),
        Merge => t.tr("op_strip.merge").to_owned(),
        RebaseInteractive => match rebase_step {
            Some((step, total)) => t.trf("op_strip.interactive_step", &[("step", &step), ("total", &total)]),
            None => t.tr("op_strip.interactive").to_owned(),
        },
        Rebase | RebaseMerge => match rebase_step {
            Some((step, total)) => t.trf("op_strip.rebase_step", &[("step", &step), ("total", &total)]),
            None => t.tr("op_strip.rebase").to_owned(),
        },
        CherryPick | CherryPickSequence => t.tr("op_strip.cherry_pick").to_owned(),
        Revert | RevertSequence => t.tr("op_strip.revert").to_owned(),
        ApplyMailbox | ApplyMailboxOrRebase => match am_step {
            Some((step, total)) => t.trf("op_strip.am_step", &[("step", &step), ("total", &total)]),
            None => t.tr("op_strip.am").to_owned(),
        },
        Bisect => t.tr("op_strip.bisect").to_owned(),
    };
    let detail = if conflicted > 0 {
        Some((t.trn("op_strip.conflicted", conflicted, &[]), false))
    } else if resolved > 0 {
        Some((t.tr("op_strip.all_resolved").to_owned(), true))
    } else {
        None
    };
    Some(StripText { title, detail, conflicts: conflicted > 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::op_fixture::{rows, snapshot};
    use glowfly_core::patch_ops::AmProgress;
    use glowfly_core::rebase::{RebaseProgress, Stop, StopKind};

    fn progress(step: usize, total: usize) -> RebaseProgress {
        let stop = Stop { kind: StopKind::Conflict, oid: None };
        RebaseProgress { step, total, stop, summary: String::new(), autostash: false, interactive: true, command: None }
    }

    fn text(title: &str, detail: Option<(&str, bool)>, conflicts: bool) -> Option<StripText> {
        Some(StripText { title: title.into(), detail: detail.map(|(d, all)| (d.into(), all)), conflicts })
    }

    #[test]
    fn titles_and_details_per_operation() {
        let en = Lang::En;
        let mut snap = snapshot("strip-text");
        assert_eq!(strip_text(en, &snap, 0), None, "clean with nothing conflicted");
        assert_eq!(strip_text(en, &snap, 2), None, "resolved rows alone don't show it");

        snap.unstaged = rows(&[("a.txt", '!'), ("b.txt", '!'), ("c.txt", 'M')]);
        snap.state = RepositoryState::Merge;
        assert_eq!(strip_text(en, &snap, 1), text("Merge in progress", Some(("2 conflicted files left", false)), true));
        snap.state = RepositoryState::Clean;
        snap.unstaged = rows(&[("a.txt", '!')]);
        assert_eq!(strip_text(en, &snap, 0), text("Conflicts to resolve", Some(("1 conflicted file left", false)), true));

        snap.unstaged.clear();
        snap.state = RepositoryState::RebaseInteractive;
        snap.rebase = Some(progress(2, 4));
        assert_eq!(strip_text(en, &snap, 0), text("Interactive rebase in progress, step 2 of 4", None, false));
        snap.state = RepositoryState::RebaseMerge;
        snap.rebase = Some(progress(0, 0));
        assert_eq!(strip_text(en, &snap, 0), text("Rebase in progress", None, false));
        snap.rebase = None;

        snap.state = RepositoryState::ApplyMailbox;
        snap.am = Some(AmProgress { step: 2, total: 3, subject: "x".into(), conflicted: false });
        assert_eq!(strip_text(en, &snap, 3), text("Applying patches, patch 2 of 3", Some(("All conflicts resolved", true)), false));
        snap.am = None;

        snap.state = RepositoryState::Bisect;
        assert_eq!(strip_text(en, &snap, 0), text("Bisect in progress", None, false));
        snap.state = RepositoryState::CherryPickSequence;
        assert_eq!(strip_text(en, &snap, 0), text("Cherry-pick in progress", None, false));
        snap.state = RepositoryState::Revert;
        assert_eq!(strip_text(en, &snap, 0), text("Revert in progress", None, false));
    }
}
