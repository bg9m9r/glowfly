//! The file history finder's rows: the tracked files plus every folder that holds them, a
//! folder shown with a trailing '/'.

/// `paths` (tracked files, '/'-separated) with each folder above them added once as "a/",
/// "a/b/", sorted byte for byte so a folder comes before what it holds.
pub fn with_folders(paths: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(paths.len() + paths.len() / 4);
    for p in &paths {
        out.extend(p.match_indices('/').map(|(i, _)| p[..=i].to_owned()));
    }
    out.extend(paths);
    out.sort_unstable();
    out.dedup();
    out
}

/// The folder a finder row names (without its trailing '/'), or None for a file row.
pub fn folder_of(label: &str) -> Option<&str> {
    label.strip_suffix('/').filter(|f| !f.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_added_once_and_sorted_with_the_files() {
        let paths = vec!["src/ui/a.rs".to_owned(), "README.md".to_owned(), "src/main.rs".to_owned(), "src/ui/b.rs".to_owned()];
        assert_eq!(with_folders(paths), ["README.md", "src/", "src/main.rs", "src/ui/", "src/ui/a.rs", "src/ui/b.rs"]);
        assert_eq!(with_folders(vec!["a.txt".into(), "b.txt".into()]), ["a.txt", "b.txt"], "root files add no folder");
        assert!(with_folders(Vec::new()).is_empty());
    }

    #[test]
    fn folder_rows_are_told_from_file_rows() {
        assert_eq!(folder_of("src/ui/"), Some("src/ui"));
        assert_eq!(folder_of("src/ui/a.rs"), None);
        assert_eq!(folder_of("/"), None);
    }
}
