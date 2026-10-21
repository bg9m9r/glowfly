//! Matching over an index agrees with `CommitQuery::matches`; stopping, lookups, and a message
//! that isn't UTF-8.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;

fn info(id: &str, summary: &str, body: &str, author: &str, email: &str) -> CommitInfo {
    CommitInfo {
        oid: Oid::from_str(id).unwrap(),
        summary: summary.into(),
        body: body.into(),
        author: author.into(),
        email: email.into(),
        time: 0,
        parents: Vec::new(),
    }
}

fn key() -> IndexKey {
    IndexKey { starts: Vec::new(), shallow: Vec::new() }
}

fn fixture() -> Vec<CommitInfo> {
    vec![
        info("abc1234500000000000000000000000000000000", "Fix the parser", "", "Ann Lee", "ann@example.com"),
        info("1111111111111111111111111111111111111111", "Add docs", "fix parser later", "Joann Smith", "joann@example.com"),
        info("2222222222222222222222222222222222222222", "Änderung am Parser", "", "Jörg", "jorg@example.org"),
        info("3333333333333333333333333333333333333333", "Refactor", "Mentions Ann Lee", "Bob", "bob@example.com"),
        info("4444444444444444444444444444444444444444", "fix: typo", "", "Ann Lee", "ann@example.com"),
        info("abc1299999999999999999999999999999999999", "Release", "", "ann", "ANN@example.com"),
    ]
}

#[test]
fn matching_agrees_with_commit_query() {
    let commits = fixture();
    let ix = CommitIndex::from_commits(key(), &commits);
    let queries = ["fix", "FIX parser", "author:ann", "author:ann@example.com", "abc12", "änderung", "author:\"Ann Lee\" fix", "lee", ""];
    for q in queries {
        let query = CommitQuery::parse(q);
        let want: Vec<u32> = (0..commits.len() as u32).filter(|&i| !query.is_empty() && query.matches(&commits[i as usize])).collect();
        assert_eq!(ix.matching(&query, &|| false), Some(want), "{q}");
    }
    assert_eq!(ix.matching(&CommitQuery::parse(""), &|| false), Some(vec![]));
    assert_eq!(ix.matching(&CommitQuery::parse("author:ann@example.com"), &|| false), Some(vec![0, 4, 5]));
}

#[test]
fn matching_stops_when_asked() {
    let ix = CommitIndex::from_commits(key(), &fixture());
    assert_eq!(ix.matching(&CommitQuery::parse("fix"), &|| true), None);
}

#[test]
fn from_commits_round_trips() {
    let commits = fixture();
    let ix = CommitIndex::from_commits(key(), &commits);
    assert_eq!((ix.len(), ix.is_empty(), ix.complete()), (6, false, true));
    assert_eq!(format!("{ix:?}"), "CommitIndex { commits: 6, complete: true }");
    for (p, c) in commits.iter().enumerate() {
        assert_eq!(ix.oid_at(p), Some(c.oid));
        assert_eq!(ix.position(c.oid), Some(p as u32));
        assert_eq!(ix.summary_at(p), Some(c.summary.as_str()));
        assert_eq!(ix.body_at(p), Some(c.body.as_str()));
        assert_eq!(ix.person_at(p), Some((c.author.as_str(), c.email.as_str())));
        assert_eq!(ix.times_at(p), Some((c.time, c.time)));
    }
    assert_eq!(ix.oid_at(6), None);
    assert_eq!(ix.people.len(), 5, "the two Ann Lee commits share one author");
}

#[test]
fn a_message_that_is_not_utf8_indexes_empty_text() {
    let dir = scratch_repo("index-latin1");
    write(&dir, "a.txt", "a");
    git(&dir, &["add", "."]);
    std::fs::write(dir.join("msg.txt"), b"Caf\xe9 au lait\n\nCr\xe8me body\n").unwrap();
    git(&dir, &["-c", "i18n.commitEncoding=latin1", "commit", "-q", "-F", "msg.txt"]);
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!((snap.commits[0].summary.as_str(), snap.commits[0].body.as_str()), ("", ""), "the graph shows no text");
    let ix = build(&dir, &IndexKey::of(&snap), None, &mut |_| {}, &|| false).unwrap().unwrap();
    assert_eq!((ix.summary_at(0), ix.body_at(0)), (Some(""), Some("")));
    assert_eq!(ix.matching(&CommitQuery::parse("caf"), &|| false), Some(vec![]));
    assert_eq!(ix.matching(&CommitQuery::parse("test"), &|| false), Some(vec![0]), "the author still matches");
    let _ = std::fs::remove_dir_all(&dir);
}
