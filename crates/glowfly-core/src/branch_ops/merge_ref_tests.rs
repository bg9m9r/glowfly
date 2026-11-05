//! Merging and rebasing by full ref: a tag (or a local branch spelled like a remote one) with the
//! branch's short name is never taken for it, and the merge commit's message is the one git
//! writes for `git merge <short name>`. Each expected message is recorded from git itself on a
//! twin merge without the shadowing refs, so the developer's own config cancels out.

use super::*;
use crate::commit_encoding::MessageEncoding;
use crate::gitcli::run;
use crate::repo::{LoadOptions, load, merge_message};
use crate::testkit::git_out;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn rev(dir: &Path, r: &str) -> String {
    git_out(dir, &["rev-parse", r]).trim().to_owned()
}

/// The full message of HEAD's commit.
fn message(dir: &Path) -> String {
    git_out(dir, &["log", "-1", "--format=%B"]).trim_end().to_owned()
}

fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> String {
    write(dir, file, text);
    git(dir, &["add", "--", file]);
    git(dir, &["commit", "-q", "-m", msg]);
    rev(dir, "HEAD")
}

/// On `main`: base, then branch x with "x work" and main with "main work". Returns base's sha.
/// merge.log is off in the repository's own config, so a global setting can't add a shortlog.
fn fixture(name: &str) -> (PathBuf, String) {
    let dir = scratch_repo(name);
    set_config(&dir, &[("merge.log", "false")]);
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "x"]);
    commit(&dir, "x.txt", "x\n", "x work");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "m.txt", "m\n", "main work");
    (dir, base)
}

fn ok(dir: &Path, cmd: &[String]) {
    let (ok, out) = run(dir, cmd);
    assert!(ok, "{cmd:?}: {out}");
}

/// The message `git merge --no-edit <short>` writes here, with HEAD put back afterwards.
fn twin_message(dir: &Path, short: &str) -> String {
    let tip = rev(dir, "HEAD");
    ok(dir, &args(&["merge", "--no-edit", short]));
    let msg = message(dir);
    git(dir, &["reset", "-q", "--hard", &tip]);
    msg
}

/// What the loaded snapshot says git's title ends with; `head` is the branch checked out now.
fn into(dir: &Path, head: Option<&str>) -> Option<String> {
    let snap = load(dir, &LoadOptions::all(10)).unwrap();
    assert_eq!(snap.head_branch.as_deref(), head);
    snap.merge_into
}

#[test]
fn builders_name_the_full_ref() {
    let x = BranchRef::Local("x".into());
    assert_eq!(merge_cmd(&x, None), ["merge", "-m", "Merge branch 'x'", "--", "refs/heads/x"]);
    assert_eq!(
        merge_cmd(&BranchRef::Remote("origin/x".into()), Some("trunk")),
        ["merge", "-m", "Merge remote-tracking branch 'origin/x' into trunk", "--", "refs/remotes/origin/x"]
    );
    assert_eq!(rebase_cmd(&x), ["rebase", "--", "refs/heads/x"]);
    assert_eq!(rebase_cmd(&BranchRef::Remote("origin/x".into())), ["rebase", "--", "refs/remotes/origin/x"]);
}

