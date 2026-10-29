//! Reading what a command brings into the working tree from git's arguments, and resolving it
//! the way git does.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;
use crate::trust::TrustScan;

fn of(a: &[&str]) -> Vec<Target> {
    targets(&args(a)).unwrap()
}

fn unmodeled(a: &[&str]) -> bool {
    matches!(targets(&args(a)), Err(Unchecked::Unmodeled(_)))
}

fn rev(s: &str) -> Target {
    Target::Rev(s.to_owned())
}

fn switch(s: &str, paths: bool) -> Target {
    Target::Switch { name: s.to_owned(), paths }
}

fn replayed(onto: Option<&str>, upstream: Option<&str>, branch: Option<&str>) -> Target {
    let s = |v: Option<&str>| v.map(str::to_owned);
    Target::Replayed { onto: s(onto), upstream: s(upstream), branch: s(branch), root: false, keep_base: false, how: How::default() }
}

fn merged(revs: &[&str]) -> Target {
    Target::Merged { revs: args(revs), how: How::default() }
}

fn picked(revs: &[&str], revert: bool, how: How) -> Target {
    Target::Picked { revs: args(revs), revert, how }
}

fn patches(file: &str, apply: &[&str], mailbox: bool) -> Target {
    Target::Patches { files: args(&[file]), apply: args(apply), mailbox }
}

/// A scratch repository with one commit.
fn committed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

#[test]
fn the_commands_that_move_the_working_tree_name_what_they_bring() {
    assert_eq!(of(&["checkout", "main", "--"]), [switch("main", false)]);
    assert_eq!(of(&["checkout", "main"]), [switch("main", true)], "maybe a file");
    assert_eq!(of(&["checkout", "-m", "main"]), [switch("main", true)], "-m merges, it takes no value");
    assert_eq!(of(&["checkout", "--track", "origin/x", "--"]), [switch("origin/x", false)]);
    assert_eq!(of(&["checkout", "-b", "new", "origin/x"]), [rev("origin/x")]);
    assert_eq!(of(&["checkout", "-b", "new"]), [], "a new branch at HEAD");
    assert_eq!(of(&["checkout", "--", "a.txt"]), [Target::Index(args(&["a.txt"]))], "files from the index");
    let paths = |tree: &str, p: &[&str], maybe| Target::Paths { tree: tree.into(), paths: args(p), maybe_paths: maybe };
    assert_eq!(of(&["checkout", "v1", "--", ".githooks"]), [paths("v1", &[".githooks"], false)]);
    assert_eq!(of(&["checkout", "v1", "a.txt"]), [paths("v1", &["a.txt"], true)]);
    assert_eq!(of(&["restore", "--source=v1", "--", ".githooks"]), [paths("v1", &[".githooks"], false)]);
    assert_eq!(of(&["restore", "-s", "v1", "x"]), [paths("v1", &["x"], false)], "-s is restore's source");
    assert_eq!(of(&["restore", ".githooks"]), [Target::Index(args(&[".githooks"]))]);
    assert_eq!(of(&["restore", "--staged", "x"]), [], "the index only");
    assert_eq!(of(&["switch", "-c", "new", "v1"]), [rev("v1")]);
    assert_eq!(of(&["switch", "topic"]), [switch("topic", false)]);
    assert_eq!(of(&["checkout", "-"]), [switch("@{-1}", true)]);
    assert_eq!(of(&["merge", "--no-ff", "-m", "msg", "topic", "other"]), [merged(&["topic", "other"])]);
    assert_eq!(of(&["merge"]), [merged(&["@{upstream}"])]);
    assert_eq!(of(&["merge", "FETCH_HEAD"]), [merged(&["FETCH_HEAD"])]);
    let x = How { xopts: args(&["ours", "patience"]), ..How::default() };
    assert_eq!(of(&["merge", "-Xours", "--strategy-option=patience", "t"]), [Target::Merged { revs: args(&["t"]), how: x }]);
    assert!(unmodeled(&["merge", "-s", "resolve", "t"]), "a strategy git merge-tree doesn't make");
    assert_eq!(of(&["am", "/p/x.patch", "/p/y.patch"]), [patches("/p/x.patch", &[], true), patches("/p/y.patch", &[], true)]);
    assert_eq!(of(&["am", "-p2", "--directory=.githooks", "x.patch"]), [patches("x.patch", &["-p2", "--directory=.githooks"], true)]);
    assert!(unmodeled(&["am", "--3way", "x.patch"]), "a three-way fallback isn't read");
    assert!(unmodeled(&["am", "--patch-format=stgit", "x.patch"]));
    assert!(unmodeled(&["am"]), "patches from standard input");
    assert_eq!(of(&["apply", "x.patch"]), [patches("x.patch", &[], false)]);
    assert_eq!(of(&["apply", "--cached", "x.patch"]), [], "the index only");
    assert_eq!(of(&["am", "--continue"]), [Target::Applying, Target::Sequencer { skip: false }]);
    assert_eq!(of(&["am", "--skip"]), [Target::Applying, Target::Sequencer { skip: true }]);
    assert_eq!(of(&["merge", "--abort"]), []);
    assert_eq!(of(&["rebase", "main"]), [replayed(None, Some("main"), None)]);
    assert_eq!(of(&["rebase", "main", "topic"]), [replayed(None, Some("main"), Some("topic"))], "topic is checked out first");
    assert_eq!(of(&["rebase", "--onto", "v2", "v1"]), [replayed(Some("v2"), Some("v1"), None)]);
    assert_eq!(of(&["rebase", "--onto=v2", "v1", "topic"]), [replayed(Some("v2"), Some("v1"), Some("topic"))]);
    assert_eq!(of(&["rebase"]), [replayed(None, None, None)]);
    assert!(matches!(of(&["rebase", "--root"]).as_slice(), [Target::Replayed { root: true, upstream: None, .. }]));
    for apply in [&["rebase", "--apply", "main"][..], &["rebase", "-C2", "main"], &["rebase", "-i", "main"], &["rebase", "-r", "main"]] {
        assert!(unmodeled(apply), "{apply:?}");
    }
    assert_eq!(of(&["rebase", "--continue"]), [Target::Applying, Target::Sequencer { skip: false }]);
    assert_eq!(of(&["cherry-pick", "--skip"]), [Target::Applying, Target::Sequencer { skip: true }]);
    assert_eq!(of(&["cherry-pick", "-x", "-s", "a..b", "c"]), [picked(&["a..b", "c"], false, How::default())], "-s signs off");
    let m = How { mainline: Some(1), ..How::default() };
    assert_eq!(of(&["revert", "-m", "1", "c"]), [picked(&["c"], true, m)]);
    let walk = How { walk: args(&["--branches", "--not"]), ..How::default() };
    assert_eq!(of(&["cherry-pick", "--branches", "--not", "main"]), [picked(&["main"], false, walk)], "rev-list options go to git");
    assert!(unmodeled(&["cherry-pick", "--stdin"]));
    assert_eq!(of(&["stash", "pop", "--index", "1"]), [Target::Stash { spec: "stash@{1}".into(), index: true }]);
    assert_eq!(of(&["stash", "apply"]), [Target::Stash { spec: "stash@{0}".into(), index: false }]);
    assert_eq!(of(&["stash", "push", "-u"]), []);
    assert_eq!(of(&["reset", "--hard", "v1"]), [rev("v1")]);
    assert_eq!(of(&["reset", "v1"]), [], "the index only");
    assert_eq!(of(&["-c", "x=y", "pull", "--rebase"]), [], "pinned by pull.rs");
    assert_eq!(of(&["commit", "-m", "x"]), []);
    assert_eq!(of(&["lfs", "fetch"]), []);
    assert_eq!(of(&["worktree", "remove", "x"]), []);
}

