//! Continuing a Finish stopped at a merge conflict: finding the failed step, cutting the plan
//! after a conflicted merge, telling a committed merge from one still in progress or aborted,
//! and the checks before the rest runs, end to end against fixture repositories.

use super::finish_tests::{branches, clean, contains, defaults, git_out, initialized, parents, release_opts, rev};
use super::*;
use crate::gitcli::run;
use crate::repo::{LoadOptions, Snapshot, load};
use crate::testutil::{git, write};
use git2::{Oid, RepositoryState};
use std::path::{Path, PathBuf};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn texts(r: &FinishResume) -> Vec<&str> {
    r.plan.steps.iter().map(|s| s.what.as_str()).collect()
}

#[test]
fn failed_step_and_conflict() {
    assert_eq!(failed_step("Could not start git.", 3), 0);
    assert_eq!(failed_step("$ git checkout develop --\n$ git merge x\nCONFLICT (content)", 3), 1);
    assert_eq!(failed_step("$ git a\n$ git b\n$ git c\n$ git d", 3), 2, "clamped to the last step");
    assert_eq!(failed_step("", 0), 0);
    assert!(stopped_by_conflict("$ git merge x\nCONFLICT (content): Merge conflict in a.txt"));
    assert!(!stopped_by_conflict("$ git branch -d x\nerror: not fully merged"));
}

#[test]
fn output_after_a_stash_command() {
    let out = "$ git stash push -m x\nSaved\n$ git checkout develop --\nerr";
    assert_eq!(after_commands(out, 1), Some("$ git checkout develop --\nerr"));
    assert_eq!(after_commands(out, 0), Some(out));
    assert_eq!(after_commands("$ git stash push -m x\nerror: no luck", 1), None, "the stash itself failed");
    assert_eq!(after_commands("", 0), None);
}

#[test]
fn finish_ends_on_the_development_branch() {
    let s = defaults();
    let b = branches(&["main", "develop", "feature/login", "release/1.2.0", "hotfix/1.0.1"]);
    for keep in [false, true] {
        let o = FinishOptions { keep, ..FinishOptions::default() };
        assert_eq!(check_finish(&s, "feature/login", &o, &b, &clean()).unwrap().ends_on(), Some("develop"), "keep {keep}");
    }
    assert_eq!(check_finish(&s, "release/1.2.0", &release_opts(false), &b, &clean()).unwrap().ends_on(), Some("develop"));
    let o = FinishOptions { tag: "v1.0.1".into(), message: "Hotfix 1.0.1".into(), keep: false };
    assert_eq!(check_finish(&s, "hotfix/1.0.1", &o, &b, &clean()).unwrap().ends_on(), Some("develop"));
}

#[test]
fn rest_after_conflict_cuts_after_merges() {
    let s = defaults();
    let b = branches(&["main", "develop", "feature/login", "release/1.2.0"]);
    let release = check_finish(&s, "release/1.2.0", &release_opts(false), &b, &clean()).unwrap();
    let tips = StartTips { branch: oid(1), bases: vec![("main".into(), oid(2)), ("develop".into(), oid(3))] };

    let r = release.rest_after_conflict(1, &tips).unwrap();
    assert_eq!((r.into.as_str(), r.tip, r.base), ("main", oid(1), oid(2)));
    assert_eq!(texts(&r), ["tag main as v1.2.0", "check out develop", "merge release/1.2.0 into develop", "delete release/1.2.0"]);
    assert_eq!((r.plan.kind, r.plan.short.as_str(), r.plan.tag.as_deref()), (FlowKind::Release, "1.2.0", Some("v1.2.0")));
    assert_eq!(r.plan.label(), "Finish release 1.2.0");

    let r = release.rest_after_conflict(4, &tips).unwrap();
    assert_eq!((r.into.as_str(), r.base), ("develop", oid(3)));
    assert_eq!(texts(&r), ["delete release/1.2.0"]);

    assert_eq!(release.rest_after_conflict(0, &tips), None, "a checkout");
    assert_eq!(release.rest_after_conflict(2, &tips), None, "a tag");
    assert_eq!(release.rest_after_conflict(9, &tips), None, "past the end");
    let kept = check_finish(&s, "feature/login", &FinishOptions { keep: true, ..Default::default() }, &b, &clean()).unwrap();
    assert_eq!(kept.rest_after_conflict(1, &tips), None, "nothing comes after the merge");
    let feature = check_finish(&s, "feature/login", &FinishOptions::default(), &b, &clean()).unwrap();
    assert!(feature.rest_after_conflict(1, &tips).is_some());
    let no_target = StartTips { branch: oid(1), bases: vec![("main".into(), oid(2))] };
    assert_eq!(feature.rest_after_conflict(1, &no_target), None);
}

