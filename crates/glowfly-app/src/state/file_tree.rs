//! A changed-file list grouped in folders: the rows the tree shows, in order, and walking its
//! files in that order. Pure: paths are git's ('/'-separated on every OS) and compared by
//! bytes, never case-folded.

use std::collections::BTreeMap;

/// One row of a changed-file list shown as a tree.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeRow {
    /// A folder (a compacted chain is one row): `path` is the deepest folder's full path, no
    /// trailing '/'; `label` the part shown ("search" or "a/b/c"); `files` how many files are
    /// under it at any depth.
    Folder { path: String, label: String, depth: usize, open: bool, files: usize },
    /// The file at `index` in the list's own order (for its FileEntry, status and line counts).
    File { index: usize, depth: usize },
}

/// A folder level: its subfolders by name (byte order) and its files as (name, index).
#[derive(Default)]
struct Node<'a> {
    folders: BTreeMap<&'a str, Node<'a>>,
    files: Vec<(&'a str, usize)>,
}

impl<'a> Node<'a> {
    fn build(paths: &[&'a str]) -> Node<'a> {
        let mut root = Node::default();
        for (index, path) in paths.iter().enumerate() {
            // A slash-ended path is a leaf named with its slash.
            let (dirs, name) = match path.strip_suffix('/') {
                Some(body) => match body.rfind('/') {
                    Some(k) => (&path[..k], &path[k + 1..]),
                    None => ("", *path),
                },
                None => match path.rfind('/') {
                    Some(k) => (&path[..k], &path[k + 1..]),
                    None => ("", *path),
                },
            };
            let mut node = &mut root;
            if !dirs.is_empty() {
                for part in dirs.split('/') {
                    node = node.folders.entry(part).or_default();
                }
            }
            node.files.push((name, index));
        }
        root.sort();
        root
    }

    fn sort(&mut self) {
        self.files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()).then(a.1.cmp(&b.1)));
        self.folders.values_mut().for_each(Node::sort);
    }

    fn count(&self) -> usize {
        self.files.len() + self.folders.values().map(Node::count).sum::<usize>()
    }

    /// Follows a chain of folders that hold no files and exactly one subfolder: the label
    /// added to `name` and the node the chain ends at.
    fn compact(&self, name: &str) -> (String, &Node<'a>) {
        let (mut label, mut node) = (name.to_owned(), self);
        while node.files.is_empty() && node.folders.len() == 1 {
            let (child, next) = node.folders.iter().next().expect("one folder");
            label.push('/');
            label.push_str(child);
            node = next;
        }
        (label, node)
    }
}

/// Visits the files of `node` in tree order (folders first, then files), skipping what lies
/// under a folder `closed` names when `fold` is set; `row` sees every row.
fn walk<'a>(node: &Node<'a>, prefix: &str, depth: usize, closed: &dyn Fn(&str) -> bool, row: &mut dyn FnMut(TreeRow)) {
    for (name, child) in &node.folders {
        let (label, end) = child.compact(name);
        let path = if prefix.is_empty() { label.clone() } else { format!("{prefix}/{label}") };
        let open = !closed(&path);
        row(TreeRow::Folder { path: path.clone(), label, depth, open, files: end.count() });
        if open {
            walk(end, &path, depth + 1, closed, row);
        }
    }
    for &(_, index) in &node.files {
        row(TreeRow::File { index, depth });
    }
}

/// The rows of `paths` as a tree, in display order: at each level folders first, then files,
/// each by the bytes of its name. A closed folder's row has `open: false` and none of its
/// descendants follow it. File rows name their place in `paths`, whatever its order.
pub fn tree_rows(paths: &[&str], closed: impl Fn(&str) -> bool) -> Vec<TreeRow> {
    let root = Node::build(paths);
    let mut rows = Vec::with_capacity(paths.len());
    walk(&root, "", 0, &closed, &mut |r| rows.push(r));
    rows
}

/// Every file index of `paths` in tree order, folds ignored.
pub fn file_order(paths: &[&str]) -> Vec<usize> {
    let root = Node::build(paths);
    let mut order = Vec::with_capacity(paths.len());
    walk(&root, "", 0, &|_| false, &mut |r| {
        if let TreeRow::File { index, .. } = r {
            order.push(index);
        }
    });
    order
}

/// Every file index of `paths` in tree order, each with whether it is shown with `closed`'s
/// folds (a fold that names no row, a stale key, hides nothing).
pub fn shown_order(paths: &[&str], closed: impl Fn(&str) -> bool) -> Vec<(usize, bool)> {
    let mut shown = vec![false; paths.len()];
    for r in tree_rows(paths, closed) {
        if let TreeRow::File { index, .. } = r {
            shown[index] = true;
        }
    }
    file_order(paths).into_iter().map(|i| (i, shown[i])).collect()
}

/// Whether `path` lies under the folder `folder`: "src/a" is under "src"; "src2/a", "src.txt"
/// and "src" itself are not.
pub fn under(folder: &str, path: &str) -> bool {
    path.len() > folder.len() && path.starts_with(folder) && path.as_bytes()[folder.len()] == b'/'
}

/// The last component of `path`, a trailing '/' kept on a slash-ended leaf.
pub fn file_name(path: &str) -> &str {
    let body = path.strip_suffix('/').unwrap_or(path);
    match body.rfind('/') {
        Some(k) => &path[k + 1..],
        None => path,
    }
}

/// The folders `path` lies in, outermost first: "a/b/c.txt" gives "a", "a/b".
pub fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    let body = path.strip_suffix('/').unwrap_or(path);
    body.match_indices('/').map(move |(k, _)| &path[..k])
}

#[cfg(test)]
#[path = "file_tree_tests.rs"]
mod tests;
