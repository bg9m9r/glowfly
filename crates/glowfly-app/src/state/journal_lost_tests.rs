use super::*;
use crate::state::op_fixture::{rows, snapshot};
use git2::RepositoryState;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn lost(n: u8, kind: LostKind, summary: &str) -> LostCommit {
    LostCommit { oid: oid(n), kind, summary: summary.into(), message: format!("{summary}\n"), author: "Test".into(), time: 1_700_000_000 }
}

fn stash(n: u8) -> LostCommit {
    lost(n, LostKind::Stash { base: oid(2), branch: Some("main".into()), untracked: false }, "On main: keep")
}

fn kept_file(what: KeptWhat) -> LostCommit {
    lost(4, LostKind::Discarded { path: "src/a.rs".into(), blob: oid(5), what }, "src/a.rs")
}

/// A snapshot with HEAD at `oid(9)` on main.
fn snap(name: &str) -> Snapshot {
    let mut s = snapshot(name);
    s.locals[0].oid = Some(oid(9));
    s.head_oid = Some(oid(9));
    s
}

#[test]
fn label_history_and_row() {
    let en = Lang::En;
    assert_eq!(label(en, &lost(1, LostKind::Commit, "")), "(no message)");
    assert_eq!(label(en, &lost(1, LostKind::Commit, "Fix it")), "Fix it");
    assert_eq!(history_from(&lost(1, LostKind::Commit, "x")), Some(oid(1)));
    assert_eq!(history_from(&stash(3)), Some(oid(2)), "a stash's history starts at its base");
    assert_eq!(history_from(&kept_file(KeptWhat::Hunk)), None, "a kept file has none");
    let r = as_row(&lost(1, LostKind::Commit, "x"));
    assert_eq!((r.subject, r.oid, r.event, r.summary.as_deref(), r.current), (Subject::Head, oid(1), Event::Other, Some("x"), false));
}

#[test]
fn restore_is_a_detached_checkout_unless_blocked() {
    let (s, en) = (snap("journal-lost-plan"), Lang::En);
    let c = lost(1, LostKind::Commit, "x");
    assert_eq!(restore_plan(&s, &c, false, en), Ok(RestorePlan::CheckoutDetached { to: oid(1), from: Some(oid(9)) }));
    assert_eq!(restore_plan(&s, &c, true, en), Err("Another operation is still running.".into()));
    let mut merging = snap("journal-lost-plan-merge");
    merging.state = RepositoryState::Merge;
    assert_eq!(restore_plan(&merging, &c, false, en), Err("Finish or abort the operation in progress first.".into()));
    let plan = restore_plan(&s, &c, false, en).unwrap();
    assert!(matches!(restore_confirm(en, &plan, &s), Msg::Confirm { ref title, .. } if title == "Check out this state?"));
}

#[test]
fn stash_actions_and_create_branch() {
    let en = Lang::En;
    let s = stash(3);
    let id = oid(3).to_string();
    let clean = snap("journal-lost-apply");
    let Msg::Job(JobMsg::Git { cmds, .. }) = apply(&clean, &s, en) else { panic!("a job on a clean tree") };
    assert_eq!(cmds, vec![glowfly_core::gitcli::args(&["stash", "apply", &id])]);
    let mut dirty = snap("journal-lost-apply-dirty");
    dirty.unstaged = rows(&[("a.txt", 'M')]);
    let Msg::Stash(crate::msg::StashMsg::Ask(ask)) = apply(&dirty, &s, en) else { panic!("asks first on a dirty tree") };
    assert!(
        matches!(ask.job(), Msg::Job(JobMsg::Git { ref cmds, .. }) if cmds[..] == [glowfly_core::gitcli::args(&["stash", "apply", &id])])
    );
    let Msg::Job(JobMsg::Git { cmds, .. }) = store(&s, en) else { panic!("a job") };
    assert_eq!(cmds, vec![glowfly_core::gitcli::args(&["stash", "store", "-m", "On main: keep", &id])]);
    let BranchMsg::NewBranch { start, name } = create_branch(&lost(1, LostKind::Commit, "x")) else { panic!("New branch") };
    assert_eq!((start, name), (Some(oid(1).to_string()), String::new()));
}

#[test]
fn apply_stash_waits_for_jobs_and_operations() {
    let en = Lang::En;
    let clean = snap("journal-lost-apply-blocked");
    assert_eq!(apply_blocked(&clean, false, en), None);
    assert_eq!(apply_blocked(&clean, true, en), Some("Another operation is still running."));
    for state in [RepositoryState::Merge, RepositoryState::CherryPickSequence, RepositoryState::RebaseInteractive] {
        let mut busy = snap("journal-lost-apply-busy");
        busy.state = state;
        assert_eq!(apply_blocked(&busy, false, en), Some("Finish or abort the operation in progress first."), "{state:?}");
    }
}

#[test]
fn footer_lines_name_the_cut_then_the_problems() {
    let en = Lang::En;
    let mut v = LostView {
        result: Some(Ok(LostCommits { commits: vec![], discarded: vec![], capped: true, incomplete: true })),
        ..Default::default()
    };
    assert_eq!(
        footer_lines(en, &v),
        vec![
            "Showing the newest 500 lost commits",
            "Git reported problems while checking this repository, so some lost commits may be missing"
        ]
    );
    v.result = Some(Ok(LostCommits::default()));
    assert!(footer_lines(en, &v).is_empty());
}

#[test]
fn kept_files_are_named_and_their_restores_reported() {
    let en = Lang::En;
    assert_eq!(chip(en, &LostKind::Commit), None);
    assert_eq!(chip(en, &stash(3).kind), Some("Dropped stash"));
    assert_eq!(chip(en, &kept_file(KeptWhat::Hunk).kind), Some("Discarded hunk"));
    assert_eq!(chip(en, &kept_file(KeptWhat::Delete).kind), Some("Deleted file"));
    assert_ne!(chip(Lang::De, &kept_file(KeptWhat::Lines).kind), chip(en, &kept_file(KeptWhat::Lines).kind));
    assert_eq!(restore_file_blocked(true, en), Some(en.tr("commit.blocked.job_running")));
    assert_eq!(restore_file_blocked(false, en), None);

    let wrote = |p: &str| Ok(Restored::Wrote { path: p.into() });
    assert_eq!(restored_toast(en, "a.rs", &wrote("a.rs")), (true, "Restored a.rs".into(), false));
    assert_eq!(restored_toast(en, "a.rs", &wrote("a (restored).rs")), (true, "Restored a.rs as a (restored).rs".into(), false));
    let (ok, text, offer) = restored_toast(en, "a.rs", &Ok(Restored::Occupied { path: "a.rs".into() }));
    assert!(!ok && offer && text.starts_with("a.rs is in your working tree"), "{text}");
    let (ok, text, offer) = restored_toast(en, "a.rs", &Ok(Restored::AlreadyThere { path: "a.rs".into() }));
    assert!(ok && !offer && text.contains("already holds this version"), "{text}");
    let (ok, text, _) = restored_toast(Lang::Es, "a.rs", &Err("disk full".into()));
    assert!(!ok && text.contains("disk full") && text.starts_with("No se pudo"), "{text}");
}