#[test]
fn a_command_glowfly_doesn_t_read_is_held() {
    for cmd in [
        &["bisect", "good"][..],
        &["my-alias"],
        &["submodule", "update", "--init"],
        &["worktree", "add", "../x"],
        &["lfs", "pull"],
        &["sparse-checkout", "set", "x"],
        &["stash", "frobnicate"],
        &["read-tree", "-u", "HEAD"],
        &["-c", "core.hooksPath=/elsewhere", "commit"],
        &["-c", "Core.HooksPath=x", "status"],
        &["--work-tree=/x", "checkout", "main"],
        &["-C", "/x", "status"],
    ] {
        assert!(unmodeled(cmd), "{cmd:?}");
    }
}

#[test]
fn a_checkout_resolves_its_name_the_way_git_does() {
    let (work, bare) = with_origin("incoming-dwim");
    remote_commit(&bare, "main", "b.txt", "b\n");
    git(&work, &["fetch", "-q"]);
    git(&work, &["branch", "-q", "-r", "-D", "origin/main"]);
    git(&work, &["fetch", "-q", "origin", "main:refs/remotes/origin/only"]);
    let repo = Repository::open(&work).unwrap();
    let only = repo.revparse_single("origin/only").unwrap().id();
    let head = repo.head().unwrap().target().unwrap();
    let get = |t: &Target| commits(&repo, &[], t);
    assert_eq!(get(&switch("nowhere", false)), Err(Unchecked::Unresolved("nowhere".into())), "held, never passed");
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    assert_eq!(get(&switch("a.txt", true)), Ok(vec![]), "a file");
    // A tag and a branch of one name: checkout takes the branch, a merge the tag.
    git(&work, &["tag", "dup", &only.to_string()]);
    git(&work, &["branch", "dup", "main"]);
    assert_eq!(get(&switch("dup", false)), Ok(vec![head]));
    // `@{push}`, which libgit2 can't resolve, as git resolves it.
    let push = repo.revparse_single("refs/remotes/origin/main").unwrap().id();
    assert_eq!(get(&rev("@{push}")), Ok(vec![push]));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_branch_only_remotes_have_needs_every_one_unless_one_is_the_default() {
    let (work, bare) = with_origin("incoming-remotes");
    let (other_work, other) = with_origin("incoming-remotes-other");
    git(&other, &["branch", "topic", "main"]);
    git(&bare, &["branch", "topic", "main"]);
    remote_commit(&other, "topic", "c.txt", "c\n");
    remote_commit(&bare, "topic", "d.txt", "d\n");
    git(&work, &["remote", "add", "other", &other.to_string_lossy()]);
    git(&work, &["fetch", "-q", "--all"]);
    let repo = Repository::open(&work).unwrap();
    let id = |r: &str| repo.revparse_single(r).unwrap().id();
    let mut both = commits(&repo, &[], &switch("topic", false)).unwrap();
    both.sort();
    let mut want = vec![id("origin/topic"), id("other/topic")];
    want.sort();
    assert_eq!(both, want, "git refuses to pick, so both are compared");
    git(&work, &["config", "checkout.defaultRemote", "other"]);
    assert_eq!(commits(&repo, &[], &switch("topic", false)), Ok(vec![id("other/topic")]));
    for d in [&work, &bare, &other_work, &other] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_replay_lists_every_commit_it_replays_and_where_it_starts() {
    let dir = committed("incoming-replay");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    for n in 0..3 {
        write(&dir, &format!("t{n}.txt"), "t\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", &format!("t{n}")]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let repo = Repository::open(&dir).unwrap();
    let id = |r: &str| repo.revparse_single(r).unwrap().id();
    let got = commits(&repo, &[], &replayed(None, Some("main"), Some("topic"))).unwrap();
    assert_eq!(got.len(), 5, "topic's tip (checked out first), main, then the three replayed: {got:?}");
    assert!(got.contains(&id("topic~2")) && got.contains(&id("main")));
    let range = commits(&repo, &[], &picked(&["main..topic"], false, How::default())).unwrap();
    assert_eq!(range.len(), 3);
    let reverted = commits(&repo, &[], &picked(&["topic"], true, How::default())).unwrap();
    assert_eq!(reverted, [id("topic~1")], "a revert brings back the parent's version");
    let _ = std::fs::remove_dir_all(&dir);
}

fn hooks_follow() -> [Follow; 1] {
    [Follow { path: ".githooks".into(), tracked: None, hooks: true }]
}

#[test]
fn a_long_replay_is_compared_commit_by_commit_where_hooks_run_from() {
    let dir = committed("incoming-long");
    let repo = Repository::open(&dir).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    let mut parent = repo.head().unwrap().peel_to_commit().unwrap();
    let plain = parent.tree().unwrap();
    // A hook deep inside a replay of 1500 commits.
    let mut hooked = repo.treebuilder(Some(&plain)).unwrap();
    let mut dir_tree = repo.treebuilder(None).unwrap();
    dir_tree.insert("post-merge", repo.blob(b"echo evil\n").unwrap(), 0o100755).unwrap();
    hooked.insert(".githooks", dir_tree.write().unwrap(), 0o040000).unwrap();
    let hooked = repo.find_tree(hooked.write().unwrap()).unwrap();
    for n in 0..1500 {
        let tree = if n == 700 { &hooked } else { &plain };
        let id = repo.commit(None, &sig, &sig, "x", tree, &[&parent]).unwrap();
        parent = repo.find_commit(id).unwrap();
    }
    repo.branch("long", &parent, false).unwrap();
    let target = replayed(None, Some("main"), Some("long"));
    assert_eq!(commits(&repo, &[], &target).map(|c| c.len()), Ok(1502), "every commit, however many");
    // Trusted as main has it: no hooks there.
    let follows = hooks_follow();
    let main = repo.head().unwrap().target().unwrap();
    let now = TrustScan { follows: follow::versions(&repo, &follows, main).unwrap(), ..TrustScan::default() };
    let saved = SavedTrust::trusted(Some(now.print()));
    let first = repo.revparse_single("long~1400").unwrap().id();
    assert_eq!(super::super::compare::check(&repo, &follows, Some(&saved), &[first]), Ok(()), "a plain one is covered");
    let got = super::super::compare::check_targets(&repo, &follows, Some(&saved), &[target], None);
    assert!(matches!(got, Err(super::super::compare::Refused::Held(_))), "{got:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
