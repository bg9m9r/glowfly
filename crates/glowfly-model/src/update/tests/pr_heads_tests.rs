//! Matching pull request heads to loaded commits (`update/pr_heads.rs`): full ids by lookup,
//! prefixes in one pass, and no history scan for a full id that isn't loaded.

use crate::update::pr_heads::heads;
use git2::Oid;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Commit `n`: its id is `n` big-endian in the first four bytes, then `fill`.
fn oid(n: u32, fill: u8) -> Oid {
    let mut b = [fill; 20];
    b[..4].copy_from_slice(&n.to_be_bytes());
    Oid::from_bytes(&b).unwrap()
}

fn loaded(commits: &[Oid]) -> HashMap<Oid, usize> {
    commits.iter().enumerate().map(|(i, o)| (*o, i)).collect()
}

/// A commit list that fails the test if anything walks it.
fn never_walked() -> impl Iterator<Item = Oid> {
    std::iter::from_fn(|| panic!("the loaded commits were scanned"))
}

#[test]
fn a_full_id_is_looked_up_and_its_miss_scans_nothing() {
    let commits = [oid(1, 0xaa), oid(2, 0xbb)];
    let (hit, miss) = (commits[1].to_string(), oid(3, 0xcc).to_string());
    let prs = [(10, hit.as_str()), (11, miss.as_str()), (12, "not hex at all, but forty chars long!!!!")];
    let found = heads(prs.into_iter(), &loaded(&commits), never_walked());
    assert_eq!(found, HashMap::from([(10, commits[1])]));
}

#[test]
fn prefixes_take_the_first_loaded_commit_they_start() {
    // Commits 0x0000_0001aa.. and 0x0000_0001ab..: a prefix of seven digits matches both.
    let mut b = [0xab; 20];
    b[..4].copy_from_slice(&1u32.to_be_bytes());
    let commits = [oid(1, 0xaa), Oid::from_bytes(&b).unwrap(), oid(0x1234_5678, 0x9f)];
    let both = &commits[0].to_string()[..9]; // "00000001a": odd length, shared by the first two
    let second = &commits[1].to_string()[..12];
    let upper = commits[2].to_string()[..7].to_uppercase();
    let prs = [
        (1, both),
        (2, second),
        (3, upper.as_str()),
        (4, "123456"),               // too short to match anything
        (5, "0000000zz"),            // not hex
        (6, "0000000fffffffffffff"), // matches nothing loaded
    ];
    let found = heads(prs.into_iter(), &loaded(&commits), commits);
    assert_eq!(found, HashMap::from([(1, commits[0]), (2, commits[1]), (3, commits[2])]));
}

#[test]
fn only_full_ids_scan_nothing_and_prefixes_share_one_pass() {
    // 100k loaded commits and 100 heads of each kind that aren't loaded: the old per-PR scan
    // formatted every id for each of them (seconds in a debug build).
    let commits: Vec<Oid> = (0..100_000).map(|n| oid(n, 0x11)).collect();
    let index = loaded(&commits);
    let full: Vec<String> = (0..100).map(|n| oid(n, 0x22).to_string()).collect();
    let short: Vec<String> = full.iter().map(|h| format!("ff{}", &h[2..12])).collect();
    let started = Instant::now();
    let none = heads(full.iter().enumerate().map(|(i, h)| (i as u64, h.as_str())), &index, never_walked());
    let some = heads(short.iter().enumerate().map(|(i, h)| (i as u64, h.as_str())), &index, commits.iter().copied());
    let took = started.elapsed();
    assert!(none.is_empty() && some.is_empty());
    eprintln!("200 unloaded heads over 100k commits: {took:?}");
    assert!(took < Duration::from_secs(2), "took {took:?}");
}
