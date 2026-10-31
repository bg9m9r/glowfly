//! A file's diff narrowed to it (and its rename or copy partner) reads the same as the whole
//! diff did: renames both ways, copies, untracked files and folders, submodules, binary and LFS
//! files, paths with glob characters, and list rows the working tree no longer matches; and it
//! reads only those paths.

use super::super::diff::{file_patch, hunk_of};
use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};

const WHOLE: u32 = 1 << 24;

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

fn lines(n: usize, tag: &str) -> String {
    (1..=n).map(|i| format!("{tag} line {i}\n")).collect()
}

/// The file's diff as it was read before narrowing: the whole diff, renames detected, and the
/// whole-file load from the whole diff again.
fn reference(dir: &Path, target: &DiffTarget, opts: LoadDiffOptions) -> (FileDiff, Option<crate::lfs::pointer::LfsChange>) {
    let pooled = Repository::open(dir).unwrap();
    let repo = super::super::attr_source::for_target(&pooled, target);
    let path = target.path();
    let diff = target_diff(&repo, target, opts, None).unwrap();
    let lfs = super::super::lfs_marks::diff_change(&repo, target, &diff);
    let mut out = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    let Some(patch) = file_patch(&diff, path).unwrap() else {
        out.binary = true;
        return (out, lfs);
    };
    out.binary = patch.delta().flags().is_binary();
    out.hunks = (0..patch.num_hunks()).map(|h| hunk_of(&patch, h).unwrap()).collect();
    if opts.whole_file && !out.binary && !out.hunks.is_empty() {
        let diff = target_diff(&repo, target, opts, Some(WHOLE)).unwrap();
        if let Some(p) = file_patch(&diff, path).unwrap()
            && p.num_hunks() == 1
        {
            out.whole = crate::whole_file::WholeFile::new(&out.hunks, hunk_of(&p, 0).unwrap()).map(Box::new);
        }
    }
    (out, lfs)
}

/// How many deltas the diff behind `target`'s view reads, given its row.
fn deltas(dir: &Path, target: &DiffTarget, listed: Option<&FileEntry>) -> usize {
    let repo = Repository::open(dir).unwrap();
    file_diff(&repo, dir, target, LoadDiffOptions::default(), listed).unwrap().deltas().len()
}

/// `target` (with its row in `rows`, if any) loads as it did before, hunks and whole file.
fn same_as_before(dir: &Path, target: &DiffTarget, rows: &[FileEntry]) {
    let listed = rows.iter().find(|f| f.path == target.path());
    for whole_file in [false, true] {
        let opts = LoadDiffOptions { whole_file, ..Default::default() };
        let v = load_listed_diff(dir, target, opts, listed).unwrap();
        let (want, lfs) = reference(dir, target, opts);
        assert_eq!(v.diff, want, "{target:?} (whole file: {whole_file})");
        assert_eq!(v.lfs, lfs, "{target:?}");
    }
}

