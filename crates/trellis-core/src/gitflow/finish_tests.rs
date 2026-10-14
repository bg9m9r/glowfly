//! Finishing feature, release and hotfix branches: classifying branches, the checks, the plan's
//! commands and texts, and end-to-end runs against fixture repositories.

use super::*;
use crate::gitcli::run;
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, scratch_repo, write};
use git2::RepositoryState;
use std::path::{Path, PathBuf};

fn defaults() -> GitFlowSettings {
    GitFlowSettings {
        production: "main".into(),
        development: "develop".into(),
        feature: DEFAULT_FEATURE.into(),
        release: DEFAULT_RELEASE.into(),
        hotfix: DEFAULT_HOTFIX.into(),
        version_tag: "v".into(),
    }
}

fn branches<'a>(locals: &[&'a str]) -> Branches<'a> {
    Branches { locals: locals.to_vec(), remote: vec![], tags: vec![], head: None, has_commits: true }
}

fn clean() -> WorkState<'static> {
    WorkState { state: RepositoryState::Clean, tracked_changes: false, elsewhere: vec![] }
}

fn release_opts(keep: bool) -> FinishOptions {
    FinishOptions { tag: "v1.2.0".into(), message: "Release 1.2.0".into(), keep }
}

const ALL: [&str; 5] = ["main", "develop", "feature/login", "release/1.2.0", "hotfix/1.0.1"];

#[test]
fn classify() {
    let s = defaults();
    assert_eq!(flow_kind(&s, "feature/x"), Some(FlowKind::Feature));
    assert_eq!(flow_kind(&s, "release/1.2.0"), Some(FlowKind::Release));
    assert_eq!(flow_kind(&s, "hotfix/1.0.1"), Some(FlowKind::Hotfix));
    for none in ["main", "develop", "feature/", "featurex", "Feature/x"] {
        assert_eq!(flow_kind(&s, none), None, "{none}");
    }
    // Overlapping prefixes from other tools: the longest match wins.
    let overlap = GitFlowSettings { feature: "f".into(), release: "feature/".into(), ..defaults() };
    assert_eq!(flow_kind(&overlap, "feature/1.0"), Some(FlowKind::Release));
    assert_eq!(flow_kind(&overlap, "fix"), Some(FlowKind::Feature));
    // A production branch that happens to start with a prefix is still not a flow branch.
    let odd = GitFlowSettings { production: "hotfix/main".into(), ..defaults() };
    assert_eq!(flow_kind(&odd, "hotfix/main"), None);
    assert_eq!(short_name(&s, FlowKind::Feature, "feature/login"), "login");
    assert_eq!(short_name(&s, FlowKind::Release, "release/1.2.0"), "1.2.0");
}

#[test]
fn branch_list_and_defaults() {
    let s = defaults();
    let mut b = branches(&["main", "develop", "feature/a", "other", "release/1.2.0", "hotfix/1.0.1"]);
    assert_eq!(
        flow_branches(&s, &b),
        [(FlowKind::Feature, "feature/a"), (FlowKind::Release, "release/1.2.0"), (FlowKind::Hotfix, "hotfix/1.0.1")]
    );
    b.head = Some("release/1.2.0");
    assert_eq!(
        flow_branches(&s, &b),
        [(FlowKind::Release, "release/1.2.0"), (FlowKind::Feature, "feature/a"), (FlowKind::Hotfix, "hotfix/1.0.1")]
    );
    b.head = Some("other");
    assert_eq!(flow_branches(&s, &b)[0], (FlowKind::Feature, "feature/a"));

    assert_eq!(default_tag(&s, FlowKind::Release, "release/1.2.0").as_deref(), Some("v1.2.0"));
    assert_eq!(
        default_tag(&GitFlowSettings { version_tag: String::new(), ..defaults() }, FlowKind::Hotfix, "hotfix/1.0.1").as_deref(),
        Some("1.0.1")
    );
    assert_eq!(default_tag(&s, FlowKind::Feature, "feature/a"), None);
    assert_eq!(default_tag_message(FlowKind::Release, "1.2.0"), "Release 1.2.0");
    assert_eq!(default_tag_message(FlowKind::Hotfix, "1.2.1"), "Hotfix 1.2.1");
    assert_eq!(default_tag_message(FlowKind::Feature, "a"), "");
    assert_eq!(targets(&s, FlowKind::Feature), "develop");
    assert_eq!(targets(&s, FlowKind::Release), "main and develop");
    assert_eq!(targets(&s, FlowKind::Hotfix), "main and develop");
}

