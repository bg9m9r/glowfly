//! Index paths one file system can't keep apart. The twins are index entries made with
//! `update-index --cacheinfo`, so every OS can hold them whatever its file system folds.

use super::bytes_tests::blob;
use super::*;
use crate::testutil::*;
use std::path::{Path, PathBuf};

const COMPOSED: &str = "caf\u{e9}.txt";
const DECOMPOSED: &str = "cafe\u{301}.txt";

fn add_entry(dir: &Path, path: &str, text: &str) {
    let id = blob(dir, text);
    git(dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{id},{path}")]);
}

/// A commit holding `paths` (contents named after them), with `core.ignorecase` as `ignorecase`.
fn twins(name: &str, paths: &[&str], ignorecase: bool) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.ignorecase", "false"]);
    // Git on macOS would otherwise compose the decomposed name given on its command line.
    git(&dir, &["config", "core.precomposeunicode", "false"]);
    write(&dir, "other.txt", "other\n");
    git(&dir, &["add", "other.txt"]);
    for p in paths {
        add_entry(&dir, p, &format!("{p}\n"));
    }
    git(&dir, &["commit", "-q", "-m", "twins"]);
    git(&dir, &["config", "core.ignorecase", if ignorecase { "true" } else { "false" }]);
    dir
}

#[test]
fn collisions_group_case_and_normalization_twins() {
    let dir = twins("collide-groups", &["a.txt", "A.txt", COMPOSED, DECOMPOSED, "Docs/x.md", "docs/y.md"], false);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let found: Vec<(Vec<&str>, bool, bool)> =
        snap.collisions.iter().map(|c| (c.paths.iter().map(String::as_str).collect(), c.case, c.unicode)).collect();
    assert_eq!(found, [(vec!["A.txt", "a.txt"], true, false), (vec![DECOMPOSED, COMPOSED], false, true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_collision_shares_a_file_when_the_file_system_or_git_folds() {
    let case = Collision { paths: vec!["A".into(), "a".into()], case: true, unicode: false };
    let nfd = Collision { paths: vec![DECOMPOSED.into(), COMPOSED.into()], case: false, unicode: true };
    let f = |case, unicode| Folding { case, unicode };
    assert!(case.shares_a_file(f(Some(true), Some(false)), false));
    assert!(!case.shares_a_file(f(Some(false), Some(true)), false));
    assert!(case.shares_a_file(f(Some(false), Some(false)), true), "git folds with core.ignorecase");
    assert!(case.shares_a_file(f(None, None), false), "unknown counts as folding");
    assert!(nfd.shares_a_file(f(Some(false), Some(true)), false));
    assert!(!nfd.shares_a_file(f(Some(true), Some(false)), true));
}

/// What the CI runners' temporary folders do: NTFS and APFS ignore case; APFS also ignores
/// Unicode normalization; ext4 does neither.
#[test]
fn the_probe_asks_the_file_system() {
    let dir = scratch_repo("collide-probe");
    let f = probe(&dir.join(".git"));
    let macos = cfg!(target_os = "macos");
    assert_eq!(f, Folding { case: Some(cfg!(windows) || macos), unicode: Some(macos) });
    let left: Vec<_> =
        std::fs::read_dir(dir.join(".git")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains("probe")).collect();
    assert!(left.is_empty(), "the probe cleans up");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn destructive_actions_on_a_shared_file_are_refused() {
    // core.ignorecase makes git treat them as one file on every file system.
    let dir = twins("collide-refuse", &["a.txt", "A.txt"], true);
    write(&dir, "a.txt", "edited\n");
    let (unstaged, _, _) = status_at(&dir, false).unwrap();
    let marked = unstaged.iter().find(|f| f.path == "a.txt").expect("a.txt is listed as changed");
    assert_eq!(marked.hazard, Some(Hazard::Twins(vec!["A.txt".into()])));

    let refused = |r: crate::error::Result<()>| r.unwrap_err().to_string();
    assert!(refused(discard_file(&dir, "a.txt")).contains("can't tell it from A.txt"));
    assert!(refused(delete_file(&dir, "a.txt", Removal::Permanent).map(|_| ())).contains("can't delete"));
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    assert!(refused(restore_file(&dir, head, "A.txt", true).map(|_| ())).contains("can't restore"));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "edited\n", "nothing was written");
    // Other files are unaffected.
    write(&dir, "other.txt", "changed\n");
    discard_file(&dir, "other.txt").unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twins_on_a_case_sensitive_file_system_stay_usable() {
    let dir = twins("collide-sensitive", &["a.txt", "A.txt"], false);
    if probe(&dir.join(".git")).case != Some(false) {
        // One file on disk here: covered by the test above.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    assert_eq!(snap.collisions.len(), 1, "still reported, for other systems' sake");
    write(&dir, "a.txt", "edited\n");
    write(&dir, "A.txt", "A.txt\n");
    let (unstaged, _, _) = status_at(&dir, false).unwrap();
    assert!(unstaged.iter().all(|f| f.hazard.is_none()), "{unstaged:?}");
    discard_file(&dir, "a.txt").unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a.txt\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_disagreeing_ignorecase_is_reported() {
    let dir = scratch_repo("collide-mismatch");
    let folds = probe(&dir.join(".git")).case == Some(true);
    git(&dir, &["config", "core.ignorecase", if folds { "false" } else { "true" }]);
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    assert!(snap.case_mismatch().is_some());
    git(&dir, &["config", "core.ignorecase", if folds { "true" } else { "false" }]);
    assert_eq!(load(&dir, &LoadOptions::all(10)).unwrap().case_mismatch(), None);
    let _ = std::fs::remove_dir_all(&dir);
}
