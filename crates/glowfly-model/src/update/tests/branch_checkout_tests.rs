//! Checking out what is already checked out (update/branch_checkout.rs): a double-click on the
//! current branch, or on the remote branch it tracks, does nothing and says nothing; a
//! failure said twice leaves one toast, its timer restarted (state/toasts.rs); and a checkout of
//! a tracker that is only behind records its fast-forward, so one failing after it moves the
//! branch back (update/branch_checkout_back.rs).

use super::tests::{git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, JobEffect};
use crate::msg::{BranchMsg, JobMsg, Msg};
use crate::state::AppState;
use crate::state::remote_branch::move_back::FfPhase;
use git2::Oid;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::testkit::git_out;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A state on a repository whose checked-out `main` tracks `origin/main`, at the same commit.
fn on_main(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn texts(st: &AppState) -> Vec<&str> {
    st.toasts.iter().map(|t| t.text.as_str()).collect()
}

fn remote(name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: "origin".into(), name: name.into() })
}

#[test]
fn double_clicking_the_current_branch_does_nothing() {
    let (mut st, dir) = on_main("checkout-head-local");
    let now = Instant::now();
    for _ in 0..3 {
        let fx = update(&mut st, Msg::Branch(BranchMsg::Checkout("main".into())), now);
        assert!(fx.is_empty() && st.job.is_none(), "no job starts");
    }
    assert!(st.toasts.is_empty(), "no toast: {:?}", texts(&st));
    assert!(st.log.iter().all(|e| e.ok), "no failure logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn double_clicking_the_remote_branch_head_tracks_does_nothing() {
    let (mut st, dir) = on_main("checkout-head-remote");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.locals.iter().any(|b| b.is_head && crate::state::remote_branch::tracks(b, "origin", "main")));
    let now = Instant::now();
    for _ in 0..3 {
        let fx = update(&mut st, remote("main"), now);
        assert!(fx.is_empty() && st.job.is_none(), "no job starts");
    }
    assert!(st.toasts.is_empty(), "no toast: {:?}", texts(&st));
    assert!(st.log.iter().all(|e| e.ok), "no failure logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_failure_twice_leaves_one_toast_with_its_timer_restarted() {
    let (mut st, dir) = on_main("checkout-gone-twice");
    let t0 = Instant::now();
    update(&mut st, remote("gone"), t0);
    assert_eq!(texts(&st), ["origin/gone no longer exists."]);
    let (id, first) = (st.toasts[0].id, st.toasts[0].first);
    let later = t0 + Duration::from_secs(3);
    update(&mut st, remote("gone"), later);
    assert_eq!(texts(&st), ["origin/gone no longer exists."], "one toast, not two");
    let t = &st.toasts[0];
    assert_eq!((t.id, t.first, t.at, t.counted), (id, first, later, Duration::ZERO), "the same toast, its timer restarted");
    assert!(t.action.is_some() && t.details.is_none(), "one Details button, not two");
    assert_eq!(st.log.iter().filter(|e| !e.ok).count(), 2, "each failure is still in the activity log");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A state on a repository whose `feature` (at `A`, the first commit) tracks `origin/feature`,
/// one commit (`B`) ahead of it; `main` is checked out and the work tree is clean.
fn behind(name: &str) -> (AppState, PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["branch", "feature"]);
    let a = git_out(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    let b = git_out(&dir, &["commit-tree", "HEAD^{tree}", "-p", &a, "-m", "B"]).trim().to_owned();
    git(&dir, &["update-ref", "refs/remotes/origin/feature", &b]);
    git(&dir, &["config", "branch.feature.remote", "origin"]);
    git(&dir, &["config", "branch.feature.merge", "refs/heads/feature"]);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir, Oid::from_str(&a).unwrap(), Oid::from_str(&b).unwrap())
}

/// What a git job among `fx` runs: its folder, commands, `expect` and `on_fail_check`.
type Run = (PathBuf, Vec<Vec<String>>, Option<(String, Oid)>, Option<Vec<String>>);

fn run_gits(fx: &[Effect]) -> Vec<Run> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Job(JobEffect::RunGit { cwd, cmds, expect, on_fail_check, .. }) => {
                Some((cwd.clone(), cmds.clone(), expect.clone(), on_fail_check.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Starts the checkout of `origin/feature` and returns the job's commands.
fn start(st: &mut AppState) -> Vec<Vec<String>> {
    let fx = update(st, remote("feature"), Instant::now());
    let mut jobs = run_gits(&fx);
    assert_eq!(jobs.len(), 1, "{fx:?}");
    let (_, cmds, _, on_fail_check) = jobs.remove(0);
    assert_eq!(on_fail_check, None);
    cmds
}

/// A failed checkout job's output for `cmds`, as the runner prints it: the first `ran` commands
/// echoed, then `tail`.
fn output(cmds: &[Vec<String>], ran: usize, tail: &str) -> String {
    let echoes: Vec<String> = cmds[..ran].iter().map(|c| glowfly_core::text::command_echo(c)).collect();
    format!("{}\n{tail}\n", echoes.join("\n"))
}

const UNTRACKED: &str = "error: The following untracked working tree files would be overwritten by checkout:";

fn done(st: &mut AppState, ok: bool, output: String) -> Vec<Effect> {
    update(st, Msg::Job(JobMsg::JobDone { ok, output, undo: None }), Instant::now())
}

fn log_texts(st: &AppState) -> Vec<&str> {
    st.log.iter().map(|l| l.text.as_str()).collect()
}

#[test]
fn fast_forward_checkout_records_the_pending_job() {
    let (mut st, dir, a, b) = behind("ffc-pending");
    let cmds = start(&mut st);
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0][..2], ["merge-base", "--is-ancestor"]);
    assert_eq!(cmds[1][0], "update-ref");
    assert_eq!(cmds[2], ["checkout", "feature", "--"]);
    let p = st.ff_checkout.as_ref().expect("pending");
    assert_eq!((p.phase, p.from, p.to, p.branch.as_str()), (FfPhase::Checkout, a, b, "feature"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkout_failing_after_the_fast_forward_starts_the_move_back() {
    let (mut st, dir, a, b) = behind("ffc-moveback");
    let cmds = start(&mut st);
    let fx = done(&mut st, false, output(&cmds, 3, UNTRACKED));
    let jobs = run_gits(&fx);
    assert_eq!(jobs.len(), 1, "{fx:?}");
    let (cwd, back, expect, _) = &jobs[0];
    assert_eq!(back, &vec![glowfly_core::branch_ops::move_back_cmd("feature", a)]);
    assert_eq!(expect, &Some(("refs/heads/feature".to_owned(), b)));
    assert_eq!(cwd, &st.snap.as_ref().unwrap().workdir);
    let job = st.job.as_ref().expect("the move-back runs");
    assert_eq!((job.label.as_str(), job.quiet), ("Move branch back", true));
    assert_eq!(st.ff_checkout.as_ref().map(|p| p.phase), Some(FfPhase::MovingBack));
    // The checkout's own failure is still said as git's explanation.
    assert!(!st.toasts.is_empty());
    assert!(st.toasts.iter().all(|t| !t.text.contains("moved back") && !t.text.contains("fast-forward")), "{:?}", texts(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_move_back_when_nothing_moved() {
    for which in 0..3 {
        let (mut st, dir, _, _) = behind(&format!("ffc-nothing-{which}"));
        let cmds = start(&mut st);
        let (ok, out) = match which {
            // The update-ref's lease refused.
            0 => (false, output(&cmds, 2, "fatal: cannot lock ref 'refs/heads/feature': is at X but expected Y")),
            // The ancestry check refused.
            1 => (false, output(&cmds, 1, "error: nope")),
            _ => (true, output(&cmds, 3, "")),
        };
        let fx = done(&mut st, ok, out);
        assert!(run_gits(&fx).is_empty(), "case {which}: {fx:?}");
        let said = log_texts(&st);
        assert!(said.iter().all(|l| !l.contains("moved back") && !l.contains("fast-forwarded from")), "case {which}: {said:?}");
        assert!(st.ff_checkout.is_none(), "case {which}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_gate_hold_before_the_checkout_still_moves_back() {
    let (mut st, dir, a, _) = behind("ffc-gate");
    let cmds = start(&mut st);
    let stopped = glowfly_core::i18n::Lang::En.trf("trust.halt.stopped", &[("why", &"x")]);
    let fx = done(&mut st, false, output(&cmds, 2, &stopped));
    let jobs = run_gits(&fx);
    assert_eq!(jobs.len(), 1, "{fx:?}");
    assert_eq!(jobs[0].1, vec![glowfly_core::branch_ops::move_back_cmd("feature", a)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn move_back_results() {
    let (mut st, dir, a, b) = behind("ffc-results");
    let cmds = start(&mut st);
    done(&mut st, false, output(&cmds, 3, UNTRACKED));
    let toasts = st.toasts.len();
    done(&mut st, true, String::new());
    let (a7, b7) = (&a.to_string()[..7], &b.to_string()[..7]);
    let last = st.log.last().unwrap();
    assert!(last.ok, "{:?}", log_texts(&st));
    assert_eq!(
        last.text,
        format!("The checkout failed after feature was fast-forwarded to origin/feature ({b7}), so feature was moved back to {a7}.")
    );
    assert_eq!(st.toasts.len(), toasts, "no new toast");
    assert!(st.ff_checkout.is_none());

    // A second repository has its own commits (their ids depend on the clock), so read them anew.
    let (mut st, dir2, a, b) = behind("ffc-results-failed");
    let (a7, b7) = (&a.to_string()[..7], &b.to_string()[..7]);
    let cmds = start(&mut st);
    done(&mut st, false, output(&cmds, 3, UNTRACKED));
    let refusal = "The repository changed since it was shown (refs/heads/feature is now abc, not def), so nothing was done.";
    done(&mut st, false, refusal.into());
    let last = st.log.iter().rev().find(|l| l.text.contains("could not be moved back")).expect("failure line");
    assert!(!last.ok && last.text.contains(a7) && last.text.contains(b7) && last.text.contains(refusal), "{}", last.text);
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok);
    assert_eq!(toast.text, format!("The checkout failed after fast-forwarding feature, and it couldn't be moved back to {a7}."));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn busy_when_moving_back() {
    let (mut st, dir, a, _) = behind("ffc-busy");
    start(&mut st);
    let pending = st.ff_checkout.clone().unwrap();
    // The checkout job is still the active one, as if another had started meanwhile.
    let mut fx = Vec::new();
    let then = Some(super::super::branch_checkout_back::Then::MoveBack(pending, None));
    super::super::branch_checkout_back::then(&mut st, then, Instant::now(), &mut fx);
    assert!(fx.is_empty());
    let last = st.log.last().unwrap();
    assert!(!last.ok && last.text.contains("could not be moved back: Another operation is still running."), "{}", last.text);
    assert!(last.text.contains(&a.to_string()[..7]));
    let _ = std::fs::remove_dir_all(&dir);
}