#[test]
fn a_working_tree_comparison_reads_only_the_file_and_its_partner() {
    let dir = scratch_repo("scope-workdir");
    for d in 0..3 {
        std::fs::create_dir_all(dir.join(format!("d{d}"))).unwrap();
        for f in 0..100 {
            write(&dir, &format!("d{d}/f{f}.txt"), &format!("file {d} {f}\n"));
        }
    }
    write(&dir, "m.txt", &lines(30, "m"));
    write(&dir, "old.txt", &lines(30, "old"));
    write(&dir, "gone.txt", &lines(30, "gone"));
    write(&dir, "moved.txt", &lines(30, "moved"));
    let c1 = commit_all(&dir, "c1");
    write(&dir, "m.txt", &format!("{}more\n", lines(30, "m")));
    git(&dir, &["mv", "old.txt", "new.txt"]);
    write(&dir, "new.txt", &format!("{}edited\n", lines(30, "old")));
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    // Moved in the working tree only: a deletion and an untracked file, never paired.
    std::fs::rename(dir.join("moved.txt"), dir.join("moved-here.txt")).unwrap();
    write(&dir, "top.txt", "untracked\n");
    std::fs::create_dir_all(dir.join("u/deep")).unwrap();
    for f in 0..100 {
        write(&dir, &format!("u/deep/u{f}.txt"), &format!("untracked {f}\n"));
    }

    let forward = compare_workdir(&dir, c1).unwrap().files;
    let new = forward.iter().find(|f| f.path == "new.txt").unwrap();
    assert_eq!((new.status, new.renamed_from.as_deref()), ('R', Some("old.txt")));
    let back = compare_from_workdir(&dir, c1).unwrap().files;
    let old = back.iter().find(|f| f.path == "old.txt").unwrap();
    assert_eq!((old.status, old.renamed_from.as_deref()), ('R', Some("new.txt")), "reversed, under the commit's name");
    for (rows, make) in [(&forward, DiffTarget::ToWorkdir as fn(Oid, String) -> DiffTarget), (&back, DiffTarget::FromWorkdir)] {
        assert!(rows.len() > 100, "every untracked file is listed");
        for f in rows.iter() {
            let target = make(c1, f.path.clone());
            same_as_before(&dir, &target, rows);
            let read = deltas(&dir, &target, Some(f));
            assert!(read <= 2, "{} read {read} deltas", f.path);
        }
    }
    // With no row to go by, a modified file is still narrowed; an added one reads everything.
    assert_eq!(deltas(&dir, &DiffTarget::ToWorkdir(c1, "m.txt".into()), None), 1);
    assert!(deltas(&dir, &DiffTarget::ToWorkdir(c1, "top.txt".into()), None) > 100);
    same_as_before(&dir, &DiffTarget::ToWorkdir(c1, "top.txt".into()), &[]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_row_the_working_tree_no_longer_matches_reads_the_whole_diff() {
    let dir = scratch_repo("scope-stale");
    write(&dir, "a.txt", &lines(30, "a"));
    write(&dir, "b.txt", &lines(30, "b"));
    let c1 = commit_all(&dir, "c1");
    git(&dir, &["mv", "a.txt", "c.txt"]);
    write(&dir, "b.txt", "changed\n");
    let stale = [FileEntry::new("c.txt", 'D'), FileEntry { renamed_from: Some("b.txt".into()), ..FileEntry::new("c.txt", 'R') }];
    for row in &stale {
        let target = DiffTarget::ToWorkdir(c1, "c.txt".into());
        same_as_before(&dir, &target, std::slice::from_ref(row));
        assert_eq!(deltas(&dir, &target, Some(row)), 2, "{row:?}: the whole diff (renames paired)");
    }
    // A row listed as deleted that is modified now still narrows (a modified file pairs with nothing).
    same_as_before(&dir, &DiffTarget::ToWorkdir(c1, "b.txt".into()), &[FileEntry::new("b.txt", 'D')]);
    // The old side of a rename, asked for by name (a file history's commit diff): the whole diff.
    let c2 = commit_all(&dir, "c2");
    same_as_before(&dir, &DiffTarget::Commit(c2, "a.txt".into()), &[]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_and_comparison_diffs_narrow_renames_and_copies() {
    let dir = scratch_repo("scope-commit");
    set_config(&dir, &[("diff.renames", "copies")]);
    write(&dir, "src.txt", &lines(40, "src"));
    write(&dir, "old.txt", &lines(40, "old"));
    std::fs::create_dir_all(dir.join("d")).unwrap();
    write(&dir, "d/x.txt", "x\n");
    let c1 = commit_all(&dir, "c1");
    write(&dir, "src.txt", &format!("{}changed\n", lines(40, "src")));
    write(&dir, "copy.txt", &format!("{}copied\n", lines(40, "src")));
    git(&dir, &["mv", "old.txt", "d/new.txt"]);
    write(&dir, "d/new.txt", &format!("{}moved\n", lines(40, "old")));
    let c2 = commit_all(&dir, "c2");

    let rows = super::super::commit_detail(&dir, c2).unwrap().files;
    let copy = rows.iter().find(|f| f.path == "copy.txt").unwrap();
    assert_eq!((copy.status, copy.renamed_from.as_deref()), ('A', Some("src.txt")), "a copy keeps its source");
    let between = compare(&dir, c1, c2).unwrap().files;
    for f in &rows {
        for (target, rows) in [(DiffTarget::Commit(c2, f.path.clone()), &rows), (DiffTarget::Between(c1, c2, f.path.clone()), &between)] {
            same_as_before(&dir, &target, rows);
            let listed = rows.iter().find(|r| r.path == f.path);
            assert!(deltas(&dir, &target, listed) <= 2, "{target:?}");
        }
    }
    // The copy's patch is against its source, not an addition.
    let v = load_listed_diff(&dir, &DiffTarget::Commit(c2, "copy.txt".into()), LoadDiffOptions::default(), Some(copy)).unwrap();
    assert!(v.diff.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin == '+').count() == 1, "{:?}", v.diff.hunks);
    // The source shows its own change, not the copy's (whose old side it is), with or without its row.
    for listed in [rows.iter().find(|f| f.path == "src.txt"), None] {
        let v = load_listed_diff(&dir, &DiffTarget::Commit(c2, "src.txt".into()), LoadDiffOptions::default(), listed).unwrap();
        assert!(v.diff.hunks.iter().flat_map(|h| &h.lines).any(|l| l.origin == '+' && l.text == "changed"), "{:?}", v.diff.hunks);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A commit of `files` (path, content) on top of `parent`, written straight into the object
/// store, so names a file system refuses can be in it.
fn tree_commit(repo: &Repository, files: &[(&str, String)], parent: Option<Oid>) -> Oid {
    let mut tb = repo.treebuilder(None).unwrap();
    for (path, text) in files {
        tb.insert(path, repo.blob(text.as_bytes()).unwrap(), 0o100644).unwrap();
    }
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let parents: Vec<git2::Commit> = parent.map(|p| repo.find_commit(p).unwrap()).into_iter().collect();
    repo.commit(None, &sig, &sig, "c", &tree, &parents.iter().collect::<Vec<_>>()).unwrap()
}

#[test]
fn glob_characters_and_a_leading_colon_name_only_their_own_file() {
    let dir = scratch_repo("scope-names");
    let repo = Repository::open(&dir).unwrap();
    let names = ["a*.txt", "a1.txt", "?.txt", "x.txt", "[ab].txt", "a.txt", "b.txt", ":lead.txt", "lead.txt", "!bang.txt", "bang.txt"];
    let c1 = tree_commit(&repo, &names.map(|n| (n, format!("{n}\n"))), None);
    let c2 = tree_commit(&repo, &names.map(|n| (n, format!("{n}\nchanged\n"))), Some(c1));
    let rows = compare(&dir, c1, c2).unwrap().files;
    assert_eq!(rows.len(), names.len());
    for name in names {
        for target in [DiffTarget::Commit(c2, name.into()), DiffTarget::Between(c1, c2, name.into())] {
            same_as_before(&dir, &target, &rows);
            assert_eq!(deltas(&dir, &target, rows.iter().find(|f| f.path == name)), 1, "{name}");
        }
    }
    // In the working tree, where the names this file system allows are.
    let work = scratch_repo("scope-names-workdir");
    let on_disk: &[&str] = if cfg!(windows) { &["[ab].txt", "a.txt", "b.txt", "!bang.txt", "bang.txt"] } else { &names };
    for n in on_disk {
        write(&work, n, "one\n");
    }
    let c1 = commit_all(&work, "c1");
    for n in on_disk {
        write(&work, n, "one\ntwo\n");
    }
    let rows = compare_workdir(&work, c1).unwrap().files;
    for n in on_disk {
        let target = DiffTarget::ToWorkdir(c1, (*n).into());
        same_as_before(&work, &target, &rows);
        assert_eq!(deltas(&work, &target, rows.iter().find(|f| f.path == *n)), 1, "{n}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn binary_lfs_and_submodule_files_read_as_before() {
    let lib = scratch_repo("scope-sub-lib");
    write(&lib, "lib.txt", "lib\n");
    commit_all(&lib, "lib");
    let dir = scratch_repo("scope-kinds");
    write(&dir, ".gitattributes", "*.psd filter=lfs diff=lfs merge=lfs -text\n");
    write(&dir, "art.psd", "version https://git-lfs.github.com/spec/v1\noid sha256:aaaa\nsize 10\n");
    std::fs::write(dir.join("blob.bin"), b"\0\x01\x02one").unwrap();
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    git(&dir, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "libs/lib"]);
    let c1 = commit_all(&dir, "c1");
    write(&dir, "art.psd", "version https://git-lfs.github.com/spec/v1\noid sha256:bbbb\nsize 20\n");
    std::fs::write(dir.join("blob.bin"), b"\0\x01\x02two").unwrap();
    write(&dir.join("libs/lib"), "lib.txt", "lib\nmore\n");
    commit_all(&dir.join("libs/lib"), "moved on");

    let rows = compare_workdir(&dir, c1).unwrap().files;
    for path in ["art.psd", "blob.bin", "libs/lib"] {
        assert!(rows.iter().any(|f| f.path == path), "{path} in {rows:?}");
        for target in [DiffTarget::ToWorkdir(c1, path.into()), DiffTarget::FromWorkdir(c1, path.into())] {
            same_as_before(&dir, &target, &rows);
            assert_eq!(deltas(&dir, &target, None), 1, "{path}");
        }
    }
    assert!(load_listed_diff(&dir, &DiffTarget::ToWorkdir(c1, "art.psd".into()), LoadDiffOptions::default(), None).unwrap().lfs.is_some());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&lib);
}
