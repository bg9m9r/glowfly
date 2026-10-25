//! The saved index: what is saved reads back the same and serves as a base, damaged files are
//! removed, other layouts and versions are not trusted, files are shared by worktrees, loads stop
//! and mark the file used, pruning, privacy and the save policy.

use super::build_tests::{branchy, commit_at, index, rows, snapshot};
use super::store::{RESAVE_COMMITS, layout};
use super::*;
use crate::repo::LoadOptions;
use crate::testutil::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A store in a fresh temporary folder (not created yet).
fn temp_store(name: &str) -> Store {
    let dir = crate::paths::temp_dir().join(format!("glowfly-index-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Store::new(dir)
}

fn done(store: &Store, repos: &[&Path]) {
    let _ = std::fs::remove_dir_all(store.dir());
    for r in repos {
        let _ = std::fs::remove_dir_all(r);
    }
}

/// A saved index of `branchy(name)` plus a commit with multibyte text and one with no message.
fn saved(name: &str) -> (PathBuf, Store, StoreFile, CommitIndex) {
    let dir = branchy(name);
    write(&dir, "cafe.txt", "x");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "Café ✨ résumé", "-m", "Naïve body 🎉 with ünïcode"]);
    write(&dir, "empty.txt", "y");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "--allow-empty-message", "-m", ""]);
    let ix = index(&dir, &snapshot(&dir, LoadOptions::all(1000)), None);
    let store = temp_store(name);
    let file = store.file_of(&dir).unwrap();
    assert_eq!(store.save(&file, &ix).unwrap(), Stored::of(&ix));
    (dir, store, file, ix)
}

fn never() -> bool {
    false
}

#[test]
fn a_saved_index_reads_back_the_same() {
    let (dir, store, file, ix) = saved("store-same");
    let (back, stored) = store.load(&file, &never).expect("loads");
    assert_eq!(stored, Stored::of(&ix));
    assert!(stored.current);
    assert_eq!((back.len(), back.complete(), back.key()), (ix.len(), ix.complete(), ix.key()));
    assert_eq!(rows(&back), rows(&ix));
    for p in 0..ix.len() {
        assert_eq!(back.times_at(p), ix.times_at(p));
        assert_eq!(back.position(ix.oid_at(p).unwrap()), Some(p as u32));
    }
    assert!((0..ix.len()).any(|p| ix.summary_at(p) == Some("Café ✨ résumé")));
    assert!((0..ix.len()).any(|p| ix.summary_at(p) == Some("") && ix.body_at(p) == Some("")));
    for q in ["body", "café", "🎉", "author:test", "author:nobody"] {
        let q = CommitQuery::parse(q);
        assert_eq!(back.matching(&q, &never), ix.matching(&q, &never));
    }
    done(&store, &[&dir]);
}

#[test]
fn an_index_of_only_empty_messages_round_trips() {
    let commits: Vec<CommitInfo> = ["1111111111111111111111111111111111111111", "2222222222222222222222222222222222222222"]
        .iter()
        .map(|id| super::tests::info(id, "", "", "A", "a@x"))
        .collect();
    let ix = CommitIndex::from_commits(IndexKey { starts: vec![commits[0].oid], shallow: Vec::new() }, &commits);
    assert!(ix.texts.is_empty(), "no text block at all");
    let store = temp_store("empty-texts");
    let file = StoreFile { path: store.dir().join("empty.idx"), repo: "somewhere".into() };
    store.save(&file, &ix).unwrap();
    let (back, _) = store.load(&file, &never).expect("not damaged");
    assert_eq!(rows(&back), rows(&ix));
    done(&store, &[]);
}

