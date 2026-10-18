use super::conflict_saved::saved_picks_with_limit;
use super::*;
use crate::conflict_pick::{ChunkPick, render_picks};
use crate::gitcli::{args, run};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// f.txt with lines 2 and 10 changed differently on main and on `other`, mid-merge of `other`;
/// `config` is set first (before any file is written).
fn merging(name: &str, config: &[(&str, &str)]) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, config);
    let base: String = (1..=12).map(|i| format!("line {i}\n")).collect();
    let side = |s: &str| base.replace("line 2\n", &format!("line 2 {s}\n")).replace("line 10\n", &format!("line 10 {s}\n"));
    write(&dir, "f.txt", &base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "f.txt", &side("theirs"));
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "f.txt", &side("ours"));
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// Saves the first chunk as ours, the second still in markers, as Save with markers does.
fn save_first_ours(dir: &Path) -> ConflictFile {
    let f = load_conflict(dir, "f.txt").unwrap();
    assert_eq!(f.chunk_count(), 2);
    let contents = render_picks(&f, &[Choice::Ours.into(), ChunkPick::default()], true, false, None).unwrap();
    write_partial_resolution(dir, "f.txt", &contents, &f.ids).unwrap();
    f
}

#[test]
fn a_partial_save_comes_back() {
    let dir = merging("saved-picks", &[]);
    let f = load_conflict(&dir, "f.txt").unwrap();
    assert_eq!(saved_picks(&dir, &f), None, "git's own markers bring nothing back");
    let f = save_first_ours(&dir);
    let saved = saved_picks(&dir, &f).expect("the pick comes back");
    assert_eq!(saved.picks, [Choice::Ours.into(), ChunkPick::default()]);
    assert!(saved.cleans.is_empty());
    // Too large to read back (a tiny limit here rather than a 4 MiB file).
    assert_eq!(saved_picks_with_limit(&dir, &f, 10), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_partial_save_comes_back_with_crlf_checkouts() {
    let dir = merging("saved-picks-crlf", &[("core.autocrlf", "true")]);
    let f = save_first_ours(&dir);
    assert!(std::fs::read_to_string(dir.join("f.txt")).unwrap().contains("\r\n"), "checked out with CR LF");
    let saved = saved_picks(&dir, &f).expect("the pick comes back");
    assert_eq!((saved.picks, saved.cleans.len()), (vec![Choice::Ours.into(), ChunkPick::default()], 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn converted_files_bring_nothing_back() {
    for attr in ["filter=lfs", "ident", "working-tree-encoding=UTF-16"] {
        let dir = merging(&format!("saved-picks-{}", attr.split('=').next().unwrap()), &[]);
        let f = save_first_ours(&dir);
        write(&dir, ".gitattributes", &format!("f.txt {attr}\n"));
        assert_eq!(saved_picks(&dir, &f), None, "{attr}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn only_text_conflicts_are_read() {
    let dir = merging("saved-picks-binary", &[]);
    let mut f = save_first_ours(&dir);
    f.content = ConflictContent::Binary;
    assert_eq!(saved_picks(&dir, &f), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn links_are_not_followed() {
    let dir = merging("saved-picks-link", &[]);
    let f = save_first_ours(&dir);
    std::fs::rename(dir.join("f.txt"), dir.join("real.txt")).unwrap();
    std::os::unix::fs::symlink("real.txt", dir.join("f.txt")).unwrap();
    assert_eq!(saved_picks(&dir, &f), None);
    let _ = std::fs::remove_dir_all(&dir);
}
