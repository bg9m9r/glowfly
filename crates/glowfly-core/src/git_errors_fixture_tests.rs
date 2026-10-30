//! `classify` on what real git prints, run through `gitcli` (so with `with_quiet_apply`), with
//! the developer's global config kept out. Only the cause is checked, never hint wording.

use super::{Cause, classify};
use crate::gitcli::{args, run_with_input};
use crate::testutil::{empty_global_config, git, remote_commit, scratch_repo, with_origin, write};
use std::path::Path;

/// Runs `git <a>` in `dir` with an empty global config; it must fail. Returns its output.
fn fails(dir: &Path, a: &[&str]) -> String {
    let env = empty_global_config(dir);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (ok, out) = run_with_input(dir, &args(a), "", &env);
    assert!(!ok, "git {a:?} should fail: {out}");
    out
}

fn commit(dir: &Path, file: &str, text: &str) {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", file]);
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
        let _ = std::fs::remove_file(d.with_file_name(format!("{}-gitconfig", d.file_name().unwrap().to_string_lossy())));
    }
}

fn files(f: &[&str]) -> Vec<String> {
    f.iter().map(|s| s.to_string()).collect()
}

/// main and side diverged: side edits a.txt and b.txt and adds new.txt, main edits a.txt.
fn diverged(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "b.txt", "b\n");
    commit(&dir, "a.txt", "one\n");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    write(&dir, "b.txt", "side\n");
    write(&dir, "new.txt", "new\n");
    commit(&dir, "a.txt", "side\n");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "main\n");
    dir
}

#[test]
fn local_untracked_conflicts_and_refs() {
    let dir = diverged("ge-local");
    // (a) A staged file with trailing spaces: merge refuses, and its state restore's patch
    // warnings stay out of the output.
    write(&dir, "g.txt", "x   \ny \n");
    git(&dir, &["add", "g.txt"]);
    let out = fails(&dir, &["merge", "side"]);
    assert!(!out.contains("<stdin>") && !out.contains("whitespace errors"), "{out}");
    assert_eq!(classify(&out), Some(Cause::LocalChanges { files: files(&["g.txt"]) }), "{out}");
    git(&dir, &["rm", "-q", "--cached", "g.txt"]);
    std::fs::remove_file(dir.join("g.txt")).unwrap();

    // (b) An unstaged edit of a file the target branch changes.
    write(&dir, "b.txt", "mine\n");
    let out = fails(&dir, &["checkout", "side", "--"]);
    assert_eq!(classify(&out), Some(Cause::LocalChanges { files: files(&["b.txt"]) }), "{out}");
    git(&dir, &["checkout", "--", "b.txt"]);

    // (c) An untracked file the merge would add.
    write(&dir, "new.txt", "mine\n");
    let out = fails(&dir, &["merge", "side"]);
    assert_eq!(classify(&out), Some(Cause::UntrackedInTheWay { files: files(&["new.txt"]) }), "{out}");
    std::fs::remove_file(dir.join("new.txt")).unwrap();

    // (d) Both edited a.txt.
    let out = fails(&dir, &["merge", "side"]);
    assert_eq!(classify(&out), Some(Cause::Conflicts { files: files(&["a.txt"]), unsure: vec![] }), "{out}");
    git(&dir, &["merge", "--abort"]);

    // (i) A ref that doesn't exist.
    let out = fails(&dir, &["merge", "nope"]);
    assert_eq!(classify(&out), Some(Cause::UnknownRef { name: "nope".into() }), "{out}");

    // (j) Another git's lock file.
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    let out = fails(&dir, &["checkout", "side", "--"]);
    std::fs::remove_file(&lock).unwrap();
    assert_eq!(classify(&out), Some(Cause::Locked { file: "index.lock".into() }), "{out}");
    cleanup(&[&dir]);
}

#[test]
fn remote_rejections_and_upstreams() {
    let (work, bare) = with_origin("ge-remote");
    // (e) The remote has a commit the branch lacks.
    commit(&work, "local.txt", "local\n");
    remote_commit(&bare, "main", "remote.txt", "remote\n");
    let out = fails(&work, &["push"]);
    assert_eq!(classify(&out), Some(Cause::PushBehind { branch: "main".into() }), "{out}");

    // (f) Fetched, the branch and its upstream have both moved on.
    git(&work, &["fetch", "-q"]);
    let out = fails(&work, &["pull", "--ff-only"]);
    assert_eq!(classify(&out), Some(Cause::Diverged { branch: None }), "{out}");
    // With no pull mode configured, git asks how to reconcile them.
    let out = fails(&work, &["pull"]);
    assert_eq!(classify(&out), Some(Cause::Diverged { branch: None }), "{out}");

    // (g) A new branch with no upstream.
    git(&work, &["checkout", "-q", "-b", "feat"]);
    let out = fails(&work, &["push"]);
    assert_eq!(classify(&out), Some(Cause::NoUpstream { push_branch: Some("feat".into()) }), "{out}");
    let out = fails(&work, &["pull", "--ff"]);
    assert_eq!(classify(&out), Some(Cause::NoUpstream { push_branch: None }), "{out}");

    // (h) Detached HEAD.
    git(&work, &["checkout", "-q", "--detach"]);
    let out = fails(&work, &["pull", "--ff"]);
    assert_eq!(classify(&out), Some(Cause::DetachedHead), "{out}");

    // (k) The upstream was deleted on the remote.
    git(&work, &["checkout", "-q", "-b", "gone"]);
    git(&work, &["push", "-q", "-u", "origin", "gone"]);
    git(&bare, &["branch", "-D", "gone"]);
    let out = fails(&work, &["pull", "--ff"]);
    assert_eq!(classify(&out), Some(Cause::UpstreamGone { branch: "gone".into() }), "{out}");
    cleanup(&[&work, &bare]);
}

#[test]
fn rename_delete_and_directory_conflicts_count_each_unmerged_path() {
    // side renames a.txt and r.txt and turns the file f into a directory; main deletes a.txt,
    // renames r.txt another way and edits f.
    let dir = scratch_repo("ge-renames");
    write(&dir, "f", "k\n");
    write(&dir, "r.txt", "x\ny\nz\n");
    commit(&dir, "a.txt", "one\ntwo\nthree\nfour\n");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    git(&dir, &["mv", "a.txt", "b.txt"]);
    git(&dir, &["mv", "r.txt", "r1.txt"]);
    git(&dir, &["rm", "-q", "f"]);
    std::fs::create_dir(dir.join("f")).unwrap();
    commit(&dir, "f/z", "z\n");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["rm", "-q", "a.txt"]);
    git(&dir, &["mv", "r.txt", "r2.txt"]);
    commit(&dir, "f", "k\nmain\n");
    let out = fails(&dir, &["merge", "side"]);
    // Each conflicted path once, as the status lists it.
    let index = git2::Repository::open(&dir).unwrap().index().unwrap();
    let mut unmerged: Vec<String> = Vec::new();
    for c in index.conflicts().unwrap().flatten() {
        for e in [c.ancestor, c.our, c.their].into_iter().flatten() {
            unmerged.push(String::from_utf8_lossy(&e.path).into_owned());
        }
    }
    unmerged.sort();
    unmerged.dedup();
    let Some(Cause::Conflicts { mut files, unsure }) = classify(&out) else { panic!("{out}") };
    files.sort();
    assert_eq!((files, unsure), (unmerged, vec![]), "{out}");
    cleanup(&[&dir]);
}
