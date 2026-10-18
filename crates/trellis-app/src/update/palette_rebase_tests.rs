//! The palette's history entries: reword, squash and drop of the selected commit with their
//! keys, and Continue, Skip and Edit remaining steps while a rebase is in progress.

use super::rebase_tests::repo;
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, PaletteMsg};
use crate::state::palette::shown;
use crate::state::palette_commands::PaletteCommand;
use crate::state::palette_commands::palette_commands_rebase::RebaseCommand;
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use std::time::Instant;
use trellis_core::rebase::{QuickEdit, Resume};
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