#[test]
fn finish_problems() {
    use FinishProblem as P;
    let s = defaults();
    let b = branches(&ALL);
    let w = clean();
    let feature = FinishOptions::default();
    let problem = |branch: &str, o: &FinishOptions, b: &Branches, w: &WorkState| check_finish(&s, branch, o, b, w).unwrap_err();

    let p = problem("main", &feature, &b, &w);
    assert_eq!((p.message(), &p), ("main isn't a feature, release or hotfix branch.".into(), &P::NotFlowBranch("main".into())));
    let p = problem("feature/gone", &feature, &b, &w);
    assert_eq!((p.message(), &p), ("feature/gone doesn't exist.".into(), &P::NoBranch("feature/gone".into())));

    let no_dev = branches(&["main", "feature/login", "release/1.2.0"]);
    let p = problem("feature/login", &feature, &no_dev, &w);
    assert_eq!(p, P::MissingBases { kind: FlowKind::Feature, missing: vec!["develop".into()] });
    assert_eq!(p.message(), "Finishing a feature needs develop, and it doesn't exist.");
    let no_main = branches(&["develop", "release/1.2.0", "hotfix/1.0.1"]);
    assert_eq!(
        problem("release/1.2.0", &release_opts(false), &no_main, &w).message(),
        "Finishing a release needs main, and it doesn't exist."
    );
    assert!(check_finish(&s, "feature/login", &feature, &branches(&["develop", "feature/login"]), &w).is_ok());
    let neither = branches(&["hotfix/1.0.1"]);
    let p = problem("hotfix/1.0.1", &release_opts(false), &neither, &w);
    assert_eq!(p, P::MissingBases { kind: FlowKind::Hotfix, missing: vec!["main".into(), "develop".into()] });
    assert_eq!(p.message(), "Finishing a hotfix needs main and develop, and neither exists.");

    let merging = WorkState { state: RepositoryState::Merge, ..clean() };
    let p = problem("feature/login", &feature, &b, &merging);
    assert_eq!((p.message(), &p), ("Finish or abort the merge in progress first.".into(), &P::InProgress("merge")));

    let dirty = WorkState { tracked_changes: true, ..clean() };
    let p = problem("feature/login", &feature, &b, &dirty);
    assert_eq!(p.message(), "Commit or stash your changes before finishing feature/login.");

    let dev_elsewhere = WorkState { elsewhere: vec!["develop"], ..clean() };
    let p = problem("feature/login", &feature, &b, &dev_elsewhere);
    assert_eq!(p, P::CheckedOutElsewhere("develop".into()));
    assert_eq!(p.message(), "develop is checked out in another worktree. Switch that worktree to another branch first.");
    let main_elsewhere = WorkState { elsewhere: vec!["main"], ..clean() };
    assert_eq!(problem("release/1.2.0", &release_opts(false), &b, &main_elsewhere), P::CheckedOutElsewhere("main".into()));
    let branch_elsewhere = WorkState { elsewhere: vec!["feature/login"], ..clean() };
    assert_eq!(problem("feature/login", &feature, &b, &branch_elsewhere), P::CheckedOutElsewhere("feature/login".into()));
    let keep = FinishOptions { keep: true, ..Default::default() };
    assert!(check_finish(&s, "feature/login", &keep, &b, &branch_elsewhere).is_ok());

    let tag = |t: &str, m: &str| FinishOptions { tag: t.into(), message: m.into(), keep: false };
    let p = problem("release/1.2.0", &tag(" ", "m"), &b, &w);
    assert_eq!((p.message(), &p), ("Enter a tag name.".into(), &P::EmptyTag));
    for bad in ["-x", "a..b"] {
        let p = problem("release/1.2.0", &tag(bad, "m"), &b, &w);
        assert_eq!(p.message(), format!("{bad} isn't a valid tag name."));
        assert_eq!(p, P::InvalidTag(bad.into()));
    }
    let mut tagged = branches(&ALL);
    tagged.tags = vec!["v1.2.0"];
    let p = problem("release/1.2.0", &release_opts(false), &tagged, &w);
    assert_eq!((p.message(), &p), ("The tag v1.2.0 already exists.".into(), &P::TagExists("v1.2.0".into())));
    let p = problem("release/1.2.0", &tag("v1.2.0", "  "), &b, &w);
    assert_eq!((p.message(), &p), ("Enter a tag message.".into(), &P::EmptyMessage));
    // A feature has no tag, so empty fields don't matter.
    assert!(check_finish(&s, "feature/login", &tag("", ""), &b, &w).is_ok());
}

