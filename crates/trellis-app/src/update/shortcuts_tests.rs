//! Headless tests for the newer keys: S/U on the open diff (its lines or the whole file), stage
//! and unstage all, going to the commit message, the shortcut sheet and its palette entry.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::lines::LineClick;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, Modal, PrefPage, PrefsState, Selection};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::askpass::Prompt;
use trellis_core::repo::{self, DiffLine, DiffTarget, FileDiff, FileEntry, Hunk, HunkOp, LoadOptions};
use trellis_core::settings::DiffMode;

/// One hunk: ` a`, `-b`, `+B`.
fn one_hunk() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![Hunk { header: "@@ -1,2 +1,2 @@".into(), lines: vec![line(' ', "a"), line('-', "b"), line('+', "B")] }];
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

fn unstaged() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

fn staged() -> DiffTarget {
    DiffTarget::Staged("f.txt".into())
}

/// A loaded repository (so effects have a working directory) showing `one_hunk` as `target`.
fn with_repo(name: &str, target: DiffTarget) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.diff = Some((target, one_hunk().into()));
    (st, dir)
}

fn key(st: &mut AppState, action: KeyAction) -> Vec<Effect> {
    update(st, Msg::Shortcut(action), Instant::now())
}

fn select_line(st: &mut AppState) {
    assert!(update(st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Only }), Instant::now()).is_empty());
    assert!(!st.view.diff_lines.is_empty());
}

fn op(fx: &[Effect]) -> Option<&LocalOp> {
    match fx {
        [Effect::LocalOp { op, .. }] => Some(op),
        _ => None,
    }
}

