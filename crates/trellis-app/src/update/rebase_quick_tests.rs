//! Headless tests for the graph's history edits: preparing, asking, the Reword dialog, running
//! them end to end, and the menu blockers.

use super::rebase_tests::{repo, running_job};
use super::selection_tests::{commit_on, git, head};
use super::tests::{drive, same_place};
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::rebase::STASH_LINE;
use crate::state::rebase_quick::{self, QUICK_STALE};
use crate::state::{AppState, Modal};
use git2::{Oid, RepositoryState};
use std::path::Path;
use std::time::Instant;
use trellis_core::rebase::{self, QuickEdit, RebaseAction, StartRequest};

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

fn log(dir: &Path) -> Vec<String> {
    git(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

fn last_toast(st: &AppState) -> (bool, &str) {
    st.toasts.last().map(|t| (t.ok, t.text.as_str())).unwrap_or((false, ""))
}

/// `edit` chosen in the menu, its preparation run, and what `update` makes of the result.
fn prepare(st: &mut AppState, edit: QuickEdit) -> Vec<Effect> {
    let fx = up(st, Msg::Rebase(RebaseMsg::Edit(edit)));
    let [e @ Effect::Rebase(RebaseEffect::PrepareEdit { .. })] = &fx[..] else { panic!("expected a preparation, got {fx:?}") };
    assert!(e.scope().is_some(), "the result comes back for this tab only");
    let msg = crate::runner::execute(fx.into_iter().next().unwrap()).expect("a result");
    up(st, msg)
}

fn started(fx: &[Effect]) -> &StartRequest {
    match fx {
        [Effect::Rebase(RebaseEffect::Start { request, .. })] => request,
        other => panic!("expected a rebase, got {other:?}"),
    }
}

fn confirm(st: &AppState) -> (&str, &str, &str) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. }) => (title, body, button),
        _ => panic!("expected a confirmation"),
    }
}

fn form(st: &mut AppState) -> &mut rebase_quick::RewordForm {
    match st.modal.as_mut() {
        Some(Modal::RewordCommit(f)) => f,
        _ => panic!("expected the Reword dialog"),
    }
}

/// `a.txt` edited, and the snapshot refreshed so it shows.
fn make_dirty(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join("a.txt"), "one\nedited\n").unwrap();
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

