//! The fast collision scan finds what the plain one did, and its cache answers unchanged
//! indexes without scanning, including the watcher's path-limited status reads.

use super::super::{key, nfc};
use super::*;
use crate::repo::bytes_tests::blob;
use crate::testutil::*;
use git2::{IndexEntry, IndexTime, Oid, Repository};
use std::cell::Cell;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

thread_local! {
    /// Scans run on this thread (tests run one per thread).
    pub(super) static SCANS: Cell<usize> = const { Cell::new(0) };
}

fn scans() -> usize {
    SCANS.with(Cell::get)
}

/// The scan as it was before the fast one: every path's key built and grouped.
fn plain_scan(index: &Index) -> Vec<Collision> {
    let mut by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in index.iter() {
        if let Ok(path) = String::from_utf8(e.path) {
            by_key.entry(key(&path)).or_default().insert(path);
        }
    }
    by_key
        .into_values()
        .filter(|set| set.len() > 1)
        .map(|set| {
            let composed: BTreeSet<String> = set.iter().map(|p| nfc(p)).collect();
            Collision { case: composed.len() > 1, unicode: composed.len() < set.len(), paths: set.into_iter().collect() }
        })
        .collect()
}

/// An in-memory index holding `paths` (raw bytes), each at `stage`.
fn index_of(paths: &[(&[u8], u16)]) -> Index {
    let mut index = Index::new().unwrap();
    for (path, stage) in paths {
        let zero = IndexTime::new(0, 0);
        let entry = IndexEntry {
            ctime: zero,
            mtime: zero,
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: Oid::from_str("8ab686eafeb1f44702738c8b0f24f2567c36da6d").unwrap(),
            flags: stage << 12,
            flags_extended: 0,
            path: path.to_vec(),
        };
        index.add(&entry).unwrap();
    }
    index
}

#[test]
fn the_fast_scan_finds_what_the_plain_scan_found() {
    let composed = "caf\u{e9}.txt";
    let decomposed = "cafe\u{301}.txt";
    let paths: Vec<(&[u8], u16)> = vec![
        (b"a.txt", 0),
        (b"A.txt", 0),
        (composed.as_bytes(), 0),
        (decomposed.as_bytes(), 0),
        ("CAFE\u{301}.TXT".as_bytes(), 0),
        // The Kelvin sign lowercases to an ASCII k.
        ("\u{212a}elvin.md".as_bytes(), 0),
        (b"kelvin.md", 0),
        (b"Docs/x.md", 0),
        (b"docs/y.md", 0),
        // One path at three conflict stages is one path.
        (b"conflict.rs", 1),
        (b"conflict.rs", 2),
        (b"conflict.rs", 3),
        (b"plain.txt", 0),
        (b"\xff\xfe-not-utf8", 0),
        (b"\xff\xfe-NOT-utf8", 0),
    ];
    let index = index_of(&paths);
    let fast = scan(&index);
    assert_eq!(fast, plain_scan(&index));
    let groups: Vec<Vec<&str>> = fast.iter().map(|c| c.paths.iter().map(String::as_str).collect()).collect();
    assert_eq!(groups.len(), 3, "{groups:?}");
    assert!(fast.iter().any(|c| c.case && c.unicode && c.paths.len() == 3), "café in three spellings: {groups:?}");
    assert!(fast.iter().any(|c| c.paths == ["A.txt", "a.txt"] && c.case && !c.unicode), "{groups:?}");
    assert!(fast.iter().any(|c| c.paths.contains(&"kelvin.md".to_owned())), "{groups:?}");
}

#[test]
fn an_index_without_collisions_has_none() {
    let index = index_of(&[(b"src/main.rs", 0), (b"src/lib.rs", 0), (b"README.md", 0)]);
    assert!(scan(&index).is_empty());
}

/// A repository whose index holds a.txt and A.txt, last written long enough ago to trust.
fn settled_twins(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.ignorecase", "false"]);
    write(&dir, "other.txt", "other\n");
    git(&dir, &["add", "other.txt"]);
    for p in ["a.txt", "A.txt"] {
        let id = blob(&dir, p);
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{id},{p}")]);
    }
    git(&dir, &["commit", "-q", "-m", "twins"]);
    settle(&dir);
    dir
}

/// Dates the index file back, as if written a while ago.
fn settle(dir: &Path) {
    let file = std::fs::File::options().write(true).open(dir.join(".git").join("index")).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(30)).unwrap();
}

fn found_in(dir: &Path) -> Vec<Vec<String>> {
    let repo = Repository::open(dir).unwrap();
    collisions(&mut repo.index().unwrap()).into_iter().map(|c| c.paths).collect()
}

#[test]
fn an_unchanged_index_is_scanned_once_and_a_changed_one_again() {
    let dir = settled_twins("collide-cache");
    let start = scans();
    assert_eq!(found_in(&dir), [vec!["A.txt".to_owned(), "a.txt".to_owned()]]);
    assert_eq!(found_in(&dir), [vec!["A.txt".to_owned(), "a.txt".to_owned()]]);
    assert_eq!(scans() - start, 1, "the second read comes from the cache");

    // The watcher's reads after saves: no scan while the index stays the same.
    write(&dir, "other.txt", "changed\n");
    for _ in 0..3 {
        crate::repo::status_of_paths(&dir, &["other.txt".to_owned()], false).unwrap().expect("read on its own");
    }
    crate::repo::status_at(&dir, false).unwrap();
    assert_eq!(scans() - start, 1, "status reads of an unchanged index don't scan");

    // A new twin pair changes the index: scanned again, and found.
    for p in ["b.txt", "B.txt"] {
        let id = blob(&dir, p);
        git(&dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{id},{p}")]);
    }
    settle(&dir);
    assert_eq!(found_in(&dir).len(), 2);
    assert_eq!(scans() - start, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_freshly_written_index_isnt_trusted_by_its_stamp() {
    let dir = settled_twins("collide-fresh");
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("100644,{},c.txt", blob(&dir, "c"))]);
    let start = scans();
    found_in(&dir);
    found_in(&dir);
    assert_eq!(scans() - start, 2, "written just now: scanned each time");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Times both scans and the cache on a generated 100,000-entry index. Run with
/// `cargo test -p glowfly-core --release collisions_timing -- --ignored --nocapture`.
#[test]
#[ignore]
fn collisions_timing() {
    const N: usize = 100_000;
    let dir = scratch_repo("collide-timing");
    let id = blob(&dir, "x\n");
    let mut lines = String::new();
    for i in 0..N {
        // Mostly ASCII, as in real trees; one in fifty has an accented name.
        let name = if i % 50 == 0 { format!("caf\u{e9}-{i}.txt") } else { format!("File-{i}.txt") };
        lines.push_str(&format!("100644 {id}\tsrc/module-{}/{name}\n", i / 500));
    }
    let mut child = Command::new("git").args(["update-index", "--index-info"]).current_dir(&dir).stdin(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(lines.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
    settle(&dir);
    let repo = Repository::open(&dir).unwrap();
    let mut index = repo.index().unwrap();
    assert_eq!(index.len(), N);
    let time = |f: &mut dyn FnMut() -> Vec<Collision>| {
        let runs = 5;
        let t = Instant::now();
        for _ in 0..runs {
            assert!(f().is_empty());
        }
        t.elapsed() / runs
    };
    let plain = time(&mut || plain_scan(&index));
    let fast = time(&mut || scan(&index));
    collisions(&mut index);
    let cached = time(&mut || collisions(&mut index));
    println!("{N} entries: plain scan {plain:?}, fast scan {fast:?}, cached {cached:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