fn snap(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(50)).unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// Runs `cmds` until one fails, echoing each the way the job runner does; the output and
/// whether all ran.
fn run_job(dir: &Path, cmds: &[Vec<String>]) -> (bool, String) {
    let mut out = String::new();
    for c in cmds {
        out.push_str(&format!("$ git {}\n", c.join(" ")));
        let (ok, o) = run(dir, c);
        out.push_str(o.trim_end());
        out.push('\n');
        if !ok {
            return (false, out);
        }
    }
    (true, out)
}

/// Runs `plan` from the repository's current state; the resume when it stops at a conflict.
fn run_plan(dir: &Path, plan: &FinishPlan) -> Option<FinishResume> {
    let tips = StartTips::of(&snap(dir), plan).unwrap();
    let (ok, out) = run_job(dir, &plan.cmds());
    if ok {
        return None;
    }
    assert!(stopped_by_conflict(&out), "{out}");
    plan.rest_after_conflict(failed_step(&out, plan.steps.len()), &tips)
}

/// Git Flow set up with tag prefix "v", and release/1.2.0 changing a.txt one way while main
/// (and, with `both`, develop) changed it another, so Finish conflicts on main (and develop).
fn conflicting_release(name: &str, both: bool) -> PathBuf {
    let dir = initialized(name, "v");
    git(&dir, &["checkout", "-q", "-b", "release/1.2.0", "develop"]);
    commit(&dir, "a.txt", "one\nrelease\n", "release");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "one\nmain\n", "main");
    if both {
        git(&dir, &["checkout", "-q", "develop"]);
        commit(&dir, "a.txt", "one\ndevelop\n", "develop");
    }
    git(&dir, &["checkout", "-q", "release/1.2.0"]);
    dir
}

fn release_plan(dir: &Path) -> FinishPlan {
    let snap = snap(dir);
    let s = snap.gitflow.settings().unwrap();
    check_finish(&s, "release/1.2.0", &release_opts(false), &Branches::of(&snap), &WorkState::of(&snap)).unwrap()
}

fn resolve(dir: &Path) {
    write(dir, "a.txt", "one\nresolved\n");
    git(dir, &["add", "a.txt"]);
    git(dir, &["commit", "-q", "--no-edit"]);
}

fn check(dir: &Path, r: &FinishResume) -> Result<FinishPlan, FinishProblem> {
    let snap = snap(dir);
    let s = snap.gitflow.settings().unwrap();
    check_resume(r, &s, &Branches::of(&snap), &WorkState::of(&snap))
}

