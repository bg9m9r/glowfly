//! The preview's pure rules: Apply sends what the menus send, staleness, resolving names, and
//! the title and headline texts.

use super::*;
use crate::msg::JobMsg;
use crate::state::op_fixture::{rows, snapshot};
use glowfly_core::gitcli::args;
use glowfly_core::op_preview::{Count, Request, Stop};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// A loaded one-commit snapshot on `main`, with a local `feature`, a remote-tracking
/// `origin/x` and HEAD at its commit.
fn snap(name: &str) -> Snapshot {
    let mut s = snapshot(name);
    s.head_branch = Some("main".into());
    let mut feature = s.locals[0].clone();
    feature.name = "feature".into();
    feature.oid = Some(oid(2));
    feature.is_head = false;
    s.locals.push(feature);
    s.remotes = vec![("origin".into(), vec![("x".into(), oid(3))])];
    s
}

fn job(s: &Snapshot, op: PreviewOp) -> OpPreviewJob {
    OpPreviewJob { workdir: s.workdir.clone(), request: Request { op, head: s.head_oid.unwrap() } }
}

fn git_cmd(m: &Msg) -> Option<(String, Vec<Vec<String>>)> {
    match m {
        Msg::Job(JobMsg::Git { label, cmds }) => Some((label.clone(), cmds.clone())),
        _ => None,
    }
}

fn preview(s: &Snapshot, op: PreviewOp, outcome: Outcome) -> OpPreview {
    OpPreview {
        op,
        head: s.head_oid.unwrap(),
        subject: Some("main".into()),
        outcome,
        after: Vec::new(),
        after_more: false,
        moved: Vec::new(),
        gone: Vec::new(),
        gone_count: Count { n: 0, more: false },
        arriving: None,
        patch_check_skipped: false,
    }
}

fn line(n: u8, summary: &str) -> CommitLine {
    CommitLine { oid: oid(n), summary: summary.into(), author: "A".into(), time: 0 }
}

#[test]
fn apply_sends_what_the_menus_send() {
    let s = snap("opp-apply");
    let t = Lang::En;
    let merge = apply_msg(t, &s, &PreviewOp::Merge { theirs: "feature".into(), oid: oid(2) }, None).unwrap();
    assert_eq!(git_cmd(&merge), Some(("Merge".into(), vec![args(&["merge", "feature"])])));
    let rebase = apply_msg(t, &s, &PreviewOp::Rebase { onto: "origin/x".into(), oid: oid(3) }, None).unwrap();
    assert_eq!(git_cmd(&rebase), Some(("Rebase".into(), vec![args(&["rebase", "origin/x"])])));
    let to = oid(4);
    let reset = PreviewOp::Reset { to };
    let soft = apply_msg(t, &s, &reset, Some(ResetMode::Soft)).unwrap();
    assert_eq!(git_cmd(&soft), Some(("Reset (soft)".into(), vec![args(&["reset", "--soft", &to.to_string()])])));
    let mixed = apply_msg(t, &s, &reset, Some(ResetMode::Mixed)).unwrap();
    assert_eq!(git_cmd(&mixed), Some(("Reset (mixed)".into(), vec![args(&["reset", "--mixed", &to.to_string()])])));
    assert!(matches!(apply_msg(t, &s, &reset, Some(ResetMode::Hard)), Some(Msg::ConfirmDanger { .. })), "a clean tree confirms first");
    assert!(apply_msg(t, &s, &reset, None).is_none());
    let pick = apply_msg(t, &s, &PreviewOp::CherryPick(vec![oid(5), oid(6)]), None).unwrap();
    assert!(matches!(pick, Msg::Selection(SelectionMsg::CherryPick(ref v)) if *v == [oid(5), oid(6)]));
    let drop = apply_msg(t, &s, &PreviewOp::Drop(vec![oid(5)]), None).unwrap();
    assert!(matches!(drop, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(ref v))) if *v == [oid(5)]));
}