#[test]
fn edits_are_prepared_or_refused() {
    let (mut st, dir, c) = repo("quick-refused");
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(c[1]))));
    match &fx[..] {
        [Effect::Rebase(RebaseEffect::PrepareEdit { workdir, edit })] => {
            assert!(same_place(workdir, &dir));
            assert_eq!(*edit, QuickEdit::Reword(c[1]));
        }
        other => panic!("expected a preparation, got {other:?}"),
    }
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(c[1])))).is_empty());
    assert_eq!(last_toast(&st), (false, "Finish or abort the merge in progress before starting an interactive rebase."));
    st.snap.as_mut().unwrap().state = RepositoryState::Clean;
    st.snap.as_mut().unwrap().head_branch = None;
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(c[1])))).is_empty());
    assert_eq!(last_toast(&st), (false, rebase::DETACHED));
    st.snap.as_mut().unwrap().head_branch = Some("main".into());
    st.job = Some(running_job(&dir));
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(vec![c[0]])))).is_empty());
    assert_eq!(last_toast(&st), (false, "Another operation is still running."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_reword_dialog() {
    let (mut st, dir, c) = repo("quick-reword-dialog");
    assert!(prepare(&mut st, QuickEdit::Reword(c[1])).is_empty());
    let f = form(&mut st);
    assert_eq!((f.draft.as_str(), f.original()), ("c2", "c2\n"));
    assert_eq!(f.subtitle(), format!("{} on main · rewrites this commit and 1 later one", short(c[1])));
    // Unchanged or blank: nothing happens.
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword)).is_empty());
    form(&mut st).draft = "  \n".into();
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword)).is_empty());
    assert!(matches!(st.modal, Some(Modal::RewordCommit(_))));

    form(&mut st).draft = "c2, reworded\n\n".into();
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    let request = started(&fx);
    assert_eq!(request.items[0].action, RebaseAction::Reword);
    assert_eq!(request.items[0].new_message.as_deref(), Some("c2, reworded"));
    assert!(!request.autostash);
    assert!(st.modal.is_none(), "the dialog closes once the job started");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(rebase_quick::REWORD_LABEL));

    // On a dirty tree the dialog was the question: the changes are stashed.
    st.job = None;
    make_dirty(&mut st, &dir);
    prepare(&mut st, QuickEdit::Reword(c[1]));
    form(&mut st).draft = "c2, again".into();
    // While a job runs the dialog stays open.
    st.job = Some(running_job(&dir));
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword)).is_empty());
    assert!(matches!(st.modal, Some(Modal::RewordCommit(_))));
    st.job = None;
    assert!(started(&up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword))).autostash);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn drops_ask_and_squashes_ask_only_on_a_dirty_tree() {
    let (mut st, dir, c) = repo("quick-asks");
    assert!(prepare(&mut st, QuickEdit::Drop(vec![c[0]])).is_empty());
    let (title, body, button) = confirm(&st);
    assert_eq!((title, button), ("Drop commit", "Drop commit"));
    assert_eq!(body, format!("Drop {} \"c1\" from main? Its changes are removed and the 2 commits after it are rewritten.", short(c[0])));
    let request = started(&up(&mut st, Msg::ConfirmAccepted)).clone();
    assert_eq!((request.items[0].action, request.autostash), (RebaseAction::Drop, false));
    st.job = None;

    let fx = prepare(&mut st, QuickEdit::SquashIntoParent(c[2]));
    assert_eq!(started(&fx).items[1].action, RebaseAction::Fixup);
    assert!(st.modal.is_none(), "a squash on a clean tree runs straight away");
    st.job = None;

    make_dirty(&mut st, &dir);
    assert!(prepare(&mut st, QuickEdit::Squash(vec![c[2], c[1]])).is_empty());
    let (title, body, button) = confirm(&st);
    assert_eq!((title, button), ("Stash changes and squash", "Squash"));
    assert_eq!(body, format!("Squash 2 commits into one?\n\n{STASH_LINE}"));
    assert!(started(&up(&mut st, Msg::ConfirmAccepted)).autostash);
    st.job = None;

    prepare(&mut st, QuickEdit::Drop(vec![c[2], c[0]]));
    let (title, body, button) = confirm(&st);
    assert_eq!((title, button), ("Drop commits", "Drop 2 commits"));
    assert!(body.starts_with("Drop 2 commits from main?") && body.ends_with(STASH_LINE), "{body}");
    assert!(started(&up(&mut st, Msg::ConfirmAccepted)).autostash);
    st.job = None;

    // Results for another tab, or with a dialog open, are dropped.
    let plan = rebase::prepare_edit(&dir, &QuickEdit::Drop(vec![c[0]])).unwrap();
    let other = dir.join("elsewhere");
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::EditPrepared { workdir: other, result: Ok(plan.clone()) })).is_empty());
    assert!(st.modal.is_none());
    st.modal = Some(Modal::About);
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::EditPrepared { workdir: dir.clone(), result: Ok(plan) })).is_empty());
    assert!(matches!(st.modal, Some(Modal::About)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edits_end_to_end() {
    let (mut st, dir, c) = repo("quick-e2e");
    drive(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(c[1]))));
    form(&mut st).draft = "c2, reworded".into();
    drive(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    assert_eq!(last_toast(&st), (true, "Reword commit succeeded"));
    assert_eq!(log(&dir), ["c3", "c2, reworded", "c1", "first"]);

    let tip = head(&dir);
    drive(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::SquashIntoParent(tip))));
    assert_eq!(last_toast(&st), (true, "Squash commits succeeded"));
    assert_eq!(log(&dir), ["c2, reworded", "c1", "first"]);
    assert_eq!(git(&dir, &["log", "-1", "--format=%B"]).trim_end(), "c2, reworded\n\nc3");

    // Two commits dropped from a multi-selection, with a dirty tree.
    make_dirty(&mut st, &dir);
    let snap = st.snap.as_ref().unwrap();
    let both = vec![snap.commits[0].oid, snap.commits[1].oid];
    drive(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(both))));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(last_toast(&st), (true, "Drop commits succeeded"));
    assert_eq!(log(&dir), ["first"]);
    // Line endings follow the machine's core.autocrlf once git writes the file back.
    let text = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, "one\nedited\n", "the edit is back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_edit_changes_nothing() {
    let (mut st, dir, c) = repo("quick-stale");
    prepare(&mut st, QuickEdit::Reword(c[1]));
    std::fs::write(dir.join("c4.txt"), "4\n").unwrap();
    let moved = commit_on(&dir, "c4", 9);
    form(&mut st).draft = "too late".into();
    drive(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    assert_eq!(last_toast(&st), (false, format!("Reword commit failed: {QUICK_STALE}").as_str()));
    assert_eq!(head(&dir), moved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn menu_blockers() {
    let (mut st, dir, c) = repo("quick-blockers");
    let snap = st.snap.as_ref().unwrap();
    let first = snap.commits[3].oid;
    let b = rebase_quick::single(snap, first);
    assert_eq!((b.reword, b.squash.as_deref(), b.drop), (None, Some(rebase::ROOT_SQUASH), None));
    assert_eq!(rebase_quick::single(snap, c[1]), Default::default());
    assert_eq!(rebase_quick::multi(snap, &[c[2], c[0]]).squash.as_deref(), Some(rebase::NOT_CONSECUTIVE));
    assert_eq!(rebase_quick::multi(snap, &[c[2], c[1]]), Default::default());
    let all = [c[2], c[1], c[0], first];
    assert_eq!(rebase_quick::multi(snap, &all).drop.as_deref(), Some(rebase::DROPS_EVERYTHING));

    // A branch merged after c3, another never merged, and a commit after the merge.
    git(&dir, &["checkout", "-q", "-b", "side", &c[0].to_string()]);
    std::fs::write(dir.join("s.txt"), "s\n").unwrap();
    let side = commit_on(&dir, "side", 10);
    git(&dir, &["checkout", "-q", "-b", "other", &c[0].to_string()]);
    std::fs::write(dir.join("o.txt"), "o\n").unwrap();
    let other = commit_on(&dir, "other", 11);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let merge = head(&dir);
    std::fs::write(dir.join("after.txt"), "after\n").unwrap();
    let after = commit_on(&dir, "after", 12);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(rebase_quick::single(snap, side).reword.as_deref(), Some(rebase::MERGE_AFTER));
    assert_eq!(rebase_quick::single(snap, c[1]).drop.as_deref(), Some(rebase::MERGE_AFTER));
    assert_eq!(rebase_quick::single(snap, merge).reword.as_deref(), Some(rebase::MERGE_SELECTED));
    assert_eq!(rebase_quick::single(snap, other).reword.as_deref(), Some(rebase::NOT_ON_BRANCH));
    assert_eq!(rebase_quick::single(snap, after).squash.as_deref(), Some(rebase::PARENT_MERGE));
    assert_eq!(rebase_quick::single(snap, after).reword, None);
    assert_eq!(rebase_quick::multi(snap, &[after, merge]).squash.as_deref(), Some(rebase::SOME_MERGE_SELECTED));

    st.snap.as_mut().unwrap().state = RepositoryState::RebaseInteractive;
    let snap = st.snap.as_ref().unwrap();
    let text = rebase::in_progress_text(RepositoryState::RebaseInteractive);
    assert_eq!(rebase_quick::single(snap, after).reword, text);
    assert_eq!(rebase_quick::multi(snap, &[after, c[2]]).drop, text);
    let _ = std::fs::remove_dir_all(&dir);
}