#[test]
fn a_loaded_index_is_a_base_like_the_one_it_was_saved_from() {
    let (dir, store, file, _) = saved("store-base");
    let (base, _) = store.load(&file, &never).unwrap();
    git(&dir, &["checkout", "-q", "-b", "later"]);
    commit_at(&dir, "later1", 900);
    git(&dir, &["checkout", "-q", "main"]);
    commit_at(&dir, "later2", 910);
    let snap = snapshot(&dir, LoadOptions::all(1000));
    assert_eq!(rows(&index(&dir, &snap, Some(&base))), rows(&index(&dir, &snap, None)));
    done(&store, &[&dir]);
}

/// Where the parts of a saved file start.
struct Layout {
    entries_at: usize,
    n_entries: usize,
}

fn layout_of(bytes: &[u8]) -> Layout {
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let repo_len = u32_at(layout::HEADER);
    let version_at = layout::HEADER + 4 + repo_len;
    let version_len = u32_at(version_at);
    let (starts, shallow) = (u32_at(32), u32_at(36));
    let entries_at = version_at + 4 + version_len + layout::OID * (starts + shallow);
    Layout { entries_at, n_entries: u32_at(layout::ENTRIES_AT) }
}

/// Writes `bytes` with a correct checksum.
fn write_fixed(path: &Path, mut bytes: Vec<u8>) {
    let n = bytes.len() - 4;
    let crc = layout::crc(&bytes[..n]);
    bytes[n..].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}

/// The field at `field` (byte offset in the 64-byte record) of entry `e` set to `v`.
fn with_entry_field(bytes: &[u8], e: usize, field: usize, v: u32) -> Vec<u8> {
    let mut b = bytes.to_vec();
    let at = layout_of(bytes).entries_at + e * layout::ENTRY + field;
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    b
}

#[test]
fn a_damaged_file_is_removed() {
    let (dir, store, file, ix) = saved("store-damaged");
    let good = std::fs::read(&file.path).unwrap();
    let l = layout_of(&good);
    let multibyte = (0..ix.len()).find(|&p| ix.summary_at(p) == Some("Café ✨ résumé")).unwrap();
    let first_text = (0..ix.len()).find(|&p| !ix.summary_at(p).unwrap().is_empty()).unwrap();
    let mut cases: Vec<(&str, Vec<u8>, bool)> = Vec::new();
    let mut flipped = good.clone();
    flipped[l.entries_at + 3] ^= 0x40;
    cases.push(("a flipped byte", flipped, false));
    cases.push(("cut short", good[..good.len() - 10].to_vec(), false));
    cases.push(("a byte appended", [good.as_slice(), &[0]].concat(), false));
    let mut magic = good.clone();
    magic[0] = b'X';
    cases.push(("bad magic", magic, false));
    let mut huge = good[..layout::HEADER].to_vec();
    huge[layout::ENTRIES_AT..layout::ENTRIES_AT + 4].copy_from_slice(&(1u32 << 31).to_le_bytes());
    huge[layout::BODY_LEN_AT..layout::BODY_LEN_AT + 8].copy_from_slice(&8u64.to_le_bytes());
    huge.extend_from_slice(&[0; 12]);
    cases.push(("a huge count and a tiny body", huge, true));
    let summary_len = ix.summary_at(multibyte).unwrap().len() as u32;
    let fields: [(&str, usize, usize, u32); 5] = [
        ("person out of range", 0, 36, 10_000),
        ("block out of range", first_text, 40, 10_000),
        ("text past the block", first_text, 44, u32::MAX / 2),
        ("a slice inside a character", multibyte, 48, summary_len - 1),
        ("parents past the parents", 0, 56, 1 << 30),
    ];
    for (what, e, field, v) in fields {
        cases.push((what, with_entry_field(&good, e, field, v), true));
    }
    assert!(l.n_entries >= 2);
    let mut dup = good.clone();
    let (a, b) = (l.entries_at, l.entries_at + layout::ENTRY);
    dup.copy_within(a..a + layout::OID, b);
    cases.push(("a duplicate oid", dup, true));
    for (what, bytes, fix_crc) in cases {
        if fix_crc {
            write_fixed(&file.path, bytes);
        } else {
            std::fs::write(&file.path, bytes).unwrap();
        }
        assert!(store.load(&file, &never).is_none(), "{what}");
        assert!(!file.path.exists(), "{what}: removed");
    }
    done(&store, &[&dir]);
}

