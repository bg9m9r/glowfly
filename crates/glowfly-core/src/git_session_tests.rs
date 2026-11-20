use super::*;
use std::cell::Cell;

fn key(p: &'static str) -> impl Fn() -> Option<PathBuf> {
    move || Some(PathBuf::from(p))
}

#[test]
fn remember_asks_once_per_key() {
    let cache: Cache<u32> = Mutex::new(BTreeMap::new());
    let asked = Cell::new(0);
    let ask = || {
        asked.set(asked.get() + 1);
        Some(7)
    };
    assert_eq!(remember(&cache, &key("a"), ask), Some(7));
    assert_eq!(remember(&cache, &key("a"), ask), Some(7));
    assert_eq!(asked.get(), 1);
    assert_eq!(remember(&cache, &key("b"), ask), Some(7));
    assert_eq!(asked.get(), 2, "another program is asked itself");
    assert_eq!(remember(&cache, &|| None, ask), Some(7));
    assert_eq!(asked.get(), 3, "so is git from the PATH");
}

#[test]
fn remember_does_not_keep_no_answer() {
    let cache: Cache<u32> = Mutex::new(BTreeMap::new());
    let asked = Cell::new(0);
    let ask = || {
        asked.set(asked.get() + 1);
        None::<u32>
    };
    assert_eq!(remember(&cache, &key("a"), ask), None);
    assert_eq!(remember(&cache, &key("a"), ask), None);
    assert_eq!(asked.get(), 2);
    assert!(locked(&cache).is_empty());
}

#[test]
fn remember_does_not_store_after_a_switch() {
    let cache: Cache<u32> = Mutex::new(BTreeMap::new());
    let calls = Cell::new(0);
    let switching = || {
        calls.set(calls.get() + 1);
        // The first call is the key before asking, the second after it.
        Some(PathBuf::from(if calls.get() == 1 { "old" } else { "new" }))
    };
    assert_eq!(remember(&cache, &switching, || Some(1)), Some(1), "the caller still gets the answer");
    assert!(locked(&cache).is_empty(), "but it is not kept under either key");
}

#[test]
fn version_matches_git_and_is_stable() {
    let dir = crate::paths::temp_dir();
    let direct = gitcli::parse_version(&gitcli::read(&dir, &gitcli::args(&["version"])).stdout);
    assert!(direct.is_some());
    assert_eq!(version(&dir), direct);
    assert_eq!(version(&dir), direct);
}

#[test]
fn a_fresh_probe_replaces_the_answer() {
    let program = Some(PathBuf::from("a-git-for-this-test"));
    note_version(program.clone(), (1, 2, 3));
    assert_eq!(locked(&VERSIONS).get(&program), Some(&(1, 2, 3)));
    note_version(program.clone(), (4, 5, 6));
    assert_eq!(locked(&VERSIONS).get(&program), Some(&(4, 5, 6)));
    locked(&VERSIONS).remove(&program);
}