#[test]
fn merge_takes_the_branch_not_a_same_named_tag() {
    for annotated in [false, true] {
        let (dir, base) = fixture(if annotated { "mref-tag-annotated" } else { "mref-tag" });
        let tag = |dir: &Path| {
            if annotated {
                git(dir, &["tag", "-a", "x", "-m", "a tag", &base]);
            } else {
                git(dir, &["tag", "x", &base]);
            }
        };
        let theirs = BranchRef::Local("x".into());
        for head in ["main", "trunk"] {
            if head == "trunk" {
                git(&dir, &["checkout", "-q", "-b", "trunk"]);
            }
            let _ = run(&dir, &args(&["tag", "-d", "x"]));
            let expected = twin_message(&dir, "x");
            assert_eq!(expected.contains(" into trunk"), head == "trunk", "{expected}");
            tag(&dir);
            ok(&dir, &merge_cmd(&theirs, into(&dir, Some(head)).as_deref()));
            assert_eq!(rev(&dir, "HEAD^2"), rev(&dir, "refs/heads/x"), "merged the branch, not the tag ({head})");
            assert_eq!(message(&dir), expected, "git's own message ({head}, annotated: {annotated})");
            git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn merge_log_still_appends_the_shortlog() {
    let (dir, base) = fixture("mref-log");
    set_config(&dir, &[("merge.log", "true")]);
    let expected = twin_message(&dir, "x");
    git(&dir, &["tag", "x", &base]);
    ok(&dir, &merge_cmd(&BranchRef::Local("x".into()), into(&dir, Some("main")).as_deref()));
    let got = message(&dir);
    assert_eq!(got.lines().next(), expected.lines().next(), "the title");
    assert!(got.contains("  x work"), "{got}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_takes_the_remote_tracking_branch_not_a_shadow() {
    let (dir, base) = fixture("mref-remote");
    git(&dir, &["checkout", "-q", "-b", "side", &base]);
    let side = commit(&dir, "s.txt", "s\n", "side work");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["update-ref", "refs/remotes/origin/x", &side]);
    let expected = twin_message(&dir, "origin/x");
    assert!(expected.starts_with("Merge remote-tracking branch 'origin/x'"), "{expected}");
    git(&dir, &["tag", "origin/x", &base]);
    git(&dir, &["update-ref", "refs/heads/origin/x", &rev(&dir, "refs/heads/x")]);
    ok(&dir, &merge_cmd(&BranchRef::Remote("origin/x".into()), into(&dir, Some("main")).as_deref()));
    assert_eq!(rev(&dir, "HEAD^2"), side, "the remote-tracking tip");
    assert_eq!(message(&dir), expected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_title_ends_as_git_ends_it() {
    let (dir, _) = fixture("mref-dest");
    let theirs = BranchRef::Local("x".into());
    let check = |head: Option<&str>| {
        let expected = twin_message(&dir, "x");
        assert_eq!(expected.lines().next(), Some(merge_title(&theirs, into(&dir, head).as_deref()).as_str()), "{head:?}");
    };
    check(Some("main"));
    git(&dir, &["checkout", "-q", "-b", "trunk"]);
    check(Some("trunk"));
    git(&dir, &["config", "--add", "merge.suppressDest", "tr*"]);
    check(Some("trunk"));
    git(&dir, &["checkout", "-q", "main"]);
    check(Some("main"));
    git(&dir, &["checkout", "-q", "--detach"]);
    check(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_takes_the_branch_not_a_same_named_tag() {
    let (dir, base) = fixture("mref-rebase");
    git(&dir, &["tag", "x", &base]);
    ok(&dir, &rebase_cmd(&BranchRef::Local("x".into())));
    ok(&dir, &args(&["merge-base", "--is-ancestor", "refs/heads/x", "HEAD"]));
    assert_eq!(rev(&dir, "HEAD~1"), rev(&dir, "refs/heads/x"), "replayed onto the branch tip, not base");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_merge_prefills_git_s_title() {
    let dir = scratch_repo("mref-conflict");
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "x"]);
    commit(&dir, "a.txt", "theirs\n", "x work");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "ours\n", "main work");
    git(&dir, &["tag", "x", &base]);
    let (merged, _) = run(&dir, &merge_cmd(&BranchRef::Local("x".into()), into(&dir, Some("main")).as_deref()));
    assert!(!merged, "the merge stops for the conflict");
    let draft = merge_message(&Repository::open(&dir).unwrap(), &MessageEncoding::UTF8).unwrap();
    assert!(draft.starts_with("Merge branch 'x'"), "{draft}");
    let _ = std::fs::remove_dir_all(&dir);
}
