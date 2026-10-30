//! Splitting a pull into its fetch and a merge or rebase of exactly the commits it fetched.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;
use std::path::Path;

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
    assert_eq!(p.fetch, args(&["-c", "x=y", "fetch", "-q", "--prune", "--depth", "3", "origin", "main"]));
    assert_eq!(p.merge, args(&["-q", "--no-edit"]));
    assert_eq!((p.remote.as_deref(), p.refspec.as_deref()), (Some("origin"), Some("main")));
    assert_eq!(p.reflog, "pull -q --no-edit --prune --depth 3 origin main", "GIT_REFLOG_ACTION as git pull sets it");
    assert!(pull(&["pull", "--dry-run"]).fetch_only());
}

#[test]
fn the_pinned_command_merges_or_rebases_exactly_the_checked_commits() {
    let (work, bare) = with_origin("pull-pinned");
    // Diverged: a local commit, and another upstream, fetched.
    write(&work, "l.txt", "l\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "local"]);
    remote_commit(&bare, "main", "r.txt", "r\n");
    git(&work, &["fetch", "-q"]);
    let repo = Repository::open(&work).unwrap();
    let oid = repo.revparse_single("origin/main").unwrap().id();
    let o = oid.to_string();
    let pinned = |a: &[&str]| pull(a).pinned(&repo, &[head(oid)], None);
    assert_eq!(
        pinned(&["pull", "--rebase=preserve"]),
        Err("fatal: invalid value for '--rebase': 'preserve'".to_owned()),
        "as git refuses it"
    );
    // Nothing chosen anywhere (the user's own config may choose): git refuses.
    let config = repo.config().unwrap();
    if config.get_string("pull.rebase").is_err() && config.get_string("pull.ff").is_err() {
        assert!(pinned(&["pull"]).is_err(), "divergent branches need a choice");
    }
    git(&work, &["config", "pull.rebase", "false"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "FETCH_HEAD"])));
    assert_eq!(pinned(&["pull", "--no-ff"]), Ok(args(&["merge", "--no-ff", "FETCH_HEAD"])));
    git(&work, &["config", "pull.ff", "only"]);
    assert!(pinned(&["pull"]).is_err(), "pull.ff=only can't fast-forward");
    // A rebase on the command line overrides pull.ff=only.
    assert_eq!(pinned(&["pull", "--rebase"]), Ok(args(&["rebase", "--onto", &o, &o])));
    git(&work, &["config", "--unset", "pull.ff"]);
    git(&work, &["config", "pull.rebase", "true"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["rebase", "--onto", &o, &o])));
    // --ff-only on the command line overrides pull.rebase.
    assert!(pinned(&["pull", "--ff-only"]).is_err());
    let fork = repo.head().unwrap().peel_to_commit().unwrap().parent_id(0).unwrap();
    let p = pull(&["pull", "--rebase=merges", "-Xours"]);
    assert_eq!(
        p.pinned(&repo, &[head(oid)], Some(fork)),
        Ok(args(&["rebase", "-Xours", "--rebase-merges", "--onto", &o, &fork.to_string()]))
    );
    git(&work, &["config", "branch.main.rebase", "false"]);
    git(&work, &["config", "pull.ff", "false"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "--no-ff", "FETCH_HEAD"])));
    git(&work, &["config", "branch.main.rebase", "true"]);
    assert!(pull(&["pull"]).pinned(&repo, &[head(oid), head(oid)], None).is_err(), "can't rebase onto two");
    assert!(pull(&["pull"]).pinned(&repo, &[], None).is_err(), "nothing to merge");
    // A rebasing pull with nothing of its own to replay fast-forwards, as git does.
    git(&work, &["reset", "-q", "--hard", "HEAD~1"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "--ff-only", "FETCH_HEAD"])));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// Two clones of one origin, set up alike by `setup` (local config and commits); then origin
/// gets a commit. `git pull <a>` runs in the first, the pinned pull in the second: both end
/// alike (success, the message of HEAD, its parents, its tree, the working tree's status, and
/// what the error classifier makes of a failure).
fn same_as_git(name: &str, a: &[&str], setup: &dyn Fn(&Path)) {
    same_as_git_by(name, a, setup, false);
}

