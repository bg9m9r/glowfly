//! Pushing stacks to a local bare origin: classifying each layer, plain and leased pushes,
//! upstreams, a stale lease and a refusing remote.

use super::read_tests::{commit, stacks_of, tip};
use super::*;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// with_origin plus main <- low (l1) <- mid (m1), none of them pushed, mid checked out.
fn stack(name: &str) -> (PathBuf, PathBuf) {
    let (dir, bare) = with_origin(name);
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m1");
    (dir, bare)
}

/// The tip of origin/<branch> as this repository last saw it.
fn seen(dir: &Path, branch: &str) -> Option<Oid> {
    let repo = git2::Repository::open(dir).unwrap();
    repo.refname_to_id(&format!("refs/remotes/origin/{branch}")).ok()
}

/// `branch` to origin under its own name, `seen` from its remote-tracking ref.
fn target(dir: &Path, branch: &str, set_upstream: bool) -> PushTarget {
    PushTarget { branch: branch.into(), remote: "origin".into(), dest: branch.into(), set_upstream, seen: seen(dir, branch) }
}

fn plan_of(dir: &Path, t: &PushTarget) -> PushPlan {
    classify(&git2::Repository::open(dir).unwrap(), t)
}

fn config(dir: &Path, key: &str) -> Option<String> {
    git2::Repository::open(dir).unwrap().config().unwrap().snapshot().unwrap().get_string(key).ok()
}

fn outcome(run: &StackPush, branch: &str) -> LayerOutcome {
    run.layers.iter().find(|l| l.target.branch == branch).unwrap().outcome.clone()
}

fn clean(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn each_layer_is_classified_against_the_remote_tip_seen() {
    let (dir, bare) = stack("stack-push-classify");
    assert_eq!(plan_of(&dir, &target(&dir, "low", true)), PushPlan::New, "never pushed");
    git(&dir, &["push", "-q", "-u", "origin", "low", "mid"]);
    assert_eq!(plan_of(&dir, &target(&dir, "low", false)), PushPlan::UpToDate);
    git(&dir, &["checkout", "-q", "low"]);
    let before = seen(&dir, "low");
    commit(&dir, "l2");
    assert_eq!(plan_of(&dir, &target(&dir, "low", false)), PushPlan::FastForward);
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    let amended = target(&dir, "low", false);
    assert_eq!(amended.seen, before);
    assert_eq!(plan_of(&dir, &amended), PushPlan::Rewritten, "the pushed tip is in the reflog");
    let unknown = PushTarget { seen: Some(Oid::from_bytes(&[7; 20]).unwrap()), ..amended };
    assert_eq!(plan_of(&dir, &unknown), PushPlan::Diverged, "a remote tip this repository lacks");

    git(&dir, &["checkout", "-q", "mid"]);
    remote_commit(&bare, "mid", "r.txt", "remote");
    git(&dir, &["fetch", "-q", "origin"]);
    assert_eq!(plan_of(&dir, &target(&dir, "mid", false)), PushPlan::Behind);
    commit(&dir, "m2");
    assert_eq!(plan_of(&dir, &target(&dir, "mid", false)), PushPlan::Diverged, "the remote tip was never on mid");

    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["branch", "-q", "-D", "mid"]);
    assert_eq!(plan_of(&dir, &target(&dir, "mid", false)), PushPlan::Gone);
    clean(&[&dir, &bare]);
}

#[test]
fn new_layers_are_pushed_with_upstreams_and_existing_upstreams_stay() {
    let (dir, bare) = stack("stack-push-new");
    git(&dir, &["branch", "-q", "other", "main"]);
    git(&dir, &["push", "-q", "origin", "other:x"]);
    git(&dir, &["branch", "-q", "-u", "origin/x", "other"]);
    git(&dir, &["checkout", "-q", "other"]);
    commit(&dir, "o1");
    let other = PushTarget { dest: "x".into(), seen: seen(&dir, "x"), ..target(&dir, "other", false) };
    let targets = [target(&dir, "main", false), target(&dir, "low", true), target(&dir, "mid", true), other];
    let run = push(&dir, &targets).unwrap();
    assert_eq!(outcome(&run, "main"), LayerOutcome::UpToDate);
    assert_eq!(outcome(&run, "low"), LayerOutcome::Pushed { plan: PushPlan::New, upstream: true });
    assert_eq!(outcome(&run, "mid"), LayerOutcome::Pushed { plan: PushPlan::New, upstream: true });
    assert_eq!(outcome(&run, "other"), LayerOutcome::Pushed { plan: PushPlan::FastForward, upstream: false });
    assert!(run.ok() && run.landed() == 4, "{run:?}");
    for b in ["low", "mid"] {
        assert_eq!(tip(&bare, &format!("refs/heads/{b}")), tip(&dir, b));
    }
    assert_eq!(tip(&bare, "refs/heads/x"), tip(&dir, "other"));
    assert_eq!(config(&dir, "branch.low.remote").as_deref(), Some("origin"));
    assert_eq!(config(&dir, "branch.low.merge").as_deref(), Some("refs/heads/low"));
    assert_eq!(config(&dir, "branch.other.merge").as_deref(), Some("refs/heads/x"), "its upstream isn't moved");
    assert!(!run.output.contains("refs/heads/main:"), "the up-to-date layer isn't pushed: {}", run.output);
    assert!(run.output.contains("$ git push --porcelain -u origin refs/heads/low:refs/heads/low refs/heads/mid:refs/heads/mid"));
    clean(&[&dir, &bare]);
}

