//! LFS marks on file rows and LFS sides of a diff. The fixtures commit pointer text with no
//! `filter=lfs` attribute, so git stores it as it is whether or not git-lfs is installed.

use crate::lfs::pointer::{LfsChange, LfsSide, Pointer};
use crate::repo::{
    DiffTarget, FileEntry, LoadDiffOptions, commit_detail, compare_from_workdir, compare_workdir, load_diff_for_view, status_at,
    status_of_paths,
};
use crate::testutil::{git, lfs_or_skip, scratch_repo, write};
use git2::{Oid, Repository};
use std::path::Path;

/// Pointer text naming an object of `size` bytes whose oid is `digit` 64 times.
fn ptr(size: u64, digit: char) -> String {
    format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n", digit.to_string().repeat(64))
}

fn pointer(size: u64, digit: char) -> LfsSide {
    LfsSide::Pointer(Pointer { oid: digit.to_string().repeat(64), size })
}

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// (path, status, lfs) for each row.
fn marks(rows: &[FileEntry]) -> Vec<(&str, char, bool)> {
    rows.iter().map(|f| (f.path.as_str(), f.status, f.lfs)).collect()
}

fn view(dir: &Path, target: &DiffTarget) -> Option<LfsChange> {
    load_diff_for_view(dir, target, LoadDiffOptions::default()).unwrap().lfs
}

