//! Matching over an index agrees with `CommitQuery::matches`; stopping, lookups, and a message
//! git wrote in Latin-1.

use super::*;
use crate::commit_query::CommitQuery;
use crate::mailmap::Mailmap;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;

pub(super) fn info(id: &str, summary: &str, body: &str, author: &str, email: &str) -> CommitInfo {
    CommitInfo {
        oid: Oid::from_str(id).unwrap(),
        summary: summary.into(),
        body: body.into(),
        author: crate::repo::Author::new(author, email),
        time: 0,
        parents: Default::default(),
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
        assert_eq!(ix.matching(&query, &Mailmap::none(), &|| false), Some(want), "{q}");
    }
    assert_eq!(ix.matching(&CommitQuery::parse(""), &Mailmap::none(), &|| false), Some(vec![]));
    assert_eq!(ix.matching(&CommitQuery::parse("author:ann@example.com"), &Mailmap::none(), &|| false), Some(vec![0, 4, 5]));
}

#[test]
fn matching_stops_when_asked() {
    let ix = CommitIndex::from_commits(key(), &fixture());
    assert_eq!(ix.matching(&CommitQuery::parse("fix"), &Mailmap::none(), &|| true), None);
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
        assert_eq!(ix.summary_at(p), Some(&*c.summary));
        assert_eq!(ix.body_at(p), Some(&*c.body));
        assert_eq!(ix.person_at(p), Some((c.author.name(), c.author.email())));
        assert_eq!(ix.times_at(p), Some((c.time, c.time)));
    }
    assert_eq!(ix.oid_at(6), None);
    assert_eq!(ix.people.len(), 5, "the two Ann Lee commits share one author");
}

#[test]
fn a_latin1_message_from_git_indexes_its_decoded_text() {
    let dir = scratch_repo("index-latin1-git");
    write(&dir, "a.txt", "a");
    git(&dir, &["add", "."]);
    std::fs::write(dir.join("msg.txt"), b"Caf\xe9 au lait\n\nCr\xe8me body\n").unwrap();
    // git records `encoding latin1` in the commit, and the text is read in it.
    git(&dir, &["-c", "i18n.commitEncoding=latin1", "commit", "-q", "-F", "msg.txt"]);
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!((&*snap.commits[0].summary, &*snap.commits[0].body), ("Café au lait", "Crème body"));
    let ix = build(&dir, &IndexKey::of(&snap), None, &mut |_| {}, &|| false).unwrap().unwrap();
    assert_eq!((ix.summary_at(0), ix.body_at(0)), (Some("Café au lait"), Some("Crème body")));
    assert_eq!(ix.matching(&CommitQuery::parse("café"), &Mailmap::none(), &|| false), Some(vec![0]));
    assert_eq!(ix.matching(&CommitQuery::parse("test"), &Mailmap::none(), &|| false), Some(vec![0]), "the author still matches");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_index_maps_stored_signatures_when_it_answers() {
    let dir = scratch_repo("index-mailmap");
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let mut parent: Option<git2::Commit> = None;
    for (i, (name, email)) in
        [("Ada Lovelace", "ada@example.com"), ("Grace Hopper", "grace@example.com"), ("G. Hopper", "ghopper@navy.example")]
            .into_iter()
            .enumerate()
    {
        let sig = git2::Signature::new(name, email, &git2::Time::new(1_700_000_000 + i as i64, 0)).unwrap();
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "work", &tree, &parents).unwrap();
        parent = Some(repo.find_commit(oid).unwrap());
    }
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    let ix = build(&dir, &IndexKey::of(&snap), None, &mut |_| {}, &|| false).unwrap().unwrap();
    let map = crate::mailmap::MailmapText::from_text("Grace Hopper <grace@example.com> <ghopper@navy.example>\n").open();
    let find = |ix: &CommitIndex, q: &str, map: &Mailmap| ix.matching(&CommitQuery::parse(q), map, &|| false).unwrap();

    // Newest first: G. Hopper, Grace, Ada.
    assert_eq!(find(&ix, "author:grace@example.com", &map), [0, 1]);
    assert_eq!(find(&ix, "grace hopper", &map), [0, 1], "free text matches the mapped name");
    assert!(find(&ix, "author:ghopper", &map).is_empty(), "the address mapped away is gone");
    assert_eq!(find(&ix, "author:grace@example.com", &Mailmap::none()), [1]);
    assert_eq!(find(&ix, "author:ghopper", &Mailmap::none()), [0]);
    assert_eq!(ix.person_at(0), Some(("G. Hopper", "ghopper@navy.example")), "stored as written");

    let store = Store::new(crate::paths::temp_dir().join(format!("glowfly-index-mailmap-{}", std::process::id())));
    let file = store.file_of(&dir).unwrap();
    store.save(&file, &ix).unwrap();
    let (back, _) = store.load(&file, &|| false).expect("loads");
    assert_eq!(find(&back, "author:grace@example.com", &map), [0, 1]);
    assert_eq!(back.person_at(0), Some(("G. Hopper", "ghopper@navy.example")));
    assert_eq!(super::store::FORMAT, 3, "the saved layout is as it was");
    let _ = std::fs::remove_dir_all(store.dir());
    let _ = std::fs::remove_dir_all(&dir);
}