#[test]
fn names_resolve_to_local_remote_and_sha() {
    let s = snap("opp-resolve");
    assert_eq!(resolve(&s, "feature"), Some(oid(2)));
    assert_eq!(resolve(&s, "origin/x"), Some(oid(3)));
    assert_eq!(resolve(&s, &oid(7).to_string()), Some(oid(7)));
    assert_eq!(resolve(&s, "origin/none"), None);
    assert_eq!(resolve(&s, "nothing"), None);
}

#[test]
fn a_preview_is_stale_once_head_or_the_named_ref_moves() {
    let mut s = snap("opp-stale");
    let merge = job(&s, PreviewOp::Merge { theirs: "feature".into(), oid: oid(2) });
    let reset = job(&s, PreviewOp::Reset { to: oid(9) });
    assert!(!stale(&s, &merge) && !stale(&s, &reset));
    s.locals[1].oid = Some(oid(8));
    assert!(stale(&s, &merge), "feature moved");
    assert!(!stale(&s, &reset));
    s.head_oid = Some(oid(9));
    assert!(stale(&s, &reset), "HEAD moved");
}

#[test]
fn apply_is_blocked_while_busy_loading_stale_or_with_nothing_to_do() {
    let mut s = snap("opp-blocked");
    let t = Lang::En;
    let op = PreviewOp::Reset { to: oid(9) };
    let mut d = PreviewDialog { job: job(&s, op.clone()), result: None };
    assert_eq!(apply_blocker(t, true, &s, &d).as_deref(), Some(crate::state::blocked::busy(t)));
    assert_eq!(apply_blocker(t, false, &s, &d).as_deref(), Some("Working out the result…"));
    d.result = Some(Err("boom".into()));
    assert_eq!(apply_blocker(t, false, &s, &d), None, "a failed preview still applies");
    d.result = Some(Ok(preview(&s, op.clone(), Outcome::Nothing)));
    assert_eq!(apply_blocker(t, false, &s, &d).as_deref(), Some("There is nothing to apply."));
    d.result = Some(Ok(preview(&s, op, Outcome::Done)));
    assert_eq!(apply_blocker(t, false, &s, &d), None);
    s.head_oid = Some(oid(1));
    assert_eq!(apply_blocker(t, false, &s, &d).as_deref(), Some("The branch changed since this preview. Preview again first."));
}

#[test]
fn titles_name_the_operation() {
    let s = snap("opp-titles");
    let t = Lang::En;
    assert_eq!(title(t, &s, &PreviewOp::Merge { theirs: "feature".into(), oid: oid(2) }), "Preview: merge feature into main");
    assert_eq!(title(t, &s, &PreviewOp::Rebase { onto: "origin/x".into(), oid: oid(3) }), "Preview: rebase main onto origin/x");
    let sha = oid(0x1a).to_string();
    assert_eq!(title(t, &s, &PreviewOp::Rebase { onto: sha, oid: oid(0x1a) }), "Preview: rebase main onto 1a1a1a1");
    assert_eq!(title(t, &s, &PreviewOp::Reset { to: oid(0x1a) }), "Preview: reset main to 1a1a1a1");
    assert_eq!(title(t, &s, &PreviewOp::CherryPick(vec![oid(1), oid(2)])), "Preview: cherry-pick 2 commits onto main");
    assert_eq!(title(t, &s, &PreviewOp::Drop(vec![oid(1)])), "Preview: drop 1 commit from main");
    let de = title(Lang::De, &s, &PreviewOp::Drop(vec![oid(1)]));
    assert!(de.contains("main") && de != title(t, &s, &PreviewOp::Drop(vec![oid(1)])), "{de}");
}

