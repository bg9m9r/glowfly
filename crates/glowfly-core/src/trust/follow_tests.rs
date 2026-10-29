//! Hooks that follow the working tree: found where `core.hooksPath` or a hook's link points
//! into it, part of the print with the version HEAD tracks, and an incoming commit's version
//! read from its objects, so a pull or checkout that brings other hooks is noticed first.

use super::super::incoming::{allows, commits, targets};
use super::super::pull::{Pull, fetched};
use super::super::tests::hook;
use super::*;
use crate::settings::SavedTrust;
use crate::testutil::*;
use git2::Repository;

fn scan(dir: &Path) -> TrustScan {
    super::super::scan_now(&Repository::open(dir).unwrap())
}

/// A repository whose `core.hooksPath` is the tracked folder `.githooks`, with a post-merge
/// hook committed there.
fn tracked_hooks(name: &str) -> (PathBuf, PathBuf) {
    let (work, bare) = with_origin(name);
    hook(&work.join(".githooks"), "post-merge");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "hooks"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    (work, bare)
}

fn trusted(dir: &Path) -> SavedTrust {
    SavedTrust::trusted(Some(scan(dir).print()))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_hooks_folder_in_the_working_tree_follows_the_branch() {
    let (work, bare) = tracked_hooks("follow-tracked");
    let s = scan(&work);
    assert_eq!(s.hooks, ["post-merge"]);
    let repo = Repository::open(&work).unwrap();
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    let id = tree.get_name(".githooks").unwrap().id();
    assert_eq!(s.follows, [Follow { path: ".githooks".into(), tracked: Some((".githooks".into(), id)), hooks: true }]);
    // Committing another version of the folder is another print: it asks again.
    let before = s.print();
    std::fs::write(work.join(".githooks/post-merge"), "#!/bin/sh\necho two\n").unwrap();
    git(&work, &["commit", "-q", "-am", "edit"]);
    let after = scan(&work);
    assert!(!before.covers(&after.print()));
    assert_eq!(after.gained_since(&before).follows.len(), 1);
    cleanup(&[&work, &bare]);
}

#[test]
fn hooks_in_the_git_dir_follow_nothing() {
    let dir = scratch_repo("follow-git-dir");
    hook(&dir.join(".git/hooks"), "post-checkout");
    assert!(scan(&dir).follows.is_empty());
    // Pointing core.hooksPath at the git dir's own folder is still the git dir.
    git(&dir, &["config", "core.hooksPath", ".git/hooks"]);
    assert!(scan(&dir).follows.is_empty());
    cleanup(&[&dir]);
}

#[test]
fn an_untracked_hooks_folder_is_covered_by_the_tracked_folder_around_it() {
    // husky: `.husky/_` is made by `husky install` and ignored; the scripts its hooks run
    // (`.husky/pre-commit`) are tracked beside it.
    let dir = scratch_repo("follow-husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    write(&dir, ".husky/pre-commit", "npm test\n");
    write(&dir, ".gitignore", ".husky/_\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "husky"]);
    hook(&dir.join(".husky/_"), "pre-commit");
    git(&dir, &["config", "core.hooksPath", ".husky/_"]);
    let s = scan(&dir);
    let f = &s.follows[0];
    assert_eq!((f.path.as_str(), f.tracked.as_ref().map(|t| t.0.as_str())), (".husky/_", Some(".husky")));
    // A folder nothing above tracks still follows, with no version: a branch that adds it is new.
    git(&dir, &["config", "core.hooksPath", "local-hooks"]);
    assert_eq!(scan(&dir).follows, [Follow { path: "local-hooks".into(), tracked: None, hooks: true }]);
    cleanup(&[&dir]);
}

#[test]
fn a_tracked_hooks_folder_changed_by_a_pull_is_not_covered() {
    let (work, bare) = tracked_hooks("follow-pull");
    let saved = trusted(&work);
    let pull = Pull::of(&crate::gitcli::args(&["pull", "--ff"])).expect("a pull");
    let incoming = |work: &Path| {
        git(work, &pull.fetch.iter().map(String::as_str).collect::<Vec<_>>());
        let repo = Repository::open(work).unwrap();
        let heads = fetched(&repo).expect("FETCH_HEAD");
        assert_eq!(heads.len(), 1, "{heads:?}");
        incoming(&repo, &scan(work).follows, heads[0].oid).unwrap()
    };
    // Something else changed upstream: the hooks the pull brings are the trusted ones.
    remote_commit(&bare, "main", "b.txt", "other\n");
    assert!(allows(Some(&saved), &incoming(&work)), "same hooks");
    // The hook changed upstream: held before git runs it as post-merge.
    remote_commit(&bare, "main", ".githooks/post-merge", "#!/bin/sh\necho changed\n");
    let inc = incoming(&work);
    assert!(!allows(Some(&saved), &inc), "{inc:?}");
    assert_eq!(inc.hooks, ["post-merge"], "its hooks are read from the commit");
    // Trusting it covers it, and the checked-out hook it becomes.
    let both = SavedTrust::trusted(Some(saved.print.clone().unwrap().with(&inc.print())));
    assert!(allows(Some(&both), &inc));
    git(&work, &["-c", "core.hooksPath=.git/hooks", "merge", "-q", "--ff-only", "@{u}"]);
    assert!(both.print.as_ref().unwrap().covers(&scan(&work).print()), "no second question once it lands");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_checkout_of_another_branch_compares_its_hooks() {
    let (work, bare) = tracked_hooks("follow-checkout");
    let saved = trusted(&work);
    git(&work, &["checkout", "-q", "-b", "side"]);
    std::fs::write(work.join(".githooks/post-checkout"), "#!/bin/sh\necho hi\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["-c", "core.hooksPath=.git/hooks", "commit", "-q", "-m", "add"]);
    git(&work, &["-c", "core.hooksPath=.git/hooks", "checkout", "-q", "main"]);
    let repo = Repository::open(&work).unwrap();
    let follows = scan(&work).follows;
    let check = |args: &[&str]| {
        targets(&crate::gitcli::args(args))
            .iter()
            .flat_map(|t| commits(&repo, &follows, t).unwrap())
            .all(|oid| allows(Some(&saved), &incoming(&repo, &follows, oid).unwrap()))
    };
    assert!(!check(&["checkout", "side", "--"]), "side adds a hook");
    assert!(!check(&["merge", "--no-ff", "side"]));
    assert!(!check(&["cherry-pick", "side"]), "a pick replays it");
    assert!(!check(&["rebase", "main", "side"]), "the branch a rebase checks out first");
    assert!(!check(&["checkout", "side", "--", ".githooks"]), "the folder from another tree");
    assert!(check(&["checkout", "side", "--", "a.txt"]), "a file away from the hooks");
    assert!(check(&["checkout", "main", "--"]), "the branch already trusted");
    assert!(check(&["checkout", "-b", "new"]), "a new branch at HEAD");
    cleanup(&[&work, &bare]);
}

/// A commit on HEAD whose tree is HEAD's with `name` (a folder holding `post-merge` with
/// `script`) added beside what is there: libgit2 writes it, so two names one folding file
/// system takes for the same folder can both be in the tree.
fn with_folder(repo: &Repository, name: &str, script: &str) -> Oid {
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let blob = repo.blob(script.as_bytes()).unwrap();
    let mut inner = repo.treebuilder(None).unwrap();
    inner.insert("post-merge", blob, 0o100755).unwrap();
    let inner = inner.write().unwrap();
    let mut top = repo.treebuilder(Some(&head.tree().unwrap())).unwrap();
    top.insert(name, inner, 0o040000).unwrap();
    let tree = repo.find_tree(top.write().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    repo.commit(None, &sig, &sig, "twin", &tree, &[&head]).unwrap()
}

#[test]
fn a_folder_another_case_or_spelling_of_the_hooks_folder_is_compared_too() {
    let (work, bare) = tracked_hooks("follow-twins");
    let saved = trusted(&work);
    let repo = Repository::open(&work).unwrap();
    let follows = scan(&work).follows;
    // `.GITHOOKS` beside `.githooks`: on a folding file system either may be the one on disk.
    for twin in [".GITHOOKS", ".githooks\u{200c}", ".githooks."] {
        let oid = with_folder(&repo, twin, "#!/bin/sh\necho twin\n");
        let v = versions(&repo, &follows, oid).unwrap();
        let tracked = v[0].tracked.as_ref().unwrap();
        assert!(tracked.0.contains('|'), "both are recorded: {tracked:?}");
        assert!(!follow_allows(&saved, &repo, &follows, oid), "{twin}: held");
    }
    cleanup(&[&work, &bare]);
}

fn follow_allows(saved: &SavedTrust, repo: &Repository, follows: &[Follow], oid: Oid) -> bool {
    allows(Some(saved), &incoming(repo, follows, oid).unwrap())
}

#[test]
fn a_hooks_folder_reached_through_a_link_follows_both_places() {
    let dir = scratch_repo("follow-through-link");
    hook(&dir.join("tools"), "post-checkout");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "tools"]);
    // A link (a junction on Windows) inside the tree to the tracked folder.
    let Some(link) = alias_of(&dir.join("tools")) else {
        eprintln!("no link could be made, skipping");
        return cleanup(&[&dir]);
    };
    let name = link.file_name().unwrap().to_string_lossy().into_owned();
    git(&dir, &["config", "core.hooksPath", &name]);
    let paths: Vec<String> = scan(&dir).follows.iter().map(|f| f.path.clone()).collect();
    assert_eq!(paths, [name.clone(), "tools".to_owned()], "as spelled and where it leads");
    // A branch changing the folder it leads to brings other hooks, though the link is the same.
    let saved = trusted(&dir);
    let off = |args: &[&str]| git(&dir, &[&["-c", "core.hooksPath=.git/hooks"], args].concat());
    off(&["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("tools/post-checkout"), "#!/bin/sh\necho changed\n").unwrap();
    off(&["commit", "-q", "-am", "edit"]);
    off(&["checkout", "-q", "main"]);
    let repo = Repository::open(&dir).unwrap();
    let side = repo.revparse_single("side").unwrap().id();
    assert!(!follow_allows(&saved, &repo, &scan(&dir).follows, side));
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    cleanup(&[&dir]);
}

#[test]
fn hooks_a_partial_clone_lacks_are_listed_without_reading_them() {
    let (src, clone) = partial_clone_of("follow-partial", |src| {
        hook(&src.join(".githooks"), "post-merge");
        git(src, &["add", "."]);
        git(src, &["commit", "-q", "-m", "hooks"]);
        std::fs::write(src.join(".githooks/post-merge"), "#!/bin/sh\necho two\n").unwrap();
        git(src, &["commit", "-q", "-am", "two"]);
    });
    git(&clone, &["config", "core.hooksPath", ".githooks"]);
    let repo = Repository::open(&clone).unwrap();
    let follows = scan(&clone).follows;
    // HEAD~1's hook blob was never fetched: its version is still read from the trees.
    let old = repo.revparse_single("HEAD~1").unwrap().id();
    let inc = incoming(&repo, &follows, old).expect("trees are enough");
    assert_eq!(inc.hooks, ["post-merge"]);
    assert!(inc.hook_digests.is_empty(), "the missing blob isn't read");
    cleanup(&[&src, &clone]);
}

#[test]
fn a_tree_that_can_t_be_read_can_t_be_compared() {
    let (work, bare) = tracked_hooks("follow-missing-tree");
    let repo = Repository::open(&work).unwrap();
    let follows = scan(&work).follows;
    let oid = with_folder(&repo, "tools", "x");
    // Drop the loose object of the commit's new top tree.
    let tree = repo.find_commit(oid).unwrap().tree_id().to_string();
    std::fs::remove_file(work.join(".git/objects").join(&tree[..2]).join(&tree[2..])).unwrap();
    let repo = Repository::open(&work).unwrap();
    assert!(versions(&repo, &follows, oid).is_err(), "the caller holds the job");
    cleanup(&[&work, &bare]);
}

#[cfg(unix)]
#[test]
fn a_hook_linked_into_the_working_tree_follows_it() {
    let dir = scratch_repo("follow-link");
    hook(&dir.join("scripts"), "check.sh");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "scripts"]);
    std::os::unix::fs::symlink(dir.join("scripts/check.sh"), dir.join(".git/hooks/pre-commit")).unwrap();
    let f = &scan(&dir).follows;
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!((f[0].path.as_str(), f[0].hooks), ("scripts/check.sh", false));
    cleanup(&[&dir]);
}

#[test]
fn a_hooks_path_only_the_file_system_can_resolve_follows_the_whole_tree() {
    for (n, spelled) in [".GITHO~1", ".githooks::$INDEX_ALLOCATION"].into_iter().enumerate() {
        let (work, bare) = tracked_hooks(&format!("follow-unclear-{n}"));
        git(&work, &["config", "core.hooksPath", spelled]);
        let s = scan(&work);
        assert!(s.follows.iter().any(|f| f.path.is_empty()), "{spelled}: {:?}", s.follows);
        // Any commit that changes anything is another version.
        let head = Repository::open(&work).unwrap().head().unwrap().target().unwrap();
        let saved = trusted(&work);
        write(&work, "b.txt", "b\n");
        git(&work, &["-c", "core.hooksPath=", "add", "."]);
        git(&work, &["-c", "core.hooksPath=", "commit", "-q", "-m", "b"]);
        let repo = Repository::open(&work).unwrap();
        let next = repo.head().unwrap().target().unwrap();
        assert_ne!(head, next);
        let got = super::super::compare::check(&repo, &s.follows, Some(&saved), &[next]);
        assert!(got.is_err(), "{spelled}: {got:?}");
        cleanup(&[&work, &bare]);
    }
}
