//! `LineMap::line` over hand-made changed regions, and `line_maps` / `pr_line_maps` over
//! fixture histories: shifted, edited, renamed, binary, deleted and absent files.

use super::{LineChange, LineMap, PrLineMaps, line_maps, pr_line_maps};
use crate::testutil::{git, scratch_repo, write};
use git2::{Oid, Repository};
use std::path::Path;

fn change(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> LineChange {
    LineChange { old_start, old_lines, new_start, new_lines }
}

fn with(changes: Vec<LineChange>) -> LineMap {
    LineMap { path: Some("a.txt".into()), changes: Some(changes) }
}

fn numbered(lines: &[&str]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn ten() -> Vec<String> {
    (1..=10).map(|n| format!("line {n}")).collect()
}

fn commit_all(dir: &Path, msg: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

fn paths(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn an_insertion_at_the_start_shifts_every_line() {
    let m = with(vec![change(0, 0, 1, 2)]);
    assert_eq!((m.line(1), m.line(7)), (Some(3), Some(9)));
}

#[test]
fn an_insertion_after_a_line_shifts_only_the_lines_after_it() {
    let m = with(vec![change(5, 0, 6, 3)]);
    assert_eq!((1..=7).map(|n| m.line(n)).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 9, 10].map(Some));
}

#[test]
fn a_deletion_drops_its_lines_and_pulls_later_ones_up() {
    let m = with(vec![change(4, 2, 3, 0)]);
    assert_eq!((m.line(3), m.line(4), m.line(5), m.line(6)), (Some(3), None, None, Some(4)));
}

#[test]
fn a_replacement_drops_the_line_and_shifts_by_the_growth() {
    let m = with(vec![change(2, 1, 2, 3)]);
    assert_eq!((m.line(1), m.line(2), m.line(3)), (Some(1), None, Some(5)));
}

#[test]
fn line_zero_binary_and_gone_map_nowhere() {
    assert_eq!(with(vec![]).line(0), None);
    assert_eq!(LineMap { path: Some("a.bin".into()), changes: None }.line(1), None);
    assert_eq!(LineMap::gone().line(1), None);
    assert_eq!(LineMap::same("a.txt").line(4), Some(4));
}

#[test]
fn lines_shift_past_added_and_removed_lines_above() {
    let dir = scratch_repo("line-map-shift");
    let head_lines = ten();
    write(&dir, "a.txt", &numbered(&head_lines.iter().map(String::as_str).collect::<Vec<_>>()));
    let head = commit_all(&dir, "head");
    let mut local: Vec<String> = head_lines.clone();
    local.insert(2, "new a".into());
    local.insert(3, "new b".into());
    local.retain(|l| l != "line 8");
    write(&dir, "a.txt", &numbered(&local.iter().map(String::as_str).collect::<Vec<_>>()));
    let here = commit_all(&dir, "local");
    let maps = line_maps(&dir, head, here, &paths(&["a.txt"])).unwrap();
    let m = &maps["a.txt"];
    assert_eq!(m.path.as_deref(), Some("a.txt"));
    assert_eq!([1, 3, 7, 8, 10].map(|n| m.line(n)), [Some(1), Some(5), Some(9), None, Some(11)]);
}

#[test]
fn an_edited_commented_line_maps_to_none() {
    let dir = scratch_repo("line-map-edit");
    let lines = ten();
    write(&dir, "a.txt", &numbered(&lines.iter().map(String::as_str).collect::<Vec<_>>()));
    let head = commit_all(&dir, "head");
    let mut local = lines.clone();
    local[5] = "edited".into();
    write(&dir, "a.txt", &numbered(&local.iter().map(String::as_str).collect::<Vec<_>>()));
    let here = commit_all(&dir, "local");
    let m = &line_maps(&dir, head, here, &paths(&["a.txt"])).unwrap()["a.txt"];
    assert_eq!((m.line(5), m.line(6), m.line(7)), (Some(5), None, Some(7)));
}

#[test]
fn a_renamed_file_follows_the_rename() {
    let dir = scratch_repo("line-map-rename");
    let body = numbered(&["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta"]);
    write(&dir, "old.txt", &body);
    let parent = commit_all(&dir, "parent");
    std::fs::remove_file(dir.join("old.txt")).unwrap();
    write(&dir, "new.txt", &format!("added\n{body}"));
    let head = commit_all(&dir, "rename");
    let m = &line_maps(&dir, head, parent, &paths(&["new.txt"])).unwrap()["new.txt"];
    assert_eq!(m.path.as_deref(), Some("old.txt"));
    assert_eq!((m.line(1), m.line(2), m.line(9)), (None, Some(1), Some(8)));
}

#[test]
fn local_behind_or_ahead_of_the_head_shifts_the_other_way() {
    let dir = scratch_repo("line-map-behind");
    let lines = ten();
    write(&dir, "a.txt", &numbered(&lines.iter().map(String::as_str).collect::<Vec<_>>()));
    let older = commit_all(&dir, "older");
    let mut newer = lines.clone();
    for (i, l) in ["x", "y", "z"].iter().enumerate() {
        newer.insert(5 + i, (*l).to_owned());
    }
    write(&dir, "a.txt", &numbered(&newer.iter().map(String::as_str).collect::<Vec<_>>()));
    let head = commit_all(&dir, "head adds three");
    // Local is behind: the head's added lines aren't there, and the lines after them move up.
    let behind = &line_maps(&dir, head, older, &paths(&["a.txt"])).unwrap()["a.txt"];
    assert_eq!([5, 6, 8, 9, 13].map(|n| behind.line(n)), [Some(5), None, None, Some(6), Some(10)]);
    // Local is ahead (it added the lines): the head's later lines move down.
    let ahead = &line_maps(&dir, older, head, &paths(&["a.txt"])).unwrap()["a.txt"];
    assert_eq!([5, 6, 10].map(|n| ahead.line(n)), [Some(5), Some(9), Some(13)]);
}

#[test]
fn unchanged_deleted_absent_and_binary_files() {
    let dir = scratch_repo("line-map-kinds");
    write(&dir, "same.txt", "one\ntwo\n");
    write(&dir, "gone.txt", "bye\n");
    write(&dir, "a.txt", "lower\n");
    std::fs::write(dir.join("pic.bin"), [0u8, 1, 2, 0, 3]).unwrap();
    let head = commit_all(&dir, "head");
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    std::fs::write(dir.join("pic.bin"), [0u8, 9, 9, 0, 3, 4]).unwrap();
    let here = commit_all(&dir, "local");
    let maps = line_maps(&dir, head, here, &paths(&["same.txt", "gone.txt", "nowhere.txt", "pic.bin", "A.txt"])).unwrap();
    assert_eq!(maps["same.txt"], LineMap::same("same.txt"));
    assert_eq!(maps["gone.txt"], LineMap::gone());
    assert_eq!(maps["nowhere.txt"], LineMap::gone());
    assert_eq!(maps["pic.bin"].path.as_deref(), Some("pic.bin"));
    assert_eq!(maps["pic.bin"].line(1), None);
    // Byte for byte: "A.txt" isn't "a.txt", even where the file system folds case.
    assert_eq!(maps["A.txt"], LineMap::gone());
}

#[test]
fn a_mode_only_change_keeps_every_line() {
    let dir = scratch_repo("line-map-mode");
    write(&dir, "run.sh", "echo hi\necho bye\n");
    let head = commit_all(&dir, "head");
    git(&dir, &["update-index", "--chmod=+x", "run.sh"]);
    git(&dir, &["commit", "-q", "-m", "mode"]);
    let here = Repository::open(&dir).unwrap().head().unwrap().peel_to_commit().unwrap().id();
    let m = &line_maps(&dir, head, here, &paths(&["run.sh"])).unwrap()["run.sh"];
    assert_eq!((m.path.as_deref(), m.line(2)), (Some("run.sh"), Some(2)));
}

#[test]
fn pr_line_maps_finds_the_head_and_whether_old_lines_hold() {
    let dir = scratch_repo("line-map-pr");
    write(&dir, "a.txt", "one\ntwo\n");
    let base = commit_all(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    write(&dir, "a.txt", "zero\none\ntwo\n");
    let head = commit_all(&dir, "head");
    write(&dir, "a.txt", "zero\none\ntwo\nthree\n");
    let local = commit_all(&dir, "local");
    let p = paths(&["a.txt"]);
    assert_eq!(pr_line_maps(&dir, &"f".repeat(40), local, base, base, &p).unwrap(), PrLineMaps::HeadMissing);
    let upper = head.to_string()[..12].to_ascii_uppercase();
    let PrLineMaps::Mapped { old_kept, files } = pr_line_maps(&dir, &upper, local, base, base, &p).unwrap() else {
        panic!("the head is here");
    };
    assert!(old_kept);
    assert_eq!(files["a.txt"].line(3), Some(3));
    // The target takes the head's commit: merge-base(target, head) is now the head itself.
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "take", &head.to_string()]);
    let target = Repository::open(&dir).unwrap().head().unwrap().peel_to_commit().unwrap().id();
    let PrLineMaps::Mapped { old_kept, .. } = pr_line_maps(&dir, &head.to_string(), local, target, base, &p).unwrap() else {
        panic!("the head is here");
    };
    assert!(!old_kept);
}