#[test]
fn commit_details_mark_added_and_deleted_pointers() {
    let dir = scratch_repo("lfs-marks-commit");
    write(&dir, "art.psd", &ptr(13_002_342, 'a'));
    write(&dir, "notes.txt", "plain\n");
    let added = commit_all(&dir, "add");
    let d = commit_detail(&dir, added).unwrap();
    assert_eq!(marks(&d.files), [("art.psd", 'A', true), ("notes.txt", 'A', false)]);
    std::fs::remove_file(dir.join("art.psd")).unwrap();
    let deleted = commit_all(&dir, "remove");
    assert_eq!(marks(&commit_detail(&dir, deleted).unwrap().files), [("art.psd", 'D', true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn status_marks_staged_and_untracked_pointers() {
    let dir = scratch_repo("lfs-marks-status");
    write(&dir, "plain.txt", "one\n");
    commit_all(&dir, "base");
    write(&dir, "plain.txt", "two\n");
    write(&dir, "staged.psd", &ptr(10, 'b'));
    git(&dir, &["add", "staged.psd"]);
    write(&dir, "loose.bin", &ptr(20, 'c'));
    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    assert_eq!(marks(&staged), [("staged.psd", 'A', true)]);
    assert_eq!(marks(&unstaged), [("loose.bin", '?', true), ("plain.txt", 'M', false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unstaged_row_counts_the_index_pointer_and_paths_reads_match() {
    let dir = scratch_repo("lfs-marks-unstaged");
    write(&dir, "art.psd", &ptr(10, 'a'));
    write(&dir, "plain.txt", "one\n");
    commit_all(&dir, "base");
    write(&dir, "art.psd", "the real content, not a pointer\n");
    write(&dir, "plain.txt", "two\n");
    write(&dir, "loose.bin", &ptr(20, 'c'));
    let (unstaged, _, _) = status_at(&dir, false).unwrap();
    let want = [("art.psd", 'M', true), ("loose.bin", '?', true), ("plain.txt", 'M', false)];
    assert_eq!(marks(&unstaged), want);
    let paths: Vec<String> = ["art.psd", "loose.bin", "plain.txt"].map(String::from).into();
    let read = status_of_paths(&dir, &paths, false).unwrap().expect("these paths read on their own");
    assert_eq!(marks(&read.unstaged), want, "a paths read marks rows as a whole read does");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_working_tree_comparison_marks_a_pointer_working_copy() {
    let dir = scratch_repo("lfs-marks-compare-workdir");
    write(&dir, "plain.txt", "one\n");
    let base = commit_all(&dir, "base");
    write(&dir, "art.psd", &ptr(10, 'a'));
    write(&dir, "plain.txt", "two\n");
    assert_eq!(marks(&compare_workdir(&dir, base).unwrap().files), [("art.psd", 'A', true), ("plain.txt", 'M', false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_lfs_badge_is_the_same_both_ways() {
    let dir = scratch_repo("lfs-marks-compare-both-ways");
    write(
        &dir,
        "plain.txt",
        "one
",
    );
    write(&dir, "art.psd", &ptr(10, 'a'));
    let base = commit_all(&dir, "base");
    std::fs::remove_file(dir.join("art.psd")).unwrap();
    write(&dir, "loose.psd", &ptr(20, 'b'));
    write(
        &dir,
        "plain.txt",
        "two
",
    );
    let forward = compare_workdir(&dir, base).unwrap();
    assert_eq!(marks(&forward.files), [("art.psd", 'D', true), ("loose.psd", 'A', true), ("plain.txt", 'M', false)]);
    let reversed = compare_from_workdir(&dir, base).unwrap();
    assert_eq!(marks(&reversed.files), [("art.psd", 'A', true), ("loose.psd", 'D', true), ("plain.txt", 'M', false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn view_diffs_carry_the_pointer_sides() {
    let dir = scratch_repo("lfs-marks-view");
    write(&dir, "plain.txt", "one\n");
    commit_all(&dir, "base");
    write(&dir, "art.psd", &ptr(10, 'a'));
    git(&dir, &["add", "art.psd"]);
    let added = LfsChange { old: LfsSide::Absent, new: pointer(10, 'a'), new_in_workdir: false, filtered: false };
    assert_eq!(view(&dir, &DiffTarget::Staged("art.psd".into())), Some(added.clone()));
    let c = commit_all(&dir, "add");
    assert_eq!(view(&dir, &DiffTarget::Commit(c, "art.psd".into())), Some(added));
    write(&dir, "art.psd", &ptr(20, 'b'));
    let c = commit_all(&dir, "change");
    let changed = LfsChange { old: pointer(10, 'a'), new: pointer(20, 'b'), new_in_workdir: false, filtered: false };
    assert_eq!(view(&dir, &DiffTarget::Commit(c, "art.psd".into())), Some(changed));
    write(&dir, "plain.txt", "two\n");
    assert_eq!(view(&dir, &DiffTarget::Unstaged("plain.txt".into())), None, "a plain text diff");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unstaged_regular_file_over_a_pointer_reads_the_working_copy() {
    let dir = scratch_repo("lfs-marks-regular");
    write(&dir, "art.psd", &ptr(10, 'a'));
    commit_all(&dir, "base");
    let content = "the real content, smudged\n";
    write(&dir, "art.psd", content);
    let target = DiffTarget::Unstaged("art.psd".into());
    let size = content.len() as u64;
    let want = |filtered| LfsChange { old: pointer(10, 'a'), new: LfsSide::Regular { size }, new_in_workdir: true, filtered };
    assert_eq!(view(&dir, &target), Some(want(false)));
    // Written after the commit and never added through, so the stored pointer stays as it is.
    write(&dir, ".gitattributes", "*.psd filter=lfs diff=lfs merge=lfs -text\n");
    assert_eq!(view(&dir, &target), Some(want(true)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_git_lfs_stores_is_marked_end_to_end() {
    if lfs_or_skip("a_file_git_lfs_stores_is_marked_end_to_end").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-marks-real");
    git(&dir, &["lfs", "install", "--local"]);
    git(&dir, &["lfs", "track", "*.bin"]);
    write(&dir, "data.bin", "binary-ish content\n");
    let c = commit_all(&dir, "add");
    let files = commit_detail(&dir, c).unwrap().files;
    assert_eq!(marks(&files), [(".gitattributes", 'A', false), ("data.bin", 'A', true)]);
    let _ = std::fs::remove_dir_all(&dir);
}
