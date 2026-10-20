//! A state's preview: its history and what restoring it would move.

use super::preview::{Count, HISTORY, preview};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn commit(dir: &Path, text: &str) -> Oid {
    commit_file(dir, "a.txt", text)
}

fn commit_file(dir: &Path, file: &str, text: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", text]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn history_and_counts() {
    let dir = scratch_repo("journal-preview");
    let commits: Vec<Oid> = (1..=10).map(|i| commit(&dir, &format!("c{i}"))).collect();
    let top = commits[9];
    // A merge on top: the history follows the first parent.
    git(&dir, &["checkout", "-q", "-b", "side", &commits[7].to_string()]);
    let side = commit_file(&dir, "b.txt", "side");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let merge = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let p = preview(&dir, merge, None).unwrap();
    assert_eq!(p.history.len(), HISTORY);
    let summaries: Vec<&str> = p.history.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["merge side", "c10", "c9", "c8", "c7", "c6", "c5", "c4"]);
    assert!(!p.history.iter().any(|c| c.oid == side));
    assert_eq!((p.leaving, p.arriving), (None, None));

    git(&dir, &["reset", "-q", "--hard", &commits[7].to_string()]);
    let now = commits[7];
    let back = preview(&dir, top, Some(now)).unwrap();
    assert_eq!((back.leaving, back.arriving), (Some(Count { n: 0, more: false }), Some(Count { n: 2, more: false })));
    let forward = preview(&dir, now, Some(top)).unwrap();
    assert_eq!((forward.leaving, forward.arriving), (Some(Count { n: 2, more: false }), Some(Count { n: 0, more: false })));
    assert_eq!(preview(&dir, now, Some(now)).unwrap().leaving, None, "nothing to count against itself");

    let missing = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert!(preview(&dir, missing, Some(now)).unwrap_err().is_not_found());
    let _ = std::fs::remove_dir_all(&dir);
}