#[test]
fn resume_release_after_conflict_on_main() {
    let dir = conflicting_release("gitflow-resume-main", false);
    let work = rev(&dir, "release/1.2.0");
    let main = rev(&dir, "main");
    let r = run_plan(&dir, &release_plan(&dir)).expect("stopped at the conflict on main");
    assert_eq!((r.into.as_str(), r.tip.to_string(), r.base.to_string()), ("main", work.clone(), main));
    let merging = snap(&dir);
    assert!(r.merging(&merging));
    assert_eq!(resume_state(&r, true, &merging), ResumeState::Waiting);

    resolve(&dir);
    let done = snap(&dir);
    assert!(!r.merging(&done));
    assert_eq!(resume_state(&r, true, &done), ResumeState::Ready);
    assert_eq!(resume_state(&r, false, &done), ResumeState::Ready);
    let plan = check(&dir, &r).unwrap();
    assert_eq!(plan.resume_label(), "Continue finish release 1.2.0");
    assert_eq!(run_plan(&dir, &plan), None, "the rest runs through");

    assert_eq!(rev(&dir, "main^2"), work);
    assert_eq!(git_out(&dir, &["cat-file", "-t", "v1.2.0"]), "tag");
    assert_eq!(rev(&dir, "v1.2.0^{commit}"), rev(&dir, "main"));
    assert!(contains(&dir, &work, "develop"));
    assert_eq!(parents(&dir, "develop"), 2);
    assert_eq!(rev(&dir, "refs/heads/release/1.2.0"), "");
    assert_eq!(git_out(&dir, &["symbolic-ref", "--short", "HEAD"]), "develop");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resume_stops_again_on_develop() {
    let dir = conflicting_release("gitflow-resume-develop", true);
    let r = run_plan(&dir, &release_plan(&dir)).unwrap();
    resolve(&dir);
    let plan = check(&dir, &r).unwrap();
    let again = run_plan(&dir, &plan).expect("stopped at the conflict on develop");
    assert_eq!((again.into.as_str(), texts(&again)), ("develop", vec!["delete release/1.2.0"]));
    assert_eq!(again.base.to_string(), rev(&dir, "develop"));
    assert_eq!(resume_state(&again, true, &snap(&dir)), ResumeState::Waiting);
    resolve(&dir);
    assert_eq!(resume_state(&again, true, &snap(&dir)), ResumeState::Ready);
    assert_eq!(run_plan(&dir, &check(&dir, &again).unwrap()), None);
    assert_eq!(rev(&dir, "refs/heads/release/1.2.0"), "");
    assert_eq!(rev(&dir, "v1.2.0^{commit}"), rev(&dir, "main"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resume_checks_out_the_target_when_head_moved() {
    let dir = conflicting_release("gitflow-resume-head-moved", true);
    let r = run_plan(&dir, &release_plan(&dir)).unwrap();
    resolve(&dir);
    let plan = check(&dir, &r).unwrap();
    assert_eq!(plan.steps[0].what, "tag main as v1.2.0", "the tag names main, so no checkout first");
    let again = run_plan(&dir, &plan).expect("stopped at the conflict on develop");
    resolve(&dir);
    let develop = rev(&dir, "develop");
    git(&dir, &["checkout", "-q", "release/1.2.0"]);
    assert_eq!(resume_state(&again, true, &snap(&dir)), ResumeState::Ready);
    let plan = check(&dir, &again).unwrap();
    let steps: Vec<&str> = plan.steps.iter().map(|s| s.what.as_str()).collect();
    assert_eq!(steps, ["check out develop", "delete release/1.2.0"]);
    assert_eq!(texts(&again), ["delete release/1.2.0"], "the remembered steps are unchanged");
    assert_eq!(run_plan(&dir, &plan), None, "the delete runs from develop");
    assert_eq!(rev(&dir, "refs/heads/release/1.2.0"), "");
    assert_eq!(git_out(&dir, &["symbolic-ref", "--short", "HEAD"]), "develop");
    assert_eq!(rev(&dir, "develop"), develop);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn aborted_or_changed_resumes_are_gone() {
    let dir = conflicting_release("gitflow-resume-gone", false);
    let r = run_plan(&dir, &release_plan(&dir)).unwrap();
    git(&dir, &["merge", "--abort"]);
    let aborted = snap(&dir);
    assert_eq!(resume_state(&r, false, &aborted), ResumeState::Waiting, "a stale snapshot isn't an abort");
    assert_eq!(resume_state(&r, true, &aborted), ResumeState::Gone(ResumeGone::Aborted("main".into())));

    git(&dir, &["checkout", "-q", "release/1.2.0"]);
    commit(&dir, "b.txt", "more\n", "more");
    git(&dir, &["checkout", "-q", "main"]);
    let moved = ResumeGone::BranchMoved("release/1.2.0".into());
    assert_eq!(resume_state(&r, false, &snap(&dir)), ResumeState::Gone(moved.clone()));
    git(&dir, &["branch", "-D", "release/1.2.0"]);
    let deleted = ResumeGone::BranchGone("release/1.2.0".into());
    assert_eq!(resume_state(&r, false, &snap(&dir)), ResumeState::Gone(deleted.clone()));

    git(&dir, &["config", "--unset", "gitflow.branch.develop"]);
    assert_eq!(resume_state(&r, true, &snap(&dir)), ResumeState::Gone(ResumeGone::NotSetUp));
    git(&dir, &["config", "gitflow.branch.develop", "develop"]);
    git(&dir, &["config", "gitflow.branch.master", "trunk"]);
    assert_eq!(resume_state(&r, true, &snap(&dir)), ResumeState::Gone(ResumeGone::SettingsChanged));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn base_gone_while_waiting() {
    let dir = conflicting_release("gitflow-resume-base-gone", false);
    let r = run_plan(&dir, &release_plan(&dir)).unwrap();
    git(&dir, &["merge", "--abort"]);
    git(&dir, &["checkout", "-q", "develop"]);
    git(&dir, &["branch", "-D", "main"]);
    assert_eq!(resume_state(&r, true, &snap(&dir)), ResumeState::Gone(ResumeGone::BaseGone("main".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_resume_problems() {
    use FinishProblem as P;
    let dir = conflicting_release("gitflow-resume-checks", false);
    let r = run_plan(&dir, &release_plan(&dir)).unwrap();
    resolve(&dir);
    assert!(check(&dir, &r).is_ok());

    write(&dir, "a.txt", "changed\n");
    assert_eq!(check(&dir, &r), Err(P::TrackedChanges("release/1.2.0".into())));
    git(&dir, &["checkout", "--", "a.txt"]);

    git(&dir, &["tag", "v1.2.0", "develop"]);
    assert_eq!(check(&dir, &r), Err(P::TagExists("v1.2.0".into())));
    git(&dir, &["tag", "-d", "v1.2.0"]);

    let linked = crate::paths::temp_dir().join(format!("glowfly-test-gitflow-resume-linked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", &linked.display().to_string(), "develop"]);
    assert_eq!(check(&dir, &r), Err(P::CheckedOutElsewhere("develop".into())));
    git(&dir, &["worktree", "remove", "--force", &linked.display().to_string()]);
    assert!(check(&dir, &r).is_ok());

    let snap = snap(&dir);
    let s = snap.gitflow.settings().unwrap();
    let merging = WorkState { state: RepositoryState::Merge, ..WorkState::of(&snap) };
    assert_eq!(check_resume(&r, &s, &Branches::of(&snap), &merging), Err(P::InProgress("merge")));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&linked);
}
