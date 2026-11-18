//! The files a repository's config may include (`stamp::include_entries`): a relative include
//! is looked up beside the files found before it, the list stays bounded however many includes
//! the config holds, and an include past a limit makes the scan follow the whole working tree
//! and never reuse its stamp.

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

fn asks_about(scan: &TrustScan, value: &str) -> bool {
    scan.config.contains(&ConfigCommand { key: "include.path".into(), value: value.into() })
}

#[test]
fn a_relative_include_is_looked_up_beside_the_files_before_it() {
    let dir = scratch_repo("stamp-include-beside");
    let other = git_spelling(&dir.join("other"));
    include(&dir, ["sub/a.cfg".to_owned(), "b.cfg".to_owned(), format!("{other}/c.cfg"), "d.cfg".to_owned()].into_iter());
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    let git_dir = repo.path();
    assert_eq!(found[0].paths, [git_dir.join("sub/a.cfg")]);
    assert_eq!(found[1].paths, [git_dir.join("b.cfg"), git_dir.join("sub").join("b.cfg")]);
    assert_eq!(found[2].paths, [dir.join("other").join("c.cfg")]);
    // Beside the file named in full too.
    assert_eq!(found[3].paths, [git_dir.join("d.cfg"), git_dir.join("sub").join("d.cfg"), dir.join("other").join("d.cfg")]);
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
    // None is in the working tree, so none is a reason to ask and the stamp holds.
    let scan = scan_now(&repo);
    assert!(scan.is_empty(), "{:?}", scan.config);
    assert_eq!(stamp(&repo, &scan), stamp(&repo, &scan));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn includes_in_more_folders_than_are_listed_follow_the_whole_working_tree() {
    let dir = scratch_repo("stamp-include-folders");
    // Each in a folder of its own, so each adds places the next one may be in.
    include(&dir, (0..10).map(|i| format!("f{i}/x.cfg")));
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    let listed: Vec<(usize, bool)> = found.iter().map(|i| (i.paths.len(), i.unsure)).collect();
    // The sixth fills the folders; its own places were all known before it.
    let sure = [1, 2, 4, 8, 16, 32].map(|n| (n, false));
    assert_eq!(listed, [&sure[..], &[(0, true); 4]].concat());
    let scan = scan_now(&repo);
    assert!(!asks_about(&scan, "f5/x.cfg") && asks_about(&scan, "f6/x.cfg") && asks_about(&scan, "f9/x.cfg"), "{:?}", scan.config);
    assert_eq!(scan.follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), [""]);
    assert_ne!(stamp(&repo, &scan), stamp(&repo, &scan));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn relative_includes_past_the_most_listed_are_not_listed() {
    let dir = scratch_repo("stamp-include-count");
    let tracked = git_spelling(&dir.join("tracked.cfg"));
    include(&dir, (0..=MAX_INCLUDES).map(|i| format!("many{i}.cfg")).chain([tracked.clone()]));
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    assert_eq!(found.len(), MAX_INCLUDES + 2);
    assert!(!found[MAX_INCLUDES - 1].unsure && found[MAX_INCLUDES - 1].paths.len() == 1);
    assert!(found[MAX_INCLUDES].unsure && found[MAX_INCLUDES].paths.is_empty());
    // One named in full is one place however many came before it.
    assert_eq!((found[MAX_INCLUDES + 1].paths.as_slice(), found[MAX_INCLUDES + 1].unsure), (&[dir.join("tracked.cfg")][..], false));
    let scan = scan_now(&repo);
    assert!(asks_about(&scan, &format!("many{MAX_INCLUDES}.cfg")) && asks_about(&scan, &tracked), "{:?}", scan.config);
    assert_ne!(stamp(&repo, &scan), stamp(&repo, &scan));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_past_the_longest_followed_is_not_listed() {
    let dir = scratch_repo("stamp-include-long");
    // Under a condition that isn't met, so libgit2 lists the settings without opening the files.
    let config = dir.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    // The second fits as a value, but not joined to the git dir.
    let values = ["a".repeat(MAX_PATH_BYTES + 1), "b".repeat(MAX_PATH_BYTES - 8), "after.cfg".to_owned()];
    let lines: String = values
        .iter()
        .map(|v| {
            format!(
                "	path = {v}
"
            )
        })
        .collect();
    std::fs::write(
        &config,
        format!(
            "{text}[includeIf \"gitdir:/nowhere/\"]
{lines}"
        ),
    )
    .unwrap();
    let repo = Repository::open(&dir).unwrap();
    let found = include_entries(&repo);
    let listed: Vec<(usize, bool)> = found.iter().map(|i| (i.paths.len(), i.unsure)).collect();
    assert_eq!(listed, [(0, true), (0, true), (1, false)]);
    let scan = scan_now(&repo);
    assert_eq!(scan.follows.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), [""]);
    assert_ne!(stamp(&repo, &scan), stamp(&repo, &scan));
    let _ = std::fs::remove_dir_all(&dir);
}
