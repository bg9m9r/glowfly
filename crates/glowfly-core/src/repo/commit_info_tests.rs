//! A loaded commit's shared author and boxed texts: interning and lookup keys, and a walk over a
//! fixture history (many authors, unicode names, an empty email, a `.mailmap`) reading exactly
//! what git holds, with one shared copy per author.

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

/// Writes a commit object by hand, so its author can be one git's porcelain won't make.
fn raw_commit(repo: &git2::Repository, tree: git2::Oid, parent: git2::Oid, author: &str, msg: &str) -> git2::Oid {
    let text = format!(
        "tree {tree}\nparent {parent}\nauthor {author} 1700000900 +0000\ncommitter Test <test@example.com> 1700000900 +0000\n\n{msg}"
    );
    repo.odb().unwrap().write(git2::ObjectType::Commit, text.as_bytes()).unwrap()
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
    let no_email = raw_commit(&repo, tree.id(), parent.unwrap().id(), "Nobody <>", "no email\n");
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
