//! Headless tests for where a force push with lease goes when config sends a plain push to
//! another remote than the upstream's (`remote.pushDefault`): the toolbar's, a row's and a
//! rejected push toast's confirmations and commands name that destination, git refusing the
//! plain push (or no tracking ref to lease against) refuses the force push, the base checks
//! look at the destination, and the confirmed message is re-checked against it. On a
//! hand-built snapshot (`toolbar_tips::tests`).

use super::update;
use crate::effect::{Effect, JobEffect};
use crate::msg::{BranchMsg, JobMsg, Msg};
use crate::state::AppState;
use crate::state::branches::plain_push;
use crate::state::branches::push::{force_blocked, force_confirm, force_label, head_force_blocked, head_force_confirm};
use crate::state::branches::rejected_push::rejected_local;
use crate::state::toolbar_tips::tests::{diverge, push_to, state};
use git2::Oid;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, PushRefusal, Tracking};
use std::time::Instant;

const EN: Lang = Lang::En;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// main (one commit ahead of origin/main) with a fork that has main at the returned tip, two
/// commits main lacks, and a plain push of main that goes to fork (`remote.pushDefault`).
/// Neither remote's default branch is main.
fn forked(name: &str) -> (AppState, Oid) {
    let mut st = state(name);
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 1, 0, true);
    let tip = oid(200);
    snap.remotes.push(("fork".into(), vec![("main".into(), tip)]));
    snap.remote_heads = vec![("origin".into(), "trunk".into()), ("fork".into(), "trunk".into())];
    push_to(snap, "fork", Ok("refs/heads/main"), Some(tip), (1, 2));
    (st, tip)
}

fn head(st: &AppState) -> BranchInfo {
    st.snap.as_ref().unwrap().head_info().unwrap().clone()
}

/// Where the head branch's plain push goes now.
fn push_now(st: &mut AppState, remote: &str, dest: Result<&str, PushRefusal>, tip: Option<Oid>, counts: (usize, usize)) {
    push_to(st.snap.as_mut().unwrap(), remote, dest, tip, counts);
}

/// Sends `msg` with no job running (a job started by an earlier one is over).
fn go(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    st.job = None;
    update(st, msg, Instant::now())
}

/// The git command among `fx`.
fn git_cmd(fx: &[Effect]) -> Option<Vec<String>> {
    fx.iter().find_map(|e| match e {
        Effect::Job(JobEffect::RunGit { cmds, .. }) => cmds.first().cloned(),
        _ => None,
    })
}

fn confirm(m: Msg) -> (String, Msg) {
    match m {
        Msg::ConfirmDanger { body, then, .. } => (body, *then),
        _ => panic!("not a confirmation"),
    }
}

