//! A loaded commit's shared author and texts: interning and lookup keys, the body preview, texts
//! shared on clone and by a reload, and a walk over a fixture history (many authors, unicode
//! names, an empty email, a `.mailmap`) reading exactly what git holds, with one shared copy per
//! author.

use super::*;
use crate::commit_query::{CommitQuery, authors};
use crate::repo::walk::flatten_body;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use std::collections::HashSet;

#[test]
fn authors_are_interned_by_name_and_email() {
    let mut table = Authors::default();
    let ann = table.get("Ann", "ann@example.com");
    assert!(ann.ptr_eq(&table.get("Ann", "ann@example.com")), "the second sighting shares the first");
    let shouted = table.get("Ann", "ANN@example.com");
    assert!(!ann.ptr_eq(&shouted) && ann != shouted, "case is kept, as git keeps it");
    assert_ne!(table.get("ab", "c"), table.get("a", "bc"), "the separator keeps the two strings apart");
    let empty = table.get("", "");
    assert_eq!((empty.name(), empty.email()), ("", ""));
    assert_eq!(table.len(), 5);
    assert_eq!(Author::new("Zoë", "z@example.org"), table.get("Zoë", "z@example.org"), "equal across tables");
    assert_eq!(format!("{ann:?}"), "Ann <ann@example.com>");
}

#[test]
fn text_reads_as_a_str() {
    let t: Text = "Fix the parser".into();
    assert_eq!(t, "Fix the parser");
    assert_eq!(t, String::from("Fix the parser"));
    assert!(t.starts_with("Fix") && t.as_str().len() == 14);
    assert_eq!(String::from(&t), "Fix the parser");
    assert_eq!(format!("{t} {t:?}"), "Fix the parser \"Fix the parser\"");
    assert_eq!(Text::from(String::new()), Text::default());
}

#[test]
fn a_walk_reads_what_git_holds_and_shares_each_author() {
    let dir = scratch_repo("commit-info-walk");
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.find_tree(repo.index().unwrap().write_tree().unwrap()).unwrap();
    let mut people: Vec<(String, String)> = (0..12).map(|i| (format!("Person {i}"), format!("p{i}@example.com"))).collect();
    people.push(("Zoë Ångström".into(), "zoe@example.org".into()));
    people.push(("李小龍".into(), "lee@example.cn".into()));
    let mut parent: Option<git2::Commit> = None;
    for i in 0..60 {
        let (name, email) = &people[i % people.len()];
        let sig = git2::Signature::new(name, email, &git2::Time::new(1_700_000_000 + i as i64, 0)).unwrap();
        let msg = if i % 4 == 0 { format!("Change {i}\n\nBody  line one\n  line two {i}\n") } else { format!("Change {i}") };
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &parents).unwrap();
        parent = Some(repo.find_commit(oid).unwrap());
    }
    let no_email = raw_commit(&repo, tree.id(), Some(parent.unwrap().id()), b"Nobody <>", None, b"no email\n");
    repo.reference("refs/heads/main", no_email, true, "test").unwrap();
    // The graph shows signatures as written; a .mailmap doesn't change them.
    write(&dir, ".mailmap", "Mapped Name <mapped@example.com> Person 0 <p0@example.com>\n");

    let snap = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert_eq!(snap.commits.len(), 61);
    for c in snap.commits.iter() {
        let g = repo.find_commit(c.oid).unwrap();
        let a = g.author();
        assert_eq!((c.author.name(), c.author.email()), (a.name().unwrap(), a.email().unwrap()));
        assert_eq!(
            (c.summary.as_str(), c.body.as_str()),
            (g.summary().unwrap_or_default(), flatten_body(g.body().unwrap_or_default()).as_str())
        );
        assert_eq!(c.time, g.time().seconds());
        assert_eq!(c.parents.to_vec(), g.parent_ids().collect::<Vec<_>>());
    }
    let head = &snap.commits[0];
    assert_eq!((head.author.name(), head.author.email(), head.summary.as_str()), ("Nobody", "", "no email"));
    assert_eq!(snap.commits[4].body, "Body line one line two 56", "the body flattened as before");

    // One copy per author, shared by all of their commits.
    let copies: HashSet<*const str> = snap.commits.iter().map(|c| c.author.name() as *const str).collect();
    assert_eq!(copies.len(), people.len() + 1);
    let zoe: Vec<&CommitInfo> = snap.commits.iter().filter(|c| c.author.name() == "Zoë Ångström").collect();
    assert_eq!(zoe.len(), 4);
    assert!(zoe.iter().all(|c| c.author.ptr_eq(&zoe[0].author)));

    // Search and the author picker see the same names and emails.
    let rows = |q: &str| CommitQuery::parse(q).matching_rows(&snap.commits, &HashSet::new());
    assert_eq!(rows("author:zoë").len(), 4);
    assert_eq!(rows("author:李小龍").len(), 4);
    assert_eq!(rows("author:p0@example.com").len(), 5, "an address matches the whole email");
    assert_eq!(rows("ångström").len(), 4, "free text matches the author's name");
    let picker = authors(&snap.commits);
    assert_eq!(picker.len(), people.len() + 1);
    assert!(picker.iter().any(|a| a.name == "李小龍" && a.email == "lee@example.cn" && a.commits == 4));
    assert!(picker.iter().any(|a| a.name == "Nobody" && a.email.is_empty() && a.commits == 1));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The cut the walk used to store: words joined by spaces, "…" once past 240 bytes.
