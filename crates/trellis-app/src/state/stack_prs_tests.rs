//! A stack's pull request targets, its base and why its run is refused.

use super::*;
use git2::{Oid, RepositoryState};
use trellis_core::forge::{Provider, PullRequest, Target};
use trellis_core::gitcli::{args, run};
use trellis_core::repo::{BranchInfo, LoadOptions, Tracking, load};
use trellis_core::stack::{StackLink, Stacks};

fn github() -> ForgeState {
    let t = Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    ForgeState::new(Some(t), "origin".into(), None)
}

fn link(branch: &str, on: &str, behind: bool) -> StackLink {
    StackLink { branch: branch.into(), on: on.into(), base: Oid::zero(), behind }
}

/// `git <a>` in `dir`, with an identity; its trimmed output.
fn git(dir: &std::path::Path, a: &[&str]) -> String {
    let mut all = vec!["-c", "user.name=T", "-c", "user.email=t@example.com"];
    all.extend(a);
    let (ok, out) = run(dir, &args(&all));
    assert!(ok, "{a:?}: {out}");
    out.trim().to_owned()
}

/// main (the fixture's one commit), develop one commit past it, low one past develop and mid one
/// past low; mid stacked on low. origin has main and develop. With `flow`, Git Flow is set up
/// with main and develop.
fn stacked(name: &str, flow: bool) -> Snapshot {
    let dir = crate::state::op_fixture::snapshot(name).workdir;
    git(&dir, &["branch", "-M", "main"]);
    let mut at = git(&dir, &["rev-parse", "HEAD"]);
    for b in ["develop", "low", "mid"] {
        at = git(&dir, &["commit-tree", "HEAD^{tree}", "-p", &at, "-m", b]);
        git(&dir, &["update-ref", &format!("refs/heads/{b}"), &at]);
    }
    let mut snap = load(&dir, &LoadOptions::all(50)).unwrap();
    let tip = |n: &str| snap.local_oid(n).unwrap();
    snap.remotes = vec![("origin".into(), vec![("develop".into(), tip("develop")), ("main".into(), tip("main"))])];
    snap.stacks = Stacks { links: vec![link("mid", "low", false)] };
    if flow {
        snap.gitflow.production = Some("main".into());
        snap.gitflow.development = Some("develop".into());
        (snap.gitflow.feature, snap.gitflow.release, snap.gitflow.hotfix) = (Some("f/".into()), Some("r/".into()), Some("h/".into()));
    }
    snap
}

fn pr(id: u64, source: &str) -> PullRequest {
    PullRequest {
        id,
        title: "PR".into(),
        description: String::new(),
        source: source.into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

fn done(snap: Snapshot) {
    let _ = std::fs::remove_dir_all(&snap.workdir);
}

#[test]
fn each_layer_targets_the_branch_below_by_its_name_on_the_host() {
    let mut snap = stacked("stack-prs-layers", false);
    let low: &mut BranchInfo = snap.locals.iter_mut().find(|b| b.name == "low").unwrap();
    low.tracking = Some(Tracking { remote: "origin".into(), merge: "feature/low".into() });
    let fs = github();
    let layers = pr_layers(&snap, &fs, "mid");
    let shown: Vec<(&str, &str, Option<&str>)> =
        layers.iter().map(|l| (l.branch.as_str(), l.source.as_str(), l.target.as_deref())).collect();
    assert_eq!(shown, [("low", "feature/low", Some("main")), ("mid", "mid", Some("feature/low"))]);
    done(snap);
}

#[test]
fn the_bottom_layer_targets_the_base_it_is_closest_to() {
    let snap = stacked("stack-prs-base-flow", true);
    let mut fs = github();
    assert_eq!(base_for(&snap, &fs, "low").as_deref(), Some("develop"), "low is one commit past develop");
    fs.default_branch = Some("main".into());
    assert_eq!(base_for(&snap, &fs, "low").as_deref(), Some("develop"));
    assert_eq!(base_for(&snap, &fs, "main").as_deref(), Some("main"), "a tie goes to the default branch");
    done(snap);
    let snap = stacked("stack-prs-base-plain", false);
    assert_eq!(base_for(&snap, &github(), "low").as_deref(), Some("main"), "develop isn't a base without Git Flow");
    done(snap);
}

#[test]
fn a_run_is_refused_without_a_host_behind_a_restack_or_with_every_pr_open() {
    let mut snap = stacked("stack-prs-blockers", false);
    let mut fs = github();
    assert_eq!(prs_blocker(Lang::En, &snap, &fs, "mid"), None);
    let no_host = ForgeState::new(None, "origin".into(), None);
    assert!(prs_blocker(Lang::En, &snap, &no_host, "mid").unwrap().contains("code host"));
    snap.stacks.links[0].behind = true;
    let en = prs_blocker(Lang::En, &snap, &fs, "low").unwrap();
    assert_eq!(en, "mid needs a restack onto low first.");
    assert_ne!(prs_blocker(Lang::De, &snap, &fs, "low"), Some(en));
    snap.stacks.links[0].behind = false;
    fs.prs = vec![pr(1, "low")];
    assert_eq!(prs_blocker(Lang::En, &snap, &fs, "mid"), None, "mid has none yet");
    fs.prs.push(pr(2, "mid"));
    assert_eq!(prs_blocker(Lang::En, &snap, &fs, "mid").as_deref(), Some("Every branch in the stack already has an open pull request."));
    snap.state = RepositoryState::Merge;
    assert_eq!(prs_blocker(Lang::En, &snap, &fs, "mid").as_deref(), rebase_onto::in_progress(Lang::En, RepositoryState::Merge));
    done(snap);
}

#[test]
fn a_run_knows_its_step_and_what_it_left() {
    let layer = |b: &str| PrLayer { branch: b.into(), source: format!("h-{b}"), target: None };
    let run = StackPrRun { left: [layer("top")].into(), current: Some(layer("mid")), index: 2, total: 3, ..Default::default() };
    assert_eq!(run.step_of("h-mid"), Some((2, 3)));
    assert_eq!(run.step_of("mid"), None, "by the name on the host");
    assert_eq!(run.not_created(), ["mid", "top"]);
}