/// `same_as_git` with `run_split` (`split`) or `run_pinned` with nothing to compare. Returns what
/// git pull and the split printed.
fn same_as_git_by(name: &str, a: &[&str], setup: &dyn Fn(&Path), split: bool) -> (String, String) {
    let [seed, bare, real, pinned] = twins(name, setup);
    remote_commit(&bare, "main", "r.txt", "r\n");
    let said = pull_both(&real, &pinned, a, split);
    for d in [&seed, &bare, &real, &pinned] {
        let _ = std::fs::remove_dir_all(d);
    }
    said
}

/// An origin (with the repository it was pushed from) and two clones of it set up alike by
/// `setup`: seed, origin, the clone git pull runs in, the one the split runs in.
fn twins(name: &str, setup: &dyn Fn(&Path)) -> [std::path::PathBuf; 4] {
    let (seed, bare) = with_origin(name);
    let clone = |n: &str| {
        let dir = seed.with_file_name(format!("{}-{n}", seed.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&dir);
        git(dir.parent().unwrap(), &["-c", "core.autocrlf=false", "clone", "-q", &bare.display().to_string(), &dir.display().to_string()]);
        set_config(&dir, TEST_CONFIG);
        setup(&dir);
        dir
    };
    let (real, pinned) = (clone("real"), clone("pinned"));
    [seed, bare, real, pinned]
}

/// The config both repositories of a comparison get.
const TEST_CONFIG: &[(&str, &str)] =
    &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")];

/// `git pull <a>` in `real`, the split (`split`) or pinned pull in `pinned`: both end alike
/// (success, the message of HEAD, its parents, its tree, the reflog's last entry, the working
/// tree's status, and what the error classifier makes of a failure). What each printed.
fn pull_both(real: &Path, pinned: &Path, a: &[&str], split: bool) -> (String, String) {
    let cmd = args(a);
    let (real_ok, real_out) = crate::gitcli::run_with_input(real, &cmd, "", crate::gitcli::NO_PROMPTS);
    let mut run = |c: &[String], input: &str, extra: &[(&str, &str)]| {
        let env: Vec<(&str, &str)> = crate::gitcli::NO_PROMPTS.iter().chain(extra).copied().collect();
        crate::gitcli::run_with_input(pinned, c, input, &env)
    };
    let p = Pull::of(&cmd).unwrap();
    let ran = if split { run_split(pinned, &p, &mut run) } else { run_pinned(pinned, &p, &[], None, None, &mut run) };
    let PullRun::Ran { ok, output } = ran else { panic!("it runs") };
    assert_eq!(ok, real_ok, "{a:?}: git said {real_out}; pinned said {output}");
    let end = |d: &Path| {
        let r = Repository::open(d).unwrap();
        let c = r.head().unwrap().peel_to_commit().unwrap();
        let reflog = crate::gitcli::run_with_input(d, &args(&["reflog", "-1", "--format=%gs"]), "", &[]).1;
        // The reflog says what made the last change, as git pull words it.
        let reflog = reflog.split(':').next().unwrap_or_default().trim().to_owned();
        let status = crate::gitcli::run_with_input(d, &args(&["status", "--porcelain"]), "", &[]).1;
        (c.message().unwrap_or_default().to_owned(), c.parent_count(), c.tree_id(), reflog, status)
    };
    assert_eq!(end(pinned), end(real), "{a:?}: {output}");
    assert_eq!(
        crate::git_errors::classify(&output),
        crate::git_errors::classify(&real_out),
        "{a:?}: git said {real_out}; split said {output}"
    );
    (real_out, output)
}

#[test]
fn a_split_pull_fails_as_git_pull_does_and_is_explained_the_same() {
    use crate::git_errors::{Cause, classify};
    // Divergent branches with no choice anywhere (the user's own config may make one).
    let (probe, probe_bare) = with_origin("pull-split-probe");
    let config = Repository::open(&probe).unwrap().config().unwrap().snapshot().unwrap();
    let chosen = config.get_string("pull.rebase").is_ok() || config.get_string("pull.ff").is_ok();
    let _ = std::fs::remove_dir_all(&probe);
    let _ = std::fs::remove_dir_all(&probe_bare);
    if !chosen {
        let (git_said, split) = same_as_git_by("pull-split-diverged", &["pull"], &local_commit, true);
        assert!(matches!(classify(&split), Some(Cause::Diverged { branch: None })), "git said: {git_said} -- split said: {split}");
    }
    // The upstream was deleted on the remote.
    let gone = |d: &Path| set_config(d, &[("branch.main.merge", "refs/heads/gone")]);
    let (git_said, split) = same_as_git_by("pull-split-gone", &["pull"], &gone, true);
    assert!(matches!(classify(&split), Some(Cause::UpstreamGone { .. })), "git said: {git_said} -- split said: {split}");
    // No upstream at all.
    let none = |d: &Path| {
        git(d, &["config", "--unset", "branch.main.merge"]);
        git(d, &["config", "--unset", "branch.main.remote"]);
    };
    let (git_said, split) = same_as_git_by("pull-split-none", &["pull"], &none, true);
    assert!(matches!(classify(&split), Some(Cause::NoUpstream { push_branch: None })), "git said: {git_said} -- split said: {split}");
}

#[test]
fn a_fetch_that_may_store_into_the_branch_checked_out_runs_as_git_pull() {
    let mirror = |d: &Path| {
        git(d, &["config", "--replace-all", "remote.origin.fetch", "+refs/heads/*:refs/heads/*"]);
        set_config(d, &[("pull.rebase", "false")]);
    };
    same_as_git_by("pull-split-mirror", &["pull"], &mirror, true);
    let (work, bare) = with_origin("pull-stores-head");
    mirror(&work);
    let repo = Repository::open(&work).unwrap();
    let stores = |a: &[&str]| pull(a).stores_into_head(&repo);
    assert!(stores(&["pull"]), "a configured refspec stores into main");
    assert!(stores(&["pull", "origin", "x:y"]), "configured ones apply to named refspecs too");
    git(&work, &["config", "--replace-all", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"]);
    assert!(!stores(&["pull"]));
    assert!(stores(&["pull", "origin", "a:x", "b:main"]), "any named refspec, not just the first");
    assert!(stores(&["pull", "origin", "main:heads/main"]), "a short destination as git spells it out");
    assert!(stores(&["pull", "origin", "refs/heads/*:refs/heads/*"]), "a pattern that matches main");
    assert!(stores(&["pull", "origin", "+refs/heads/m*:refs/heads/m*"]));
    assert!(!stores(&["pull", "origin", "refs/heads/*:refs/heads/feature/*"]), "a pattern that can't");
    assert!(stores(&["pull", "origin", "a:x*y*"]), "a destination it can't read");
    assert!(!stores(&["pull", "origin", "main:"]), "no destination");
    git(&work, &["config", "--add", "remote.origin.fetch", "+refs/tags/*:refs/heads/*"]);
    assert!(stores(&["pull"]), "a configured pattern that matches main");
    // The branch moved while the fetch ran all the same (something else moved it): the pull
    // stops with git's words, files untouched.
    git(&work, &["config", "--replace-all", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"]);
    set_config(&work, &[("pull.rebase", "false")]);
    let old = repo.head().unwrap().target().unwrap();
    remote_commit(&bare, "main", "r.txt", "r\n");
    let mut run = |c: &[String], input: &str, extra: &[(&str, &str)]| {
        let env: Vec<(&str, &str)> = crate::gitcli::NO_PROMPTS.iter().chain(extra).copied().collect();
        let ran = crate::gitcli::run_with_input(&work, c, input, &env);
        if c.iter().any(|a| a == "fetch") {
            git(&work, &["update-ref", "refs/heads/main", "refs/remotes/origin/main"]);
        }
        ran
    };
    let PullRun::Ran { ok: false, output } = run_pinned(&work, &pull(&["pull"]), &[], None, None, &mut run) else { panic!("it stops") };
    assert_eq!(crate::git_errors::classify(&output), Some(crate::git_errors::Cause::FetchMovedHead { from: old.to_string() }), "{output}");
    assert!(!work.join("r.txt").exists(), "the working tree is left alone: {output}");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

fn local_commit(d: &Path) {
    write(d, "l.txt", "l\n");
    git(d, &["add", "."]);
    git(d, &["commit", "-q", "-m", "local"]);
}

#[test]
fn a_pinned_pull_ends_as_git_pull_does() {
    same_as_git("pull-same-log", &["pull"], &|d| {
        set_config(d, &[("pull.rebase", "false"), ("merge.log", "true")]);
        local_commit(d);
    });
    same_as_git("pull-same-noff", &["pull", "--no-ff", "--no-log"], &|d| set_config(d, &[("merge.log", "true")]));
    same_as_git("pull-same-rebase-ff", &["pull"], &|d| set_config(d, &[("pull.rebase", "true")]));
    same_as_git("pull-same-rebase", &["pull", "--rebase"], &|d| {
        set_config(d, &[("pull.ff", "only")]);
        local_commit(d);
    });
    same_as_git("pull-same-dirty", &["pull", "--rebase"], &|d| {
        write(
            d, "a.txt", "edited
",
        )
    });
    same_as_git("pull-same-ff-only", &["pull", "--ff-only"], &|d| {
        set_config(d, &[("pull.rebase", "true")]);
        local_commit(d);
    });
    same_as_git("pull-same-invalid", &["pull"], &|d| set_config(d, &[("pull.rebase", "sometimes")]));
    same_as_git("pull-same-invalid-ff", &["pull"], &|d| set_config(d, &[("pull.ff", "maybe")]));
    same_as_git("pull-same-named", &["pull", "--rebase", "origin", "main"], &local_commit);
    same_as_git("pull-same-autostash", &["pull", "--rebase", "--autostash"], &|d| write(d, "a.txt", "edited\n"));
}

#[test]
fn a_rebasing_pull_starts_from_the_fork_point_of_the_branch_its_refspec_stores_into() {
    let (work, bare) = with_origin("pull-fork-ref");
    git(&bare, &["branch", "other", "main"]);
    git(&work, &["fetch", "-q"]);
    let repo = Repository::open(&work).unwrap();
    assert_eq!(pull(&["pull", "origin", "other"]).fork_ref(&repo).as_deref(), Some("refs/remotes/origin/other"));
    assert_eq!(pull(&["pull", "origin", "+other:x"]).fork_ref(&repo).as_deref(), Some("refs/remotes/origin/other"));
    assert_eq!(pull(&["pull", "origin"]).fork_ref(&repo).as_deref(), Some("refs/remotes/origin/main"), "the upstream's own remote");
    assert_eq!(pull(&["pull", "elsewhere"]).fork_ref(&repo), None, "another remote's: no fork point");
    assert!(pull(&["pull", "origin", "other:main"]).stores_into_head(&repo));
    assert!(!pull(&["pull", "origin", "other:x"]).stores_into_head(&repo));
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
    // Executable, or git skips it on Unix (Windows doesn't check).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(work.join(".githooks/post-merge"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
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
    let mut git = |a: &[String], input: &str, _: &[(&str, &str)]| crate::gitcli::run_with_input(work, a, input, crate::gitcli::NO_PROMPTS);
    run_pinned(work, &pull(&["pull", "--ff-only"]), &follows, Some(saved), None, &mut git)
}

#[test]
fn a_pinned_pull_merges_what_it_checked_and_holds_other_hooks() {
    let (work, bare) = following("pull-run");
    let saved = crate::settings::SavedTrust::trusted(Some(crate::trust::scan_at(&work).unwrap().print()));
    // Upstream changed something else: the pull runs, without a second fetch.
    remote_commit(&bare, "main", "b.txt", "b\n");
    let PullRun::Ran { ok: true, output } = run(&work, &saved) else { panic!("it runs") };
    assert_eq!(output.matches("$ git fetch").count(), 1, "{output}");
    assert!(output.contains("$ git merge --ff-only FETCH_HEAD"), "{output}");
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

#[test]
fn a_pull_the_pinned_one_can_t_match_is_held_before_it_fetches() {
    let (work, bare) = following("pull-held");
    let saved = crate::settings::SavedTrust::trusted(Some(crate::trust::scan_at(&work).unwrap().print()));
    remote_commit(&bare, "main", "b.txt", "b\n");
    git(&work, &["config", "submodule.recurse", "true"]);
    let PullRun::Refused { refused: crate::trust::compare::Refused::Unchecked(crate::trust::incoming::Unchecked::Unmodeled(_)), output } =
        run(&work, &saved)
    else {
        panic!("held")
    };
    assert!(!output.contains("$ git fetch"), "{output}");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[path = "pull_head_tests.rs"]
mod head;
