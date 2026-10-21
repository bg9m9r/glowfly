//! Headless tests for the graph's squashes, which ask for the combined message first, and for
//! history edits with a merge later on the branch.

use super::rebase_quick_tests::{form, log, make_dirty, prepare, short, started, up};
use super::rebase_tests::repo;
use super::selection_tests::{commit_on, git, head};
use super::tests::drive;
use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::Modal;
use crate::state::rebase_quick::{self, BLANK};
use glowfly_core::rebase::{QuickEdit, RebaseAction};

#[test]
fn squash_opens_its_message_dialog() {
    let (mut st, dir, c) = repo("quick-squash-dialog");
    assert!(prepare(&mut st, QuickEdit::Squash(vec![c[1], c[0]])).is_empty());
    let f = form(&mut st);
    assert!(f.squashes());
    assert_eq!(f.draft, "c1\n\nc2", "the joined messages, oldest first");
    assert_eq!(
        (f.title(), f.hint(), f.button(false), f.button(true)),
        ("Squash commits", "Message of the combined commit", "Squash", "Stash and squash")
    );
    assert_eq!(f.subtitle(), "2 commits on main become one · rewrites 1 later commit");
    assert_eq!(f.blocker(true), None, "an unchanged message is fine for a squash");
    f.draft = " \n".into();
    assert_eq!(f.blocker(true), Some(BLANK));
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword)).is_empty());

    form(&mut st).draft = "One commit for both\n\n".into();
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    let request = started(&fx);
    let first = request.items[0].commit().unwrap();
    assert_eq!((first.action, first.new_message.as_deref()), (RebaseAction::Reword, Some("One commit for both")));
    assert_eq!(request.items[1].commit().unwrap().action, RebaseAction::Fixup);
    assert!(!request.autostash && st.modal.is_none());
    st.job = None;

    // Into the parent, driven end to end.
    let tip = head(&dir);
    drive(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::SquashIntoParent(tip))));
    let f = form(&mut st);
    assert_eq!(f.subtitle(), format!("{} into its parent {} on main · no later commits change", short(tip), short(c[1])));
    assert_eq!(f.draft, "c2\n\nc3");
    f.draft = "Two and three".into();
    drive(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    assert_eq!(log(&dir), ["Two and three", "c1", "first"]);
    assert_eq!(git(&dir, &["log", "-1", "--format=%B"]).trim_end(), "Two and three");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn squash_on_a_dirty_tree_stashes_from_the_dialog() {
    let (mut st, dir, c) = repo("quick-squash-dirty");
    make_dirty(&mut st, &dir);
    assert!(prepare(&mut st, QuickEdit::Squash(vec![c[2], c[1]])).is_empty());
    assert!(matches!(st.modal, Some(Modal::RewordCommit(_))), "no separate confirmation");
    let request = started(&up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword))).clone();
    assert!(request.autostash, "the dialog said the changes are stashed");
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edits_across_a_merge() {
    let (mut st, dir, c) = repo("quick-across-merge");
    // A side branch from c3, merged, then one more commit.
    git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("s.txt"), "s\n").unwrap();
    commit_on(&dir, "side", 10);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    std::fs::write(dir.join("after.txt"), "after\n").unwrap();
    commit_on(&dir, "after", 12);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(rebase_quick::single(st.snap.as_ref().unwrap(), c[1]).drop, None);

    drive(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(vec![c[1]]))));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!log(&dir).contains(&"c2".to_owned()), "{:?}", log(&dir));
    assert!(!dir.join("c2.txt").exists() && dir.join("s.txt").exists());
    let parents = git(&dir, &["log", "-1", "--format=%P", "HEAD~1"]);
    assert_eq!(parents.split_whitespace().count(), 2, "HEAD~1 is still the merge");
    let _ = std::fs::remove_dir_all(&dir);
}
