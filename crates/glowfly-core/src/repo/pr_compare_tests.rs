//! `pr_compare` over fixture histories: which source tip is picked (the listed head or the
//! remote-tracking branch), the commits in `target..source`, and the comparison's outcomes.

use super::pr_compare::{range_commits, source_tip};
use super::{PrCompare, SourceFrom, SourceTip, branch_changes, is_commit_prefix, pr_compare};
use crate::testutil::scratch_repo;
use git2::{Oid, Repository, Signature, Time};
use std::path::PathBuf;

/// Commits `file` (with `file` as its text) on top of `parents` at `secs`, moving
/// `refs/heads/<branch>` to it.
fn commit(repo: &Repository, branch: &str, parents: &[Oid], file: &str, secs: i64) -> Oid {
    let sig = Signature::new("Ada", "ada@example.com", &Time::new(secs, 0)).unwrap();
    let base = parents.first().map(|p| repo.find_commit(*p).unwrap().tree().unwrap());
    let mut tb = repo.treebuilder(base.as_ref()).unwrap();
    tb.insert(file, repo.blob(file.as_bytes()).unwrap(), 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let parents: Vec<_> = parents.iter().map(|p| repo.find_commit(*p).unwrap()).collect();
    let refs: Vec<_> = parents.iter().collect();
    let oid = repo.commit(None, &sig, &sig, file, &tree, &refs).unwrap();
    repo.reference(&format!("refs/heads/{branch}"), oid, true, file).unwrap();
    oid
}

fn fixture(name: &str) -> (PathBuf, Repository) {
    let dir = scratch_repo(name);
    let repo = Repository::open(&dir).unwrap();
    (dir, repo)
}

fn summaries(r: &super::RangeCommits) -> Vec<&str> {
    r.commits.iter().map(|c| c.summary.as_str()).collect()
}

#[test]
fn commit_prefixes_are_7_to_64_hex_characters() {
    for n in [7, 12, 40, 64] {
        assert!(is_commit_prefix(&"a1".repeat(32)[..n]), "{n}");
    }
    assert!(!is_commit_prefix("abcdef"));
    assert!(!is_commit_prefix(&"a".repeat(65)));
    assert!(!is_commit_prefix("not-hex-at-all"));
    assert!(!is_commit_prefix(""));
}

#[test]
fn the_source_tip_prefers_a_head_that_is_here() {
    let (dir, repo) = fixture("pr-compare-tip");
    let base = commit(&repo, "main", &[], "a.txt", 1_000);
    let branch = commit(&repo, "feature", &[base], "f.txt", 2_000);
    let head = commit(&repo, "local", &[branch], "g.txt", 3_000);
    let tip = |b: Option<Oid>, h: Option<&str>| source_tip(&repo, b, h);
    let (h, b) = (head.to_string(), branch.to_string());
    // The branch is missing: the head is used.
    assert_eq!(tip(None, Some(&h)), Some(SourceTip { oid: head, from: SourceFrom::Head }));
    // The branch is at the head (a 12-character prefix, as Bitbucket Cloud lists it).
    assert_eq!(tip(Some(branch), Some(&b[..12])), Some(SourceTip { oid: branch, from: SourceFrom::Branch }));
    // The branch points elsewhere and the head is here.
    assert_eq!(tip(Some(branch), Some(&h)), Some(SourceTip { oid: head, from: SourceFrom::Head }));
    assert_eq!(tip(Some(head), Some(&b)), Some(SourceTip { oid: branch, from: SourceFrom::Head }));
    // The head is unknown, or not in this repository.
    assert_eq!(tip(Some(branch), None), Some(SourceTip { oid: branch, from: SourceFrom::Branch }));
    let absent = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(tip(Some(branch), Some(absent)), Some(SourceTip { oid: branch, from: SourceFrom::Branch }));
    assert_eq!(tip(None, Some(absent)), None);
    assert_eq!(tip(None, None), None);
    // An uppercase full id is found.
    assert_eq!(tip(None, Some(&h.to_uppercase())), Some(SourceTip { oid: head, from: SourceFrom::Head }));
    // A blob's id is not a commit; nor is too short a prefix or text that isn't hex.
    let blob = repo.blob(b"f.txt").unwrap().to_string();
    assert_eq!(tip(None, Some(&blob)), None);
    assert_eq!(tip(None, Some(&h[..6])), None);
    assert_eq!(tip(None, Some("feature")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn range_commits_are_the_two_dot_range_oldest_first() {
    let (dir, repo) = fixture("pr-compare-range");
    let base = commit(&repo, "main", &[], "a.txt", 1_000);
    let c1 = commit(&repo, "feature", &[base], "c1", 2_000);
    let c2 = commit(&repo, "feature", &[c1], "c2", 3_000);
    let c3 = commit(&repo, "feature", &[c2], "c3", 4_000);
    let r = range_commits(&repo, base, c3, 500).unwrap();
    assert_eq!(summaries(&r), ["c1", "c2", "c3"]);
    assert_eq!(r.more, 0);
    let first = &r.commits[0];
    assert_eq!(
        (first.oid, first.author.as_str(), first.email.as_str(), first.time, first.merge),
        (c1, "Ada", "ada@example.com", 2_000, false)
    );

    // The newest 2 of 3, oldest first, and one more.
    let r = range_commits(&repo, base, c3, 2).unwrap();
    assert_eq!(summaries(&r), ["c2", "c3"]);
    assert_eq!(r.more, 1);

    // The source merged the target in: the target's commits are left out.
    let m1 = commit(&repo, "main", &[base], "m1", 5_000);
    let merge = commit(&repo, "feature", &[c3, m1], "merge", 6_000);
    let r = range_commits(&repo, m1, merge, 500).unwrap();
    assert_eq!(summaries(&r), ["c1", "c2", "c3", "merge"]);
    assert!(r.commits[3].merge);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pr_compare_outcomes() {
    let (dir, repo) = fixture("pr-compare-outcomes");
    let base = commit(&repo, "main", &[], "a.txt", 1_000);
    let target = commit(&repo, "main", &[base], "m.txt", 2_000);
    let f1 = commit(&repo, "feature", &[base], "f.txt", 3_000);
    let f2 = commit(&repo, "feature", &[f1], "g.txt", 4_000);
    let PrCompare::Loaded { source, changes, commits } = pr_compare(&dir, target, Some(f2), None).unwrap() else { panic!() };
    assert_eq!(source, SourceTip { oid: f2, from: SourceFrom::Branch });
    assert_eq!(Some(changes), branch_changes(&dir, target, f2).unwrap());
    assert_eq!(summaries(&commits), ["f.txt", "g.txt"]);
    // From the listed head when the branch is gone.
    let PrCompare::Loaded { source, .. } = pr_compare(&dir, target, None, Some(&f2.to_string())).unwrap() else { panic!() };
    assert_eq!(source, SourceTip { oid: f2, from: SourceFrom::Head });

    // An orphan source shares no history.
    let sig = Signature::new("Ada", "ada@example.com", &Time::new(5_000, 0)).unwrap();
    let tree = repo.find_commit(f2).unwrap().tree().unwrap();
    let orphan = repo.commit(None, &sig, &sig, "orphan", &tree, &[]).unwrap();
    assert_eq!(
        pr_compare(&dir, target, Some(orphan), None).unwrap(),
        PrCompare::Unrelated(SourceTip { oid: orphan, from: SourceFrom::Branch })
    );
    assert_eq!(pr_compare(&dir, target, None, None).unwrap(), PrCompare::NoSource);
    let absent = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert!(pr_compare(&dir, absent, Some(f2), None).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
