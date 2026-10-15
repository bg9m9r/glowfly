use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use git2::Oid;

fn commit(summary: &str, author: &str, email: &str) -> CommitInfo {
    CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: summary.into(),
        body: String::new(),
        author: author.into(),
        email: email.into(),
        time: 0,
        parents: Vec::new(),
    }
}

#[test]
fn plain_text_is_one_phrase() {
    let q = CommitQuery::parse("  Fix   the ");
    assert_eq!(q, CommitQuery { text: "fix the".into(), authors: vec![] });
    assert!(q.matches(&commit("Fix the Parser", "Ada", "ada@example.com")));
    assert!(!q.matches(&commit("the fix", "Ada", "ada@example.com")));
}

#[test]
fn author_terms_are_parsed() {
    let q = CommitQuery::parse("author:ada fix");
    assert_eq!(q.authors(), ["ada"]);
    assert_eq!(q.text, "fix");
    assert_eq!(CommitQuery::parse("Author:ADA").authors(), ["ada"]);
    let q = CommitQuery::parse("author:\"Ada Lovelace\" parser");
    assert_eq!(q.authors(), ["ada lovelace"]);
    assert_eq!(q.text, "parser");
    let q = CommitQuery::parse("fix author:\"Ada Love");
    assert_eq!(q.authors(), ["ada love"]);
    assert_eq!(q.text, "fix");
}

#[test]
fn an_empty_author_term_is_ignored() {
    assert!(CommitQuery::parse("author:").is_empty());
    assert!(CommitQuery::parse("  author:\"\"  ").is_empty());
    let q = CommitQuery::parse("author: fix");
    assert!(q.authors().is_empty());
    assert_eq!(q.text, "fix");
    assert!(CommitQuery::parse("").is_empty());
}

#[test]
fn author_values_match_name_or_email() {
    let ada = commit("Fix the Parser", "Ada Lovelace", "ada@example.com");
    let jorg = commit("Löser", "Jörg Müller", "jm@example.org");
    for q in ["author:example.com", "author:lovelace", "author:ADA@"] {
        assert!(CommitQuery::parse(q).matches(&ada), "{q}");
    }
    assert!(!CommitQuery::parse("author:example.com").matches(&jorg));
    assert!(CommitQuery::parse("author:jörg").matches(&jorg));
    assert!(CommitQuery::parse("author:JÖRG").matches(&jorg));
    // Text AND author.
    assert!(CommitQuery::parse("author:ada parser").matches(&ada));
    assert!(!CommitQuery::parse("author:ada löser").matches(&ada));
    assert!(!CommitQuery::parse("author:jörg parser").matches(&ada));
    // Several author terms: either.
    let both = CommitQuery::parse("author:ada author:jörg");
    assert!(both.matches(&ada) && both.matches(&jorg));
    // An id prefix with an author term.
    assert!(CommitQuery::parse("author:ada 0abc").matches(&ada));
    assert!(!CommitQuery::parse("author:jörg 0abc").matches(&ada));
}

#[test]
fn a_full_address_matches_only_that_email() {
    let ann = commit("one", "Ann", "ann@example.com");
    let joann = commit("two", "Joann", "joann@example.com");
    let a = commit("three", "A", "A@X.com");
    let ba = commit("four", "Ba", "ba@x.com");
    let commits = [ann.clone(), joann, a, ba];
    // As the picker writes it, and in any case.
    assert_eq!(CommitQuery::parse(&with_author("", Some("ann@example.com"))).matching_rows(&commits), [0]);
    assert_eq!(CommitQuery::parse("author:ANN@Example.COM").matching_rows(&commits), [0]);
    assert_eq!(CommitQuery::parse("author:a@x.com").matching_rows(&commits), [2]);
    assert_eq!(CommitQuery::parse("author:joann@example.com").matching_rows(&commits), [1]);
    // A longer domain is a different address.
    assert!(!CommitQuery::parse("author:ann@example.co").matches(&ann));
    // Partial addresses still match as text.
    for q in ["author:ann@", "author:@example.com", "author:ann@example"] {
        assert_eq!(CommitQuery::parse(q).matching_rows(&commits), [0, 1], "{q}");
    }
    // Several picked addresses: either, exactly.
    assert_eq!(CommitQuery::parse("author:ann@example.com author:a@x.com").matching_rows(&commits), [0, 2]);
}

