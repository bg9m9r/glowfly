//! The paths a working-tree line count is asked about: the rows of one status read, so the
//! diffs behind the counts read only those paths and not the whole tree.

use super::{FileEntry, Hazard, MAX_COUNTED_FILES};
use std::collections::BTreeSet;

/// The changed paths of a status read: the union of both lists' first `MAX_COUNTED_FILES` rows
/// (a staged rename's old path too), as git stores them. Both diffs of a count take the union,
/// so a path that libgit2 reports on one side only (an intent-to-add file, a conflicted one)
/// is counted as the whole-tree scan counts it.
#[derive(Clone, Default, PartialEq)]
pub struct WipPaths {
    /// Sorted and without duplicates.
    pub paths: Vec<Vec<u8>>,
    /// The staged list had more rows than were taken.
    pub staged_more: bool,
    /// The unstaged list had more rows than were taken.
    pub unstaged_more: bool,
    /// A staged rename's old name isn't known (not UTF-8): the staged diff reads the whole tree
    /// so the rename is still paired.
    pub staged_whole: bool,
}

impl WipPaths {
    /// The paths of the first `MAX_COUNTED_FILES` rows of each list (both sorted by path).
    pub fn of(staged: &[FileEntry], unstaged: &[FileEntry]) -> Self {
        Self::of_capped(staged, unstaged, MAX_COUNTED_FILES)
    }

    /// `of` with the first `cap` rows of each list.
    pub(crate) fn of_capped(staged: &[FileEntry], unstaged: &[FileEntry], cap: usize) -> Self {
        let mut set: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut staged_whole = false;
        for row in staged.iter().take(cap) {
            set.insert(bytes_of(row));
            match &row.renamed_from {
                Some(old) => {
                    set.insert(old.as_bytes().to_vec());
                }
                None => staged_whole |= row.status == 'R',
            }
        }
        set.extend(unstaged.iter().take(cap).map(bytes_of));
        Self { paths: set.into_iter().collect(), staged_more: staged.len() > cap, unstaged_more: unstaged.len() > cap, staged_whole }
    }

    /// No path to count (and nothing to read: an empty path list would mean every path).
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

/// The row's path as git stores it.
fn bytes_of(row: &FileEntry) -> Vec<u8> {
    match &row.hazard {
        Some(Hazard::NotUtf8(bytes)) => bytes.clone(),
        _ => row.path.as_bytes().to_vec(),
    }
}

/// Counts only: effects are logged, and a path is the user's.
impl std::fmt::Debug for WipPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} paths", self.paths.len())?;
        for (on, what) in [(self.staged_more, "staged more"), (self.unstaged_more, "unstaged more"), (self.staged_whole, "staged whole")] {
            if on {
                write!(f, " ({what})")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, status: char) -> FileEntry {
        FileEntry::new(path, status)
    }

    #[test]
    fn the_union_is_sorted_and_deduplicated() {
        let staged = [row("b", 'M'), row("c", 'A')];
        let unstaged = [row("a", 'M'), row("b", 'M')];
        let p = WipPaths::of(&staged, &unstaged);
        assert_eq!(p.paths, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
        assert!(!p.is_empty() && !p.staged_more && !p.unstaged_more && !p.staged_whole);
        assert!(WipPaths::of(&[], &[]).is_empty());
    }

    #[test]
    fn a_rename_brings_its_old_path() {
        let mut moved = row("new", 'R');
        moved.renamed_from = Some("old".to_owned());
        let p = WipPaths::of(&[moved], &[]);
        assert_eq!(p.paths, vec![b"new".to_vec(), b"old".to_vec()]);
        assert!(!p.staged_whole);
    }

    #[test]
    fn a_name_that_is_not_utf8_is_used_by_its_bytes() {
        let mut odd = row("a-shown", '?');
        odd.hazard = Some(Hazard::NotUtf8(vec![b'a', 0xFF]));
        assert_eq!(WipPaths::of(&[], &[odd]).paths, vec![vec![b'a', 0xFF]]);
    }

    #[test]
    fn the_cap_marks_a_list_that_was_cut() {
        let rows = [row("a", 'M'), row("b", 'M'), row("c", 'M')];
        let p = WipPaths::of_capped(&rows, &rows[..2], 2);
        assert_eq!((p.staged_more, p.unstaged_more), (true, false));
        assert_eq!(p.paths.len(), 2);
        let p = WipPaths::of_capped(&rows[..1], &rows, 2);
        assert_eq!((p.staged_more, p.unstaged_more), (false, true));
    }

    #[test]
    fn a_rename_without_its_old_name_reads_the_whole_staged_tree() {
        let p = WipPaths::of(&[row("new", 'R')], &[]);
        assert!(p.staged_whole);
        assert!(!WipPaths::of(&[row("new", 'M')], &[]).staged_whole);
    }

    #[test]
    fn debug_prints_counts_only() {
        let mut p = WipPaths::of(&[row("secret/path.txt", 'M')], &[]);
        p.staged_more = true;
        let shown = format!("{p:?}");
        assert_eq!(shown, "1 paths (staged more)");
        assert!(!shown.contains("secret"));
    }
}
