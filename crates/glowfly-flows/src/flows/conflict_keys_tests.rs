//! The conflict keys end to end, pressed as chords through the default keymap: O, F7, T and
//! Ctrl+Enter resolve a file with two conflicts, and a rebase through three conflicted files
//! finishes with Alt+C, without the mouse.

use super::conflict_tests::opened_two;
use super::tests::{drive, git, scratch_repo, state};
use crate::keymap::{BindingId, Chord, KeyName, When, binding, fold_ctrl};
use crate::msg::{ConflictMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::conflict::keys::conflict_key;
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run};
use glowfly_core::repo::Choice;
use std::path::{Path, PathBuf};

/// Presses `chord` with nothing but the conflict view open: the binding the keymap finds, sent
/// as `ui/shortcuts/global.rs` sends it. Panics when the key does nothing.
fn press(st: &mut AppState, chord: Chord) {
    let id = st.keymap.lookup(chord, fold_ctrl()).unwrap_or_else(|| panic!("{chord:?} is bound"));
    let msg = match id {
        BindingId::NextChange => Msg::Conflict(ConflictMsg::ConflictStep(1)),
        BindingId::PrevChange => Msg::Conflict(ConflictMsg::ConflictStep(-1)),
        _ => conflict_key(st, id).unwrap_or_else(|| panic!("{id:?} does something now")),
    };
    drive(st, msg);
}

/// Writes `files` with `text` each and commits them as `msg`.
fn commit(dir: &Path, files: &[&str], text: &str, msg: &str) {
    for f in files {
        std::fs::write(dir.join(f), text).unwrap();
    }
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

/// A merge where f.txt conflicts twice, far enough apart to stay two conflicts.
fn two_in_one(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let file = |a: &str, b: &str| format!("{a}\n1\n2\n3\n4\n5\n6\n7\n{b}\n");
    commit(&dir, &["f.txt"], &file("top", "end"), "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, &["f.txt"], &file("their top", "their end"), "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &["f.txt"], &file("our top", "our end"), "ours");
    assert!(!run(&dir, &args(&["merge", "other"])).0, "the merge conflicts");
    dir
}

#[test]
fn the_conflict_keys_bind_where_the_issue_says() {
    use KeyName as K;
    for (id, chord) in [
        (BindingId::ConflictTakeOurs, Chord::plain(K::Letter('O'))),
        (BindingId::ConflictTakeTheirs, Chord::plain(K::Letter('T'))),
        (BindingId::ConflictTakeBoth, Chord::plain(K::Letter('B'))),
        (BindingId::ConflictTakeBothTheirsFirst, Chord::shift(K::Letter('B'))),
        (BindingId::ConflictClear, Chord::plain(K::Backspace)),
        (BindingId::ConflictMarkResolved, Chord::cmd(K::Enter)),
    ] {
        let b = binding(id);
        assert_eq!((b.defaults, b.when, b.rebindable), (&[chord][..], When::Conflict, true), "{id:?}");
        assert_eq!(b.group.title(), "Diff and conflicts");
    }
}

#[test]
fn o_f7_t_and_ctrl_enter_resolve_a_file_with_two_conflicts() {
    let dir = two_in_one("conflict-keys-two");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("f.txt".into())));
    assert_eq!(st.session.conflict.view.as_ref().map(|c| c.chunk_count()), Some(2));

    press(&mut st, Chord::plain(KeyName::Letter('O')));
    press(&mut st, Chord::plain(KeyName::F(7)));
    press(&mut st, Chord::plain(KeyName::Letter('T')));
    let c = st.session.conflict.view.as_ref().unwrap();
    assert_eq!((c.current, c.can_save()), (1, true));
    press(&mut st, Chord::cmd(KeyName::Enter));

    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "our top\n1\n2\n3\n4\n5\n6\n7\ntheir end\n");
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.unstaged.iter().any(|f| f.status == '!'), "marked resolved");
    assert!(st.session.conflict.view.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn backspace_clears_and_b_keys_take_both_in_order() {
    let (mut st, dir) = opened_two("conflict-keys-both");
    let pick = |st: &AppState| st.session.conflict.view.as_ref().unwrap().picks[0].clone();
    press(&mut st, Chord::plain(KeyName::Letter('B')));
    assert_eq!(pick(&st), glowfly_core::conflict_pick::ChunkPick::Sides(Choice::OursThenTheirs));
    press(&mut st, Chord::shift(KeyName::Letter('B')));
    assert_eq!(pick(&st), glowfly_core::conflict_pick::ChunkPick::Sides(Choice::TheirsThenOurs));
    press(&mut st, Chord::plain(KeyName::Backspace));
    assert!(pick(&st).is_open(), "cleared");
    // Mark resolved is disabled again, so Ctrl+Enter does nothing.
    let id = st.keymap.lookup(Chord::cmd(KeyName::Enter), fold_ctrl()).unwrap();
    assert!(conflict_key(&st, id).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rebase of `feature` onto main stopped on one commit that conflicts in three files.
fn rebase_three(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let files = ["a.txt", "b.txt", "c.txt"];
    commit(&dir, &files, "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, &files, "feature\n", "feature");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &files, "main\n", "main");
    git(&dir, &["checkout", "-q", "feature"]);
    assert!(!run(&dir, &args(&["rebase", "main"])).0, "the rebase stops on conflicts");
    dir
}

/// The designer's sign-off check: open the first file, then only keys.
#[test]
fn a_rebase_through_three_conflicted_files_finishes_from_the_keyboard() {
    let dir = rebase_three("conflict-keys-rebase");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    for file in ["a.txt", "b.txt", "c.txt"] {
        assert_eq!(st.session.conflict.view.as_ref().map(|c| c.path.as_str()), Some(file), "steps to the next file");
        press(&mut st, Chord::plain(KeyName::Letter('T')));
        press(&mut st, Chord::cmd(KeyName::Enter));
    }
    assert!(st.session.conflict.view.is_none(), "the last file closes the view");
    let alt_c = Chord::alt(KeyName::Letter('C'));
    let id = st.keymap.lookup(alt_c, fold_ctrl());
    assert_eq!(id, Some(BindingId::RebaseContinue));
    press(&mut st, alt_c);
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean, "the rebase finished");
    for f in ["a.txt", "b.txt", "c.txt"] {
        assert_eq!(std::fs::read_to_string(dir.join(f)).unwrap(), "feature\n");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
