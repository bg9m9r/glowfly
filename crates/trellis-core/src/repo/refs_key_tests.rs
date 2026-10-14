use super::refs_key;
use crate::testutil::*;

#[test]
fn refs_key_follows_refs_config_and_stashes_but_not_the_working_tree() {
    let dir = scratch_repo("refs-key");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let key = || refs_key(&dir).unwrap();
    let k0 = key();
    write(&dir, "a.txt", "two\n");
    write(&dir, "new.txt", "new\n");
    git(&dir, &["add", "new.txt"]);
    assert_eq!(key(), k0, "staging and editing are status changes");

    git(&dir, &["stash", "-q"]);
    let k1 = key();
    assert_ne!(k1, k0, "a stash");
    write(&dir, "a.txt", "three\n");
    git(&dir, &["stash", "-q"]);
    let k2 = key();
    git(&dir, &["stash", "drop", "-q", "stash@{1}"]);
    assert_ne!(key(), k2, "dropping an older stash rewrites only the reflog");

    let k3 = key();
    git(&dir, &["branch", "side"]);
    assert_ne!(key(), k3, "a new branch");
    let k4 = key();
    git(&dir, &["checkout", "-q", "side"]);
    assert_ne!(key(), k4, "HEAD moved to another branch");
    let k5 = key();
    git(&dir, &["config", "branch.side.description", "x"]);
    assert_ne!(key(), k5, "config");
    let _ = std::fs::remove_dir_all(&dir);
}
