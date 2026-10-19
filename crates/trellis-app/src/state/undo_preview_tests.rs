//! The Undo and Redo previews: a commit, a checkout that made a branch, a pull over made-up
//! commits in the graph, a pop, the Redo wording, and a preview for every kind of action.

use super::*;
use crate::state::op_fixture;
use crate::state::toolbar_tips::tests::diverge;
use trellis_core::repo::ConflictIds;
use trellis_core::undo::{Changes, Created, DeleteVia, MoveOp, ResetMode, Saved};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn lines(p: &TipPreview) -> Vec<String> {
    p.rows.iter().filter_map(|r| if let TipRow::Line(l) = r { Some(l.clone()) } else { None }).collect()
}

#[test]
fn a_commit_goes_back_to_its_parent() {
    let snap = op_fixture::snapshot("undo-preview-commit");
    let to = snap.head_oid.unwrap();
    let a = Action::Commit { branch: Some("main".into()), from: oid(7), to, message: "one".into(), amend: false };
    let p = preview(Lang::En, &a, Some(&snap), false);
    assert_eq!(p.heading, "Reverts");
    assert_eq!(p.rows[0], TipRow::Commit { sha: short_sha(to), summary: "one".into() });
    assert_eq!(lines(&p), [format!("main goes back to {}", short_sha(oid(7)))]);
    let redo = preview(Lang::En, &a, Some(&snap), true);
    assert_eq!(redo.heading, "Redoes");
    assert_eq!(lines(&redo), [format!("main moves to {} again", short_sha(to))]);
    let bare = preview(Lang::En, &a, None, false);
    assert_eq!(bare.rows[0], TipRow::Line(short_sha(to)), "no snapshot: the short SHA alone");
}

#[test]
fn a_checkout_that_made_a_branch_offers_to_delete_it() {
    let created = Created { name: "feature/x".into(), oid: oid(3), upstream: None };
    let a = Action::Checkout { from: HeadPos::Branch("dev".into()), to: HeadPos::Branch("feature/x".into()), created: Some(created) };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(lines(&p), ["Checks out dev again", "Asks whether to delete feature/x"]);
}

#[test]
fn a_pull_takes_off_the_commits_it_brought() {
    let mut snap = op_fixture::snapshot("undo-preview-pull");
    diverge(&mut snap, 3, 0, true);
    let base = snap.commits.last().unwrap().oid;
    let to = snap.head_oid.unwrap();
    let a = Action::Moved { op: MoveOp::Pull, detail: None, branch: Some("main".into()), from: base, to };
    let p = preview(Lang::En, &a, Some(&snap), false);
    assert_eq!(p.rows[0], TipRow::Line("Takes off 3 commits".into()));
    let commits = p.rows.iter().filter(|r| matches!(r, TipRow::Commit { .. })).count();
    assert_eq!(commits, 3);
    assert!(matches!(&p.rows[1], TipRow::Commit { summary, .. } if summary == "Ahead 3"), "newest first: {:?}", p.rows);
    assert_eq!(p.rows.last(), Some(&TipRow::Line(format!("main goes back to {}", short_sha(base)))));
    assert_eq!(p.note, None);
    assert_eq!(preview(Lang::En, &a, Some(&snap), true).rows[0], TipRow::Line("Puts back 3 commits".into()));
}

#[test]
fn a_pop_names_its_stash() {
    let a = Action::StashApply { pop: true, stash: oid(1), head: oid(2), message: "On main: wip".into(), left: Changes(Vec::new()) };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(lines(&p), ["On main: wip", "Takes the changes off and puts the stash back"]);
}

#[test]
fn every_action_has_a_preview() {
    let saved = Saved { blob: oid(5), executable: false };
    let ids = ConflictIds { ancestor: None, ours: Some(oid(1)), theirs: Some(oid(2)) };
    let branch = Some("main".to_owned());
    let all = [
        Action::Checkout { from: HeadPos::Detached(oid(1)), to: HeadPos::Branch("main".into()), created: None },
        Action::Commit { branch: branch.clone(), from: oid(1), to: oid(2), message: "m".into(), amend: true },
        Action::Reset { mode: ResetMode::Hard, branch: branch.clone(), from: oid(1), to: oid(2), stash: None },
        Action::DeleteBranch { name: "x".into(), oid: oid(1), upstream: None },
        Action::DiscardAll { stash: oid(1), head: oid(2) },
        Action::Discard { path: "a.txt".into(), what: Discarded::Hunk, before: Some(saved), after: None },
        Action::Restore { path: "a.txt".into(), oid: oid(1), before: None, after: Some(saved) },
        Action::CreateBranch { name: "x".into(), oid: oid(1), upstream: None },
        Action::CreateTag { name: "v1".into(), oid: oid(1) },
        Action::Moved { op: MoveOp::Merge, detail: Some("dev".into()), branch: None, from: oid(1), to: oid(2) },
        Action::StashPush { stash: oid(1), head: oid(2), message: "m".into(), replay: None },
        Action::StashApply { pop: false, stash: oid(1), head: oid(2), message: "m".into(), left: Changes(Vec::new()) },
        Action::StashDrop { stash: oid(1), message: "m".into() },
        Action::Resolve { path: "a.txt".into(), ids, head: None, staged: None, before: None, after: None },
        Action::DeleteFile { path: "a.txt".into(), before: saved, via: DeleteVia::Permanent, staged: None },
    ];
    for a in &all {
        for redo in [false, true] {
            let p = preview(Lang::En, a, None, redo);
            assert!(!p.rows.is_empty(), "{a:?} (redo {redo})");
            for l in lines(&p) {
                assert!(!l.contains('{') && !l.starts_with("undo."), "{a:?}: {l}");
            }
        }
    }
}
