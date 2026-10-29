//! What patches, todo steps, FETCH_HEAD and checkouts of paths bring in, read through git.

use super::*;
use crate::gitcli::args;
use crate::testutil::*;

fn committed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn hooks_follow() -> [Follow; 1] {
    [Follow { path: ".githooks".into(), tracked: None, hooks: true }]
}

fn patches(file: &str, apply: &[&str], mailbox: bool) -> Target {
    Target::Patches { files: args(&[file]), apply: args(apply), mailbox }
}

const MAIL: &str = "From 0 Mon Sep 17 00:00:00 2001\nFrom: a <a@b>\nSubject: [PATCH] x\n\n---\n\
    diff --git a/.GitHooks/post-applypatch b/.GitHooks/post-applypatch\nnew file mode 100755\n\
    --- /dev/null\n+++ b/.GitHooks/post-applypatch\n@@ -0,0 +1 @@\n+echo hi\n";

fn patched(got: Result<Vec<git2::Oid>, Unchecked>) -> Option<String> {
    match got {
        Err(Unchecked::Patched { path, digest }) => {
            assert!(!digest.is_empty());
            Some(path)
        }
        _ => None,
    }
}

#[test]
fn a_mailbox_that_writes_where_hooks_run_from_is_held_as_git_decodes_it() {
    let dir = committed("incoming-am");
    let repo = Repository::open(&dir).unwrap();
    write(&dir, "hooks.patch", MAIL);
    write(&dir, "plain.patch", &MAIL.replace(".GitHooks/post-applypatch", "src/x.txt"));
    let follows = hooks_follow();
    let held = commits(&repo, &follows, &patches("hooks.patch", &[], true));
    assert_eq!(patched(held), Some(".GitHooks/post-applypatch".into()), "any spelling the volume folds");
    assert_eq!(commits(&repo, &follows, &patches("plain.patch", &[], true)), Ok(vec![]));
    assert!(matches!(commits(&repo, &follows, &patches("missing.patch", &[], true)), Err(Unchecked::Unreadable(_))));
    // `--directory` moves a plain patch into the hooks folder.
    let moved = MAIL.replace(".GitHooks/post-applypatch", "post-applypatch");
    write(&dir, "moved.patch", &moved);
    assert_eq!(commits(&repo, &follows, &patches("moved.patch", &[], true)), Ok(vec![]));
    assert!(patched(commits(&repo, &follows, &patches("moved.patch", &["--directory=.githooks"], true))).is_some());
    // A mail whose body is base64: only decoding it as git am does shows the path.
    let body = MAIL.split_once("\n\n").unwrap().1;
    let encoded = base64_lines(body.as_bytes());
    let mime = format!(
        "From 0 Mon Sep 17 00:00:00 2001\nFrom: a <a@b>\nSubject: [PATCH] x\nMIME-Version: 1.0\n\
         Content-Type: text/plain; charset=UTF-8\nContent-Transfer-Encoding: base64\n\n{encoded}"
    );
    write(&dir, "b64.patch", &mime);
    assert!(patched(commits(&repo, &follows, &patches("b64.patch", &[], true))).is_some(), "decoded as git am decodes it");
    // A plain patch through git apply.
    write(&dir, "diff.patch", body.split_once("---\n").unwrap().1);
    assert!(patched(commits(&repo, &follows, &patches("diff.patch", &[], false))).is_some());
    // Another patch format isn't read.
    write(&dir, "hg.patch", &format!("# HG changeset patch\n{body}"));
    assert!(matches!(commits(&repo, &follows, &patches("hg.patch", &[], true)), Err(Unchecked::Unmodeled(_))));
    // `am --continue`: the patches left in rebase-apply.
    let apply = repo.path().join("rebase-apply");
    std::fs::create_dir_all(&apply).unwrap();
    std::fs::write(apply.join("applying"), "").unwrap();
    std::fs::write(apply.join("next"), "2\n").unwrap();
    std::fs::write(apply.join("last"), "2\n").unwrap();
    std::fs::write(apply.join("0001"), MAIL).unwrap();
    std::fs::write(apply.join("0002"), MAIL.replace(".GitHooks/post-applypatch", "src/x.txt")).unwrap();
    assert_eq!(commits(&repo, &follows, &Target::Applying), Ok(vec![]), "the one applied already doesn't count");
    std::fs::write(apply.join("next"), "1\n").unwrap();
    assert!(patched(commits(&repo, &follows, &Target::Applying)).is_some());
    git(&dir, &["config", "am.threeWay", "true"]);
    assert!(matches!(commits(&repo, &follows, &patches("plain.patch", &[], true)), Err(Unchecked::Unmodeled(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `bytes` in base64, in lines of 76.
fn base64_lines(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out.as_bytes().chunks(76).map(|l| format!("{}\n", String::from_utf8_lossy(l))).collect()
}

#[test]
fn a_todo_step_resolves_as_git_resolves_it_or_is_held() {
    let dir = committed("incoming-todo");
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let todo = repo.path().join("rebase-merge");
    std::fs::create_dir_all(&todo).unwrap();
    std::fs::write(todo.join("onto"), format!("{head}\n")).unwrap();
    let write_todo = |text: &[u8]| std::fs::write(todo.join("git-rebase-todo"), text).unwrap();
    let seq = || commits(&repo, &[], &Target::Sequencer { skip: false });
    // A label set in this todo is what the steps before it leave; `reset done` is one an
    // earlier step left (refs/rewritten/done).
    git(&dir, &["update-ref", "refs/rewritten/done", &head.to_string()]);
    write_todo(format!("pick {head} x\nlabel l\nreset l\nreset done\nexec make\nmerge -C {head} l # m\n").as_bytes());
    assert_eq!(seq(), Ok(vec![head, head, head, head]));
    write_todo(b"pick deadbeefdeadbeef x\n");
    assert_eq!(seq(), Err(Unchecked::Unresolved("deadbeefdeadbeef".into())));
    write_todo(b"frobnicate x\n");
    assert!(matches!(seq(), Err(Unchecked::Unresolved(_))), "a step Glowfly doesn't know");
    write_todo(b"pick \xff\xfe x\n");
    assert!(matches!(seq(), Err(Unchecked::Unreadable(_))), "a todo that isn't UTF-8 is held, never skipped");
    // A search by message resolves as git resolves it.
    assert_eq!(commits(&repo, &[], &Target::Rev(":/first".into())), Ok(vec![head]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_of_fetch_head_takes_every_commit_marked_for_merging_peeled() {
    let dir = committed("incoming-fetch-head");
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    git(&dir, &["tag", "-a", "-m", "t", "v1"]);
    let tag = repo.revparse_single("v1").unwrap().id();
    let other = repo.blob(b"x").unwrap();
    std::fs::write(
        repo.path().join("FETCH_HEAD"),
        format!("{head}\t\tbranch 'main' of x\n{tag}\t\ttag 'v1' of x\n{other}\tnot-for-merge\tbranch 'c' of x\n"),
    )
    .unwrap();
    let got = commits(&repo, &[], &Target::Merged { revs: args(&["FETCH_HEAD"]), how: How::default() });
    assert_eq!(got, Ok(vec![head, head]), "a tag is merged as its commit");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_checkout_of_paths_compares_the_tree_or_index_only_when_they_reach_the_hooks() {
    let dir = committed("incoming-paths");
    let repo = Repository::open(&dir).unwrap();
    let follows = hooks_follow();
    let paths = |p: &[&str]| Target::Paths { tree: "HEAD".into(), paths: args(p), maybe_paths: false };
    let tree = repo.head().unwrap().peel_to_tree().unwrap().id();
    assert_eq!(commits(&repo, &follows, &paths(&["a.txt"])), Ok(vec![]));
    assert_eq!(commits(&repo, &follows, &paths(&[".githooks/post-checkout"])), Ok(vec![tree]));
    assert_eq!(commits(&repo, &follows, &paths(&["."])), Ok(vec![tree]), "the whole tree");
    assert_eq!(commits(&repo, &follows, &paths(&["*.sh"])), Ok(vec![tree]), "a wildcard may reach them");
    let missing = Target::Paths { tree: "nope".into(), paths: args(&[".githooks"]), maybe_paths: false };
    assert!(commits(&repo, &follows, &missing).is_err());
    // From the index: what is staged there, which can differ from the files.
    std::fs::create_dir_all(dir.join(".githooks")).unwrap();
    write(&dir, ".githooks/post-checkout", "echo staged\n");
    git(&dir, &["add", "."]);
    write(&dir, ".githooks/post-checkout", "echo edited\n");
    let staged = commits(&repo, &follows, &Target::Index(args(&[".githooks"]))).unwrap();
    assert!(follow::versions(&repo, &follows, staged[0]).unwrap()[0].tracked.is_some(), "the staged hook, not HEAD's none");
    assert_eq!(commits(&repo, &follows, &Target::Index(args(&["a.txt"]))), Ok(vec![]));
    let _ = std::fs::remove_dir_all(&dir);
}