#[test]
fn another_format_is_ignored_but_kept() {
    let (dir, store, file, ix) = saved("store-format");
    let mut bytes = std::fs::read(&file.path).unwrap();
    bytes[layout::FORMAT_AT..layout::FORMAT_AT + 4].copy_from_slice(&(store::FORMAT + 1).to_le_bytes());
    write_fixed(&file.path, bytes);
    assert!(store.load(&file, &never).is_none());
    assert!(file.path.exists(), "left for a later save");
    store.save(&file, &ix).unwrap();
    assert!(store.load(&file, &never).is_some(), "replaced");
    done(&store, &[&dir]);
}

#[test]
fn a_file_for_another_repository_is_ignored() {
    let (b, store, b_file, _) = saved("store-repo-b");
    let a = scratch_repo("store-repo-a");
    commit_at(&a, "one", 100);
    let a_file = store.file_of(&a).unwrap();
    assert_ne!(a_file, b_file);
    std::fs::copy(&b_file.path, &a_file.path).unwrap();
    assert!(store.load(&a_file, &never).is_none());
    assert!(!a_file.path.exists());
    done(&store, &[&a, &b]);
}

#[test]
fn positions_are_trusted_only_from_this_version() {
    let (dir, store, file, ix) = saved("store-version");
    let patch_version = |bytes: &mut Vec<u8>| {
        let repo_len = u32::from_le_bytes(bytes[layout::HEADER..layout::HEADER + 4].try_into().unwrap()) as usize;
        let at = layout::HEADER + 4 + repo_len + 4;
        bytes[at] = if bytes[at] == b'9' { b'8' } else { b'9' };
    };
    let mut bytes = std::fs::read(&file.path).unwrap();
    patch_version(&mut bytes);
    write_fixed(&file.path, bytes);
    let (_, stored) = store.load(&file, &never).expect("still a base");
    assert!(!stored.current);

    // An incomplete index built with another cap.
    let capped =
        CommitIndex::from_commits(ix.key().clone(), &[super::tests::info("1111111111111111111111111111111111111111", "s", "", "A", "a@x")])
            .marked_incomplete();
    store.save(&file, &capped).unwrap();
    assert!(store.load(&file, &never).unwrap().1.current, "today's cap");
    let mut bytes = std::fs::read(&file.path).unwrap();
    bytes[20..24].copy_from_slice(&1234u32.to_le_bytes());
    write_fixed(&file.path, bytes);
    let (_, stored) = store.load(&file, &never).unwrap();
    assert!(!stored.complete && !stored.current);
    done(&store, &[&dir]);
}

