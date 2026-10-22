//! Putting a re-read of some work-tree paths (`status_paths`, `status_index`) into the lists a
//! whole read gave, so they look as a whole read would.

use super::FileEntry;
use std::collections::HashSet;

/// Whether `path` is one of `covered` or below one ('/' boundary). `fold_case`: core.ignorecase.
pub fn covers(covered: &[String], path: &str, fold_case: bool) -> bool {
    if fold_case {
        let path = path.to_lowercase();
        covered.iter().any(|c| under(&c.to_lowercase(), &path))
    } else {
        covered.iter().any(|c| under(c, path))
    }
}

/// Whether `path` is `dir` or inside it.
fn under(dir: &str, path: &str) -> bool {
    path.strip_prefix(dir).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Replaces the rows of `rows` under `covered` with `fresh` (a `PathsStatus`'s), keeping conflicted ('!')
/// rows and the sort order; a row with the same path as a fresh row is dropped even if not covered.
pub fn merge_unstaged(rows: &mut Vec<FileEntry>, covered: &[String], fresh: Vec<FileEntry>, fold_case: bool) {
    let fold = |s: &str| if fold_case { s.to_lowercase() } else { s.to_owned() };
    let dirs: Vec<String> = covered.iter().map(|c| fold(c)).collect();
    let fresh_paths: HashSet<String> = fresh.iter().map(|f| fold(&f.path)).collect();
    rows.retain(|r| {
        if r.status == '!' {
            return true;
        }
        let path = fold(&r.path);
        !dirs.iter().any(|d| under(d, &path)) && !fresh_paths.contains(&path)
    });
    rows.extend(fresh);
    rows.sort_by(|a, b| a.path.cmp(&b.path));
}

/// Replaces the staged rows under `covered` with `fresh` (an index read's), keeping the sort
/// order. A row whose rename source (`renamed_from`) is covered goes too, since the read paired
/// that path afresh, as does a row with the same path as a fresh row.
pub fn merge_staged(rows: &mut Vec<FileEntry>, covered: &[String], fresh: Vec<FileEntry>, fold_case: bool) {
    let fold = |s: &str| if fold_case { s.to_lowercase() } else { s.to_owned() };
    let dirs: Vec<String> = covered.iter().map(|c| fold(c)).collect();
    let fresh_paths: HashSet<String> = fresh.iter().map(|f| fold(&f.path)).collect();
    let covered_path = |p: &str| dirs.iter().any(|d| under(d, &fold(p)));
    rows.retain(|r| {
        !covered_path(&r.path) && !r.renamed_from.as_deref().is_some_and(covered_path) && !fresh_paths.contains(&fold(&r.path))
    });
    rows.extend(fresh);
    rows.sort_by(|a, b| a.path.cmp(&b.path));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(path: &str, status: char) -> FileEntry {
        FileEntry::new(path, status)
    }

    fn shown(rows: &[FileEntry]) -> Vec<(&str, char)> {
        rows.iter().map(|f| (f.path.as_str(), f.status)).collect()
    }

    #[test]
    fn a_folder_covers_what_is_below_it_only() {
        let c = vec!["a".to_owned()];
        assert!(covers(&c, "a", false));
        assert!(covers(&c, "a/b", false));
        assert!(covers(&c, "a/b/c.txt", false));
        assert!(!covers(&c, "ab", false));
        assert!(!covers(&c, "a.b", false));
        assert!(!covers(&c, "b/a", false));
        assert!(!covers(&[], "a", false));
    }

    #[test]
    fn fresh_rows_replace_the_covered_ones() {
        let mut rows = vec![e("a/x.txt", 'M'), e("a/y.txt", 'D'), e("b.txt", 'M'), e("c.txt", '?')];
        merge_unstaged(&mut rows, &["a".into()], vec![e("a/z.txt", '?'), e("a/x.txt", 'D')], false);
        assert_eq!(shown(&rows), [("a/x.txt", 'D'), ("a/z.txt", '?'), ("b.txt", 'M'), ("c.txt", '?')]);
        // An empty read of a path removes its row.
        merge_unstaged(&mut rows, &["b.txt".into()], Vec::new(), false);
        assert_eq!(shown(&rows), [("a/x.txt", 'D'), ("a/z.txt", '?'), ("c.txt", '?')]);
    }

    #[test]
    fn conflicts_stay_and_the_order_holds() {
        let mut rows = vec![e("a.txt", 'M'), e("f.txt", '!'), e("z.txt", 'M')];
        merge_unstaged(&mut rows, &["f.txt".into(), "a.txt".into()], vec![e("b.txt", '?')], false);
        assert_eq!(shown(&rows), [("b.txt", '?'), ("f.txt", '!'), ("z.txt", 'M')]);
    }

    #[test]
    fn a_row_with_a_fresh_row_s_path_is_dropped_even_outside_covered() {
        let mut rows = vec![e("sub", 'M'), e("x.txt", 'M')];
        merge_unstaged(&mut rows, &["y".into()], vec![e("x.txt", 'D')], false);
        assert_eq!(shown(&rows), [("sub", 'M'), ("x.txt", 'D')]);
    }

    #[test]
    fn case_folds_only_under_ignorecase() {
        let covered = vec!["Readme.md".to_owned()];
        assert!(covers(&covered, "README.md", true));
        assert!(!covers(&covered, "README.md", false));
        let mut rows = vec![e("README.md", 'M')];
        merge_unstaged(&mut rows, &covered, vec![e("Readme.md", 'M')], true);
        assert_eq!(shown(&rows), [("Readme.md", 'M')], "never listed in both spellings");
        let mut rows = vec![e("README.md", 'M')];
        merge_unstaged(&mut rows, &covered, Vec::new(), false);
        assert_eq!(shown(&rows), [("README.md", 'M')]);
    }

    fn renamed(path: &str, from: &str) -> FileEntry {
        FileEntry { renamed_from: Some(from.to_owned()), ..e(path, 'R') }
    }

    #[test]
    fn staged_rows_follow_their_rename_source() {
        let mut rows = vec![e("a.txt", 'M'), renamed("new.txt", "old.txt"), e("z.txt", 'A')];
        // old.txt was read again: the rename it was half of is paired afresh.
        merge_staged(&mut rows, &["old.txt".into()], vec![e("old.txt", 'D')], false);
        assert_eq!(shown(&rows), [("a.txt", 'M'), ("old.txt", 'D'), ("z.txt", 'A')]);
        merge_staged(&mut rows, &["a.txt".into(), "old.txt".into(), "new.txt".into()], vec![renamed("new.txt", "old.txt")], false);
        assert_eq!(shown(&rows), [("new.txt", 'R'), ("z.txt", 'A')], "uncovered rows stay, in order");
        merge_staged(&mut rows, &["dir".into()], vec![e("dir/b.txt", 'A'), e("dir/a.txt", 'A')], false);
        assert_eq!(shown(&rows), [("dir/a.txt", 'A'), ("dir/b.txt", 'A'), ("new.txt", 'R'), ("z.txt", 'A')]);
    }

    #[test]
    fn staged_rows_fold_case_only_under_ignorecase() {
        let mut rows = vec![e("README.md", 'M'), renamed("b.txt", "A.txt")];
        merge_staged(&mut rows, &["Readme.md".into(), "a.txt".into()], Vec::new(), true);
        assert!(rows.is_empty(), "{:?}", shown(&rows));
        let mut rows = vec![e("README.md", 'M'), renamed("b.txt", "A.txt")];
        merge_staged(&mut rows, &["Readme.md".into(), "a.txt".into()], Vec::new(), false);
        assert_eq!(shown(&rows), [("README.md", 'M'), ("b.txt", 'R')]);
    }
}
