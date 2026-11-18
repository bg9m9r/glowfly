//! The files a repository's config may include (`stamp::include_entries`): a relative include
//! is looked up beside the files found before it, and the list stays bounded however many
//! includes the config holds.

use super::super::{ConfigCommand, scan_now};
use super::*;
use crate::testutil::*;

/// Appends one `[include]` section naming `paths` to the repository's config.
fn include(dir: &Path, paths: impl Iterator<Item = String>) {
    let config = dir.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    let lines: String = paths.map(|p| format!("\tpath = {p}\n")).collect();
    std::fs::write(&config, format!("{text}[include]\n{lines}")).unwrap();
}

#[test]
fn a_relative_include_is_looked_up_beside_the_files_before_it() {
    let dir = scratch_repo("stamp-include-beside");
    include(&dir, ["sub/a.cfg".to_owned(), "b.cfg".to_owned()].into_iter());
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    let git_dir = repo.path();
    assert_eq!(found[0].paths, [git_dir.join("sub/a.cfg")]);
    assert_eq!(found[1].paths, [git_dir.join("b.cfg"), git_dir.join("sub").join("b.cfg")]);
    assert!(found.iter().all(|i| !i.unsure));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn many_includes_in_one_folder_each_name_one_file() {
    let dir = scratch_repo("stamp-include-many");
    include(&dir, (0..200).map(|i| format!("many{i}.cfg")));
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    assert_eq!(found.len(), 200);
    assert!(found.iter().all(|i| i.paths.len() == 1 && !i.unsure));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn includes_in_more_folders_than_are_listed_are_a_reason_to_ask() {
    let dir = scratch_repo("stamp-include-folders");
    // Each in a folder of its own, so each adds places the next one may be in.
    include(&dir, (0..40).map(|i| format!("f{i}/x.cfg")));
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    assert_eq!(found.len(), 40);
    assert!(found.iter().all(|i| i.paths.len() <= MAX_FOLDERS), "{:?}", found.iter().map(|i| i.paths.len()).collect::<Vec<_>>());
    assert!(!found[0].unsure && found[39].unsure);
    // None is in the working tree, but where the later ones are can't be told.
    let scan = scan_now(&repo);
    assert!(scan.config.contains(&ConfigCommand { key: "include.path".into(), value: "f39/x.cfg".into() }), "{:?}", scan.config);
    assert!(!scan.config.contains(&ConfigCommand { key: "include.path".into(), value: "f0/x.cfg".into() }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn includes_past_the_most_listed_are_a_reason_to_ask() {
    let dir = scratch_repo("stamp-include-count");
    include(&dir, (0..=MAX_INCLUDES).map(|i| format!("many{i}.cfg")));
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    assert_eq!(found.len(), MAX_INCLUDES + 1);
    assert!(!found[MAX_INCLUDES - 1].unsure);
    assert!(found[MAX_INCLUDES].unsure && found[MAX_INCLUDES].paths.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