/// The stack pushed, then low amended and mid restacked onto it, plus `solo` (pushed, then one
/// commit more). Returns the old remote tips of low and mid's targets.
fn restacked(name: &str) -> (PathBuf, PathBuf, Vec<PushTarget>) {
    let (dir, bare) = stack(name);
    git(&dir, &["branch", "-q", "solo", "main"]);
    git(&dir, &["push", "-q", "-u", "origin", "low", "mid", "solo"]);
    git(&dir, &["checkout", "-q", "solo"]);
    commit(&dir, "s1");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    let p = plan(&stacks_of(&dir), &RestackScope::Above("low".into()), Some("low"), false);
    assert!(!p.is_empty(), "mid needs a restack");
    restack(&dir, &p).unwrap();
    let targets = vec![target(&dir, "low", false), target(&dir, "mid", false), target(&dir, "solo", false)];
    (dir, bare, targets)
}

#[test]
fn rewritten_layers_replace_their_pushed_versions_with_a_lease() {
    let (dir, bare, targets) = restacked("stack-push-rewritten");
    let run = push(&dir, &targets).unwrap();
    assert_eq!(outcome(&run, "low"), LayerOutcome::Pushed { plan: PushPlan::Rewritten, upstream: false });
    assert_eq!(outcome(&run, "mid"), LayerOutcome::Pushed { plan: PushPlan::Rewritten, upstream: false });
    assert_eq!(outcome(&run, "solo"), LayerOutcome::Pushed { plan: PushPlan::FastForward, upstream: false });
    for b in ["low", "mid", "solo"] {
        assert_eq!(tip(&bare, &format!("refs/heads/{b}")), tip(&dir, b), "{b}");
    }
    let lease = |t: &PushTarget| format!("--force-with-lease=refs/heads/{}:{}", t.dest, t.seen.unwrap());
    assert!(run.output.contains(&lease(&targets[0])) && run.output.contains(&lease(&targets[1])), "{}", run.output);
    assert!(!run.output.contains("--force-with-lease=refs/heads/solo"), "{}", run.output);
    assert_eq!(run.output.matches("--force-with-lease").count(), 2);
    clean(&[&dir, &bare]);
}

#[test]
fn a_stale_lease_leaves_the_remote_alone_and_the_other_layers_still_go() {
    let (dir, bare, targets) = restacked("stack-push-stale");
    remote_commit(&bare, "mid", "r.txt", "someone else");
    let foreign = tip(&bare, "refs/heads/mid");
    let run = push(&dir, &targets[..2]).unwrap();
    assert_eq!(outcome(&run, "low"), LayerOutcome::Pushed { plan: PushPlan::Rewritten, upstream: false });
    assert_eq!(outcome(&run, "mid"), LayerOutcome::Stale);
    assert_eq!(tip(&bare, "refs/heads/mid"), foreign, "the foreign commit stays");
    assert_eq!(tip(&bare, "refs/heads/low"), tip(&dir, "low"));
    assert!(!run.ok());
    assert_eq!(run.landed(), 1);
    clean(&[&dir, &bare]);
}

#[test]
fn a_refusing_remote_is_reported_and_behind_or_diverged_layers_never_run() {
    let (dir, bare, mut targets) = restacked("stack-push-rejected");
    set_config(&bare, &[("receive.denyNonFastForwards", "true")]);
    git(&dir, &["checkout", "-q", "-b", "late", "main"]);
    git(&dir, &["push", "-q", "-u", "origin", "late"]);
    remote_commit(&bare, "late", "r.txt", "ahead");
    git(&dir, &["fetch", "-q", "origin"]);
    git(&dir, &["checkout", "-q", "-b", "split", "main"]);
    git(&dir, &["push", "-q", "-u", "origin", "split"]);
    remote_commit(&bare, "split", "q.txt", "theirs");
    git(&dir, &["fetch", "-q", "origin"]);
    commit(&dir, "ours");
    targets.truncate(1);
    targets.push(target(&dir, "late", false));
    targets.push(target(&dir, "split", false));
    let run = push(&dir, &targets).unwrap();
    match outcome(&run, "low") {
        LayerOutcome::Rejected(why) => assert!(why.contains("rejected"), "{why}"),
        other => panic!("{other:?}: {}", run.output),
    }
    assert_eq!(outcome(&run, "late"), LayerOutcome::NotPushed(PushPlan::Behind));
    assert_eq!(outcome(&run, "split"), LayerOutcome::NotPushed(PushPlan::Diverged));
    assert!(!run.output.contains("refs/heads/late") && !run.output.contains("refs/heads/split"), "{}", run.output);
    assert_eq!(run.landed(), 0);
    clean(&[&dir, &bare]);
}
