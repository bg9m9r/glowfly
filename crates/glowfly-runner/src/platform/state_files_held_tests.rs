//! Tests of what a session holds about recent.txt and tabs.txt: a copy kept of a file with
//! lines it couldn't read, a leading BOM, and a file whose start read failed (one read per save,
//! the merge of what it finds, refusals reported once, and Clear recent).

use super::recent::*;
use super::tabs::*;
use super::tests::scratch;
use super::*;

fn read(p: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(p)
}

fn denied(_: &Path) -> std::io::Result<Vec<u8>> {
    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
}

fn listed(file: &Path) -> Vec<PathBuf> {
    parse_recent_bytes(&std::fs::read(file).unwrap()).0
}

fn tabs_in(file: &Path) -> SavedTabs {
    parse_tabs_bytes(&std::fs::read(file).unwrap()).0
}

#[test]
fn a_file_of_undecodable_lines_is_copied_before_the_first_save() {
    let dir = scratch("held-undecodable");
    let file = dir.join("recent.txt");
    let original: &[u8] = b"\xff\xfe\n\x80abc\n";
    std::fs::write(&file, original).unwrap();
    assert!(load_recent_with(&file, &[], read).is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), original, "reading changes nothing");
    let new = dir.join("new");
    let notes = save_recent_to(&file, std::slice::from_ref(&new)).unwrap();
    assert_eq!(notes.kept, Some((dir.join("recent.txt.bak"), 2)));
    assert_eq!(std::fs::read(dir.join("recent.txt.bak")).unwrap(), original);
    assert_eq!(listed(&file), std::slice::from_ref(&new));
    assert_eq!(save_recent_to(&file, std::slice::from_ref(&new)).unwrap(), SaveNotes::default());
    assert!(!dir.join("recent.txt.bak-2").exists(), "one copy a session");
    // The same for tabs.txt: the copy keeps the line that was read too.
    let tabs = dir.join("tabs.txt");
    let original: &[u8] = b"0\n/w/a\n\xc3(\n";
    std::fs::write(&tabs, original).unwrap();
    assert_eq!(load_tabs_with(&tabs, &[], read).0, [PathBuf::from("/w/a")]);
    let notes = save_tabs_to(&tabs, std::slice::from_ref(&new), &[], 0).unwrap();
    assert_eq!(notes.kept, Some((dir.join("tabs.txt.bak"), 1)));
    assert_eq!(std::fs::read(dir.join("tabs.txt.bak")).unwrap(), original);
    assert_eq!(tabs_in(&tabs).0, std::slice::from_ref(&new));
    assert_eq!(save_tabs_to(&tabs, &[new], &[], 0).unwrap(), SaveNotes::default());
    assert!(!dir.join("tabs.txt.bak-2").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_file_is_never_copied() {
    let dir = scratch("held-clean");
    let file = dir.join("recent.txt");
    save_recent_to(&file, &[dir.join("one")]).unwrap();
    assert_eq!(load_recent_with(&file, &[], read), [dir.join("one")]);
    assert_eq!(save_recent_to(&file, &[dir.join("two")]).unwrap(), SaveNotes::default());
    assert!(!dir.join("recent.txt.bak").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_leading_bom_is_stripped() {
    let (a, b, c) = (PathBuf::from("/w/a"), PathBuf::from("/w/b"), PathBuf::from("/w/c"));
    assert_eq!(parse_recent_bytes(b"\xEF\xBB\xBF/w/a\r\n/w/b"), (vec![a.clone(), b.clone()], 0));
    assert_eq!(parse_recent("\u{feff}/w/a"), std::slice::from_ref(&a));
    let ((paths, _, active), skipped) = parse_tabs_bytes(b"\xEF\xBB\xBF2\n/w/a\n/w/b\n/w/c");
    assert_eq!((paths, active, skipped), (vec![a.clone(), b.clone(), c.clone()], 2, 0));
    // Only a leading one: another is part of the line.
    assert_eq!(parse_recent("/w/a\n\u{feff}/w/b")[1], PathBuf::from("\u{feff}/w/b"));
    // Through the files, and the next save writes none.
    let dir = scratch("held-bom");
    let (recent, tabs) = (dir.join("recent.txt"), dir.join("tabs.txt"));
    std::fs::write(&recent, b"\xEF\xBB\xBF/w/a\r\n/w/b").unwrap();
    std::fs::write(&tabs, b"\xEF\xBB\xBF2\n/w/a\n/w/b\n/w/c").unwrap();
    assert_eq!(load_recent_with(&recent, &[], read), [a.clone(), b.clone()]);
    assert_eq!(load_tabs_with(&tabs, &[], read), (vec![a.clone(), b.clone(), c.clone()], vec![None; 3], 2));
    save_recent_to(&recent, &[a.clone(), b.clone()]).unwrap();
    save_tabs_to(&tabs, &[a, b, c], &[], 2).unwrap();
    assert!(!std::fs::read(&recent).unwrap().starts_with(b"\xEF\xBB\xBF"));
    assert!(!std::fs::read(&tabs).unwrap().starts_with(b"\xEF\xBB\xBF"));
    assert!(!dir.join("recent.txt.bak").exists() && !dir.join("tabs.txt.bak").exists(), "a BOM isn't a skipped line");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_blocked_save_reads_once_and_merges() {
    let dir = scratch("held-merge");
    let file = dir.join("recent.txt");
    let [one, two, new, new2] = ["one", "two", "new", "new2"].map(|n| dir.join(n));
    save_recent_to(&file, &[one.clone(), two.clone()]).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    let notes = save_recent_with(&file, std::slice::from_ref(&new), false, read).unwrap();
    assert_eq!(notes, SaveNotes { read_late: Some(vec![one.clone(), two.clone()]), kept: None });
    assert_eq!(listed(&file), [new.clone(), one.clone(), two.clone()]);
    // Still carried, and reported only once.
    assert_eq!(save_recent_to(&file, &[new2.clone(), new.clone()]).unwrap(), SaveNotes::default());
    assert_eq!(listed(&file), [new2.clone(), new.clone(), one.clone(), two.clone()]);
    // A save holding `one` owns it: removing it later removes it.
    save_recent_to(&file, &[one.clone(), new2.clone()]).unwrap();
    assert_eq!(listed(&file), [one.clone(), new2.clone(), two.clone()]);
    save_recent_to(&file, std::slice::from_ref(&new2)).unwrap();
    assert_eq!(listed(&file), [new2.clone(), two.clone()]);
    // A full list leaves no room: the carried ones are dropped, for good.
    let fresh: Vec<PathBuf> = (0..12).map(|k| dir.join(format!("fresh{k}"))).collect();
    save_recent_to(&file, &fresh).unwrap();
    assert_eq!(listed(&file), fresh);
    save_recent_to(&file, std::slice::from_ref(&new)).unwrap();
    assert_eq!(listed(&file), [new]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_blocked_save_that_still_cannot_read_is_refused_and_reported_once() {
    let dir = scratch("held-refused");
    let file = dir.join("recent.txt");
    save_recent_to(&file, &[dir.join("one")]).unwrap();
    let before = std::fs::read(&file).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    let first = save_recent_with(&file, &[dir.join("new")], false, denied);
    assert!(matches!(&first, Err(SaveRefused::Unread { report: true, detail }) if detail.contains("recent.txt")), "{first:?}");
    let second = save_recent_with(&file, &[dir.join("new")], false, denied);
    assert!(matches!(second, Err(SaveRefused::Unread { report: false, .. })), "{second:?}");
    assert_eq!(std::fs::read(&file).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_recent_after_a_blocked_save() {
    let dir = scratch("held-clear-blocked");
    let file = dir.join("recent.txt");
    save_recent_to(&file, &[dir.join("one"), dir.join("two")]).unwrap();
    let before = std::fs::read(&file).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    assert!(matches!(save_recent_with(&file, &[dir.join("a")], false, denied), Err(SaveRefused::Unread { report: true, .. })));
    // Clear recent is always reported, even after the block was.
    let cleared = save_recent_with(&file, &[], true, denied);
    assert!(matches!(cleared, Err(SaveRefused::Unread { report: true, .. })), "{cleared:?}");
    assert_eq!(std::fs::read(&file).unwrap(), before);
    // Once it reads, the file is emptied, and nothing on it is carried into later saves.
    assert_eq!(save_recent_with(&file, &[], true, read).unwrap(), SaveNotes::default());
    assert!(listed(&file).is_empty());
    save_recent_to(&file, &[dir.join("x")]).unwrap();
    assert_eq!(listed(&file), [dir.join("x")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_late_entry_removed_after_the_merged_save_stays_removed() {
    let dir = scratch("held-late-removed");
    let file = dir.join("recent.txt");
    let [a, x, y] = ["a", "x", "y"].map(|n| dir.join(n));
    save_recent_to(&file, &[x.clone(), y.clone()]).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    assert_eq!(save_recent_to(&file, std::slice::from_ref(&a)).unwrap().read_late, Some(vec![x.clone(), y.clone()]));
    // The model merges the late entries and saves the merged list, then removes `x`.
    save_recent_to(&file, &[a.clone(), x.clone(), y.clone()]).unwrap();
    save_recent_to(&file, &[a.clone(), y.clone()]).unwrap();
    assert_eq!(listed(&file), [a.clone(), y.clone()]);
    save_recent_to(&file, std::slice::from_ref(&y)).unwrap();
    assert_eq!(listed(&file), [y]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_recent_drops_carried_entries() {
    let dir = scratch("held-clear-carried");
    let file = dir.join("recent.txt");
    save_recent_to(&file, &[dir.join("one"), dir.join("two")]).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    assert!(save_recent_to(&file, &[dir.join("new")]).unwrap().read_late.is_some());
    clear_recent_to(&file).unwrap();
    assert!(listed(&file).is_empty());
    save_recent_to(&file, &[dir.join("x")]).unwrap();
    assert_eq!(listed(&file), [dir.join("x")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_late_read_with_skipped_lines_copies_first() {
    let dir = scratch("held-late-skipped");
    let file = dir.join("recent.txt");
    let original: &[u8] = b"/w/a\n\xff\n";
    std::fs::write(&file, original).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    let notes = save_recent_to(&file, &[dir.join("x")]).unwrap();
    assert_eq!(notes.read_late, Some(vec![PathBuf::from("/w/a")]));
    assert_eq!(notes.kept, Some((dir.join("recent.txt.bak"), 1)));
    assert_eq!(std::fs::read(dir.join("recent.txt.bak")).unwrap(), original);
    assert_eq!(listed(&file), [dir.join("x"), PathBuf::from("/w/a")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_blocked_tabs_save_merges_the_saved_tabs() {
    let dir = scratch("held-tabs");
    let file = dir.join("tabs.txt");
    let [a, b, c] = ["a", "b", "c"].map(|n| dir.join(n));
    let label = |s: &str| Some(s.to_owned());
    save_tabs_to(&file, &[a.clone(), b.clone()], &[None, label("B")], 1).unwrap();
    assert!(load_tabs_with(&file, &[], denied).0.is_empty());
    let notes = save_tabs_with(&file, &[c.clone(), a.clone()], &[label("C")], 0, read).unwrap();
    assert_eq!(notes.read_late, Some(vec![a.clone(), b.clone()]));
    assert_eq!(tabs_in(&file), (vec![c, a.clone(), b.clone()], vec![label("C"), None, label("B")], 0));
    // `b` is still carried; `a` was the session's, and is closed now.
    assert_eq!(save_tabs_to(&file, &[], &[], 3).unwrap(), SaveNotes::default());
    assert_eq!(tabs_in(&file), (vec![b], vec![label("B")], 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_gone_since_the_failed_read_unblocks() {
    let dir = scratch("held-gone");
    let file = dir.join("recent.txt");
    save_recent_to(&file, &[dir.join("one")]).unwrap();
    assert!(load_recent_with(&file, &[], denied).is_empty());
    std::fs::remove_file(&file).unwrap();
    assert_eq!(save_recent_to(&file, &[dir.join("new")]).unwrap(), SaveNotes::default());
    assert_eq!(listed(&file), [dir.join("new")]);
    let _ = std::fs::remove_dir_all(&dir);
}
