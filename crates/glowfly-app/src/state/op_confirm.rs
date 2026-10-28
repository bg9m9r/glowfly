//! Asking before the in-progress banner's Abort, Skip and Reset bisect, which lose work: what
//! each does, what is lost (the resolved files, edits in conflicted ones) and whether Undo
//! brings it back. A merge, single cherry-pick or single revert with work to keep (resolved or
//! staged files, conflicted ones) aborts keeping it first (`glowfly_core::undo::abort_saving`):
//! Undo puts the operation back in progress with that work, and edits to other files stay.

use super::AppState;
use super::op_actions::action_text;
use super::rebase_banner::action_msg;
use crate::msg::{JobMsg, Msg};
use git2::RepositoryState;
use glowfly_core::gitcli;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// A banner command that loses work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Risky {
    Abort,
    Skip,
    ResetBisect,
}

impl Risky {
    /// The kind of `args`, told from its option (`--skip`, `--abort`) or `bisect reset`; None
    /// for Continue.
    pub fn of(args: &[&str]) -> Option<Risky> {
        match args {
            ["bisect", "reset", ..] => Some(Risky::ResetBisect),
            [_, "--skip", ..] => Some(Risky::Skip),
            [_, "--abort", ..] => Some(Risky::Abort),
            _ => None,
        }
    }
}

/// Whether an abort can keep the operation's work for Undo: a merge, single cherry-pick or
/// single revert (a sequence's commits would not come back with it) with something to keep,
/// staged or conflicted. Edits to other files don't matter: the abort leaves them in place.
pub fn can_keep(snap: &Snapshot) -> bool {
    use RepositoryState::*;
    matches!(snap.state, Merge | CherryPick | Revert) && (!snap.staged.is_empty() || snap.unstaged.iter().any(|f| f.status == '!'))
}

/// The changed tracked files outside the operation (neither staged nor conflicted), which a
/// kept abort leaves as they are.
fn others(snap: &Snapshot) -> usize {
    let in_op = |path: &str| snap.staged.iter().any(|s| s.path == path || s.renamed_from.as_deref() == Some(path));
    snap.unstaged.iter().filter(|f| !matches!(f.status, '?' | '!') && !in_op(&f.path)).count()
}

