use super::*;
use crate::repo::{self, BlameOptions, FileAtOptions, LoadDiffOptions};
use crate::testutil::{git, partial_clone, scratch_repo, set_config, write};
use std::path::PathBuf;

fn rev(dir: &Path, spec: &str) -> Oid {
    let repo = Repository::open(dir).unwrap();
    repo.revparse_single(spec).unwrap().peel_to_commit().unwrap().id()
}

fn blob(dir: &Path, spec: &str) -> Oid {
    let repo = Repository::open(dir).unwrap();
    repo.revparse_single(spec).unwrap().id()
}

fn missing_of(e: Error) -> MissingContent {
    match e {
        Error::MissingContent(m) => m,
        other => panic!("expected MissingContent, got {other:?}"),
    }
}

/// Downloads `m`'s objects as the app does.
fn download(dir: &Path, m: &MissingContent) {
    for cmd in fetch_cmds(&m.remote, &m.oids) {
        let (ok, out) = gitcli::run(dir, &cmd);
        assert!(ok, "{out}");
    }
}

fn cleanup(dirs: &[&PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn the_promisor_remote_is_named() {
    let (src, clone) = partial_clone("partial-promisor");
    assert_eq!(promisor_remote(&clone).as_deref(), Some("origin"));
    assert!(looks_partial(&Repository::open(&clone).unwrap()));
    assert_eq!(promisor_remote(&src), None, "a full repository");
    assert!(!looks_partial(&Repository::open(&src).unwrap()));

    git(&clone, &["remote", "rename", "origin", "my.remote"]);
    assert_eq!(promisor_remote(&clone).as_deref(), Some("my.remote"), "a dotted name");

    set_config(&clone, &[("core.repositoryformatversion", "1"), ("extensions.partialClone", "other")]);
    assert_eq!(promisor_remote(&clone).as_deref(), Some("other"), "the extension names it first");
    cleanup(&[&src, &clone]);
}

#[test]
fn an_older_partial_clone_with_the_extension_opens() {
    let (src, clone) = partial_clone("partial-extension");
    set_config(&clone, &[("core.repositoryformatversion", "1"), ("extensions.partialclone", "origin")]);
    allow_extension();
    Repository::open(&clone).expect("libgit2 opens it");
    repo::load(&clone, &Default::default()).expect("the repository loads");
    cleanup(&[&src, &clone]);
}

#[test]
fn a_diff_reports_its_missing_blob_and_reads_after_the_download() {
    let (src, clone) = partial_clone("partial-diff");
    let target = DiffTarget::Commit(rev(&clone, "HEAD"), "a.txt".into());
    let opts = LoadDiffOptions::default();
    let m = missing_of(repo::load_diff_for_view(&clone, &target, opts).unwrap_err());
    assert_eq!(m.oids, [blob(&src, "HEAD~1:a.txt")]);
    assert_eq!((m.path.as_str(), m.remote.as_str()), ("a.txt", "origin"));
    download(&clone, &m);
    let v = repo::load_diff_for_view(&clone, &target, opts).expect("reads once downloaded");
    let lines: Vec<&str> = v.diff.hunks.iter().flat_map(|h| h.lines.iter().map(|l| l.text.as_str())).collect();
    assert_eq!(lines, ["one", "two"]);
    cleanup(&[&src, &clone]);
}

#[test]
fn the_file_view_and_blame_report_and_then_read() {
    let (src, clone) = partial_clone("partial-file");
    let older = rev(&clone, "HEAD~1");
    let m = missing_of(repo::file_at(&clone, older, "a.txt").unwrap_err());
    assert_eq!(m.oids, [blob(&src, "HEAD~1:a.txt")]);
    assert!(matches!(repo::file_bytes_at(&clone, older, "a.txt", FileAtOptions::default()), Err(Error::MissingContent(_))));

    let b = missing_of(repo::blame_file(&clone, "a.txt", None, &BlameOptions::default()).unwrap_err());
    assert_eq!(b.oids, [blob(&src, "HEAD~1:a.txt")], "the older version from the history, HEAD's is checked out");
    download(&clone, &m);
    assert_eq!(repo::file_at(&clone, older, "a.txt").unwrap().lines, ["one"]);
    assert!(repo::file_bytes_at(&clone, older, "a.txt", FileAtOptions::default()).is_ok());
    let blame = repo::blame_file(&clone, "a.txt", None, &BlameOptions::default()).expect("blames once downloaded");
    assert_eq!(blame.lines.len(), 2);
    cleanup(&[&src, &clone]);
}

#[test]
fn complete_repositories_pass_and_a_lost_blob_keeps_its_error() {
    let (src, clone) = partial_clone("partial-complete");
    let r = Repository::open(&src).unwrap();
    require(&r, &src, "a.txt", [blob(&src, "HEAD~1:a.txt"), Oid::zero()]).expect("everything is there");

    let dir = scratch_repo("partial-lost");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "two\n");
    git(&dir, &["commit", "-q", "-am", "second"]);
    let lost = blob(&dir, "HEAD~1:a.txt").to_string();
    std::fs::remove_file(dir.join(".git/objects").join(&lost[..2]).join(&lost[2..])).unwrap();
    let target = DiffTarget::Commit(rev(&dir, "HEAD"), "a.txt".into());
    let e = repo::load_diff_for_view(&dir, &target, LoadDiffOptions::default()).unwrap_err();
    assert!(matches!(e, Error::Git(_)), "{e:?}");
    cleanup(&[&src, &clone, &dir]);
}

#[test]
fn only_the_object_store_sides_of_a_delta_are_wanted() {
    let dir = scratch_repo("partial-delta-sides");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "two\n");
    let repo = Repository::open(&dir).unwrap();
    let head = rev(&dir, "HEAD");
    let indexed = blob(&dir, "HEAD:a.txt");
    let diff = repo.diff_index_to_workdir(None, None).unwrap();
    let d = diff.deltas().next().unwrap();
    assert_eq!(delta_blobs(&DiffTarget::Unstaged("a.txt".into()), &d), [indexed], "the index side only");
    let tree = repo.find_commit(head).unwrap().tree().unwrap();
    let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), None).unwrap();
    let d = diff.deltas().next().unwrap();
    assert_eq!(delta_blobs(&DiffTarget::ToWorkdir(head, "a.txt".into()), &d), [indexed], "the commit side only");
    let _ = std::fs::remove_dir_all(&dir);

    let (parent, _sub) = crate::testutil::with_submodule("partial-delta-gitlink");
    let repo = Repository::open(&parent).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let parent_tree = head.parent(0).unwrap().tree().unwrap();
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&head.tree().unwrap()), None).unwrap();
    let link = diff.deltas().find(|d| d.new_file().path() == Some(Path::new("libs/lib"))).unwrap();
    assert!(delta_blobs(&DiffTarget::Commit(head.id(), "libs/lib".into()), &link).is_empty(), "a gitlink is never fetched");
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn downloads_go_in_batches_with_the_noop_negotiator() {
    let ids: Vec<Oid> = (0..450u32).map(|i| Oid::hash_object(git2::ObjectType::Blob, &i.to_le_bytes()).unwrap()).collect();
    let cmds = fetch_cmds("origin", &ids);
    let head = gitcli::args(&[
        "-c",
        "fetch.negotiationAlgorithm=noop",
        "fetch",
        "origin",
        "--no-tags",
        "--no-write-fetch-head",
        "--recurse-submodules=no",
        "--filter=blob:none",
    ]);
    assert_eq!(cmds.iter().map(|c| c.len() - head.len()).collect::<Vec<_>>(), [200, 200, 50]);
    for c in &cmds {
        assert_eq!(c[..head.len()], head[..]);
        assert_eq!(gitcli::subcommand(c), Some("fetch"));
    }
    assert_eq!(cmds[2].last(), Some(&ids[449].to_string()));
    assert_eq!(gitcli::subcommand(&gitcli::args(&["status", "-s"])), Some("status"));
    assert_eq!(gitcli::subcommand(&gitcli::args(&["-C", "x", "--no-pager", "--git-dir=y", "log"])), Some("log"));
    assert_eq!(gitcli::subcommand(&gitcli::args(&["-c", "a=b"])), None);
}

#[test]
fn raw_records_give_both_sides() {
    let a = "1".repeat(40);
    let b = "2".repeat(40);
    let z = "0".repeat(40);
    let text = format!("\n:100644 100644 {a} {b} M\0a.txt\0\n:000000 100644 {z} {a} A\0a.txt\0");
    assert_eq!(parse_raw(text.as_bytes()), [Oid::from_str(&a).unwrap(), Oid::from_str(&b).unwrap()]);
}