fn old_capped(body: &str) -> String {
    let mut out = String::new();
    for word in body.split_whitespace() {
        if out.len() > 240 {
            out.push('…');
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

#[test]
fn the_preview_shows_what_the_graph_showed_before_bodies_were_whole() {
    let words = ["a", "fix", "résumé", "ünïcode✨", "x", "longer-word-here", "🎉🎉", "ok"];
    for n in [0, 1, 10, 30, 35, 36, 37, 40, 41, 60, 200] {
        for shift in 0..words.len() {
            let raw: Vec<&str> = (0..n).map(|i| words[(i + shift) % words.len()]).collect();
            let body = flatten_body(&raw.join("\n  "));
            let (start, cut) = body_preview(&body);
            let shown = if cut { format!("{start}…") } else { start.to_owned() };
            assert_eq!(shown, old_capped(&body), "{n} words from {shift}");
            assert!(body.starts_with(start));
        }
    }
    assert_eq!(body_preview(""), ("", false));
}

#[test]
fn texts_are_shared_on_clone_and_empty_ones_share_one_allocation() {
    let a = Text::from("a body");
    assert!(a.clone().ptr_eq(&a));
    assert!(Text::from("").ptr_eq(&Text::default()) && Text::from(String::new()).ptr_eq(&Text::default()));
    assert_eq!(String::from(a.clone()), "a body");
    assert_eq!(a, Text::from("a body".to_owned()));
}

#[test]
fn a_reload_shares_the_texts_of_the_commits_it_takes_again() {
    let dir = scratch_repo("commit-info-share");
    for i in 0..5 {
        write(&dir, "f.txt", &i.to_string());
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", &format!("Change {i}"), "-m", &format!("Body {i} {}", "word ".repeat(80))]);
    }
    let opts = LoadOptions::all(100);
    let first = load(&dir, &opts).unwrap();
    let again = super::super::load_reusing(&dir, &opts, &Default::default(), Some(&first.prior())).unwrap();
    assert_eq!(again.commits.len(), 5);
    for (a, b) in again.commits.iter().zip(first.commits.iter()) {
        assert!(a.body.ptr_eq(&b.body) && a.summary.ptr_eq(&b.summary), "{}", a.summary);
        assert!(a.body.len() > BODY_PREVIEW, "held whole");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