#[test]
fn headlines_say_what_happens() {
    let s = snap("opp-headlines");
    let t = Lang::En;
    let merge = PreviewOp::Merge { theirs: "feature".into(), oid: oid(2) };
    assert_eq!(headline(t, &preview(&s, merge.clone(), Outcome::Done)), ["Merging makes a merge commit with no conflicts."]);
    assert_eq!(headline(t, &preview(&s, merge.clone(), Outcome::Nothing)), ["main already contains feature; merging changes nothing."]);
    let mut ff = preview(&s, merge.clone(), Outcome::FastForward);
    ff.arriving = Some(Count { n: 3, more: false });
    assert_eq!(headline(t, &ff), ["main moves forward to feature: 3 commits arrive."]);
    let stop = Stop { why: StopWhy::Conflicts(vec!["a".into(), "b".into()]), applied: 0, waiting: Vec::new() };
    let lines = headline(t, &preview(&s, merge, Outcome::Stopped(stop)));
    assert_eq!(lines, ["Merging stops with conflicts in 2 files.", "main moves only once the operation finishes."]);

    let rebase = PreviewOp::Rebase { onto: "origin/main".into(), oid: oid(3) };
    let stop = Stop { why: StopWhy::Conflicts(vec!["a".into()]), applied: 1, waiting: vec![line(8, "w1"), line(9, "w2")] };
    let mut p = preview(&s, rebase.clone(), Outcome::Stopped(stop));
    p.after = vec![AfterRow::Stopped { at: Some(line(0x1a, "Fix parser")) }];
    let lines = headline(t, &p);
    assert_eq!(lines[0], "Rebasing moves 4 commits onto origin/main.");
    assert_eq!(
        lines[1],
        "It stops at \"Fix parser\" (1a1a1a1) with conflicts in 1 file. 1 commit is applied before it. 2 more commits wait until it's resolved."
    );
    assert_eq!(lines[2], "main moves only once the operation finishes.");

    let pick = PreviewOp::CherryPick(vec![oid(1), oid(2)]);
    assert_eq!(headline(t, &preview(&s, pick.clone(), Outcome::Done)), ["Cherry-picking makes 2 new commits on main."]);
    let mut p = preview(&s, pick, Outcome::Stopped(Stop { why: StopWhy::Empty, applied: 0, waiting: Vec::new() }));
    p.after = vec![AfterRow::Stopped { at: Some(line(0x1a, "Fix parser")) }];
    let lines = headline(t, &p);
    assert_eq!(lines.len(), 2, "a cherry-pick commits as it goes: {lines:?}");
    assert!(lines[1].ends_with("its changes are already there, so it would make an empty commit."), "{}", lines[1]);

    let mut drop = preview(&s, PreviewOp::Drop(vec![oid(1)]), Outcome::Done);
    drop.moved = vec![line(2, "c"), line(3, "d")];
    assert_eq!(headline(t, &drop), ["Dropping removes 1 commit.", "2 later commits are rewritten."]);

    let reset = PreviewOp::Reset { to: oid(0x1a) };
    assert_eq!(headline(t, &preview(&s, reset.clone(), Outcome::Done)), ["main moves to 1a1a1a1."]);
    assert_eq!(headline(t, &preview(&s, reset.clone(), Outcome::Nothing)), ["main is already at this commit; resetting changes nothing."]);
    let de = headline(Lang::De, &preview(&s, reset, Outcome::Done));
    assert_ne!(de, ["main moves to 1a1a1a1."]);
}

#[test]
fn notes_and_pick_blockers() {
    let mut s = snap("opp-notes");
    let t = Lang::En;
    let mut p = preview(&s, PreviewOp::Rebase { onto: "main".into(), oid: oid(3) }, Outcome::Done);
    assert!(notes(t, &s, &p).is_empty());
    p.patch_check_skipped = true;
    s.unstaged = rows(&[("a.txt", 'M')]);
    assert_eq!(
        notes(t, &s, &p),
        ["Commits already on main weren't looked for: there are too many to compare.", "Uncommitted changes aren't part of this preview."]
    );
    let head = s.head_oid.unwrap();
    assert!(pick_blocker(t, &s, &[head]).is_some(), "HEAD's own commit can't be picked onto it");
    let merge = s.commits[0].oid;
    std::sync::Arc::get_mut(&mut s.commits).unwrap()[0].parents = vec![oid(1), oid(2)];
    s.head_oid = Some(oid(9));
    assert_eq!(pick_blocker(t, &s, &[merge]).as_deref(), Some("Preview doesn't cover cherry-picking a merge commit yet."));
}
