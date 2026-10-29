//! Reading what a command brings into the working tree from git's arguments, and resolving it
//! the way git does.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;
use crate::trust::TrustScan;

fn of(a: &[&str]) -> Vec<Target> {
    targets(&args(a))
}

fn rev(s: &str) -> Target {
    Target::Rev(s.to_owned())
}

fn switch(s: &str, paths: bool) -> Target {
    Target::Switch { name: s.to_owned(), paths }
}

fn replayed(onto: Option<&str>, upstream: Option<&str>, branch: Option<&str>) -> Target {
    let s = |v: Option<&str>| v.map(str::to_owned);
    Target::Replayed { onto: s(onto), upstream: s(upstream), branch: s(branch), root: false, keep_base: false }
}

/// A scratch repository with one commit.
fn committed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(
        &dir, "a.txt", "one
",
    );
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
    assert_eq!(of(&["checkout", "--", "a.txt"]), [], "files from the index");
    let paths = |tree: &str, p: &[&str], maybe| Target::Paths { tree: tree.into(), paths: args(p), maybe_paths: maybe };
    assert_eq!(of(&["checkout", "v1", "--", ".githooks"]), [paths("v1", &[".githooks"], false)]);
    assert_eq!(of(&["checkout", "v1", "a.txt"]), [paths("v1", &["a.txt"], true)]);
    assert_eq!(of(&["switch", "-c", "new", "v1"]), [rev("v1")]);
    assert_eq!(of(&["switch", "topic"]), [switch("topic", false)]);
    assert_eq!(of(&["checkout", "-"]), [switch("@{-1}", true)]);
    assert_eq!(of(&["merge", "--no-ff", "-m", "msg", "topic", "other"]), [Target::Merged(args(&["topic", "other"]))]);
    assert_eq!(of(&["merge"]), [Target::Merged(args(&["@{upstream}"]))]);
    assert_eq!(of(&["merge", "FETCH_HEAD"]), [Target::Merged(args(&["FETCH_HEAD"]))]);
    assert_eq!(of(&["am", "--3way", "/p/x.patch"]), [Target::Mailbox(args(&["/p/x.patch"]))]);
    assert_eq!(of(&["am", "--continue"]), [Target::Sequencer]);
    assert_eq!(of(&["am", "--skip"]), [Target::Sequencer]);
    assert_eq!(of(&["am", "-p2", "x.patch"]), [Target::Mailbox(Vec::new())], "other paths: not read");
    assert_eq!(of(&["am", "--directory=.githooks", "x.patch"]), [Target::Mailbox(Vec::new())]);
    assert_eq!(of(&["merge", "--abort"]), []);
    assert_eq!(of(&["rebase", "main"]), [replayed(None, Some("main"), None)]);
    assert_eq!(of(&["rebase", "main", "topic"]), [replayed(None, Some("main"), Some("topic"))], "topic is checked out first");
    assert_eq!(of(&["rebase", "--onto", "v2", "v1"]), [replayed(Some("v2"), Some("v1"), None)]);
    assert_eq!(of(&["rebase", "--onto=v2", "v1", "topic"]), [replayed(Some("v2"), Some("v1"), Some("topic"))]);
    assert_eq!(of(&["rebase"]), [replayed(None, None, None)]);
    assert!(matches!(of(&["rebase", "--root"]).as_slice(), [Target::Replayed { root: true, upstream: None, .. }]));
    assert_eq!(of(&["rebase", "--continue"]), [Target::Sequencer]);
    assert_eq!(of(&["cherry-pick", "--skip"]), [Target::Sequencer]);
    assert_eq!(of(&["cherry-pick", "-x", "a..b", "c"]), [Target::Picked { revs: args(&["a..b", "c"]), revert: false }]);
    assert_eq!(of(&["revert", "-m", "1", "c"]), [Target::Picked { revs: args(&["c"]), revert: true }]);
    assert_eq!(of(&["-c", "x=y", "pull", "--rebase"]), [], "pinned by pull.rs");
    assert_eq!(of(&["commit", "-m", "x"]), []);
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
    // libgit2 doesn't know `@{push}`: held.
    assert!(matches!(get(&rev("@{push}")), Err(Unchecked::Unresolved(_))));
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
    let picked = commits(&repo, &[], &Target::Picked { revs: args(&["main..topic"]), revert: false }).unwrap();
    assert_eq!(picked.len(), 3);
    let reverted = commits(&repo, &[], &Target::Picked { revs: args(&["topic"]), revert: true }).unwrap();
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
    let got = super::super::compare::check_targets(&repo, &follows, Some(&saved), &[target]);
    assert!(matches!(got, Err(super::super::compare::Refused::Held(_))), "{got:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_mailbox_that_writes_where_hooks_run_from_is_held() {
    let dir = committed("incoming-am");
    let repo = Repository::open(&dir).unwrap();
    let mail = "From 0 Mon Sep 17 00:00:00 2001\nSubject: [PATCH] x\n\n---\n\
        diff --git a/.GitHooks/post-applypatch b/.GitHooks/post-applypatch\nnew file mode 100755\n\
        --- /dev/null\n+++ b/.GitHooks/post-applypatch\n@@ -0,0 +1 @@\n+echo hi\n";
    write(&dir, "hooks.patch", mail);
    write(&dir, "plain.patch", &mail.replace(".GitHooks/post-applypatch", "src/x.txt"));
    let follows = hooks_follow();
    let held = commits(&repo, &follows, &Target::Mailbox(args(&["hooks.patch"])));
    assert_eq!(held, Err(Unchecked::Patched(".GitHooks/post-applypatch".into())), "any spelling the volume folds");
    assert_eq!(commits(&repo, &follows, &Target::Mailbox(args(&["plain.patch"]))), Ok(vec![]));
    assert!(matches!(commits(&repo, &follows, &Target::Mailbox(args(&["missing.patch"]))), Err(Unchecked::Unreadable(_))));
    assert_eq!(commits(&repo, &follows, &Target::Mailbox(Vec::new())), Err(Unchecked::Unknown));
    // `am --continue`: the patches left in rebase-apply.
    let apply = repo.path().join("rebase-apply");
    std::fs::create_dir_all(&apply).unwrap();
    std::fs::write(apply.join("next"), "2\n").unwrap();
    std::fs::write(apply.join("last"), "2\n").unwrap();
    std::fs::write(apply.join("0001"), mail).unwrap();
    std::fs::write(apply.join("0002"), mail.replace(".GitHooks/post-applypatch", "src/x.txt")).unwrap();
    assert_eq!(commits(&repo, &follows, &Target::Sequencer), Ok(vec![]), "the one applied already doesn't count");
    std::fs::write(apply.join("next"), "1\n").unwrap();
    assert!(matches!(commits(&repo, &follows, &Target::Sequencer), Err(Unchecked::Patched(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_todo_step_that_doesn_t_resolve_is_unresolved() {
    let dir = committed("incoming-todo");
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let todo = repo.path().join("rebase-merge");
    std::fs::create_dir_all(&todo).unwrap();
    std::fs::write(todo.join("onto"), format!("{head}\n")).unwrap();
    let write_todo = |text: &str| std::fs::write(todo.join("git-rebase-todo"), text).unwrap();
    write_todo(&format!("pick {head} x\nlabel l\nreset l\nexec make\nmerge -C {head} l # m\n"));
    assert_eq!(commits(&repo, &[], &Target::Sequencer), Ok(vec![head, head, head]));
    write_todo("pick deadbeefdeadbeef x\n");
    assert_eq!(commits(&repo, &[], &Target::Sequencer), Err(Unchecked::Unresolved("deadbeefdeadbeef".into())));
    write_todo("frobnicate x\n");
    assert!(matches!(commits(&repo, &[], &Target::Sequencer), Err(Unchecked::Unresolved(_))), "a step Glowfly doesn't know");
    // A search by message: libgit2 and git can find different commits.
    assert_eq!(commits(&repo, &[], &rev(":/first")), Err(Unchecked::Unresolved(":/first".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_of_fetch_head_takes_every_commit_marked_for_merging() {
    let dir = committed("incoming-fetch-head");
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let other = repo.blob(b"x").unwrap();
    let a = repo
        .commit(
            None,
            &repo.signature().unwrap(),
            &repo.signature().unwrap(),
            "a",
            &repo.head().unwrap().peel_to_tree().unwrap(),
            &[&repo.find_commit(head).unwrap()],
        )
        .unwrap();
    std::fs::write(
        repo.path().join("FETCH_HEAD"),
        format!("{head}\t\tbranch 'main' of x\n{a}\t\tbranch 'b' of x\n{other}\tnot-for-merge\tbranch 'c' of x\n"),
    )
    .unwrap();
    assert_eq!(commits(&repo, &[], &Target::Merged(args(&["FETCH_HEAD"]))), Ok(vec![head, a]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_checkout_of_paths_compares_the_tree_only_when_they_reach_the_hooks() {
    let dir = committed("incoming-paths");
    let repo = Repository::open(&dir).unwrap();
    let follows = [Follow { path: ".githooks".into(), tracked: None, hooks: true }];
    let paths = |p: &[&str]| Target::Paths { tree: "HEAD".into(), paths: args(p), maybe_paths: false };
    let tree = repo.head().unwrap().peel_to_tree().unwrap().id();
    assert_eq!(commits(&repo, &follows, &paths(&["a.txt"])), Ok(vec![]));
    assert_eq!(commits(&repo, &follows, &paths(&[".githooks/post-checkout"])), Ok(vec![tree]));
    assert_eq!(commits(&repo, &follows, &paths(&["."])), Ok(vec![tree]), "the whole tree");
    assert_eq!(commits(&repo, &follows, &paths(&["*.sh"])), Ok(vec![tree]), "a wildcard may reach them");
    let missing = Target::Paths { tree: "nope".into(), paths: args(&[".githooks"]), maybe_paths: false };
    assert!(commits(&repo, &follows, &missing).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
