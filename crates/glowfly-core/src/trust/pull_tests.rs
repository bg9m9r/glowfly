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
    assert_eq!(p.fetch, args(&["-c", "x=y", "fetch", "--update-head-ok", "-q", "--prune", "--depth", "3", "origin", "main"]));
    assert_eq!(p.merge, args(&["-q", "--no-edit"]));
    assert!(p.named);
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
    let msg = "Merge branch 'main' of ../origin";
    let pinned = |a: &[&str]| pull(a).pinned(&repo, &[head(oid)], msg, None);
    // Nothing chosen anywhere (the user's own config may choose): git refuses.
    let config = repo.config().unwrap();
    if config.get_string("pull.rebase").is_err() && config.get_string("pull.ff").is_err() {
        assert!(pinned(&["pull"]).is_err(), "divergent branches need a choice");
    }
    git(&work, &["config", "pull.rebase", "false"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "--no-log", "-m", msg, &o])));
    assert_eq!(pinned(&["pull", "--no-ff"]), Ok(args(&["merge", "--no-ff", "--no-log", "-m", msg, &o])));
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
        p.pinned(&repo, &[head(oid)], msg, Some(fork)),
        Ok(args(&["rebase", "-Xours", "--rebase-merges", "--onto", &o, &fork.to_string()]))
    );
    git(&work, &["config", "branch.main.rebase", "false"]);
    git(&work, &["config", "pull.ff", "false"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "--no-ff", "--no-log", "-m", msg, &o])));
    git(&work, &["config", "branch.main.rebase", "true"]);
    assert!(pull(&["pull"]).pinned(&repo, &[head(oid), head(oid)], msg, None).is_err(), "can't rebase onto two");
    assert!(pull(&["pull"]).pinned(&repo, &[], msg, None).is_err(), "nothing to merge");
    // A rebasing pull with nothing of its own to replay fast-forwards, as git does.
    git(&work, &["reset", "-q", "--hard", "HEAD~1"]);
    assert_eq!(pinned(&["pull"]), Ok(args(&["merge", "--ff-only", &o])));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// Two clones of one origin, set up alike by `setup` (local config and commits); then origin
/// gets a commit. `git pull <a>` runs in the first, the pinned pull in the second: both end
/// alike (success, the message of HEAD, its parents, its tree).
fn same_as_git(name: &str, a: &[&str], setup: &dyn Fn(&Path)) {
    let (seed, bare) = with_origin(name);
    let clone = |n: &str| {
        let dir = seed.with_file_name(format!("{}-{n}", seed.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&dir);
        git(dir.parent().unwrap(), &["-c", "core.autocrlf=false", "clone", "-q", &bare.display().to_string(), &dir.display().to_string()]);
        set_config(
            &dir,
            &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")],
        );
        setup(&dir);
        dir
    };
    let (real, pinned) = (clone("real"), clone("pinned"));
    remote_commit(&bare, "main", "r.txt", "r\n");
    let cmd = args(a);
    let (real_ok, real_out) = crate::gitcli::run_with_input(&real, &cmd, "", crate::gitcli::NO_PROMPTS);
    let mut run = |c: &[String], input: &str| crate::gitcli::run_with_input(&pinned, c, input, crate::gitcli::NO_PROMPTS);
    let PullRun::Ran { ok, output } = run_pinned(&pinned, &Pull::of(&cmd).unwrap(), &[], None, None, &mut run) else { panic!("it runs") };
    assert_eq!(ok, real_ok, "{a:?}: git said {real_out}; pinned said {output}");
    let end = |d: &Path| {
        let r = Repository::open(d).unwrap();
        let c = r.head().unwrap().peel_to_commit().unwrap();
        (c.message().unwrap_or_default().to_owned(), c.parent_count(), c.tree_id())
    };
    assert_eq!(end(&pinned), end(&real), "{a:?}: {output}");
    for d in [&seed, &bare, &real, &pinned] {
        let _ = std::fs::remove_dir_all(d);
    }
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
    assert!(output.contains("$ git merge --ff-only --no-log -m"), "{output}");
    assert!(!output.contains("fmt-merge-msg --no-log"), "merge.log decides the message");
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