/// What the banner button `label` running `args` sends: Continue as it is, the others behind a
/// danger confirmation.
pub fn banner_msg(st: &AppState, label: &'static str, args: &'static [&'static str]) -> Msg {
    let Some(snap) = &st.snap else { return Msg::git(label, args) };
    let Some(risky) = Risky::of(args) else { return action_msg(snap.state, label, args) };
    let t = st.settings.lang();
    let resolved = st.session.conflict_files.resolved_rows(snap).len();
    let conflicted = snap.unstaged.iter().filter(|f| f.status == '!').count();
    let keep = risky == Risky::Abort && can_keep(snap);
    let others = if keep { others(snap) } else { 0 };
    let (title, what) = words(t, risky, args);
    let then = match risky {
        Risky::Abort => Msg::Job(JobMsg::Abort { label: label.into(), args: gitcli::args(args), keep }),
        Risky::Skip => action_msg(snap.state, label, args),
        Risky::ResetBisect => Msg::git(label, args),
    };
    Msg::confirm_danger(title, body(t, what, resolved, conflicted, others, keep), action_text(t, label), then)
}

/// The dialog's title and its sentence on what happens, for `risky` running `args`.
fn words(t: Lang, risky: Risky, args: &[&str]) -> (&'static str, &'static str) {
    match (risky, args.first().copied().unwrap_or_default()) {
        (Risky::ResetBisect, _) => (t.tr("op.confirm.reset_bisect.title"), t.tr("op.confirm.reset_bisect.body")),
        (Risky::Skip, "am") => (t.tr("op.confirm.skip_patch.title"), t.tr("op.confirm.skip_patch.body")),
        (Risky::Skip, _) => (t.tr("op.confirm.skip_commit.title"), t.tr("op.confirm.skip_commit.body")),
        (Risky::Abort, "merge") => (t.tr("op.confirm.abort_merge.title"), t.tr("op.confirm.abort_merge.body")),
        (Risky::Abort, "rebase") => (t.tr("op.confirm.abort_rebase.title"), t.tr("op.confirm.abort_rebase.body")),
        (Risky::Abort, "cherry-pick") => (t.tr("op.confirm.abort_cherry_pick.title"), t.tr("op.confirm.abort_cherry_pick.body")),
        (Risky::Abort, "revert") => (t.tr("op.confirm.abort_revert.title"), t.tr("op.confirm.abort_revert.body")),
        (Risky::Abort, _) => (t.tr("op.confirm.abort_am.title"), t.tr("op.confirm.abort_am.body")),
    }
}

/// The dialog's paragraphs: what happens, what is lost (the `resolved` files, edits in the
/// `conflicted` ones), whether Undo brings it back (`keep`) and, then, that the `others`
/// changed files stay.
fn body(t: Lang, what: &str, resolved: usize, conflicted: usize, others: usize, keep: bool) -> String {
    let mut paras = vec![what.to_owned()];
    let mut lost = Vec::new();
    if resolved > 0 {
        lost.push(t.trn("op.confirm.resolved", resolved, &[]));
    }
    if conflicted > 0 && keep {
        lost.push(t.trn("op.confirm.conflicted_kept", conflicted, &[]));
    } else if conflicted > 0 {
        lost.push(t.trn("op.confirm.conflicted_lost", conflicted, &[]));
    }
    let said = !lost.is_empty();
    if said {
        paras.push(lost.join(" "));
    }
    if keep {
        let mut kept = if said { t.tr("op.confirm.undo_kept") } else { t.tr("op.confirm.undo_kept_changes") }.to_owned();
        if others > 0 {
            kept = format!("{kept} {}", t.tr("op.confirm.others_stay"));
        }
        paras.push(kept);
    } else if said {
        paras.push(t.tr("op.confirm.no_undo").to_owned());
    }
    paras.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::RebaseMsg;
    use crate::state::op_actions::state_actions;
    use crate::state::op_fixture::{rows, snapshot};
    use glowfly_core::rebase::Resume;
    use glowfly_core::repo::{FileEntry, ResolvedFile};
    use glowfly_core::settings::Settings;

    fn st(name: &str, state: RepositoryState) -> AppState {
        let mut st = AppState::new(Settings::default(), Vec::new(), std::time::Instant::now());
        let mut snap = snapshot(name);
        snap.state = state;
        st.snap = Some(snap);
        st
    }

    /// The banner's action of `state` whose label starts with `prefix`.
    fn action(state: RepositoryState, prefix: &str) -> (&'static str, &'static [&'static str]) {
        *state_actions(state).unwrap().iter().find(|(l, _)| l.starts_with(prefix)).unwrap()
    }

    fn asked(m: Msg) -> (String, String, String, Msg) {
        match m {
            Msg::ConfirmDanger { title, body, button, then, .. } => (title, body, button, *then),
            _ => panic!("expected a danger confirmation"),
        }
    }

    fn resolved(paths: &[&str]) -> Vec<ResolvedFile> {
        paths.iter().map(|p| ResolvedFile { entry: FileEntry::new(*p, ' '), both_sides: true }).collect()
    }

    #[test]
    fn titles_bodies_and_buttons_per_kind() {
        let cases = [
            (RepositoryState::Merge, "Abort", "Abort the merge?", "Abort merge", "The merge stops"),
            (RepositoryState::RebaseInteractive, "Abort", "Abort the rebase?", "Abort rebase", "replayed so far are dropped"),
            (RepositoryState::CherryPick, "Abort", "Abort the cherry-pick?", "Abort cherry-pick", "The cherry-pick stops"),
            (RepositoryState::RevertSequence, "Abort", "Abort the revert?", "Abort revert", "The revert stops"),
            (RepositoryState::ApplyMailbox, "Abort", "Stop applying patches?", "Abort", "patches"),
            (
                RepositoryState::CherryPick,
                "Skip",
                "Skip this commit?",
                "Skip commit",
                "This commit is left out and the next one is applied.",
            ),
            (RepositoryState::ApplyMailbox, "Skip", "Skip this patch?", "Skip patch", "This patch is left out"),
            (RepositoryState::Bisect, "Reset", "End the bisect?", "Reset bisect", "good and bad marks are forgotten"),
        ];
        for (i, (state, prefix, title, button, what)) in cases.into_iter().enumerate() {
            let st = st(&format!("confirm-kind-{i}"), state);
            let (label, args) = action(state, prefix);
            let (t, body, b, _) = asked(banner_msg(&st, label, args));
            assert_eq!((t.as_str(), b.as_str()), (title, button), "{state:?} {label}");
            assert!(body.contains(what), "{body}");
            assert!(!body.contains("\n\n"), "nothing lost, nothing more to say: {body}");
        }
    }

    #[test]
    fn lost_lines_and_undo() {
        let mut st = st("confirm-lost", RepositoryState::CherryPickSequence);
        st.session.conflict_files.resolved = resolved(&["a.txt", "b.txt"]);
        st.snap.as_mut().unwrap().unstaged = rows(&[("c.txt", '!')]);
        let (label, args) = action(RepositoryState::CherryPickSequence, "Abort");
        let (_, body, _, then) = asked(banner_msg(&st, label, args));
        let paras: Vec<&str> = body.split("\n\n").collect();
        assert_eq!(paras[1], "The 2 files you resolved go back to how they were. Edits in the 1 conflicted file are lost too.");
        assert_eq!(paras[2], "This can't be undone.");
        assert!(matches!(then, Msg::Job(JobMsg::Abort { keep: false, .. })), "a sequence isn't kept");

        st.snap.as_mut().unwrap().state = RepositoryState::Merge;
        let (label, args) = action(RepositoryState::Merge, "Abort");
        let (_, body, _, then) = asked(banner_msg(&st, label, args));
        let paras: Vec<&str> = body.split("\n\n").collect();
        assert_eq!(
            paras[1..],
            [
                "The 2 files you resolved go back to how they were. Edits in the 1 conflicted file go too.",
                "Glowfly keeps them first, so Undo brings them back and picks up where you left off."
            ]
        );
        let Msg::Job(JobMsg::Abort { label: job, args: cmd, keep: true }) = then else { panic!("a kept abort") };
        assert_eq!((job.as_str(), cmd), ("Abort merge", gitcli::args(&["merge", "--abort"])));

        let snap = st.snap.as_mut().unwrap();
        snap.unstaged = rows(&[("a.txt", 'M'), ("README.md", 'M'), ("new.txt", '?')]);
        snap.staged = rows(&[("a.txt", 'M'), ("b.txt", 'M')]);
        let (_, body, _, _) = asked(banner_msg(&st, label, args));
        assert!(body.ends_with("picks up where you left off. Your other changes stay as they are."), "{body}");

        st.session.conflict_files.resolved.clear();
        st.snap.as_mut().unwrap().unstaged.clear();
        let (_, body, _, _) = asked(banner_msg(&st, label, args));
        assert!(
            body.ends_with("\n\nThe changes so far are kept first, so Undo brings them back and picks up where you left off."),
            "{body}"
        );
    }

    #[test]
    fn continue_passes_through_and_skip_and_am_keep_their_messages() {
        let rebase = st("confirm-pass", RepositoryState::RebaseInteractive);
        let (label, args) = action(RepositoryState::RebaseInteractive, "Continue");
        assert!(matches!(banner_msg(&rebase, label, args), Msg::Rebase(RebaseMsg::Resume(Resume::Continue))));
        let (label, args) = action(RepositoryState::RebaseInteractive, "Skip");
        let (_, _, _, then) = asked(banner_msg(&rebase, label, args));
        assert!(matches!(then, Msg::Rebase(RebaseMsg::Resume(Resume::Skip))));

        let am = st("confirm-am", RepositoryState::ApplyMailbox);
        let (label, args) = action(RepositoryState::ApplyMailbox, "Abort");
        let (_, _, _, then) = asked(banner_msg(&am, label, args));
        let Msg::Job(JobMsg::Abort { label, keep: false, .. }) = then else { panic!("an abort") };
        assert_eq!(label, "Abort", "the patch headline goes by this label");

        let bisect = st("confirm-bisect", RepositoryState::Bisect);
        let (label, args) = action(RepositoryState::Bisect, "Reset");
        let (_, _, _, then) = asked(banner_msg(&bisect, label, args));
        assert!(matches!(then, Msg::Job(JobMsg::Git { label, .. }) if label == "Reset bisect"));
    }

    #[test]
    fn keeping_needs_a_single_operation_with_work_to_keep() {
        let mut snap = snapshot("confirm-keep");
        snap.staged = rows(&[("a.txt", 'M')]);
        for (state, want) in [
            (RepositoryState::Merge, true),
            (RepositoryState::CherryPick, true),
            (RepositoryState::Revert, true),
            (RepositoryState::CherryPickSequence, false),
            (RepositoryState::RevertSequence, false),
            (RepositoryState::Rebase, false),
            (RepositoryState::RebaseInteractive, false),
            (RepositoryState::RebaseMerge, false),
            (RepositoryState::ApplyMailbox, false),
            (RepositoryState::Bisect, false),
        ] {
            snap.state = state;
            assert_eq!(can_keep(&snap), want, "{state:?}");
        }
        snap.state = RepositoryState::Merge;
        snap.unstaged = rows(&[("b.txt", '!')]);
        assert!(can_keep(&snap), "a conflicted file is kept too");
        snap.unstaged = rows(&[("README.md", 'M'), ("new.txt", '?')]);
        assert!(can_keep(&snap), "an edit unrelated to the operation stays in place");
        snap.staged.clear();
        snap.unstaged = rows(&[("b.txt", '!')]);
        assert!(can_keep(&snap), "only conflicts");
        snap.unstaged = rows(&[("new.txt", '?'), ("a.txt", 'M')]);
        assert!(!can_keep(&snap), "nothing staged or conflicted: nothing of the operation's to keep");
    }
}