fn cmds_and_texts(plan: &FinishPlan) -> Vec<(Vec<String>, &str)> {
    plan.steps.iter().map(|s| (s.cmd.clone(), s.what.as_str())).collect()
}

#[test]
fn finish_steps() {
    let s = defaults();
    let b = branches(&ALL);
    let w = clean();
    let plan = check_finish(&s, "feature/login", &FinishOptions::default(), &b, &w).unwrap();
    assert_eq!(
        cmds_and_texts(&plan),
        [
            (args(&["checkout", "develop", "--"]), "check out develop"),
            (args(&["merge", "--no-ff", "--no-edit", "refs/heads/feature/login"]), "merge feature/login into develop"),
            (args(&["branch", "-d", "feature/login"]), "delete feature/login"),
        ]
    );
    assert_eq!(plan.cmds().len(), 3);
    assert_eq!((plan.kind, plan.short.as_str(), plan.tag.as_ref()), (FlowKind::Feature, "login", None));
    assert_eq!(plan.label(), "Finish feature login");
    assert_eq!(plan.summary(&s), "Finished feature login: merged into develop and deleted feature/login.");
    let kept = check_finish(&s, "feature/login", &FinishOptions { keep: true, ..Default::default() }, &b, &w).unwrap();
    assert_eq!(kept.cmds(), plan.cmds()[..2]);
    assert_eq!(kept.summary(&s), "Finished feature login: merged into develop.");

    let o = FinishOptions { tag: " v1.2.0 ".into(), message: "  Release 1.2.0\n".into(), keep: false };
    let plan = check_finish(&s, "release/1.2.0", &o, &b, &w).unwrap();
    let merge = args(&["merge", "--no-ff", "--no-edit", "refs/heads/release/1.2.0"]);
    assert_eq!(
        cmds_and_texts(&plan),
        [
            (args(&["checkout", "main", "--"]), "check out main"),
            (merge.clone(), "merge release/1.2.0 into main"),
            (args(&["tag", "-a", "v1.2.0", "-m", "Release 1.2.0", "refs/heads/main"]), "tag main as v1.2.0"),
            (args(&["checkout", "develop", "--"]), "check out develop"),
            (merge, "merge release/1.2.0 into develop"),
            (args(&["branch", "-d", "release/1.2.0"]), "delete release/1.2.0"),
        ]
    );
    assert_eq!(plan.tag.as_deref(), Some("v1.2.0"));
    assert_eq!(plan.label(), "Finish release 1.2.0");
    assert_eq!(plan.summary(&s), "Finished release 1.2.0: merged into main and develop, tagged v1.2.0 and deleted release/1.2.0.");
    let kept = check_finish(&s, "release/1.2.0", &release_opts(true), &b, &w).unwrap();
    assert_eq!(kept.cmds(), plan.cmds()[..5]);
    assert_eq!(kept.summary(&s), "Finished release 1.2.0: merged into main and develop and tagged v1.2.0.");

    let o = FinishOptions { tag: "v1.0.1".into(), message: "Hotfix 1.0.1".into(), keep: false };
    let plan = check_finish(&s, "hotfix/1.0.1", &o, &b, &w).unwrap();
    let cmds = plan.cmds();
    assert_eq!(cmds.len(), 6);
    assert_eq!(cmds[0], args(&["checkout", "main", "--"]));
    assert_eq!(cmds[1], args(&["merge", "--no-ff", "--no-edit", "refs/heads/hotfix/1.0.1"]));
    assert_eq!(cmds[2], args(&["tag", "-a", "v1.0.1", "-m", "Hotfix 1.0.1", "refs/heads/main"]));
    assert_eq!(cmds[5], args(&["branch", "-d", "hotfix/1.0.1"]));
    assert_eq!(plan.label(), "Finish hotfix 1.0.1");
}

