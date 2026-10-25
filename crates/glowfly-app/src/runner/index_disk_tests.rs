//! The index thread's store: a finished build is saved and read back by the next session, a
//! saved index of another history is the base, a damaged file is rebuilt and replaced, and
//! without a store nothing is read or saved.

use super::*;
use glowfly_core::testkit::{git, set_identity};

/// A repository with one commit, in a fresh temporary folder, and the key of its history.
fn disk_repo(name: &str) -> (PathBuf, IndexKey) {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-index-disk-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    set_identity(&dir);
    (dir.clone(), commit(&dir, "one"))
}

/// Adds a commit and gives the new history's key.
fn commit(dir: &Path, msg: &str) -> IndexKey {
    git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
    let head = git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap();
    IndexKey { starts: vec![head], shallow: Vec::new() }
}

fn store_dir(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-index-disk-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn disk(store: &Path) -> Disk {
    let d = Disk::default();
    d.attach(store.to_path_buf());
    d
}

fn run(d: &Disk, dir: &Path, key: &IndexKey) -> Done {
    d.build(dir, key, None, &mut |_| {}, &|| false).unwrap().expect("not stopped")
}

fn oids(ix: &CommitIndex) -> Vec<git2::Oid> {
    (0..ix.len()).map(|p| ix.oid_at(p).unwrap()).collect()
}

#[test]
fn a_finished_build_is_saved_and_the_next_session_reads_it() {
    let (dir, key) = disk_repo("next-session");
    let store = store_dir("next-session");
    let first = disk(&store);
    let done = run(&first, &dir, &key);
    assert_eq!(done.source, Source::Built);
    let file = done.save.clone().expect("worth saving");
    first.save(&file, &done.index);
    assert!(file.path.exists());

    let again = run(&disk(&store), &dir, &key);
    assert_eq!(again.source, Source::Loaded);
    assert_eq!(oids(&again.index), oids(&done.index));
    assert!(again.save.is_none(), "nothing new to save");
    let _ = std::fs::remove_dir_all(&store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_saved_index_for_another_history_is_the_base() {
    let (dir, key) = disk_repo("base");
    let store = store_dir("base");
    let first = disk(&store);
    let done = run(&first, &dir, &key);
    first.save(&done.save.unwrap(), &done.index);

    let key = commit(&dir, "two");
    let again = run(&disk(&store), &dir, &key);
    assert_eq!(again.source, Source::Updated);
    let fresh = commit_index::build(&dir, &key, None, &mut |_| {}, &|| false).unwrap().unwrap();
    assert_eq!(oids(&again.index), oids(&fresh));
    assert!(again.save.is_none(), "one new commit is fewer than RESAVE_COMMITS");
    let _ = std::fs::remove_dir_all(&store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_is_built_again_and_replaced() {
    let (dir, key) = disk_repo("damaged");
    let store = store_dir("damaged");
    let file = Store::new(store.clone()).file_of(&dir).unwrap();
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(&file.path, b"not an index at all").unwrap();
    let d = disk(&store);
    let done = run(&d, &dir, &key);
    assert_eq!(done.source, Source::Built);
    assert_eq!(done.save.as_ref(), Some(&file));
    d.save(&file, &done.index);
    assert!(Store::new(store.clone()).load(&file, &|| false).is_some(), "the new file reads");
    let _ = std::fs::remove_dir_all(&store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_store_nothing_is_read_or_saved() {
    let (dir, key) = disk_repo("no-store");
    let store = store_dir("no-store");
    let d = Disk::default();
    let done = run(&d, &dir, &key);
    assert_eq!((done.source, done.save.is_none()), (Source::Built, true));
    let file = Store::new(store.clone()).file_of(&dir).unwrap();
    d.save(&file, &done.index);
    assert!(!store.exists(), "no folder made");
    let _ = std::fs::remove_dir_all(&dir);
}
