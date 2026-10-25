use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use git2::Oid;
use std::collections::HashSet;

/// No commits found touching a path term.
fn none() -> HashSet<Oid> {
    HashSet::new()
}

fn commit(summary: &str, author: &str, email: &str) -> CommitInfo {
    CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: summary.into(),
        body: Default::default(),
        author: crate::repo::Author::new(author, email),
        time: 0,
        parents: Default::default(),
    }
}

#[test]
fn plain_text_is_one_phrase() {
    let q = CommitQuery::parse("  Fix   the ");
    assert_eq!(q, CommitQuery { text: "fix the".into(), authors: vec![], paths: vec![] });
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
    assert_eq!(CommitQuery::parse(&with_author("", Some("ann@example.com"))).matching_rows(&commits, &none()), [0]);
    assert_eq!(CommitQuery::parse("author:ANN@Example.COM").matching_rows(&commits, &none()), [0]);
    assert_eq!(CommitQuery::parse("author:a@x.com").matching_rows(&commits, &none()), [2]);
    assert_eq!(CommitQuery::parse("author:joann@example.com").matching_rows(&commits, &none()), [1]);
    // A longer domain is a different address.
    assert!(!CommitQuery::parse("author:ann@example.co").matches(&ann));
    // Partial addresses still match as text.
    for q in ["author:ann@", "author:@example.com", "author:ann@example"] {
        assert_eq!(CommitQuery::parse(q).matching_rows(&commits, &none()), [0, 1], "{q}");
    }
    // Several picked addresses: either, exactly.
    assert_eq!(CommitQuery::parse("author:ann@example.com author:a@x.com").matching_rows(&commits, &none()), [0, 2]);
}