#[test]
fn worktrees_share_a_file_and_repositories_do_not() {
    let main = scratch_repo("store-worktrees");
    commit_at(&main, "one", 100);
    let wt = main.with_file_name(format!("{}-wt", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&main, &["worktree", "add", "-q", "-b", "side", &wt.to_string_lossy()]);
    let other = scratch_repo("store-worktrees-other");
    commit_at(&other, "one", 100);
    let store = temp_store("worktrees");
    let file = store.file_of(&main).unwrap();
    assert_eq!(store.file_of(&wt), Some(file.clone()));
    assert_ne!(store.file_of(&other).unwrap().path, file.path);
    let name = file.path.file_name().unwrap().to_string_lossy().into_owned();
    let stem = name.strip_suffix(".idx").expect("an .idx file");
    assert!(stem.len() == 32 && stem.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()), "{name}");
    let empty = crate::paths::temp_dir().join(format!("glowfly-index-store-no-repo-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    assert_eq!(store.file_of(&empty), None);
    done(&store, &[&wt, &main, &other, &empty]);
}

#[test]
fn stopping_a_load_gives_none_and_keeps_the_file() {
    let (dir, store, file, _) = saved("store-stop");
    assert!(store.load(&file, &|| true).is_none());
    assert!(file.path.exists());
    done(&store, &[&dir]);
}

fn set_age(path: &Path, now: SystemTime, age: Duration) {
    std::fs::OpenOptions::new().write(true).open(path).unwrap().set_modified(now - age).unwrap();
}

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[test]
fn loading_marks_the_file_used() {
    let (dir, store, file, _) = saved("store-used");
    set_age(&file.path, SystemTime::now(), 10 * DAY);
    store.load(&file, &never).unwrap();
    let age = SystemTime::now().duration_since(std::fs::metadata(&file.path).unwrap().modified().unwrap()).unwrap_or_default();
    assert!(age < Duration::from_secs(60), "{age:?}");
    done(&store, &[&dir]);
}

#[test]
fn prune_removes_unused_then_oldest_beyond_the_total() {
    let store = temp_store("prune");
    let d = store.dir();
    std::fs::create_dir_all(d.join("folder.idx")).unwrap();
    let now = SystemTime::now();
    let files = [
        ("unused.idx", 40 * DAY),
        ("oldest.idx", 4 * DAY),
        ("older.idx", 3 * DAY),
        ("newer.idx", 2 * DAY),
        ("newest.idx", DAY),
        (".newest.idx.12.0.tmp", 2 * DAY),
        (".newest.idx.12.1.tmp", Duration::from_secs(60)),
        ("notes.txt", 90 * DAY),
    ];
    for (name, age) in files {
        std::fs::write(d.join(name), [0u8; 100]).unwrap();
        set_age(&d.join(name), now, age);
    }
    let keep = d.join("oldest.idx");
    // The unused file and the stale temp, then past 250 bytes: oldest (kept) and newest fit,
    // newer and older don't.
    assert_eq!(store.prune_with(now, 30 * DAY, 250, Some(&keep)), 4);
    let mut left: Vec<String> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left, [".newest.idx.12.1.tmp", "folder.idx", "newest.idx", "notes.txt", "oldest.idx"]);
    assert_eq!(store.prune(now, None), 0, "the real limits keep the rest");
    done(&store, &[]);
}

#[cfg(unix)]
#[test]
fn saved_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store, file, _) = saved("store-private");
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&file.path), 0o600);
    assert_eq!(mode(store.dir()), 0o700, "the folder the save made");
    done(&store, &[&dir]);
}

#[test]
fn worth_saving_follows_the_policy() {
    let c = |id: &str| super::tests::info(id, "s", "", "A", "a@x");
    let key = |n: u8| IndexKey { starts: vec![Oid::from_bytes(&[n; 20]).unwrap()], shallow: Vec::new() };
    let ix = CommitIndex::from_commits(key(1), &[c("1111111111111111111111111111111111111111")]);
    let stored = |k: IndexKey, len: usize| Stored { key: k, len, complete: true, current: true };
    assert!(worth_saving(None, &ix), "nothing stored");
    assert!(worth_saving(Some(&Stored { current: false, ..Stored::of(&ix) }), &ix), "another version");
    assert!(!worth_saving(Some(&Stored::of(&ix)), &ix), "already there");
    assert!(!worth_saving(Some(&stored(key(2), 1 + 4_999)), &ix), "too few new commits");
    assert!(worth_saving(Some(&stored(key(2), 1 + RESAVE_COMMITS)), &ix));
    let shallow = IndexKey { shallow: vec![Oid::from_bytes(&[9; 20]).unwrap()], ..key(2) };
    assert!(worth_saving(Some(&stored(shallow, 1)), &ix), "another shallow boundary");
    let capped = CommitIndex::from_commits(key(3), &[c("1111111111111111111111111111111111111111")]).marked_incomplete();
    assert!(worth_saving(Some(&stored(key(2), 1)), &capped), "an incomplete index");
}
