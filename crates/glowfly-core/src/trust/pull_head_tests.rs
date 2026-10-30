//! A fetch that stores into the branch checked out, compared with real `git pull`: one that
//! makes an unborn branch, one held when it can't be compared, and a two-way merge over a file
//! that was only touched.

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
        set_config(&dir, &[("pull.rebase", "false")]);
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

#[test]
fn a_fetch_that_makes_the_unborn_branch_checked_out_ends_as_git_pull_does() {
    // A name, the pull, and the setup both repositories get.
    type Case<'a> = (&'a str, &'a [&'a str], fn(&Path));
    let cases: [Case; 2] =
        [("pull-unborn-named", &["pull", "origin", "main:main"], |_| {}), ("pull-unborn-mirror", &["pull", "origin", "main"], mirror)];
    for (name, a, setup) in cases {
        for split in [true, false] {
            let dirs = unborn_twins(&format!("{name}-{split}"), &setup);
            let repo = Repository::open(&dirs[3]).unwrap();
            assert!(pull(a).stores_into_head(&repo), "{a:?}: an unborn branch is matched too");
            // The split runs it as git pull; the pinned pull checks the commit out itself.
            let (_, ours) = pull_both(&dirs[2], &dirs[3], a, split);
            assert!(dirs[3].join("a.txt").exists(), "{a:?}: {ours}");
            remove(&dirs);
        }
    }
}

#[test]
fn a_pinned_pull_holds_a_fetch_that_makes_the_unborn_branch_checked_out() {
    let follows = [Follow { path: ".githooks".to_owned(), tracked: None, hooks: true }];
    let dirs = unborn_twins("pull-unborn-held", &|_| {});
    let work = &dirs[3];
    let mut git = |a: &[String], input: &str, _: &[(&str, &str)]| crate::gitcli::run_with_input(work, a, input, crate::gitcli::NO_PROMPTS);
    // Known before it fetches.
    let held = run_pinned(work, &pull(&["pull", "origin", "main:main"]), &follows, None, None, &mut git);
    let PullRun::Refused { refused: Refused::Unchecked(Unchecked::Unmodeled(_)), output } = held else { panic!("held: {held:?}") };
    assert!(!output.contains("$ git fetch"), "{output}");
    // A wildcard named isn't matched before the fetch; HEAD born by it is held after.
    let held = run_pinned(work, &pull(&["pull", "origin", "refs/heads/*:refs/heads/*"]), &follows, None, None, &mut git);
    let PullRun::Refused { refused: Refused::Unchecked(Unchecked::Unmodeled(why)), output } = held else { panic!("held: {held:?}") };
    assert_eq!(why, "a fetch that moved the branch checked out", "{output}");
    assert!(output.contains("$ git fetch"), "{output}");
    assert!(!work.join("a.txt").exists(), "nothing checked out: {output}");
    remove(&dirs);
}

#[test]
fn a_file_only_touched_doesn_t_stop_the_working_tree_following_a_fetched_branch() {
    let touched = |d: &Path| {
        mirror(d);
        set_config(d, &[("pull.rebase", "false")]);
        // Same content, a newer time: the index's stat no longer matches.
        let f = std::fs::File::options().write(true).open(d.join("a.txt")).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(120)).unwrap();
    };
    let dirs = twins("pull-touched", &touched);
    remote_commit(&dirs[1], "main", "a.txt", "two\n");
    let (_, pinned) = pull_both(&dirs[2], &dirs[3], &["pull"], false);
    assert!(pinned.contains("fast-forwarding your working tree"), "{pinned}");
    assert_eq!(std::fs::read_to_string(dirs[3].join("a.txt")).unwrap(), "two\n");
    remove(&dirs);
}
