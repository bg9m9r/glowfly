use super::resolve_undo::{parse_ids, path_batches};
use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// A merge in progress where each of `files` conflicts ("ours" against "theirs" on one line).
/// Contents come from `content(file, side)`, side being "base", "ours" or "theirs".
fn merging_with(name: &str, files: &[&str], content: impl Fn(&str, &str) -> Vec<u8>) -> PathBuf {
    let dir = scratch_repo(name);
    let put = |side: &str| {
        for f in files {
            std::fs::write(dir.join(f), content(f, side)).unwrap();
        }
    };
    put("base");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put("theirs");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put("ours");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

fn merging(name: &str, files: &[&str]) -> PathBuf {
    merging_with(name, files, |_, side| format!("top\n{side}\nend\n").into_bytes())
}

fn listed(dir: &Path) -> Vec<(String, bool)> {
    resolved_paths(dir).unwrap().into_iter().map(|r| (r.entry.path, r.both_sides)).collect()
}

fn conflicted(dir: &Path, path: &str) -> bool {
    Repository::open(dir).unwrap().index().unwrap().conflict_get(Path::new(path)).is_ok()
}

fn markers_in(dir: &Path, path: &str) -> usize {
    marker_regions(&std::fs::read(dir.join(path)).unwrap())
}

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

#[test]
fn git_add_records_resolve_undo_data() {
    let dir = merging("reuc-git-add", &["a.txt", "b.txt"]);
    assert!(listed(&dir).is_empty(), "nothing resolved yet");
    git(&dir, &["add", "a.txt"]);
    assert_eq!(listed(&dir), [("a.txt".to_owned(), true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_resolution_written_here_can_be_marked_conflicted_again() {
    let dir = merging("reuc-write", &["a.txt"]);
    let f = load_conflict(&dir, "a.txt").unwrap();
    let out = render_output(&f, &[Choice::Theirs]).unwrap();
    write_resolution(&dir, "a.txt", &out, &f.ids).unwrap();
    assert!(!conflicted(&dir, "a.txt"));
    assert_eq!(listed(&dir), [("a.txt".to_owned(), true)], "add_frombuffer keeps the record");
    mark_conflicted(&dir, &owned(&["a.txt"])).unwrap();
    assert!(conflicted(&dir, "a.txt"));
    assert!(markers_in(&dir, "a.txt") >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_whole_side_keeps_the_record_and_a_deletion_cannot_come_back() {
    let dir = merging_with("reuc-side", &["t.txt", "bin.dat", "d.txt"], |f, side| match (f, side) {
        ("bin.dat", _) => format!("{side}\0data").into_bytes(),
        ("d.txt", "ours") => b"keep\nchanged\n".to_vec(),
        ("d.txt", _) => b"keep\n".to_vec(),
        _ => format!("top\n{side}\nend\n").into_bytes(),
    });
    // Theirs deletes d.txt (the merge above wrote it on that side, so remove it there now).
    git(&dir, &["merge", "--abort"]);
    git(&dir, &["checkout", "-q", "other"]);
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["commit", "-q", "-m", "delete d"]);
    git(&dir, &["checkout", "-q", "main"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0);

    for path in ["t.txt", "bin.dat"] {
        let f = load_conflict(&dir, path).unwrap();
        resolve_with_side(&dir, path, Side::Ours, &f.ids).unwrap();
        assert!(!conflicted(&dir, path), "{path}");
    }
    let d = load_conflict(&dir, "d.txt").unwrap();
    assert_eq!(d.content, ConflictContent::DeletedBy(Side::Theirs));
    resolve_with_side(&dir, "d.txt", Side::Theirs, &d.ids).unwrap();
    let mut found = listed(&dir);
    found.sort();
    assert_eq!(found, [("bin.dat".to_owned(), true), ("d.txt".to_owned(), false), ("t.txt".to_owned(), true)]);

    mark_conflicted(&dir, &owned(&["t.txt", "bin.dat"])).unwrap();
    assert!(conflicted(&dir, "t.txt") && conflicted(&dir, "bin.dat"));
    assert!(markers_in(&dir, "t.txt") >= 1);

    let index = std::fs::read(dir.join(".git/index")).unwrap();
    let err = mark_conflicted(&dir, &owned(&["d.txt"])).unwrap_err();
    assert!(err.to_string().contains("both versions (d.txt)"), "{err}");
    assert!(!dir.join("d.txt").exists());
    assert_eq!(std::fs::read(dir.join(".git/index")).unwrap(), index, "the index is unchanged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_conflicted_refuses_a_path_without_a_record_and_leaves_the_file() {
    let dir = merging("reuc-none", &["a.txt"]);
    write(&dir, "note.txt", "my edits\n");
    git(&dir, &["add", "note.txt"]);
    write(&dir, "note.txt", "more edits\n");
    let err = mark_conflicted(&dir, &owned(&["a.txt", "note.txt"])).unwrap_err();
    assert!(err.to_string().contains("has no saved conflict to restore"), "{err}");
    assert_eq!(std::fs::read(dir.join("note.txt")).unwrap(), b"more edits\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_resolved_stages_every_path_and_batches_split_long_lists() {
    let dir = merging("reuc-mark", &["a.txt", "b.txt", "c d.txt"]);
    mark_resolved(&dir, &owned(&["a.txt", "b.txt", "c d.txt"])).unwrap();
    let repo = Repository::open(&dir).unwrap();
    assert!(!repo.index().unwrap().has_conflicts());
    assert_eq!(listed(&dir).len(), 3);

    let paths = owned(&["aa", "bb", "cccccccc"]);
    let groups: Vec<Vec<String>> = path_batches(&paths, 5).into_iter().map(|g| g.to_vec()).collect();
    assert_eq!(groups, [owned(&["aa", "bb"]), owned(&["cccccccc"])]);
    assert_eq!(path_batches(&owned(&["toolong"]), 3), [owned(&["toolong"]).as_slice()]);
    assert!(path_batches(&[], 5).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_merge_forgets_the_last_ones_resolutions() {
    let dir = merging("reuc-forget", &["a.txt"]);
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "--no-edit"]);
    // A second branch that conflicts only in b.txt.
    write(&dir, "b.txt", "base\n");
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    git(&dir, &["checkout", "-q", "-b", "again"]);
    write(&dir, "b.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "b theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "b.txt", "ours\n");
    git(&dir, &["commit", "-q", "-am", "b ours"]);
    assert!(!run(&dir, &args(&["merge", "again"])).0);
    assert!(listed(&dir).is_empty(), "{:?}", listed(&dir));
    git(&dir, &["add", "b.txt"]);
    assert_eq!(listed(&dir), [("b.txt".to_owned(), true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflict_summary_counts_markers_and_lists_resolved_files() {
    let dir = merging("reuc-summary", &["a.txt", "b.txt"]);
    git(&dir, &["add", "b.txt"]);
    let s = conflict_summary(&dir, &owned(&["a.txt"]), true).unwrap();
    assert_eq!(s.counts, [("a.txt".to_owned(), Some(1))]);
    assert_eq!(s.resolved.len(), 1);
    assert!(conflict_summary(&dir, &owned(&["a.txt"]), false).unwrap().resolved.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn a_name_that_is_not_utf8_is_listed_with_its_hazard() {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::OsStr::from_bytes(b"n\xffme.txt");
    let dir = scratch_repo("reuc-not-utf8");
    let put = |text: &str| std::fs::write(dir.join(name), text).unwrap();
    put("base\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put("theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put("ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0);
    git(&dir, &["add", "-A"]);
    let found = resolved_paths(&dir).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].entry.hazard, Some(Hazard::NotUtf8(b"n\xffme.txt".to_vec())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_undo_ids_match_the_loaded_conflict() {
    let dir = merging("reuc-ids", &["a.txt", "b.txt"]);
    assert_eq!(resolve_undo_ids(&dir, "a.txt").unwrap(), None, "nothing resolved yet");
    let f = load_conflict(&dir, "a.txt").unwrap();
    let out = render_output(&f, &[Choice::Ours]).unwrap();
    write_resolution(&dir, "a.txt", &out, &f.ids).unwrap();
    assert_eq!(resolve_undo_ids(&dir, "a.txt").unwrap(), Some(f.ids));
    assert_eq!(resolve_undo_ids(&dir, "b.txt").unwrap(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_ids_matches_the_path_exactly() {
    let (o1, o2, o3) = ("1".repeat(40), "2".repeat(40), "3".repeat(40));
    let id = |s: &str| Some(git2::Oid::from_str(s).unwrap());
    let rec = |stage: u8, oid: &str, path: &str| format!("100644 {oid} {stage}\t{path}\0");
    let out = [
        rec(1, &o1, "a.txt"),
        rec(2, &o2, "a.txt"),
        rec(3, &o3, "a.txt"),
        rec(2, &o3, "a.txt.bak"),
        rec(3, &o1, "a.txt.bak"),
        rec(1, &o2, "my file.txt"),
        rec(2, &o1, "my file.txt"),
        rec(3, &o3, "my file.txt"),
        // add/add: no base.
        rec(2, &o1, "new.txt"),
        rec(3, &o2, "new.txt"),
    ]
    .concat();
    let out = out.as_bytes();
    assert_eq!(parse_ids(out, "a.txt"), Some(ConflictIds { ancestor: id(&o1), ours: id(&o2), theirs: id(&o3) }));
    assert_eq!(parse_ids(out, "a.txt.bak"), Some(ConflictIds { ancestor: None, ours: id(&o3), theirs: id(&o1) }));
    assert_eq!(parse_ids(out, "my file.txt"), Some(ConflictIds { ancestor: id(&o2), ours: id(&o1), theirs: id(&o3) }));
    assert_eq!(parse_ids(out, "new.txt"), Some(ConflictIds { ancestor: None, ours: id(&o1), theirs: id(&o2) }));
    assert_eq!(parse_ids(out, "a.tx"), None);
    assert_eq!(parse_ids(out, "A.txt"), None, "never folds case");
    assert_eq!(parse_ids(out, " a.txt"), None);
    assert_eq!(parse_ids(b"", "a.txt"), None);
}
