//! Headless tests for Space in a focused Unstaged or Staged Files list when the open diff has a
//! line selection: it does what S or U does (the selected lines of the current file, the picked
//! files before them, the whole file without a selection), and nothing where S or U does nothing.

use super::wip_picks_actions_tests::{a_and_c, local_op, wip_key, x_and_z};
use super::wip_picks_tests::{lists, send, un};
use crate::effect::{Effect, FilesEffect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::wip_lists::cursor;
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk, HunkOp};
use glowfly_core::settings::DiffMode;

/// One hunk: ` a`, `-b`, `+B`, `+c`.
fn diff() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let lines = vec![line(' ', "a"), line('-', "b"), line('+', "B"), line('+', "c")];
    FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1,2 +1,3 @@".into(), lines }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
        old_lines: None,
    }
}

/// The diff of the file under the keys arriving, so it is on screen.
fn arrive(st: &mut AppState) {
    let target = cursor(st).cloned().unwrap();
    let generation = st.session.diff.generation;
    send(st, Msg::Repo(RepoMsg::DiffLoaded { target, reload: false, generation, result: Ok(diff()), lfs: None, raw: None, gitlink: None }));
}

fn select(st: &mut AppState, line: usize) {
    send(st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how: LineClick::Toggle }));
}

/// Whether `fx` stages or unstages whole files.
fn whole(fx: &[Effect]) -> bool {
    fx.iter().any(|e| {
        matches!(
            e,
            Effect::Files(FilesEffect::LocalOp {
                op: LocalOp::StageFile(_) | LocalOp::StagePaths(_) | LocalOp::UnstagePaths(_) | LocalOp::StageAll,
                ..
            })
        )
    })
}

/// A state in the hunk view with a.txt's diff on screen and its two added lines selected.
fn selected(name: &str) -> AppState {
    let mut st = lists(name);
    st.settings.diff_mode = DiffMode::Hunks;
    arrive(&mut st);
    select(&mut st, 2);
    select(&mut st, 3);
    st
}

#[test]
fn space_stages_the_selected_lines_as_s_does() {
    let mut st = selected("space-lines");
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    let op = local_op(&fx).expect("a line operation");
    assert!(matches!(op, LocalOp::Lines { op: HunkOp::Stage, path, hunk: 0, lines, .. } if path == "a.txt" && *lines == [2, 3]), "{op:?}");
    assert!(!whole(&fx));
    assert!(st.cues.diff_lines.is_empty());
    assert!(st.cues.stage_move_on.is_none());
    let mut twin = selected("space-lines-twin");
    let fx_s = send(&mut twin, Msg::Shortcut(KeyAction::Stage));
    assert_eq!(local_op(&fx_s), Some(op));
}

#[test]
fn space_in_staged_files_unstages_the_selected_lines_as_u_does() {
    let ready = |name: &str| {
        let mut st = lists(name);
        st.settings.diff_mode = DiffMode::Hunks;
        send(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("x.txt".into()))));
        arrive(&mut st);
        select(&mut st, 2);
        st
    };
    let mut st = ready("space-lines-staged");
    let fx = wip_key(&mut st, true, ListKey::Toggle);
    let op = local_op(&fx).expect("a line operation");
    assert!(matches!(op, LocalOp::Lines { op: HunkOp::Unstage, path, lines, .. } if path == "x.txt" && *lines == [2]), "{op:?}");
    assert!(!whole(&fx));
    let mut twin = ready("space-lines-staged-twin");
    let fx_u = send(&mut twin, Msg::Shortcut(KeyAction::Unstage));
    assert_eq!(local_op(&fx_u), Some(op));
}

#[test]
fn space_without_a_selection_takes_the_whole_file() {
    let mut st = lists("space-lines-none");
    st.settings.diff_mode = DiffMode::Hunks;
    arrive(&mut st);
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("a.txt".into())));
    assert!(st.cues.stage_move_on.is_some());

    let mut st = lists("space-lines-none-staged");
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("x.txt".into()))));
    arrive(&mut st);
    let fx = wip_key(&mut st, true, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["x.txt".into()])));
}

#[test]
fn space_on_another_file_takes_that_file_whole() {
    let mut st = selected("space-lines-other");
    wip_key(&mut st, false, ListKey::Move(ListMove::Step { down: true }));
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("b.txt".into())));

    // A selection that belongs to another diff is not this file's.
    let mut st = selected("space-lines-foreign");
    st.cues.diff_lines.target = Some(un("b.txt"));
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("a.txt".into())));
}

#[test]
fn space_in_the_other_list_leaves_the_lines_alone() {
    let mut st = selected("space-lines-other-list");
    let fx = wip_key(&mut st, true, ListKey::Toggle);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(!st.cues.diff_lines.is_empty());
}

#[test]
fn several_picked_files_win_over_the_selected_lines() {
    let mut st = a_and_c("space-lines-picks");
    st.settings.diff_mode = DiffMode::Hunks;
    arrive(&mut st);
    select(&mut st, 2);
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StagePaths(vec!["a.txt".into(), "c.txt".into()])));

    let mut st = x_and_z("space-lines-picks-staged");
    st.settings.diff_mode = DiffMode::Hunks;
    arrive(&mut st);
    select(&mut st, 2);
    let fx = wip_key(&mut st, true, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["x.txt".into(), "z.txt".into()])));
}

#[test]
fn a_hidden_selection_takes_the_whole_file() {
    let mut st = selected("space-lines-hidden");
    st.settings.diff_mode = DiffMode::WholeFile;
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("a.txt".into())));
}

#[test]
fn space_on_a_conflicted_file_does_nothing() {
    let mut st = selected("space-lines-conflicted");
    st.snap.as_mut().unwrap().unstaged[0].status = '!';
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.modal.is_none());
    assert!(!st.cues.diff_lines.is_empty());
}

#[test]
fn a_refused_line_operation_stages_nothing() {
    let mut st = selected("space-lines-refused");
    st.session.diff.generation += 1;
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert!(local_op(&fx).is_none(), "{fx:?}");
    assert!(st.log.last().is_some_and(|l| !l.ok), "the refusal is logged");
}
