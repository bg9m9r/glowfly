//! Pulls whose fetch may store into the branch checked out, or that pull into an unborn branch,
//! compared with real `git pull`: the split hands them to git pull before it fetches (the
//! pinned pull holds them), so the working tree is only ever brought along by git itself.

use super::*;
use crate::trust::Follow;
use crate::trust::compare::Refused;
use crate::trust::incoming::Unchecked;
use std::path::PathBuf;

/// An origin (with the repository it was pushed from) and two new empty repositories (`git
/// init`) with it as `origin`, set up alike by `setup`: seed, origin, the one git pull runs in,
/// the one the split runs in.
fn unborn_twins(name: &str, setup: &dyn Fn(&Path)) -> [PathBuf; 4] {
    let (seed, bare) = with_origin(name);
    let init = |n: &str| {
        let dir = seed.with_file_name(format!("{}-{n}", seed.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        set_config(&dir, TEST_CONFIG);
        git(&dir, &["remote", "add", "origin", &bare.display().to_string()]);
        setup(&dir);
        dir
    };
    let (real, pinned) = (init("real"), init("pinned"));
    [seed, bare, real, pinned]
}

fn remove(dirs: &[PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn mirror(d: &Path) {
    git(d, &["config", "--replace-all", "remote.origin.fetch", "+refs/heads/*:refs/heads/*"]);
}

/// `name` ignored (in `.git/info/exclude`) and written with "mine".
fn ignored(d: &Path, name: &str) {
    write(d, ".git/info/exclude", &format!("{name}\n"));
    write(d, name, "mine\n");
}

/// What a pinned pull with something to compare does with `a` in `work`.
fn pinned(work: &Path, a: &[&str]) -> PullRun {
    let follows = [Follow { path: ".githooks".to_owned(), tracked: None, hooks: true }];
    let mut git = |c: &[String], input: &str, _: &[(&str, &str)]| crate::gitcli::run_with_input(work, c, input, crate::gitcli::NO_PROMPTS);
    run_pinned(work, &pull(a), &follows, None, None, &mut git)
}

/// Held before the fetch, as a pull the pinned one doesn't model.
fn held_before_fetching(run: PullRun) {
    let PullRun::Refused { refused: Refused::Unchecked(Unchecked::Unmodeled(_)), output } = &run else { panic!("held: {run:?}") };
    assert!(!output.contains("$ git fetch"), "{output}");
}

#[test]
fn a_pull_into_an_unborn_branch_ends_as_git_pull_does() {
    // A name, the pull, and the setup both repositories get.
    type Case<'a> = (&'a str, &'a [&'a str], fn(&Path));
    let cases: [Case; 5] = [
        ("pull-unborn", &["pull", "origin", "main"], |_| {}),
        ("pull-unborn-noff", &["pull", "--no-ff", "origin", "main"], |_| {}),
        ("pull-unborn-named", &["pull", "origin", "main:main"], |_| {}),
        ("pull-unborn-mirror", &["pull", "origin", "main"], mirror),
        ("pull-unborn-ignored", &["pull", "origin", "main:main"], |d| ignored(d, "a.txt")),
    ];
    for (name, a, setup) in cases {
        let dirs = unborn_twins(name, &setup);
        // Still unborn: held before anything is fetched.
        held_before_fetching(pinned(&dirs[3], a));
        let (_, split) = pull_both(&dirs[2], &dirs[3], a, true);
        assert!(!split.contains("$ git fetch"), "{a:?}: git pull alone, no fetch of the split's own: {split}");
        let file = |d: &Path| std::fs::read_to_string(d.join("a.txt")).ok();
        assert_eq!(file(&dirs[3]), file(&dirs[2]), "{a:?}");
        remove(&dirs);
    }
}

#[test]
fn a_pull_whose_fetch_may_store_into_the_branch_checked_out_is_git_pull_s_alone() {
    let ignores = |d: &Path| ignored(d, "c.txt");
    for (name, a) in
        [("pull-wildcard", &["pull", "origin", "refs/heads/*:refs/heads/*"][..]), ("pull-ignored", &["pull", "origin", "main:main"])]
    {
        let dirs = twins(name, &ignores);
        // origin's new c.txt lands where an ignored c.txt is: git pull keeps that file.
        remote_commit(&dirs[1], "main", "c.txt", "c\n");
        let (_, split) = pull_both(&dirs[2], &dirs[3], a, true);
        assert!(!split.contains("$ git fetch"), "{a:?}: {split}");
        let file = |d: &Path| std::fs::read_to_string(d.join("c.txt")).unwrap();
        assert_eq!(file(&dirs[3]), file(&dirs[2]), "{a:?}");
        assert_eq!(file(&dirs[3]), "mine\n", "{a:?}: the ignored file is kept");
        held_before_fetching(pinned(&dirs[3], a));
        remove(&dirs);
    }
}

#[test]
fn a_store_into_the_branch_checked_out_the_early_check_misses_is_refused_by_the_fetch() {
    let mirror_remote = |d: &Path| {
        let url = crate::gitcli::run_with_input(d, &args(&["remote", "get-url", "origin"]), "", &[]).1;
        git(d, &["remote", "add", "mirror", url.trim()]);
        git(d, &["config", "--replace-all", "remote.mirror.fetch", "+refs/heads/*:refs/heads/*"]);
    };
    let none = |_: &Path| {};
    type Case<'a> = (&'a str, &'a [&'a str], &'a dyn Fn(&Path));
    let cases: [Case; 4] = [
        ("pull-short-pattern", &["pull", "origin", "r*:r*"], &none),
        ("pull-refmap", &["pull", "origin", "main", "--refmap=+refs/heads/*:refs/heads/*"], &none),
        ("pull-all", &["pull", "--all"], &mirror_remote),
        ("pull-c-refspec", &["-c", "remote.origin.fetch=+refs/heads/*:refs/heads/*", "pull"], &none),
    ];
    for (name, a, setup) in cases {
        let dirs = twins(name, &|d| {
            set_config(d, &[("pull.rebase", "false")]);
            setup(d);
        });
        remote_commit(&dirs[1], "main", "r.txt", "r\n");
        let repo = Repository::open(&dirs[3]).unwrap();
        assert!(!pull(a).stores_into_head(&repo), "{a:?}: the early check misses it");
        let head = || Repository::open(&dirs[3]).unwrap().head().unwrap().target();
        let before = head();
        // Held after the fetch git refused, with nothing moved.
        let PullRun::Refused { refused: Refused::Unchecked(Unchecked::Unmodeled(_)), output } = pinned(&dirs[3], a) else {
            panic!("{a:?}: held")
        };
        assert!(output.contains("refusing to fetch into"), "{a:?}: {output}");
        assert_eq!(head(), before, "{a:?}: {output}");
        // The split hands it to git pull, which ends as git pull does.
        let (_, split) = pull_both(&dirs[2], &dirs[3], a, true);
        let pulled = split.lines().any(|l| l.starts_with("$ git ") && l.split(' ').any(|w| w == "pull"));
        assert!(split.contains("refusing to fetch into") && pulled, "{a:?}: {split}");
        assert!(dirs[3].join("r.txt").exists(), "{a:?}: {split}");
        remove(&dirs);
    }
}

#[test]
fn a_remote_saying_git_s_refusal_is_only_a_failed_fetch() {
    let (work, bare) = with_origin("pull-remote-says");
    let follows = [Follow { path: ".githooks".to_owned(), tracked: None, hooks: true }];
    // The remote prints git's refusal on its own lines, then the fetch fails.
    let said =
        "remote: fatal: refusing to fetch into branch 'refs/heads/main' checked out at '/x'\nfatal: could not read from remote repository";
    let mut git = |c: &[String], input: &str, _: &[(&str, &str)]| {
        if c.iter().any(|a| a == "fetch") {
            return (false, said.to_owned());
        }
        crate::gitcli::run_with_input(&work, c, input, crate::gitcli::NO_PROMPTS)
    };
    let gated = run_pinned(&work, &pull(&["pull"]), &follows, None, None, &mut git);
    assert!(matches!(&gated, PullRun::Ran { ok: false, .. }), "not held under the remote's words: {gated:?}");
    let PullRun::Ran { ok: false, output } = run_split(&work, &pull(&["pull"]), &mut git) else { panic!("it fails") };
    assert!(!output.lines().any(|l| l.starts_with("$ git pull")), "no git pull of its own: {output}");
    remove(&[work, bare]);
}