fn confirmation(st: &AppState) -> (String, Msg) {
    confirm(force_confirm(EN, st.snap.as_ref().unwrap(), &head(st)).expect("a confirmation"))
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

/// Whether `m` force pushes main to `dest` on `remote`, expecting `seen` there.
fn forces(m: &Msg, remote: &str, dest: &str, seen: Option<Oid>) -> bool {
    matches!(m, Msg::Branch(BranchMsg::ForcePush { branch, remote: r, dest: d, seen: s }) if branch == "main" && r == remote && d == dest && *s == seen)
}

fn to_fork(tip: Oid) -> Msg {
    Msg::Branch(BranchMsg::ForcePush { branch: "main".into(), remote: "fork".into(), dest: "refs/heads/main".into(), seen: Some(tip) })
}

#[test]
fn the_toolbar_confirmation_and_command_go_to_the_push_remote() {
    let (mut st, tip) = forked("force-dest-toolbar");
    let (body, _) = confirm(head_force_confirm(EN, st.snap.as_ref().unwrap()));
    assert_eq!(
        body,
        "Overwrite the remote branch with your local history? fork/main has 2 commits that main doesn't have. \
         Force pushing replaces fork/main with main, and those commits are no longer on fork."
    );
    let fx = go(&mut st, Msg::Job(JobMsg::Push { force: true }));
    let cmd = git_cmd(&fx).expect("a push");
    assert!(cmd.contains(&format!("--force-with-lease=refs/heads/main:{tip}")), "{cmd:?}");
    assert!(!cmd.contains(&"-u".to_owned()), "{cmd:?}");
    assert_eq!(cmd[cmd.len() - 3..], ["--", "fork", "refs/heads/main:refs/heads/main"]);
}

#[test]
fn the_row_confirmation_names_the_push_remote() {
    let (mut st, tip) = forked("force-dest-row");
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(force_label(EN, snap, &b), "fork/main");
    let (body, then) = confirmation(&st);
    assert!(body.contains("fork/main has 2 commits that main doesn't have"), "{body}");
    assert!(body.contains("no longer on fork"), "{body}");
    assert!(body.contains("If fork/main has moved since the last fetch"), "{body}");
    assert!(forces(&then, "fork", "refs/heads/main", Some(tip)));
    let cmd = git_cmd(&go(&mut st, then)).expect("a push");
    assert!(cmd.contains(&format!("--force-with-lease=refs/heads/main:{tip}")), "{cmd:?}");
    assert_eq!(cmd[cmd.len() - 3..], ["--", "fork", "refs/heads/main:refs/heads/main"]);
    // Not there as last fetched: the lease expects it absent.
    push_now(&mut st, "fork", Ok("refs/heads/main"), None, (1, 0));
    let (body, then) = confirmation(&st);
    assert!(body.contains("As last fetched, fork/main isn't on fork"), "{body}");
    let cmd = git_cmd(&go(&mut st, then)).expect("a push");
    assert!(cmd.contains(&"--force-with-lease=refs/heads/main:".to_owned()), "{cmd:?}");
}

#[test]
fn the_toast_confirmation_goes_to_the_push_remote() {
    let (mut st, tip) = forked("force-dest-toast");
    let rejected = " ! [rejected]        main -> main (non-fast-forward)";
    let to = |remote: &str| args(&["push", "--", remote, "refs/heads/main:refs/heads/main"]);
    let snap = st.snap.as_ref().unwrap();
    let b = rejected_local(EN, snap, &to("fork"), rejected).expect("the branch git rejected");
    assert_eq!(b.name, "main");
    let (body, then) = confirm(force_confirm(EN, snap, b).unwrap());
    assert!(body.contains("fork/main"), "{body}");
    assert!(forces(&then, "fork", "refs/heads/main", Some(tip)));
    assert!(rejected_local(EN, snap, &to("origin"), rejected).is_none(), "origin isn't where it goes");
    push_now(&mut st, "fork", Ok("refs/heads/main"), None, (1, 0));
    assert!(rejected_local(EN, st.snap.as_ref().unwrap(), &to("fork"), rejected).is_none(), "never fetched");
}

#[test]
fn the_base_checks_look_at_the_destination() {
    let (mut st, _) = forked("force-dest-base");
    let b = head(&st);
    let heads = |st: &mut AppState, origin: &str, fork: &str| {
        st.snap.as_mut().unwrap().remote_heads = vec![("origin".into(), origin.into()), ("fork".into(), fork.into())];
    };
    heads(&mut st, "main", "trunk");
    assert_eq!(force_blocked(EN, st.snap.as_ref().unwrap(), &b), None, "origin's main isn't where it goes");
    heads(&mut st, "trunk", "main");
    assert_eq!(
        force_blocked(EN, st.snap.as_ref().unwrap(), &b).as_deref(),
        Some("main is the default branch of fork, so it is never force pushed: others build on it.")
    );
    let (body, _) = confirm(head_force_confirm(EN, st.snap.as_ref().unwrap()));
    let warning = "main is the default branch of fork, which others build on: this overwrites fork/main with your local main. \
                   fork/main has 2 commits that main doesn't have.";
    assert!(body.contains(warning), "{body}");
}

#[test]
fn a_missing_push_remote_force_pushes_to_the_upstream() {
    let mut st = state("force-dest-missing");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 1, 3, true);
    let up = snap.remotes[0].1[0].1;
    snap.remote_heads = vec![("origin".into(), "trunk".into())];
    push_to(snap, "origin", Ok("refs/heads/main"), Some(up), (1, 3)).missing = Some(("branch.main.pushRemote".into(), "gone".into()));
    let (body, then) = confirmation(&st);
    assert!(body.contains("origin/main has 3 commits that main doesn't have"), "{body}");
    let cmd = git_cmd(&go(&mut st, then)).expect("a push");
    assert_eq!(cmd[cmd.len() - 3..], ["--", "origin", "refs/heads/main:refs/heads/main"]);
    let (body, _) = confirm(head_force_confirm(EN, st.snap.as_ref().unwrap()));
    assert_eq!(body, "Overwrite the remote branch with your local history?");
}

#[test]
fn a_destination_without_a_tracking_ref_is_refused() {
    let (mut st, _) = forked("force-dest-no-lease");
    let no_lease = |st: &mut AppState, remote: &str| {
        let snap = st.snap.as_mut().unwrap();
        push_to(snap, remote, Ok("refs/heads/main"), None, (1, 0)).dest.as_mut().unwrap().tracking_ref = None;
    };
    no_lease(&mut st, "fork");
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    let why = force_blocked(EN, snap, &b).expect("refused");
    assert!(why.starts_with("No lease can be taken for main: fork/main has no remote-tracking branch"), "{why}");
    assert_eq!(force_confirm(EN, snap, &b).err().as_deref(), Some(why.as_str()));
    assert_eq!(head_force_blocked(EN, snap).as_deref(), Some(why.as_str()));
    assert_ne!(force_blocked(Lang::De, snap, &b), Some(why.clone()));
    assert!(go(&mut st, Msg::Job(JobMsg::Push { force: true })).is_empty());
    assert_eq!(last_log(&st), why);
    assert!(go(&mut st, to_fork(oid(200))).is_empty());
    assert_eq!(last_log(&st), why);
    // A URL carrying a password never reaches the text.
    let url = "https://user:secret@example.invalid/r.git";
    no_lease(&mut st, url);
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    let why = force_blocked(EN, snap, &b).expect("refused");
    assert!(why.contains("***") && !why.contains("secret"), "{why}");
    assert!(!force_label(EN, snap, &b).contains("secret"));
}

