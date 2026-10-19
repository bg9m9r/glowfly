//! The palette's history entries: reword, squash and drop of the selected commit with their
//! keys, Continue, Skip and Edit remaining steps while a rebase is in progress, and Restore
//! original order while the panel is open.

use super::rebase_tests::{repo, with_panel};
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, PaletteMsg};
use crate::state::palette::shown;
use crate::state::palette_commands::PaletteCommand;
use crate::state::palette_commands::palette_commands_rebase::RebaseCommand;
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use std::time::Instant;
use trellis_core::rebase::{QuickEdit, RebaseBase, Resume};
use trellis_core::repo::FileEntry;

/// Each shown entry's label and key hint.
fn entries(st: &AppState) -> Vec<(String, Option<String>)> {
    shown(st, "").into_iter().map(|s| (s.entry.label.clone(), s.entry.shortcut(&st.keymap).map(str::to_owned))).collect()
}

fn find<'a>(all: &'a [(String, Option<String>)], label: &str) -> Option<&'a Option<String>> {
    all.iter().find(|(l, _)| l == label).map(|(_, k)| k)
}

#[test]
fn edits_of_the_selected_commit() {
    let (mut st, dir, c) = repo("palette-rebase-edits");
    st.selection = Selection::Commit(c[1]);
    let sha = &c[1].to_string()[..7];
    let all = entries(&st);
    use crate::keymap::BindingId as I;
    let hint = |id| st.keymap.hint(id).map(str::to_owned);
    assert_eq!(find(&all, &format!("Reword commit {sha}…")), Some(&hint(I::RewordCommit)));
    assert_eq!(find(&all, &format!("Squash {sha} into parent")), Some(&hint(I::SquashCommit)));
    assert_eq!(find(&all, &format!("Drop commit {sha}…")), Some(&hint(I::DropCommit)));
    assert!(find(&all, "Continue rebase").is_none(), "no rebase in progress");

    let run = |st: &mut AppState, c: RebaseCommand| {
        update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Rebase(c))), Instant::now())
    };
    let fx = run(&mut st, RebaseCommand::Edit(QuickEdit::Reword(c[1])));
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::PrepareEdit { edit: QuickEdit::Reword(o), .. })] if *o == c[1]), "{fx:?}");

    // The first commit has no parent to squash into.
    let first = st.snap.as_ref().unwrap().commits.last().unwrap().oid;
    st.selection = Selection::Commit(first);
    let sha = first.to_string()[..7].to_owned();
    assert!(find(&entries(&st), &format!("Squash {sha} into parent")).is_none(), "a blocked edit isn't offered");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn entries_while_rebasing() {
    let (mut st, dir, _) = repo("palette-rebase-resume");
    st.selection = Selection::Wip;
    st.snap.as_mut().unwrap().state = RepositoryState::RebaseInteractive;
    let all = entries(&st);
    use crate::keymap::BindingId as I;
    assert_eq!(find(&all, "Continue rebase"), Some(&st.keymap.hint(I::RebaseContinue).map(str::to_owned)));
    assert_eq!(find(&all, "Skip commit (rebase)"), Some(&st.keymap.hint(I::RebaseSkip).map(str::to_owned)));
    assert!(find(&all, "Edit remaining rebase steps…").is_some());

    use crate::msg::RebaseMsg as R;
    assert!(matches!(RebaseCommand::Resume(Resume::Continue).msg(), Msg::Rebase(R::Resume(Resume::Continue))));
    assert!(matches!(RebaseCommand::Resume(Resume::Skip).msg(), Msg::Rebase(R::Resume(Resume::Skip))));
    assert!(matches!(RebaseCommand::EditRemaining.msg(), Msg::Rebase(R::OpenRemaining)));
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("c1.txt", '!'));
    let all = entries(&st);
    assert!(find(&all, "Continue rebase").is_none(), "conflicted files remain");
    assert!(find(&all, "Skip commit (rebase)").is_some());

    st.snap.as_mut().unwrap().state = RepositoryState::RebaseMerge;
    assert!(find(&entries(&st), "Edit remaining rebase steps…").is_none(), "only an interactive rebase");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_onto_the_selected_commit() {
    use super::selection_tests::{commit_on, git};
    use crate::msg::{JobMsg, RepoMsg};
    use trellis_core::rebase::RebaseBase;
    let (mut st, dir, c) = repo("palette-rebase-onto");
    git(&dir, &["checkout", "-q", "-b", "other", &c[0].to_string()]);
    std::fs::write(dir.join("o.txt"), "o\n").unwrap();
    let other = commit_on(&dir, "other", 9);
    git(&dir, &["checkout", "-q", "main"]);
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.selection = Selection::Commit(other);
    let sha = &other.to_string()[..7];
    let all = entries(&st);
    assert_eq!(find(&all, &format!("Rebase main onto {sha}")), Some(&None));
    assert_eq!(find(&all, &format!("Interactive rebase main onto {sha}…")), Some(&None));

    let plain = RebaseCommand::Onto { oid: other, interactive: false }.msg();
    let Msg::Job(JobMsg::Git { label, cmds }) = plain else { panic!("expected a git job") };
    assert_eq!((label.as_str(), cmds), ("Rebase", vec![trellis_core::gitcli::args(&["rebase", &other.to_string()])]));
    let msg = Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Rebase(RebaseCommand::Onto { oid: other, interactive: true })));
    let fx = update(&mut st, msg, Instant::now());
    let planned = matches!(&fx[..], [Effect::Rebase(RebaseEffect::LoadPlan { base: RebaseBase::OntoCommit(o), .. })] if *o == other);
    assert!(planned, "{fx:?}");

    // Not for HEAD, an ancestor, or a detached HEAD.
    st.rebase = None;
    for oid in [c[2], c[0]] {
        st.selection = Selection::Commit(oid);
        let sha = &oid.to_string()[..7];
        assert!(find(&entries(&st), &format!("Rebase main onto {sha}")).is_none());
    }
    st.selection = Selection::Commit(other);
    st.snap.as_mut().unwrap().head_branch = None;
    assert!(!entries(&st).iter().any(|(l, _)| l.contains(" onto ")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_original_order_while_the_panel_is_open() {
    let (st, dir, _) = repo("palette-rebase-restore-none");
    assert!(find(&entries(&st), "Restore original order").is_none(), "no panel open");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir, _) = with_panel("palette-rebase-restore", RebaseBase::FromCommit(git2::Oid::zero()));
    assert_eq!(find(&entries(&st), "Restore original order"), Some(&None));
    assert!(matches!(RebaseCommand::RestoreOrder.msg(), Msg::Rebase(crate::msg::RebaseMsg::RebaseReset)));
    let order = |st: &AppState| st.rebase.as_ref().unwrap().items.iter().filter_map(|s| s.oid()).collect::<Vec<_>>();
    let original = order(&st);
    update(&mut st, Msg::Rebase(crate::msg::RebaseMsg::RebaseMove { from: 0, to: 2 }), Instant::now());
    assert_ne!(order(&st), original);
    let run = Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Rebase(RebaseCommand::RestoreOrder)));
    update(&mut st, run, Instant::now());
    assert_eq!(order(&st), original, "back to the original order");

    st.rebase.as_mut().unwrap().stale = true;
    let all = entries(&st);
    assert!(find(&all, "Reload rebase steps").is_some() && find(&all, "Restore original order").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
