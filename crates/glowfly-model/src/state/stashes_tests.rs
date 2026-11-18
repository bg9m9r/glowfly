//! The Stash dialog's file rows: the working-tree files merged from the staged and unstaged
//! lists, and the old path a staged rename keeps.

use super::*;
use crate::state::files::staged_rename_from;
use crate::state::op_fixture::snapshot;

fn renamed(path: &str, status: char, from: &str) -> FileEntry {
    FileEntry { renamed_from: Some(from.to_owned()), ..FileEntry::new(path, status) }
}

#[test]
fn a_staged_rename_keeps_its_old_path_under_an_unstaged_change() {
    let mut snap = snapshot("stashes-wip-rename");
    snap.staged = vec![renamed("new.txt", 'R', "old.txt")];
    snap.unstaged = vec![FileEntry::new("new.txt", 'M')];
    let rows: Vec<_> = wip_files(&snap).into_iter().map(|f| (f.path, f.status, f.renamed_from)).collect();
    assert_eq!(rows, [("new.txt".to_owned(), 'M', Some("old.txt".to_owned()))]);

    snap.unstaged.clear();
    let rows: Vec<_> = wip_files(&snap).into_iter().map(|f| (f.path, f.status, f.renamed_from)).collect();
    assert_eq!(rows, [("new.txt".to_owned(), 'R', Some("old.txt".to_owned()))], "a staged-only rename keeps R");

    snap.staged = vec![FileEntry::new("n.txt", 'A')];
    snap.unstaged = vec![FileEntry::new("n.txt", 'M')];
    assert_eq!(wip_files(&snap)[0].renamed_from, None, "no old path to keep");
}

#[test]
fn the_old_path_of_a_staged_rename_is_looked_up_by_its_new_path() {
    let mut snap = snapshot("stashes-staged-rename-from");
    snap.staged = vec![renamed("new.txt", 'R', "old.txt"), FileEntry::new("n.txt", 'A'), renamed("c.txt", 'A', "src.txt")];
    assert_eq!(staged_rename_from(&snap, "new.txt"), Some("old.txt"));
    assert_eq!(staged_rename_from(&snap, "n.txt"), None);
    assert_eq!(staged_rename_from(&snap, "c.txt"), None, "a copy isn't a rename");
    assert_eq!(staged_rename_from(&snap, "absent.txt"), None);
}
