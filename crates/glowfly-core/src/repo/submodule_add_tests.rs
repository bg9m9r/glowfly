//! The disk and index checks of a new submodule's folder against real repositories.

use super::*;
use crate::submodule_ops::AddPathProblem as P;
use crate::testutil::*;
use std::path::Path;

/// A repository with a.txt and docs/guide.md committed.
fn repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    write(&dir, "docs/guide.md", "guide\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn check(dir: &Path, rel: &str) -> Result<Vec<String>, P> {
    check_submodule_path(dir, rel)
}

#[test]
fn index_entries_refuse() {
    let dir = repo("sm-add-index");
    assert_eq!(check(&dir, "a.txt"), Err(P::InIndex { gitlink: false }));
    assert_eq!(check(&dir, "docs"), Err(P::TrackedInside));
    assert_eq!(check(&dir, "a.txt/x"), Err(P::UnderTrackedFile("a.txt".into())));
    // A name that only starts like a tracked one is fine.
    assert_eq!(check(&dir, "doc"), Ok(Vec::new()));
    assert_eq!(check(&dir, "docs2/x"), Ok(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_gitlink_refuses() {
    let (parent, _sub) = with_submodule("sm-add-gitlink");
    assert_eq!(check(&parent, "libs/lib"), Err(P::InIndex { gitlink: true }));
    let lib = parent.with_file_name(format!("trellis-test-sm-add-gitlink-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lib);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn what_is_on_disk() {
    let dir = repo("sm-add-disk");
    std::fs::create_dir_all(dir.join("untracked")).unwrap();
    write(&dir, "untracked/f.txt", "x\n");
    assert_eq!(check(&dir, "untracked"), Err(P::Exists { file: false }));
    write(&dir, "loose.txt", "x\n");
    assert_eq!(check(&dir, "loose.txt"), Err(P::Exists { file: true }));
    assert_eq!(check(&dir, "loose.txt/sub"), Err(P::ParentFile("loose.txt".into())));
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    assert_eq!(check(&dir, "empty"), Ok(Vec::new()), "git clones into an empty folder");
    assert_eq!(check(&dir, "libs/new"), Ok(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignored_and_left_over_modules_refuse() {
    let dir = repo("sm-add-ignored");
    write(&dir, ".gitignore", "/vendored\n");
    assert_eq!(check(&dir, "vendored"), Err(P::Ignored));
    std::fs::create_dir_all(dir.join(".git").join("modules").join("libs").join("lib")).unwrap();
    match check(&dir, "libs/lib") {
        Err(P::ModuleExists(d)) => assert!(d.ends_with(Path::new("modules").join("libs").join("lib")), "{}", d.display()),
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_parent_refuses() {
    let dir = repo("sm-add-link");
    let real = dir.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let Some(link) = alias_of(&real) else {
        eprintln!("skipped: this account can't make folder links");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let name = link.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(check(&dir, &format!("{name}/lib")), Err(P::ParentLink(name.clone())));
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn windows_names_warn_elsewhere() {
    if crate::names::blocks_here() {
        return; // parse_add_path refuses them on Windows before this check runs
    }
    let dir = repo("sm-add-aux");
    let warnings = check(&dir, "aux").unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("won't check out on Windows"), "{warnings:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_repository_is_unreadable() {
    let dir = crate::paths::temp_dir().join(format!("trellis-test-sm-add-none-{}", std::process::id()));
    assert!(matches!(check(&dir, "lib"), Err(P::Unreadable(_))));
}