#[test]
fn a_refused_plain_push_refuses_force_push_with_its_text() {
    let (mut st, tip) = forked("force-dest-refused");
    push_now(&mut st, "fork", Err(PushRefusal::Nothing), None, (0, 0));
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    let why = plain_push::refused(EN, &b).expect("git refuses");
    assert_eq!(force_blocked(EN, snap, &b).as_deref(), Some(why.as_str()));
    assert_eq!(head_force_blocked(EN, snap).as_deref(), Some(why.as_str()));
    assert_eq!(force_confirm(EN, snap, &b).err().as_deref(), Some(why.as_str()));
    assert!(go(&mut st, Msg::Job(JobMsg::Push { force: true })).is_empty());
    assert_eq!(last_log(&st), why);
    assert!(go(&mut st, to_fork(tip)).is_empty(), "nothing goes to the upstream either");
    assert_eq!(last_log(&st), why);
}

#[test]
fn the_recheck_compares_the_destination_the_confirmation_named() {
    let (mut st, tip) = forked("force-dest-recheck");
    let (_, then) = confirmation(&st);
    // (a) Its tip moved.
    push_now(&mut st, "fork", Ok("refs/heads/main"), Some(oid(201)), (1, 2));
    assert!(go(&mut st, to_fork(tip)).is_empty());
    assert!(last_log(&st).starts_with("fork/main moved since you confirmed, so main wasn't force pushed"), "{}", last_log(&st));
    // (b) It now pushes to origin.
    let up = st.snap.as_ref().unwrap().remotes[0].1[0].1;
    push_now(&mut st, "origin", Ok("refs/heads/main"), Some(up), (1, 0));
    assert!(go(&mut st, to_fork(tip)).is_empty());
    assert!(last_log(&st).starts_with("main no longer pushes to fork/main, so it wasn't force pushed"), "{}", last_log(&st));
    // (c) Only the upstream changed.
    push_now(&mut st, "fork", Ok("refs/heads/main"), Some(tip), (1, 2));
    let b = st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.is_head).unwrap();
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "other".into() });
    b.upstream = Some("origin/other".into());
    assert!(git_cmd(&go(&mut st, then)).is_some(), "the destination is the one confirmed");
    // (d) Confirmed against the upstream, which then moved to another name that git refuses.
    let (mut st, _) = forked("force-dest-recheck-upstream");
    let up = st.snap.as_ref().unwrap().remotes[0].1[0].1;
    push_now(&mut st, "origin", Ok("refs/heads/main"), Some(up), (1, 0));
    let (_, then) = confirmation(&st);
    let b = st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.is_head).unwrap();
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "other".into() });
    push_now(&mut st, "origin", Err(PushRefusal::NameDiffers { merge: "other".into() }), None, (0, 0));
    assert!(go(&mut st, then).is_empty());
    assert!(last_log(&st).starts_with("The upstream of main is no longer origin/main"), "{}", last_log(&st));
    // (e) A name without refs/ is a branch.
    let (mut st, tip) = forked("force-dest-recheck-short");
    let short = Msg::Branch(BranchMsg::ForcePush { branch: "main".into(), remote: "fork".into(), dest: "main".into(), seen: Some(tip) });
    assert!(git_cmd(&go(&mut st, short)).is_some());
}

#[test]
fn a_destination_that_is_not_a_branch_keeps_its_real_ref() {
    let (mut st, tip) = forked("force-dest-ref");
    // The fork's default branch is main: replacing refs/for/main is still no default-branch push.
    st.snap.as_mut().unwrap().remote_heads = vec![("origin".into(), "trunk".into()), ("fork".into(), "main".into())];
    let snap = st.snap.as_mut().unwrap();
    push_to(snap, "fork", Ok("refs/for/main"), Some(tip), (1, 2)).dest.as_mut().unwrap().tracking_ref =
        Some("refs/remotes/fork/for/main".into());
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(force_blocked(EN, snap, &b), None);
    assert_eq!(force_label(EN, snap, &b), "fork refs/for/main");
    let (_, then) = confirmation(&st);
    assert!(forces(&then, "fork", "refs/for/main", Some(tip)));
    let cmd = git_cmd(&go(&mut st, then)).expect("a push");
    assert!(cmd.contains(&format!("--force-with-lease=refs/for/main:{tip}")), "{cmd:?}");
    assert_eq!(cmd[cmd.len() - 3..], ["--", "fork", "refs/heads/main:refs/for/main"]);
    // This repository: the push is a no-op, and its default branch isn't checked.
    st.snap.as_mut().unwrap().remote_heads = vec![("origin".into(), "main".into())];
    push_now(&mut st, ".", Ok("refs/heads/main"), Some(tip), (0, 0));
    let b = head(&st);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(force_blocked(EN, snap, &b), None);
    let fx = go(&mut st, Msg::Job(JobMsg::Push { force: true }));
    let cmd = git_cmd(&fx).expect("a push");
    assert_eq!(cmd[cmd.len() - 3..], ["--", ".", "refs/heads/main:refs/heads/main"]);
}