#[test]
fn s_stages_the_file_or_the_selected_lines() {
    let (mut st, dir) = with_repo("keys-stage", unstaged());
    let fx = key(&mut st, KeyAction::Stage);
    assert_eq!(op(&fx), Some(&LocalOp::StageFile("f.txt".into())), "{fx:?}");

    let generation = st.diff_gen;
    select_line(&mut st);
    let fx = key(&mut st, KeyAction::Stage);
    assert!(matches!(op(&fx), Some(LocalOp::Lines { op: HunkOp::Stage, lines, .. }) if *lines == [2]), "{fx:?}");
    assert!(st.view.diff_lines.is_empty(), "staging the lines clears the selection");
    assert_eq!(st.diff_gen, generation + 1);
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn u_unstages_the_file_or_the_selected_lines() {
    let (mut st, dir) = with_repo("keys-unstage", staged());
    let fx = key(&mut st, KeyAction::Unstage);
    assert_eq!(op(&fx), Some(&LocalOp::UnstagePaths(vec!["f.txt".into()])), "{fx:?}");
    select_line(&mut st);
    let fx = key(&mut st, KeyAction::Unstage);
    assert!(matches!(op(&fx), Some(LocalOp::Lines { op: HunkOp::Unstage, .. })), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_and_u_ignore_other_diffs_and_views() {
    let (mut st, dir) = with_repo("keys-ignored", staged());
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S on a staged diff");
    st.diff = Some((unstaged(), one_hunk().into()));
    assert!(key(&mut st, KeyAction::Unstage).is_empty(), "U on an unstaged diff");
    st.diff = Some((DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()), one_hunk().into()));
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S on a commit diff");
    st.diff = None;
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S with no diff");

    st.diff = Some((unstaged(), one_hunk().into()));
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S under the conflict view");
    st.conflict = None;
    st.history = Some(FileHistoryView::loading("f.txt".into(), None));
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S under the file history");
    st.history = None;
    st.prefs = Some(PrefsState::new(PrefPage::General));
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S under Preferences");
    st.prefs = None;

    st.snap.as_mut().unwrap().unstaged.push(FileEntry { path: "f.txt".into(), status: '!' });
    assert!(key(&mut st, KeyAction::Stage).is_empty(), "S on a conflicted file");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hidden_selection_stages_the_whole_file() {
    let (mut st, dir) = with_repo("keys-hidden", unstaged());
    select_line(&mut st);
    st.settings.diff_mode = DiffMode::WholeFile;
    let fx = key(&mut st, KeyAction::Stage);
    assert_eq!(op(&fx), Some(&LocalOp::StageFile("f.txt".into())), "{fx:?}");
    assert!(st.log.is_empty());

    st.settings.diff_mode = DiffMode::Hunks;
    st.settings.diff_ignore_whitespace = true;
    let fx = key(&mut st, KeyAction::Stage);
    assert_eq!(op(&fx), Some(&LocalOp::StageFile("f.txt".into())), "{fx:?}");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_and_unstage_all_need_files() {
    let (mut st, dir) = with_repo("keys-all", unstaged());
    assert_eq!(op(&key(&mut st, KeyAction::StageAll)), Some(&LocalOp::StageAll));
    assert!(key(&mut st, KeyAction::UnstageAll).is_empty(), "nothing staged");

    let snap = st.snap.as_mut().unwrap();
    snap.staged = std::mem::take(&mut snap.unstaged);
    assert!(key(&mut st, KeyAction::StageAll).is_empty(), "nothing unstaged");
    assert_eq!(op(&key(&mut st, KeyAction::UnstageAll)), Some(&LocalOp::UnstagePaths(vec!["a.txt".into()])));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_all_refuses_while_files_are_conflicted() {
    let (mut st, dir) = with_repo("keys-all-conflict", unstaged());
    st.snap.as_mut().unwrap().unstaged.push(FileEntry { path: "f.txt".into(), status: '!' });
    assert!(key(&mut st, KeyAction::StageAll).is_empty(), "the key stages nothing");
    assert!(update(&mut st, Msg::StageAll, Instant::now()).is_empty(), "nor does the button");
    let toast = st.toasts.last().expect("a toast says why");
    assert!(!toast.ok && toast.text.contains("conflict view"), "{}", toast.text);
    assert!(st.log.is_empty(), "a keypress doesn't open the log");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_sheet_key_works_under_preferences_or_a_credential_prompt() {
    let (mut st, dir) = with_repo("keys-covered", unstaged());
    let snap = st.snap.as_mut().unwrap();
    snap.staged = snap.unstaged.clone();
    let quiet = |st: &mut AppState, what: &str| {
        st.diff = Some((unstaged(), one_hunk().into()));
        for a in [KeyAction::Stage, KeyAction::StageAll, KeyAction::UnstageAll, KeyAction::FocusMessage] {
            assert!(key(st, a).is_empty(), "{a:?} under {what}");
        }
        st.diff = Some((staged(), one_hunk().into()));
        assert!(key(st, KeyAction::Unstage).is_empty(), "Unstage under {what}");
        assert!(!st.view.focus_message, "FocusMessage under {what}");
        assert_eq!(st.selection, Selection::None, "FocusMessage under {what}");
    };
    st.prefs = Some(PrefsState::new(PrefPage::General));
    quiet(&mut st, "Preferences");
    st.prefs = None;
    update(&mut st, Msg::CredentialPrompt { id: 1, prompt: Prompt::new("Password: ", None) }, Instant::now());
    assert!(st.askpass.showing());
    quiet(&mut st, "a credential prompt");

    st.askpass.queue.clear();
    assert_eq!(op(&key(&mut st, KeyAction::StageAll)), Some(&LocalOp::StageAll), "works again once it's gone");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn focus_message_selects_the_wip_panel() {
    let (mut st, dir) = with_repo("keys-focus", unstaged());
    st.selection = Selection::Commit(git2::Oid::zero());
    key(&mut st, KeyAction::FocusMessage);
    assert_eq!(st.selection, Selection::Wip);
    assert!(st.view.focus_message);

    let mut st = state();
    key(&mut st, KeyAction::FocusMessage);
    assert!(!st.view.focus_message, "no repository");
    assert_eq!(st.selection, Selection::None);

    let _ = std::fs::remove_dir_all(&dir);
    let (mut st, dir) = with_repo("keys-focus-prefs", unstaged());
    st.prefs = Some(PrefsState::new(PrefPage::General));
    key(&mut st, KeyAction::FocusMessage);
    assert!(!st.view.focus_message, "under Preferences");
    let _ = std::fs::remove_dir_all(&dir);
}

fn sheet_open(st: &AppState) -> bool {
    matches!(&st.modal, Some(Modal::Shortcuts { filter }) if filter.is_empty())
}

#[test]
fn ctrl_slash_toggles_the_sheet() {
    let mut st = state();
    update(&mut st, Msg::TogglePalette, Instant::now());
    assert!(st.palette.is_some());
    key(&mut st, KeyAction::ToggleSheet);
    assert!(sheet_open(&st));
    assert!(st.palette.is_none(), "the sheet replaces the palette");
    key(&mut st, KeyAction::ToggleSheet);
    assert!(st.modal.is_none());

    update(&mut st, Msg::confirm("Delete", "Sure?".into(), "Delete", Msg::Repo(RepoMsg::Refresh)), Instant::now());
    key(&mut st, KeyAction::ToggleSheet);
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "another dialog stays");

    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    key(&mut st, KeyAction::ToggleSheet);
    assert!(st.modal.is_none(), "not over Preferences");

    let mut st = state();
    update(&mut st, Msg::CredentialPrompt { id: 1, prompt: Prompt::new("Password: ", None) }, Instant::now());
    assert!(st.askpass.showing());
    key(&mut st, KeyAction::ToggleSheet);
    assert!(st.modal.is_none(), "not under a credential prompt");
}

#[test]
fn palette_entry_and_hints_come_from_the_table() {
    let (mut st, dir) = with_repo("keys-palette", unstaged());
    let find = |st: &AppState, label: &str| entries(st).into_iter().find(|e| e.label == label);
    let sheet = find(&st, "Keyboard shortcuts").expect("Keyboard shortcuts entry");
    assert_eq!(sheet.command, C::KeyboardShortcuts);
    let prefs_at = entries(&st).iter().position(|e| e.label == "Open Preferences").unwrap();
    assert_eq!(entries(&st)[prefs_at + 1].label, "Keyboard shortcuts", "right after Open Preferences");
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(sheet.shortcut, Some("Ctrl+/"));
        assert_eq!(find(&st, "Stage all files").unwrap().shortcut, Some("Ctrl+Shift+S"));
        assert_eq!(find(&st, "Refresh").unwrap().shortcut, Some("F5"));
        assert_eq!(find(&st, "Open Preferences").unwrap().shortcut, Some("Ctrl+,"));
    }
    st.snap.as_mut().unwrap().staged.push(FileEntry { path: "b.txt".into(), status: 'A' });
    #[cfg(not(target_os = "macos"))]
    assert_eq!(find(&st, "Unstage all files").unwrap().shortcut, Some("Ctrl+Shift+U"));

    update(&mut st, Msg::TogglePalette, Instant::now());
    update(&mut st, Msg::RunPaletteCommand(C::KeyboardShortcuts), Instant::now());
    assert!(st.palette.is_none());
    assert!(sheet_open(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_stages_the_open_file_end_to_end() {
    let dir = scratch_repo("keys-e2e");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    assert!(matches!(&st.diff, Some((DiffTarget::Unstaged(p), _)) if p == "a.txt"));
    drive(&mut st, Msg::Shortcut(KeyAction::Stage));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().any(|f| f.path == "a.txt"), "staged");
    assert!(!snap.unstaged.iter().any(|f| f.path == "a.txt"), "no longer unstaged");
    assert!(st.log.iter().all(|l| l.ok), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}
