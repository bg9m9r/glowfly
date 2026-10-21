//! The common git dir that groups a repository's worktrees: the same for the main worktree and a
//! linked one, read without loading (`CommonDir::at`), and the pure name and comparison rules.

use super::*;
use crate::paths::FolderId;
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn one_commit(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn common(dir: &Path) -> CommonDir {
    load(dir, &LoadOptions::all(10)).unwrap().common
}

#[test]
fn worktrees_of_one_repository_share_it() {
    let main = one_commit("common-main");
    let linked = main.with_file_name(format!("{}-linked", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    let other = one_commit("common-other");

    let (a, b) = (common(&main), common(&linked));
    assert!(a.same(&b), "{a:?} vs {b:?}");
    let main_name = main.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!((a.name.as_str(), b.name.as_str()), (main_name.as_str(), main_name.as_str()));
    let read = CommonDir::at(&linked).expect("read without loading");
    assert!(read.same(&b) && read.name == main_name);
    assert!(!common(&other).same(&a), "another repository");

    assert_eq!(CommonDir::at(Path::new("relative/folder")), None, "never resolved against the working folder");
    let gone = main.with_file_name("trellis-common-no-such-repo");
    assert_eq!(CommonDir::at(&gone), None, "a missing folder");
    for d in [&linked, &main, &other] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn repository_names() {
    let w = Path::new("w");
    assert_eq!(repo_name_of(&w.join("app").join(".git")), "app");
    assert_eq!(repo_name_of(&w.join("app.git")), "app");
    assert_eq!(repo_name_of(&w.join("plain")), "plain");
    assert_eq!(repo_name_of(&w.join(".git")), "w");
}

#[test]
fn same_by_identity_or_spelling() {
    let c = |dir: PathBuf, folder: Option<FolderId>| CommonDir { dir, folder, name: "app".into() };
    let base = crate::paths::temp_dir();
    let id = Some(FolderId::for_tests(1));
    assert!(c(base.join("app/.git"), id).same(&c(base.join("APP~1/.git"), id)), "one identity, two spellings");
    assert!(!c(base.join("app/.git"), id).same(&c(base.join("other/.git"), Some(FolderId::for_tests(2)))));
    assert!(c(base.join("app/.git"), None).same(&c(base.join("app/./.git/"), None)), "spelling without identities");
    assert!(!c(base.join("app/.git"), None).same(&c(base.join("App/.git"), None)), "case is never folded by spelling");
}