#[test]
fn free_text_ignores_case_and_matches_id_prefixes() {
    let c = CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: "Fix the Parser".into(),
        body: "Über-long BODY text".into(),
        author: "Ada Lovelace".into(),
        email: "ada@example.com".into(),
        time: 0,
        parents: Vec::new(),
    };
    for q in ["parser", "fix the", "über-long", "body", "lovelace", "0abc", "0abcdef1234567890abcdef1234567890abcdef1"] {
        assert!(CommitQuery::parse(q).matches(&c), "{q}");
    }
    for q in ["parsers", "abcdef", "ada@", "0abcdef1234567890abcdef1234567890abcdef12"] {
        assert!(!CommitQuery::parse(q).matches(&c), "{q}");
    }
}

#[test]
fn matching_rows_are_ascending() {
    let commits = [
        commit("fix one", "Ada", "ada@example.com"),
        commit("other", "Ada", "ada@example.com"),
        commit("fix two", "Grace", "grace@example.com"),
    ];
    assert_eq!(CommitQuery::parse("fix").matching_rows(&commits), [0, 2]);
    assert_eq!(CommitQuery::parse("author:grace").matching_rows(&commits), [2]);
    assert!(CommitQuery::parse("").matching_rows(&commits).is_empty());
    assert!(CommitQuery::parse("author:").matching_rows(&commits).is_empty());
}

#[test]
fn with_author_rewrites_the_author_terms() {
    assert_eq!(with_author("fix author:ada parser author:\"Grace H\"", Some("jm@example.org")), "author:jm@example.org fix parser");
    assert_eq!(with_author("fix", Some("Ada Lovelace")), "author:\"Ada Lovelace\" fix");
    assert_eq!(with_author("author:ada fix  the", None), "fix the");
    assert_eq!(with_author("author:ada", None), "");
    assert_eq!(with_author("", Some("ada@example.com")), "author:ada@example.com");
    for (q, v) in [("fix author:x", "Ada@Example.com"), ("", "Ada Lovelace"), ("author:\"a b", "grace")] {
        assert_eq!(CommitQuery::parse(&with_author(q, Some(v))).authors(), [v.to_lowercase()], "{q} / {v}");
    }
}

#[test]
fn authors_of_a_repository_group_by_email() {
    let dir = scratch_repo("commit-query-authors");
    let commit_as = |name: &str, email: &str, msg: &str| {
        write(&dir, "a.txt", msg);
        git(&dir, &["add", "."]);
        git(&dir, &["-c", &format!("user.name={name}"), "-c", &format!("user.email={email}"), "commit", "-q", "-m", msg]);
    };
    commit_as("Test", "test@example.com", "first");
    commit_as("Grace", "grace@example.com", "second");
    commit_as("Grace", "grace@example.com", "third");
    commit_as("Grace Hopper", "Grace@Example.com", "fourth");
    commit_as("Test", "test@example.com", "fifth");
    commit_as("Ada", "ada@example.com", "sixth");

    let snap = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert_eq!(snap.commits.len(), 6);
    let list = authors(&snap.commits);
    let rows: Vec<(&str, &str, usize)> = list.iter().map(|a| (a.name.as_str(), a.email.as_str(), a.commits)).collect();
    assert_eq!(rows, [("Grace Hopper", "Grace@Example.com", 3), ("Test", "test@example.com", 2), ("Ada", "ada@example.com", 1)]);

    let grace: Vec<usize> =
        snap.commits.iter().enumerate().filter(|(_, c)| c.email.eq_ignore_ascii_case("grace@example.com")).map(|(i, _)| i).collect();
    assert_eq!(grace.len(), 3);
    assert_eq!(CommitQuery::parse("author:grace").matching_rows(&snap.commits), grace);
    let _ = std::fs::remove_dir_all(&dir);
}