#[test]
fn free_text_ignores_case_and_matches_id_prefixes() {
    let c = CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: "Fix the Parser".into(),
        body: "Über-long BODY text".into(),
        author: crate::repo::Author::new("Ada Lovelace", "ada@example.com"),
        time: 0,
        parents: Default::default(),
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
    assert_eq!(CommitQuery::parse("fix").matching_rows(&commits, &none()), [0, 2]);
    assert_eq!(CommitQuery::parse("author:grace").matching_rows(&commits, &none()), [2]);
    assert!(CommitQuery::parse("").matching_rows(&commits, &none()).is_empty());
    assert!(CommitQuery::parse("author:").matching_rows(&commits, &none()).is_empty());
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

    let grace: Vec<usize> = snap
        .commits
        .iter()
        .enumerate()
        .filter(|(_, c)| c.author.email().eq_ignore_ascii_case("grace@example.com"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(grace.len(), 3);
    assert_eq!(CommitQuery::parse("author:grace").matching_rows(&snap.commits, &none()), grace);
    let _ = std::fs::remove_dir_all(&dir);
}

fn paths(q: &str) -> Vec<PathTerm> {
    CommitQuery::parse(q).paths().to_vec()
}

fn term(v: &str) -> PathTerm {
    PathTerm::parse(v).unwrap()
}

#[test]
fn path_terms_are_parsed() {
    for q in ["path:src", "PATH:src", "file:src", "File:SRC", "path:\"src\""] {
        assert_eq!(paths(q), [term("src")], "{q}");
    }
    assert_eq!(paths("path:\"My Docs/\" fix"), [term("My Docs/")]);
    let q = CommitQuery::parse("fix path:\"My Do");
    assert_eq!(q.paths(), [term("My Do")], "an open quote runs to the end");
    assert_eq!(q.text, "fix");
    for empty in ["path:", "path:/", "path:./", "file:\"\""] {
        let q = CommitQuery::parse(empty);
        assert!(q.is_empty() && !q.has_paths(), "{empty}");
    }
    assert_eq!(paths("path:src file:SRC path:docs file:\"src \""), [term("src"), term("docs")], "repeats collapse");
    assert_eq!(paths("path:./src path:/src"), [term("/src")], "a leading ./ or / anchors at the root");
    let q = CommitQuery::parse("author:ada path:src/ui fix it file:*.rs");
    assert_eq!(q.authors(), ["ada"]);
    assert_eq!(q.text, "fix it");
    assert_eq!(q.paths(), [term("src/ui"), term("*.rs")]);
    assert!(q.has_paths() && !q.is_empty());
    assert!(CommitQuery::parse("path:x").has_paths() && !CommitQuery::parse("path:x").is_empty());
    assert!(!CommitQuery::parse("fix").has_paths());
    // A word that only contains the prefix is text.
    assert_eq!(CommitQuery::parse("xpath:src").text, "xpath:src");
}

#[test]
fn matching_rows_need_a_touching_commit_for_path_terms() {
    let oid = |i: u8| Oid::from_bytes(&[i; 20]).unwrap();
    let with_oid = |i: u8, summary: &str, author: &str| CommitInfo { oid: oid(i), ..commit(summary, author, "x@example.com") };
    let commits = [with_oid(1, "fix one", "Ada"), with_oid(2, "fix two", "Grace"), with_oid(3, "other", "Ada")];
    let touching: HashSet<Oid> = [oid(1), oid(2), oid(3)].into();
    let only = |o: &[u8]| o.iter().map(|&i| oid(i)).collect::<HashSet<_>>();
    assert_eq!(CommitQuery::parse("path:src").matching_rows(&commits, &touching), [0, 1, 2]);
    assert_eq!(CommitQuery::parse("path:src fix").matching_rows(&commits, &only(&[2, 3])), [1]);
    assert_eq!(CommitQuery::parse("path:src author:ada").matching_rows(&commits, &only(&[2, 3])), [2]);
    assert!(CommitQuery::parse("path:src").matching_rows(&commits, &none()).is_empty());
    // Without path terms the set is ignored.
    assert_eq!(CommitQuery::parse("fix").matching_rows(&commits, &none()), [0, 1]);
    assert_eq!(CommitQuery::parse("fix").matching_rows(&commits, &only(&[3])), [0, 1]);
    assert!(CommitQuery::parse("path:").matching_rows(&commits, &touching).is_empty());
}

#[test]
fn with_author_keeps_path_terms_as_written() {
    assert_eq!(
        with_author("fix path:\"My Docs/\" author:ada file:x", Some("grace@example.com")),
        "author:grace@example.com fix path:\"My Docs/\" file:x"
    );
    assert_eq!(with_author("PATH:src author:ada", None), "PATH:src");
    let q = CommitQuery::parse(&with_author("path:\"My Docs/\" file:x", Some("Ada Lovelace")));
    assert_eq!(q.paths(), [term("My Docs/"), term("x")]);
    assert_eq!(q.authors(), ["ada lovelace"]);
}

#[test]
fn text_and_author_parts_match_on_their_own() {
    let oid = Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap();
    let q = CommitQuery::parse("ada");
    assert_eq!(q.text(), "ada");
    // Free text finds the author's name, but the email only through an author term.
    assert!(q.text_matches(&oid, "", "", "Ada Lovelace"));
    assert!(!q.text_matches(&oid, "", "", "Grace"));
    assert!(q.author_matches("Grace", "grace@example.com"), "no author terms: any author");
    let q = CommitQuery::parse("author:ada@example.com");
    assert_eq!(q.text(), "");
    assert!(q.text_matches(&oid, "", "", ""), "no free text: any text");
    assert!(q.author_matches("Someone", "ADA@example.com"));
    assert!(!q.author_matches("Ada", "joada@example.com"), "a full address must be equal");
    let q = CommitQuery::parse("author:lovelace body words");
    assert!(q.text_matches(&oid, "summary", "some body words here", ""));
    assert!(q.author_matches("Ada Lovelace", "a@example.com"));
    assert!(!q.author_matches("Ada", "a@example.com"));
    assert!(CommitQuery::parse("0ABC").text_matches(&oid, "", "", ""), "an id prefix in any case");
}
