//! Splitting a pull into its fetch and a merge or rebase of exactly the commits it fetched.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;

fn pull(a: &[&str]) -> Pull {
    Pull::of(&args(a)).expect("a pull")
}

fn head(oid: Oid) -> Fetched {
    Fetched { oid, line: format!("{oid}\t\tbranch 'main' of ../origin") }
}

#[test]
fn a_pull_s_options_go_to_its_fetch_or_what_follows() {
    assert_eq!(Pull::of(&args(&["fetch"])), None);
    let p = pull(&["-c", "x=y", "pull", "-q", "--no-edit", "--prune", "--depth", "3", "origin", "main"]);
    assert_eq!(p.fetch, args(&["-c", "x=y", "fetch", "--update-head-ok", "-q", "--prune", "--depth", "3", "origin", "main"]));
    assert_eq!(p.merge, args(&["-q", "--no-edit"]));
    assert!(p.named);
    assert!(pull(&["pull", "--dry-run"]).fetch_only());
}

#[test]
fn the_pinned_command_merges_or_rebases_exactly_the_checked_commits() {
    let (work, bare) = with_origin("pull-pinned");
    let repo = Repository::open(&work).unwrap();
    let oid = repo.head().unwrap().target().unwrap();
    let msg = "Merge branch 'main' of ../origin";
    // Nothing chosen anywhere (the user's own config may choose): git refuses divergent
    // branches, as --ff-only does.
    let config = repo.config().unwrap();
    if config.get_string("pull.rebase").is_err() && config.get_string("pull.ff").is_err() {
        assert_eq!(pull(&["pull"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["merge", "--ff-only", "-m", msg, &oid.to_string()])));
    }
    git(&work, &["config", "pull.rebase", "false"]);
    git(&work, &["config", "pull.ff", "only"]);
    assert_eq!(pull(&["pull"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["merge", "--ff-only", "-m", msg, &oid.to_string()])));
    git(&work, &["config", "--unset", "pull.ff"]);
    assert_eq!(
        pull(&["pull", "--no-ff"]).pinned(&repo, &[head(oid)], msg, None),
        Ok(args(&["merge", "--no-ff", "-m", msg, &oid.to_string()]))
    );
    assert_eq!(pull(&["pull", "--no-rebase"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["merge", "-m", msg, &oid.to_string()])));
    let o = oid.to_string();
    assert_eq!(pull(&["pull", "--rebase"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["rebase", "--onto", &o, &o])));
    let fork = repo.head().unwrap().peel_to_commit().unwrap().id();
    let p = pull(&["pull", "--rebase=merges", "-Xours"]);
    assert_eq!(
        p.pinned(&repo, &[head(oid)], msg, Some(fork)),
        Ok(args(&["rebase", "-Xours", "--rebase-merges", "--onto", &o, &fork.to_string()]))
    );
    // The settings choose as git's do.
    git(&work, &["config", "pull.rebase", "true"]);
    assert_eq!(pull(&["pull"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["rebase", "--onto", &o, &o])));
    git(&work, &["config", "branch.main.rebase", "false"]);
    git(&work, &["config", "pull.ff", "false"]);
    assert_eq!(pull(&["pull"]).pinned(&repo, &[head(oid)], msg, None), Ok(args(&["merge", "--no-ff", "-m", msg, &o])));
    git(&work, &["config", "branch.main.rebase", "true"]);
    assert!(pull(&["pull"]).pinned(&repo, &[head(oid), head(oid)], msg, None).is_err(), "can't rebase onto two");
    assert!(pull(&["pull"]).pinned(&repo, &[], msg, None).is_err(), "nothing to merge");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn fetch_head_s_commits_for_merging_are_read_as_git_wrote_them() {
    let (work, bare) = with_origin("pull-fetched");
    git(&bare, &["branch", "other", "main"]);
    remote_commit(&bare, "main", "b.txt", "b\n");
    let p = pull(&["pull"]);
    git(&work, &p.fetch.iter().map(String::as_str).collect::<Vec<_>>());
    let repo = Repository::open(&work).unwrap();
    let heads = fetched(&repo).unwrap();
    let main = repo.revparse_single("origin/main").unwrap().id();
    assert_eq!(heads.iter().map(|h| h.oid).collect::<Vec<_>>(), [main], "only main is for merging, not other");
    assert!(heads[0].line.contains("branch 'main'"), "{heads:?}");
    assert_eq!(p.fork_ref(&repo).as_deref(), Some("refs/remotes/origin/main"));
    std::fs::remove_file(repo.path().join("FETCH_HEAD")).unwrap();
    assert!(fetched(&repo).is_err(), "nothing read is never nothing to check");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// A repository whose `core.hooksPath` is the tracked `.githooks`, with a post-merge hook that
/// writes `ran.txt` when it runs, pushed to its origin.
fn following(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let (work, bare) = with_origin(name);
    std::fs::create_dir_all(work.join(".githooks")).unwrap();
    write(&work, ".githooks/post-merge", "#!/bin/sh\necho one > ran.txt\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "hooks"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    (work, bare)
}

fn run(work: &std::path::Path, saved: &crate::settings::SavedTrust) -> PullRun {
    let repo = Repository::open(work).unwrap();
    let follows = crate::trust::scan_now(&repo).follows;
    assert!(!follows.is_empty());
    let mut git = |a: &[String], input: &str| crate::gitcli::run_with_input(work, a, input, crate::gitcli::NO_PROMPTS);
    run_pinned(work, &pull(&["pull", "--ff-only"]), &follows, Some(saved), &mut git)
}

#[test]
fn a_pinned_pull_merges_what_it_checked_and_holds_other_hooks() {
    let (work, bare) = following("pull-run");
    let saved = crate::settings::SavedTrust::trusted(Some(crate::trust::scan_at(&work).unwrap().print()));
    // Upstream changed something else: the pull runs, without a second fetch.
    remote_commit(&bare, "main", "b.txt", "b\n");
    let PullRun::Ran { ok: true, output } = run(&work, &saved) else { panic!("it runs") };
    assert_eq!(output.matches("$ git fetch").count(), 1, "{output}");
    assert!(output.contains("$ git merge --ff-only -m"), "{output}");
    assert!(work.join("b.txt").exists());
    assert!(std::fs::read_to_string(work.join("ran.txt")).unwrap().contains("one"), "the trusted hook ran as post-merge");
    // Upstream changed the hook: only the fetch ran, and the hook didn't.
    remote_commit(&bare, "main", ".githooks/post-merge", "#!/bin/sh\necho two > ran.txt\n");
    let head = Repository::open(&work).unwrap().head().unwrap().target();
    let PullRun::Refused { refused: crate::trust::compare::Refused::Held(inc), .. } = run(&work, &saved) else { panic!("held") };
    assert_eq!(inc.hooks, ["post-merge"]);
    assert_eq!(Repository::open(&work).unwrap().head().unwrap().target(), head, "nothing merged");
    let ran = std::fs::read_to_string(work.join("ran.txt")).unwrap_or_default();
    assert!(!ran.contains("two"), "the incoming hook never ran: {ran}");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}
