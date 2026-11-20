//! `upstream_config`: the branch settings of one config read, and the upstream names they give
//! as libgit2's `git_branch_upstream_name` gives them.

use super::*;
use crate::testutil::*;
use std::path::Path;

fn open(dir: &Path) -> Repository {
    Repository::open(dir).unwrap()
}

/// Appends `text` to the repository's own config file.
fn append_config(dir: &Path, text: &str) {
    let path = dir.join(".git/config");
    let old = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, format!("{old}\n{text}\n")).unwrap();
}

#[test]
fn upstream_names_match_libgit2() {
    let dir = scratch_repo("upstream-config-oracle");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "c"]);
    let url = "https://example.invalid/r.git";
    for r in ["origin", "mirror", "exact", "nodst"] {
        git(&dir, &["remote", "add", r, url]);
    }
    git(&dir, &["config", "--replace-all", "remote.mirror.fetch", "+refs/heads/*:refs/remotes/mirror/*"]);
    git(&dir, &["config", "--replace-all", "remote.exact.fetch", "refs/heads/main:refs/remotes/exact/only-main"]);
    git(&dir, &["config", "--add", "remote.exact.fetch", "refs/heads/*:refs/remotes/exact/*"]);
    git(&dir, &["config", "--replace-all", "remote.nodst.fetch", "refs/heads/main"]);
    git(&dir, &["config", "--add", "remote.nodst.fetch", "+refs/heads/*:refs/remotes/nodst/*"]);
    // A remote with a fetch spec and no URL isn't one.
    git(&dir, &["config", "remote.nourl.fetch", "+refs/heads/*:refs/remotes/nourl/*"]);
    let cases: [(&str, &str, &str); 11] = [
        ("dot_full", ".", "refs/heads/main"),
        ("dot_short", ".", "main"),
        ("origin_main", "origin", "refs/heads/main"),
        ("mirror_main", "mirror", "refs/heads/main"),
        ("exact_main", "exact", "refs/heads/main"),
        ("nodst_main", "nodst", "refs/heads/main"),
        ("nourl_main", "nourl", "refs/heads/main"),
        ("origin_short", "origin", "main"),
        ("exact_short", "exact", "main"),
        ("origin_nested", "origin", "refs/heads/a/b/c"),
        ("empty_merge", "origin", ""),
    ];
    for (branch, remote, merge) in cases {
        git(&dir, &["config", &format!("branch.{branch}.remote"), remote]);
        git(&dir, &["config", &format!("branch.{branch}.merge"), merge]);
    }
    append_config(
        &dir,
        "[branch \"bare_merge\"]\n\tremote = origin\n\tmerge\n[branch \"bare_remote\"]\n\tremote\n\tmerge = refs/heads/main",
    );
    let repo = open(&dir);
    let cfg = read(&repo);
    let mut memo = SpecsMemo::new(&repo);
    let mut branches: Vec<&str> = cases.iter().map(|c| c.0).collect();
    branches.extend(["bare_merge", "bare_remote", "unset"]);
    for b in &branches {
        // libgit2 maps a source with no destination to an empty name, which names no ref.
        let oracle = repo.branch_upstream_name(&format!("refs/heads/{b}")).ok().and_then(|x| x.as_str().map(str::to_owned));
        let oracle = oracle.filter(|n| !n.is_empty());
        assert_eq!(cfg.upstream_name(b, &mut memo), oracle, "upstream of {b}");
    }
    let named = |b: &str| cfg.upstream_name(b, &mut SpecsMemo::new(&repo));
    assert_eq!(named("dot_full").as_deref(), Some("refs/heads/main"));
    assert_eq!(named("dot_short").as_deref(), Some("main"), "a '.' remote's merge as written");
    assert_eq!(named("origin_main").as_deref(), Some("refs/remotes/origin/main"));
    assert_eq!(named("mirror_main").as_deref(), Some("refs/remotes/mirror/main"));
    assert_eq!(named("exact_main").as_deref(), Some("refs/remotes/exact/only-main"), "the first spec that matches wins");
    assert_eq!(named("nodst_main"), None, "the first match has no destination");
    assert_eq!(named("nourl_main"), None);
    assert_eq!(named("origin_short"), None, "a short name isn't expanded");
    assert_eq!(named("origin_nested").as_deref(), Some("refs/remotes/origin/a/b/c"), "a * matches a /");
    assert_eq!(named("empty_merge"), None);
    assert_eq!(named("bare_merge"), None);
    assert_eq!(cfg.tracking("origin_main").map(|t| (t.remote, t.merge)), Some(("origin".to_owned(), "main".to_owned())));
    assert!(cfg.tracking("empty_merge").is_none() && cfg.tracking("bare_remote").is_none() && cfg.tracking("unset").is_none());
}

#[test]
fn the_last_value_wins_and_a_later_empty_or_bare_one_clears_it() {
    let dir = scratch_repo("upstream-config-last");
    write(&dir, ".git/included.cfg", "[branch \"x\"]\n\tremote = included\n\tmerge = refs/heads/inc\n");
    set_config(&dir, &[("branch.x.remote", "first"), ("branch.x.merge", "refs/heads/first"), ("include.path", "included.cfg")]);
    let tracking = |dir: &Path, b: &str| read(&open(dir)).tracking(b).map(|t| format!("{} {}", t.remote, t.merge));
    assert_eq!(tracking(&dir, "x").as_deref(), Some("included inc"), "the included value comes last");
    append_config(&dir, "[branch \"x\"]\n\tmerge =");
    assert_eq!(tracking(&dir, "x"), None, "a later empty value");
    append_config(&dir, "[branch \"x\"]\n\tmerge = refs/heads/again\n[branch \"x\"]\n\tremote");
    assert_eq!(tracking(&dir, "x"), None, "a later bare key");
    append_config(&dir, "[branch \"x\"]\n\tremote = back");
    assert_eq!(tracking(&dir, "x").as_deref(), Some("back again"));
}

#[test]
fn branch_names_keep_their_case_and_dots() {
    let dir = scratch_repo("upstream-config-names");
    set_config(
        &dir,
        &[
            ("branch.feat.x.remote", "origin"),
            ("branch.feat.x.merge", "refs/heads/feat.x"),
            ("branch.Topic.remote", "origin"),
            ("branch.Topic.merge", "refs/heads/Topic"),
        ],
    );
    let cfg = read(&open(&dir));
    assert_eq!(cfg.tracking("feat.x").map(|t| t.merge).as_deref(), Some("feat.x"));
    assert_eq!(cfg.tracking("Topic").map(|t| t.merge).as_deref(), Some("Topic"));
    assert!(cfg.tracking("topic").is_none() && cfg.tracking("feat").is_none());
}

#[test]
fn the_push_keys_come_from_the_same_read() {
    let dir = scratch_repo("upstream-config-push");
    write(&dir, ".git/push.cfg", "[branch \"feat\"]\n\tpushRemote = fork\n");
    set_config(
        &dir,
        &[
            ("remote.pushDefault", "mine"),
            ("push.default", "current"),
            ("branch.feat.remote", "origin"),
            ("branch.feat.merge", "refs/heads/feat"),
            ("include.path", "push.cfg"),
        ],
    );
    let repo = open(&dir);
    let cfg = read(&repo);
    assert_eq!(cfg.push, crate::repo::push_config::push_remotes(&repo));
    assert_eq!(cfg.push.of_branch("feat"), Some("fork"));
    assert_eq!(cfg.push.default.as_deref(), Some("mine"));
    assert_eq!(cfg.tracking("feat").map(|t| t.remote).as_deref(), Some("origin"));
}
