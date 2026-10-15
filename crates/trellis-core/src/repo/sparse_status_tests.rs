use super::status_cli;
use super::*;
use crate::testutil::*;

#[test]
fn files_a_sparse_checkout_leaves_out_are_not_deleted() {
    let dir = scratch_repo("sparse-status");
    std::fs::create_dir_all(dir.join("src/a")).unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    write(&dir, "r.txt", "r\n");
    write(&dir, "src/a/x.txt", "x\n");
    write(&dir, "docs/d.txt", "d\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["sparse-checkout", "set", "--cone", "--no-sparse-index", "src"]);
    assert!(!dir.join("docs/d.txt").exists());

    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    assert!(unstaged.is_empty() && staged.is_empty(), "{unstaged:?} {staged:?}");

    std::fs::remove_file(dir.join("src/a/x.txt")).unwrap();
    let (unstaged, staged, _) = status_at(&dir, false).unwrap();
    let paths: Vec<(&str, char)> = unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(paths, [("src/a/x.txt", 'D')], "a deleted file inside the paths still shows");
    assert!(staged.is_empty());

    let (u2, s2, _) = status_at(&dir, true).unwrap();
    assert_eq!((u2.clone(), s2.clone()), (unstaged.clone(), staged.clone()));
    let (u3, s3) = status_cli::status(&dir).expect("git's own status");
    assert_eq!((u3, s3), (unstaged, staged), "git's status agrees");
    let _ = std::fs::remove_dir_all(&dir);
}