/// A repository with one commit on main and Git Flow set up (develop at main) with tag prefix
/// `tag_prefix`.
fn initialized(name: &str, tag_prefix: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "tag.gpgsign", "false"]);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let snap = load(&dir, &LoadOptions::all(50)).unwrap();
    let b = Branches::of(&snap);
    let s = GitFlowSettings { version_tag: tag_prefix.into(), ..suggested(&snap.gitflow, &b) };
    for c in init_cmds(&s, &snap.gitflow, &b).unwrap() {
        let (ok, out) = run(&dir, &c);
        assert!(ok, "{c:?}: {out}");
    }
    dir
}

/// Starts `branch` from `base` with one commit adding `file`; returns that commit.
fn work_on(dir: &Path, branch: &str, base: &str, file: &str) -> String {
    git(dir, &["checkout", "-q", "-b", branch, base]);
    write(dir, file, "work\n");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", &format!("work on {branch}")]);
    rev(dir, "HEAD")
}

/// Checks and runs Finish against the repository's current state.
fn finish(dir: &Path, branch: &str, o: &FinishOptions) {
    let snap = load(dir, &LoadOptions::all(50)).unwrap();
    let s = snap.gitflow.settings().unwrap();
    let plan = check_finish(&s, branch, o, &Branches::of(&snap), &WorkState::of(&snap)).unwrap();
    for c in plan.cmds() {
        let (ok, out) = run(dir, &c);
        assert!(ok, "{c:?}: {out}");
    }
}

fn rev(dir: &Path, r: &str) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", "--verify", "--quiet", r]));
    if ok { out.trim().to_owned() } else { String::new() }
}

fn git_out(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "{a:?}: {out}");
    out.trim().to_owned()
}

fn parents(dir: &Path, r: &str) -> usize {
    git_out(dir, &["rev-list", "--parents", "-n", "1", r]).split_whitespace().count() - 1
}

fn contains(dir: &Path, commit: &str, branch: &str) -> bool {
    run(dir, &args(&["merge-base", "--is-ancestor", commit, branch])).0
}

#[test]
fn finish_feature_end_to_end() {
    let dir = initialized("gitflow-finish-feature", "");
    let work = work_on(&dir, "feature/login", "develop", "login.txt");
    finish(&dir, "feature/login", &FinishOptions::default());
    assert_eq!(git_out(&dir, &["symbolic-ref", "--short", "HEAD"]), "develop");
    // A fast-forward was possible, but --no-ff makes a merge commit.
    assert_eq!(parents(&dir, "develop"), 2);
    assert!(contains(&dir, &work, "develop"));
    assert_eq!(rev(&dir, "refs/heads/feature/login"), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn finish_release_end_to_end() {
    let dir = initialized("gitflow-finish-release", "v");
    let work = work_on(&dir, "release/1.2.0", "develop", "release.txt");
    finish(&dir, "release/1.2.0", &release_opts(false));
    assert_eq!(parents(&dir, "main"), 2);
    assert!(contains(&dir, &work, "main"));
    assert_eq!(git_out(&dir, &["cat-file", "-t", "v1.2.0"]), "tag");
    assert_eq!(git_out(&dir, &["tag", "-l", "--format=%(contents)", "v1.2.0"]), "Release 1.2.0");
    assert_eq!(rev(&dir, "v1.2.0^{commit}"), rev(&dir, "main"));
    assert_eq!(parents(&dir, "develop"), 2);
    assert!(contains(&dir, &work, "develop"));
    assert_eq!(rev(&dir, "refs/heads/release/1.2.0"), "");
    assert_eq!(git_out(&dir, &["symbolic-ref", "--short", "HEAD"]), "develop");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn finish_hotfix_kept_end_to_end() {
    let dir = initialized("gitflow-finish-hotfix", "");
    let work = work_on(&dir, "hotfix/1.0.1", "main", "fix.txt");
    let o = FinishOptions { tag: "1.0.1".into(), message: "Hotfix 1.0.1".into(), keep: true };
    finish(&dir, "hotfix/1.0.1", &o);
    assert_eq!(parents(&dir, "main"), 2);
    assert_eq!(rev(&dir, "1.0.1^{commit}"), rev(&dir, "main"));
    assert!(contains(&dir, &work, "develop"));
    assert_eq!(parents(&dir, "develop"), 2);
    assert_eq!(rev(&dir, "refs/heads/hotfix/1.0.1"), work);
    let _ = std::fs::remove_dir_all(&dir);
}
