//! Headless tests for "Stash changes and undo" when the undo doesn't go through: the stash goes
//! straight back when nothing was undone, a stash that couldn't be made undoes nothing, and the
//! toast names where the changes are whatever git printed.

use super::tests::{drive, git, state};
use super::undo_stash_tests::{cleanup, has_undo_stash, head_summary, read, stash_and_undo};
use crate::effect::{StashHow, UndoResult, UndoStash};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use glowfly_core::undo::{Action, MoveOp};
use std::path::PathBuf;

/// main has l.txt and cfg.local; feature deletes cfg.local, and main merged it (no fast-forward,
/// after "on main"). Then cfg.local is made again, untracked, and l.txt is edited.
fn merged_away(name: &str) -> (AppState, PathBuf) {
    let dir = super::tests::scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    let commit = |file: &str, msg: &str| {
        std::fs::write(dir.join(file), format!("{file}\n")).unwrap();
        git(&dir, &["add", file]);
        git(&dir, &["commit", "-q", "-m", msg]);
    };
    commit("l.txt", "base");
    commit("cfg.local", "config");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    git(&dir, &["rm", "-q", "cfg.local"]);
    git(&dir, &["commit", "-q", "-m", "drop config"]);
    git(&dir, &["checkout", "-q", "main"]);
    commit("m.txt", "on main");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::git("Merge", &["merge", "--no-edit", "feature"]));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { .. })), "{:?}", st.session.undo.undo);
    std::fs::write(dir.join("cfg.local"), "mine\n").unwrap();
    std::fs::write(dir.join("l.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

#[test]
fn a_file_stashing_wouldnt_move_refuses_before_offering_the_stash() {
    let (mut st, dir) = merged_away("undo-stash-not-offered");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.contains("Move cfg.local away first"), "{}", t.text);
    let label = t.action.as_ref().map(|a| a.label.as_str());
    assert_ne!(label, Some("Stash changes and undo"), "it would be refused after stashing");
    cleanup(&[&dir]);
}

#[test]
fn an_undo_refused_after_stashing_puts_the_changes_straight_back() {
    // A hard reset back past the commit that added cfg.local; then cfg.local is ignored and the
    // user's own, which stashing (untracked files too) doesn't move.
    let dir = super::tests::scratch_repo("undo-stash-refused-after");
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("cfg.local"), "config\n").unwrap();
    git(&dir, &["add", "cfg.local"]);
    git(&dir, &["commit", "-q", "-m", "config"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::git("Reset (hard)", &["reset", "--hard", "HEAD~1"]));
    std::fs::write(dir.join(".git/info/exclude"), "cfg.local\n").unwrap();
    std::fs::write(dir.join("cfg.local"), "mine\n").unwrap();
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = head_summary(&st);
    stash_and_undo(&mut st);
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.starts_with("Move cfg.local away first"), "{}", t.text);
    assert!(t.text.ends_with("Nothing was undone, and your changes are back as they were."), "{}", t.text);
    assert_eq!(head_summary(&st), before, "nothing was undone");
    assert_eq!((read(&dir, "a.txt"), read(&dir, "cfg.local")), ("edited\n".into(), "mine\n".into()));
    assert!(!has_undo_stash(&st), "popped back");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Reset { .. })), "the entry stays: {:?}", st.session.undo);
    cleanup(&[&dir]);
}

#[test]
fn intent_to_add_files_are_named_and_nothing_is_stashed() {
    let (mut st, dir) = merged_away("undo-stash-intent-to-add");
    std::fs::remove_file(dir.join("cfg.local")).unwrap();
    std::fs::write(dir.join("n.txt"), "new\n").unwrap();
    git(&dir, &["add", "-N", "n.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let merged = head_summary(&st);
    stash_and_undo(&mut st);
    let t = st.toasts.last().unwrap();
    assert!(!t.ok && t.text.starts_with("Nothing was stashed or undone: git can't stash n.txt"), "{}", t.text);
    assert_eq!(head_summary(&st), merged);
    assert!(!has_undo_stash(&st));
    cleanup(&[&dir]);
}

#[test]
fn a_stash_that_cant_be_made_undoes_nothing() {
    let (mut st, dir) = merged_away("undo-stash-push-fails");
    std::fs::remove_file(dir.join("cfg.local")).unwrap();
    let merged = head_summary(&st);
    let lock = dir.join(".git").join("index.lock");
    // The refusal first, then git can't take the index to stash.
    drive(&mut st, Msg::Job(JobMsg::Undo));
    std::fs::write(&lock, "").unwrap();
    let id = st.toasts.last().unwrap().id;
    drive(&mut st, Msg::ToastAction(id));
    std::fs::remove_file(&lock).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!st.toasts.last().unwrap().ok);
    assert_eq!(head_summary(&st), merged, "nothing was undone");
    assert_eq!(read(&dir, "l.txt"), "edited\n");
    assert!(!has_undo_stash(&st));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { .. })), "the entry stays: {:?}", st.session.undo);
    cleanup(&[&dir]);
}

#[test]
fn the_toast_names_the_stash_whatever_became_of_it() {
    let st = state();
    let o = git2::Oid::zero();
    let before = Action::Moved { op: MoveOp::Merge, detail: Some("feature".into()), branch: None, from: o, to: o };
    let stash = |how, reason: Option<&str>| {
        Some(UndoStash { message: "Glowfly: changes before undo #1".into(), how, reason: reason.map(Into::into) })
    };
    let done =
        |how| Some(UndoResult::Done { redo: false, before: before.clone(), after: Box::new(before.clone()), stash: stash(how, None) });
    let headline = |r: Option<UndoResult>| crate::update::undo::stash_headline(&st, &r).unwrap();

    let gone = headline(done(StashHow::Gone));
    assert!(gone.contains("\"Glowfly: changes before undo #1\" with your changes is no longer in the list"), "{gone}");
    let partial = headline(done(StashHow::Partial(vec!["p.txt".into(), "q.txt".into()])));
    assert!(partial.contains("couldn't put back p.txt, q.txt") && partial.contains("#1"), "{partial}");
    let held = headline(Some(UndoResult::Failed { redo: false, stash: stash(StashHow::Left, None) }));
    assert!(held.contains("Your changes are in the stash \"Glowfly: changes before undo #1\""), "{held}");
    let refused = Some(UndoResult::Refused { redo: false, stale: false, offer_stash: false, stash: stash(StashHow::Left, Some("Can't.")) });
    assert!(headline(refused).starts_with("Can't. Nothing was undone; your changes are in the stash"));
    assert_eq!(
        headline(Some(UndoResult::Failed { redo: false, stash: stash(StashHow::Back, None) })),
        "Nothing was undone, and your changes are back as they were. The log says why."
    );
    let gone = Some(UndoResult::Refused { redo: false, stale: false, offer_stash: false, stash: stash(StashHow::Gone, Some("Can't.")) });
    assert!(
        headline(gone)
            .starts_with("Can't. Nothing was undone, and the stash \"Glowfly: changes before undo #1\" with your changes is no longer")
    );
    assert!(
        headline(Some(UndoResult::Failed { redo: false, stash: stash(StashHow::Gone, None) }))
            .starts_with("Nothing was undone, and the stash")
    );
    assert!(crate::update::undo::stash_headline(&st, &Some(UndoResult::Failed { redo: false, stash: None })).is_none());
}
