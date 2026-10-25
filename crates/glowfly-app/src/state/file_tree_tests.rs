use super::*;

fn folder(path: &str, label: &str, depth: usize, open: bool, files: usize) -> TreeRow {
    TreeRow::Folder { path: path.into(), label: label.into(), depth, open, files }
}

fn file(index: usize, depth: usize) -> TreeRow {
    TreeRow::File { index, depth }
}

fn rows(paths: &[&str]) -> Vec<TreeRow> {
    tree_rows(paths, |_| false)
}

fn labels(rows: &[TreeRow]) -> Vec<String> {
    rows.iter()
        .map(|r| match r {
            TreeRow::Folder { label, .. } => format!("{label}/"),
            TreeRow::File { index, .. } => index.to_string(),
        })
        .collect()
}

#[test]
fn folders_come_before_files_at_every_level() {
    let paths = ["b.txt", "a/x", "c/y"];
    assert_eq!(rows(&paths), vec![folder("a", "a", 0, true, 1), file(1, 1), folder("c", "c", 0, true, 1), file(2, 1), file(0, 0)]);
}

#[test]
fn names_sort_by_bytes_without_case_folding() {
    let paths = ["B/1", "a/2", "Src/3", "src/4"];
    assert_eq!(labels(&rows(&paths)), ["B/", "0", "Src/", "2", "a/", "1", "src/", "3"]);
}

#[test]
fn a_folder_comes_before_a_file_git_lists_first() {
    let paths = ["a.txt", "a/b"];
    assert_eq!(rows(&paths), vec![folder("a", "a", 0, true, 1), file(1, 1), file(0, 0)]);
}

#[test]
fn single_folder_chains_are_compacted() {
    let paths = ["src/search/index.rs", "src/search/query.rs"];
    assert_eq!(rows(&paths), vec![folder("src/search", "src/search", 0, true, 2), file(0, 1), file(1, 1)]);
    let deep = ["a/b/c/x.txt"];
    assert_eq!(rows(&deep), vec![folder("a/b/c", "a/b/c", 0, true, 1), file(0, 1)]);
    // A folder holding a file of its own keeps its subfolder as a row.
    let mixed = ["src/lib.rs", "src/search/index.rs"];
    assert_eq!(rows(&mixed), vec![folder("src", "src", 0, true, 2), folder("src/search", "search", 1, true, 1), file(1, 2), file(0, 1)]);
    // Two subfolders: no merge, but each child chain still compacts.
    let two = ["src/a/x/1", "src/b/2"];
    assert_eq!(
        rows(&two),
        vec![
            folder("src", "src", 0, true, 2),
            folder("src/a/x", "a/x", 1, true, 1),
            file(0, 2),
            folder("src/b", "b", 1, true, 1),
            file(1, 2)
        ]
    );
}

#[test]
fn file_rows_keep_their_index_whatever_the_input_order() {
    let paths = ["z.txt", "lib/b.rs", "app/main.rs", "lib/a.rs"];
    assert_eq!(
        rows(&paths),
        vec![folder("app", "app", 0, true, 1), file(2, 1), folder("lib", "lib", 0, true, 2), file(3, 1), file(1, 1), file(0, 0)]
    );
}

#[test]
fn closed_folders_hide_their_descendants() {
    let paths = ["src/a.rs", "src/ui/b.rs", "top.txt"];
    let closed = tree_rows(&paths, |f| f == "src");
    assert_eq!(closed, vec![folder("src", "src", 0, false, 2), file(2, 0)]);
    // A closed parent hides an open child.
    let parent = tree_rows(&paths, |f| f == "src");
    assert!(!parent.iter().any(|r| matches!(r, TreeRow::Folder { path, .. } if path == "src/ui")));
    // A compacted chain is closed by its full path.
    let chain = ["src/search/index.rs", "src/search/query.rs"];
    assert_eq!(tree_rows(&chain, |f| f == "src/search"), vec![folder("src/search", "src/search", 0, false, 2)]);
}

#[test]
fn edge_paths() {
    // A deleted file "a" and an added "a/b".
    assert_eq!(rows(&["a", "a/b"]), vec![folder("a", "a", 0, true, 1), file(1, 1), file(0, 0)]);
    // A slash-ended path is a leaf named with its slash.
    assert_eq!(rows(&["vendor/lib/"]), vec![folder("vendor", "vendor", 0, true, 1), file(0, 1)]);
    assert_eq!(file_name("vendor/lib/"), "lib/");
    assert_eq!(rows(&["sub/"]), vec![file(0, 0)]);
    assert!(rows(&[]).is_empty());
}

#[test]
fn file_order_ignores_folds_and_matches_the_rows() {
    let paths = ["z.txt", "lib/b.rs", "app/main.rs", "lib/a.rs", "lib/x/y.rs"];
    let from_rows: Vec<usize> = rows(&paths)
        .into_iter()
        .filter_map(|r| match r {
            TreeRow::File { index, .. } => Some(index),
            _ => None,
        })
        .collect();
    assert_eq!(file_order(&paths), from_rows);
    assert_eq!(file_order(&paths), vec![2, 4, 3, 1, 0]);
}

#[test]
fn step_visible_skips_closed_folders() {
    // Tree order: app/main.rs (2), lib/a.rs (3), lib/b.rs (1), z.txt (0).
    let paths = ["z.txt", "lib/b.rs", "app/main.rs", "lib/a.rs"];
    let closed = |f: &str| f == "lib";
    assert_eq!(step_visible(&paths, closed, None, true), Some(2));
    assert_eq!(step_visible(&paths, closed, Some(2), true), Some(0), "skips lib's files");
    assert_eq!(step_visible(&paths, closed, Some(0), false), Some(2));
    // From a hidden file: on to the visible neighbours in tree order.
    assert_eq!(step_visible(&paths, closed, Some(3), true), Some(0));
    assert_eq!(step_visible(&paths, closed, Some(3), false), Some(2));
    // The ends.
    assert_eq!(step_visible(&paths, closed, Some(0), true), None);
    assert_eq!(step_visible(&paths, closed, Some(2), false), None);
    assert_eq!(step_visible(&paths, |f| f == "lib" || f == "app", Some(0), false), None);
    assert_eq!(step_visible(&[], |_| false, None, true), None);
}

#[test]
fn prefixes_and_names() {
    assert!(under("src", "src/a"));
    assert!(!under("src", "src2/a"));
    assert!(!under("src", "src.txt"));
    assert!(!under("src", "src"));
    assert!(!under("src", "Src/a"));
    assert_eq!(ancestors("a/b/c.txt").collect::<Vec<_>>(), ["a", "a/b"]);
    assert_eq!(ancestors("top.txt").count(), 0);
    assert_eq!(file_name("a/b/c.txt"), "c.txt");
    assert_eq!(file_name("c.txt"), "c.txt");
}
