//! Search reads the whole message: a commit whose only match is far into its body (past the
//! graph's preview) is found in the loaded history, in a built index, in an updated one and in
//! one read back from the store; and an index saved before bodies were whole is never read.

use super::build_tests::{commit_at, index, snapshot};
use super::store::{FORMAT, layout};
use super::*;
use crate::repo::{BODY_PREVIEW, LoadOptions};
use crate::testutil::*;
use std::collections::HashSet;

const DEEP: &str = "Refs-ticket: GLOW-4242";

/// A history whose newest commit names `DEEP` only after a long body, below two ordinary ones.
fn repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    commit_at(&dir, "first", 100);
    commit_at(&dir, "second", 200);
    let filler = "This paragraph explains the change at length so the trailer sits far down. ".repeat(8);
    let msg = format!("Tidy the parser\n\n{filler}\n\nAnd a\nsecond   paragraph.\n\n{DEEP}\n");
    write(&dir, "deep.txt", "x");
    git(&dir, &["add", "."]);
    std::fs::write(dir.join("msg.txt"), msg).unwrap();
    git(&dir, &["commit", "-q", "-F", "msg.txt"]);
    dir
}

fn found(ix: &CommitIndex, q: &str) -> Vec<u32> {
    ix.matching(&CommitQuery::parse(q), &|| false).unwrap()
}

#[test]
fn a_match_past_the_preview_is_found_with_and_without_the_index() {
    let dir = repo("index-deep-match");
    let snap = snapshot(&dir, LoadOptions::all(100));
    let deep = &snap.commits[0];
    let at = deep.body.find(DEEP).expect("the body is whole");
    assert!(at > BODY_PREVIEW + 100, "the match is well past the preview ({at})");
    assert!(!deep.body_preview().0.contains(DEEP) && deep.body_preview().1, "the graph shows only the start");
    assert!(deep.body.contains("And a second paragraph."), "flattened to one line");

    // Without the index: the loaded-history matcher.
    let q = |s: &str| CommitQuery::parse(s).matching_rows(&snap.commits, &HashSet::new());
    assert_eq!(q("glow-4242"), vec![0]);
    assert_eq!(q("ticket: GLOW"), vec![0]);
    assert_eq!(q("a second paragraph"), vec![0], "a phrase across a line break");
    assert!(q("glow-4243").is_empty());

    // With it: fresh, updated from a base, and read back from the store.
    let ix = index(&dir, &snap, None);
    assert_eq!((found(&ix, "glow-4242"), found(&ix, "a second paragraph")), (vec![0], vec![0]));
    assert_eq!(ix.body_at(0), Some(&*deep.body), "the index holds the same whole body");
    commit_at(&dir, "third", 400);
    let newer = snapshot(&dir, LoadOptions::all(100));
    let updated = index(&dir, &newer, Some(&ix));
    assert_eq!(found(&updated, "glow-4242"), vec![1], "copied from the base");
    let store = Store::new(crate::paths::temp_dir().join(format!("glowfly-index-deep-{}", std::process::id())));
    let file = store.file_of(&dir).unwrap();
    store.save(&file, &updated).unwrap();
    let (back, _) = store.load(&file, &|| false).expect("reads back");
    assert_eq!(found(&back, "glow-4242"), vec![1]);
    let _ = std::fs::remove_dir_all(store.dir());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_index_saved_before_whole_bodies_is_rebuilt_not_read() {
    let dir = repo("index-deep-old-format");
    let snap = snapshot(&dir, LoadOptions::all(100));
    let ix = index(&dir, &snap, None);
    let store = Store::new(crate::paths::temp_dir().join(format!("glowfly-index-deep-old-{}", std::process::id())));
    let file = store.file_of(&dir).unwrap();
    store.save(&file, &ix).unwrap();
    // The same file as format 2 wrote it: its bodies would be cut, so it must not be read, as
    // it is or as a base, and is left for the next save to replace.
    let mut bytes = std::fs::read(&file.path).unwrap();
    assert_eq!(FORMAT, 3);
    bytes[layout::FORMAT_AT..layout::FORMAT_AT + 4].copy_from_slice(&2u32.to_le_bytes());
    let n = bytes.len() - 4;
    let crc = layout::crc(&bytes[..n]);
    bytes[n..].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&file.path, &bytes).unwrap();
    assert!(store.load(&file, &|| false).is_none());
    assert!(file.path.exists(), "kept until a save replaces it");
    let rebuilt = index(&dir, &snap, None);
    store.save(&file, &rebuilt).unwrap();
    let (back, stored) = store.load(&file, &|| false).expect("the rebuilt file reads");
    assert!(stored.current);
    assert_eq!(found(&back, "glow-4242"), vec![0]);
    let _ = std::fs::remove_dir_all(store.dir());
    let _ = std::fs::remove_dir_all(&dir);
}
