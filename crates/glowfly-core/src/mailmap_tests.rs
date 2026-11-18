//! The mailmap: its entry forms, its sources and their order, what it leaves out, and what it
//! shows in a load (graph authors, the author picker, search), a reload and a load for more.

use super::*;
use crate::commit_query::{CommitQuery, authors};
use crate::paths;
use crate::repo::{GraphCache, LoadOptions, MoreLoaded, Snapshot, load, load_more, load_reusing, refs_key};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const GRACE: &str = "Grace Hopper <grace@example.com> <ghopper@navy.example>\n";

/// A commit of the empty tree, signed `who` (name, email) at `time`, on top of `parent`; the
/// `main` branch moves to it.
fn commit_as(repo: &Repository, parent: Option<Oid>, who: (&str, &str), time: i64) -> Oid {
    let sig = git2::Signature::new(who.0, who.1, &git2::Time::new(time, 0)).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let parents: Vec<git2::Commit> = parent.iter().map(|p| repo.find_commit(*p).unwrap()).collect();
    let refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = repo.commit(None, &sig, &sig, "work", &tree, &refs).unwrap();
    repo.reference("refs/heads/main", oid, true, "test").unwrap();
    oid
}

/// Grace Hopper under two identities and Ada once, oldest first; no mailmap yet.
fn people(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    let repo = Repository::open(&dir).unwrap();
    let who = [("Grace Hopper", "grace@example.com"), ("G. Hopper", "ghopper@navy.example"), ("Ada Lovelace", "ada@example.com")];
    let mut parent = None;
    for (i, w) in who.into_iter().enumerate() {
        parent = Some(commit_as(&repo, parent, w, 1_700_000_000 + i as i64));
    }
    dir
}

fn shown(snap: &Snapshot) -> Vec<(String, String)> {
    snap.commits.iter().map(|c| (c.author.name().to_owned(), c.author.email().to_owned())).collect()
}

fn pair(name: &str, email: &str) -> (String, String) {
    (name.to_owned(), email.to_owned())
}

fn open(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(100)).unwrap()
}

#[test]
fn resolve_follows_the_four_entry_forms() {
    let map = MailmapText::from_text(
        "# who is who\n\
         Ann Proper <ann@old.example>\n\
         <bob@new.example> <bob@old.example>\n\
         \n\
         Cy Proper <cy@new.example> <cy@old.example>\n\
         Di Proper <di@new.example> Di Old <di@old.example>\n\
         Ed One <ed@x.example>\n\
         Ed Two <ed@x.example>\n",
    )
    .open();
    assert!(!map.is_none());
    let r = |n: &str, e: &str| map.resolve(n, e);
    let some = |n: &str, e: &str| Some(pair(n, e));
    assert_eq!(r("anything", "ann@old.example"), some("Ann Proper", "ann@old.example"), "name by email");
    assert_eq!(r("Bob", "bob@old.example"), some("Bob", "bob@new.example"), "email by email");
    assert_eq!(r("x", "cy@old.example"), some("Cy Proper", "cy@new.example"), "both by email");
    assert_eq!(r("Di Old", "di@old.example"), some("Di Proper", "di@new.example"), "both by name and email");
    assert_eq!(r("Someone", "di@old.example"), None, "another name under that address");
    assert_eq!(r("x", "ed@x.example"), some("Ed Two", "ed@x.example"), "a later line overrides an earlier one");
    assert_eq!(r("Ann Proper", "ann@old.example"), None, "already as the entry says");
    assert_eq!(r("Nobody", "nobody@example.com"), None, "no entry");
    let person = map.person(crate::commit_text::Person { name: "q".into(), email: "cy@old.example".into() });
    assert_eq!((person.name.as_str(), person.email.as_str()), ("Cy Proper", "cy@new.example"));
}

#[test]
fn a_signature_libgit2_would_trim_or_refuse_stays_as_written() {
    let map = MailmapText::from_text("Proper <other@example.com>\n").open();
    for (name, email) in
        [("Name,", "x@example.com"), ("", "a@example.com"), ("N", ""), ("A <B>", "a@example.com"), ("N", "a>b@example.com")]
    {
        assert_eq!(map.resolve(name, email), None, "{name:?} {email:?}");
    }
    assert_eq!(Mailmap::none().resolve("Name", "other@example.com"), None);
    assert!(Mailmap::none().is_none());
    assert!(MailmapText::default().open().is_none());
}

#[test]
fn text_stamps() {
    for blank in ["", "  \n\t\n"] {
        let t = MailmapText::from_text(blank);
        assert!(t.is_empty() && t.stamp() == 0, "{blank:?}");
        assert_eq!(t, MailmapText::default());
    }
    let (a, b, c) = (MailmapText::from_text(GRACE), MailmapText::from_text(GRACE), MailmapText::from_text("Ada <a@example.com>\n"));
    assert!(!a.is_empty() && a.stamp() != 0);
    assert_eq!((&a, a.stamp()), (&b, b.stamp()));
    assert!(a != c && a.stamp() != c.stamp());
    let shown = format!("{a:?}");
    assert!(!shown.contains("Hopper") && !shown.contains("example"), "{shown}");
}

#[test]
fn two_names_merge_into_one_author() {
    let dir = people("mailmap-merge");
    write(&dir, ".mailmap", GRACE);
    let snap = open(&dir);
    // Newest first: Ada, then Grace's two commits.
    assert_eq!(
        shown(&snap),
        [pair("Ada Lovelace", "ada@example.com"), pair("Grace Hopper", "grace@example.com"), pair("Grace Hopper", "grace@example.com")]
    );
    assert!(snap.commits[1].author.ptr_eq(&snap.commits[2].author), "one shared author");
    assert!(!snap.mailmap.is_empty());

    let picker = authors(&snap.commits);
    assert_eq!(picker.len(), 2);
    let grace = picker.iter().find(|a| a.email == "grace@example.com").expect("one Grace entry");
    assert_eq!((grace.name.as_str(), grace.commits), ("Grace Hopper", 2));

    let rows = |q: &str| CommitQuery::parse(q).matching_rows(&snap.commits, &HashSet::new());
    assert_eq!(rows("author:grace@example.com"), [1, 2], "finds both");
    assert!(rows("author:ghopper").is_empty(), "the address mapped away no longer matches");
    assert!(rows("G. Hopper").is_empty());
    assert_eq!(rows("hopper"), [1, 2]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn log_mailmap_false_leaves_signatures_as_written() {
    let dir = people("mailmap-off");
    write(&dir, ".mailmap", GRACE);
    set_config(&dir, &[("log.mailmap", "false")]);
    let snap = open(&dir);
    assert_eq!(snap.commits[2].author.name(), "Grace Hopper");
    assert_eq!((snap.commits[1].author.name(), snap.commits[1].author.email()), ("G. Hopper", "ghopper@navy.example"));
    assert!(snap.mailmap.is_empty());
    assert_eq!(authors(&snap.commits).len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mailmap_blob_is_read() {
    let dir = people("mailmap-blob");
    write(&dir, "people.map", GRACE);
    git(&dir, &["add", "people.map"]);
    git(&dir, &["commit", "-q", "-m", "people"]);
    set_config(&dir, &[("mailmap.blob", "HEAD:people.map")]);
    assert!(!dir.join(".mailmap").exists());
    let snap = open(&dir);
    let by: Vec<&str> = snap.commits.iter().map(|c| c.author.email()).collect();
    assert_eq!(by.iter().filter(|e| **e == "grace@example.com").count(), 2, "{by:?}");
    assert!(!by.contains(&"ghopper@navy.example"));
    assert_eq!(authors(&snap.commits).len(), 3, "Test, Ada and Grace");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sources_override_in_gits_order() {
    let dir = scratch_repo("mailmap-order");
    let repo = Repository::open(&dir).unwrap();
    commit_as(&repo, None, ("Orig", "x@example.com"), 1_700_000_000);
    let name_of = |dir: &Path| open(dir).commits[0].author.name().to_owned();
    assert_eq!(name_of(&dir), "Orig");

    write(&dir, ".mailmap", "From Tree <x@example.com>\n");
    assert_eq!(name_of(&dir), "From Tree");

    let blob = repo.blob(b"From Blob <x@example.com>\n").unwrap();
    set_config(&dir, &[("mailmap.blob", &blob.to_string())]);
    assert_eq!(name_of(&dir), "From Blob", "the blob overrides .mailmap");

    write(&dir, "file.map", "From File <x@example.com>\n");
    set_config(&dir, &[("mailmap.file", "file.map")]);
    assert_eq!(name_of(&dir), "From File", "the file overrides both; a relative path starts at the work tree");

    let abs = paths::temp_dir().join("glowfly-mailmap-order.map");
    std::fs::write(&abs, "From Abs <x@example.com>\n").unwrap();
    set_config(&dir, &[("mailmap.file", &abs.to_string_lossy())]);
    assert_eq!(name_of(&dir), "From Abs", "an absolute path works too");
    let _ = std::fs::remove_file(&abs);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_mailmap_is_not_followed() {
    let dir = people("mailmap-link");
    let names = |dir: &Path| shown(&open(dir)).into_iter().map(|p| p.0).collect::<Vec<_>>();
    let written = ["Ada Lovelace", "G. Hopper", "Grace Hopper"];

    write(&dir, "real.map", GRACE);
    if file_link("real.map", &dir.join(".mailmap")) {
        assert_eq!(names(&dir), written, "git doesn't follow a linked .mailmap");
        // The other sources still apply beside it.
        let blob = Repository::open(&dir).unwrap().blob(GRACE.as_bytes()).unwrap();
        set_config(&dir, &[("mailmap.blob", &blob.to_string())]);
        assert_eq!(names(&dir), ["Ada Lovelace", "Grace Hopper", "Grace Hopper"]);
        set_config(&dir, &[("mailmap.blob", "")]);
        std::fs::remove_file(dir.join(".mailmap")).unwrap();
    } else {
        eprintln!("skipped the link case: this system can't make a link here");
    }

    std::fs::create_dir(dir.join(".mailmap")).unwrap();
    assert_eq!(names(&dir), written, "a folder is left out");
    std::fs::remove_dir(dir.join(".mailmap")).unwrap();

    let mut big = GRACE.to_owned();
    big.push_str(&"#".repeat(1 << 20));
    write(&dir, ".mailmap", &big);
    assert_eq!(names(&dir), written, "a source over 1 MiB is left out");
    write(&dir, ".mailmap", &format!("{GRACE}\0"));
    assert_eq!(names(&dir), written, "so is one holding a NUL");
    write(&dir, ".mailmap", GRACE);
    assert_eq!(names(&dir), ["Ada Lovelace", "Grace Hopper", "Grace Hopper"], "a plain file does apply");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_decoded_name_matches_a_utf8_entry() {
    let dir = scratch_repo("mailmap-latin1");
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.treebuilder(None).unwrap().write().unwrap();
    let oid = raw_commit(&repo, tree, None, LATIN1_WHO, Some("ISO-8859-1"), LATIN1_MSG);
    repo.reference("refs/heads/main", oid, true, "test").unwrap();
    assert_eq!(open(&dir).commits[0].author.name(), "René Müller", "read as written");
    write(&dir, ".mailmap", "Rene Muller <rene@example.com> René Müller <rené@example.com>\n");
    let snap = open(&dir);
    assert_eq!((snap.commits[0].author.name(), snap.commits[0].author.email()), ("Rene Muller", "rene@example.com"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edited_mailmap_shows_on_the_next_load() {
    let dir = people("mailmap-edit");
    let opts = LoadOptions::all(100);
    let reload = |before: &Snapshot| load_reusing(&dir, &opts, &GraphCache::default(), Some(&before.prior())).unwrap();
    let first = open(&dir);
    assert_eq!(first.commits[2].author.name(), "Grace Hopper");

    write(&dir, ".mailmap", GRACE);
    assert_ne!(refs_key(&dir).unwrap(), first.refs_key, "the edit is a change a refresh loads in full");
    let second = reload(&first);
    assert_eq!(second.commits[1].author.email(), "grace@example.com");
    assert!(!second.commits[0].summary.ptr_eq(&first.commits[0].summary), "read again once");

    let third = reload(&second);
    assert_eq!(shown(&third), shown(&second));
    assert!(third.commits[0].summary.ptr_eq(&second.commits[0].summary), "unchanged: taken from the one before");
    assert_eq!(refs_key(&dir).unwrap(), second.refs_key);

    std::fs::remove_file(dir.join(".mailmap")).unwrap();
    assert_ne!(refs_key(&dir).unwrap(), third.refs_key);
    let back = reload(&third);
    assert_eq!(back.commits[1].author.email(), "ghopper@navy.example", "the written names are back");
    assert!(back.mailmap.is_empty());

    // Only the file mailmap.file names changes.
    write(&dir, "other.map", "One <grace@example.com>\n");
    set_config(&dir, &[("mailmap.file", "other.map")]);
    let one = refs_key(&dir).unwrap();
    write(&dir, "other.map", "Two <grace@example.com>\n");
    assert_ne!(refs_key(&dir).unwrap(), one);
    assert_eq!(reload(&back).commits[2].author.name(), "Two");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_more_maps_with_the_snapshots_mailmap() {
    let dir = scratch_repo("mailmap-more");
    let repo = Repository::open(&dir).unwrap();
    let mut parent = None;
    for i in 0..6 {
        let who = if i % 2 == 0 { ("Grace Hopper", "grace@example.com") } else { ("G. Hopper", "ghopper@navy.example") };
        parent = Some(commit_as(&repo, parent, who, 1_700_000_000 + i));
    }
    write(&dir, ".mailmap", GRACE);
    let mut snap = load(&dir, &LoadOptions::all(3)).unwrap();
    assert!(snap.truncated);
    let MoreLoaded::Page(page) = load_more(&dir, &snap.prior(), 6).unwrap() else { panic!("a page") };
    snap.extend_history(*page).expect("goes on from the snapshot");
    assert_eq!(snap.commits.len(), 6);
    for c in snap.commits.iter() {
        assert_eq!((c.author.name(), c.author.email()), ("Grace Hopper", "grace@example.com"));
        assert!(c.author.ptr_eq(&snap.commits[0].author), "shares the author of the rows above");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_detail_maps_author_and_committer() {
    let dir = scratch_repo("detail-mailmap");
    write(&dir, "a.txt", "one\n");
    let repo = Repository::open(&dir).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let author = git2::Signature::new("A. Lovelace", "ada@old.example", &git2::Time::new(1_767_000_000, 0)).unwrap();
    let committer = git2::Signature::new("G. Hopper", "ghopper@old.example", &git2::Time::new(1_767_086_400, 0)).unwrap();
    let oid = repo.commit(Some("HEAD"), &author, &committer, "Add a\n", &tree, &[]).unwrap();
    write(&dir, ".mailmap", "Ada Lovelace <ada@example.com> <ada@old.example>\nGrace Hopper <grace@example.com> <ghopper@old.example>\n");

    let d = crate::repo::commit_detail(&dir, oid).unwrap();
    assert_eq!((d.author.as_str(), d.email.as_str()), ("Ada Lovelace", "ada@example.com"));
    assert_eq!((d.committer.as_str(), d.committer_email.as_str()), ("Grace Hopper", "grace@example.com"));
    assert_eq!((d.author_time, d.time), (1_767_000_000, 1_767_086_400));

    set_config(&dir, &[("log.mailmap", "false")]);
    let d = crate::repo::commit_detail(&dir, oid).unwrap();
    assert_eq!((d.author.as_str(), d.email.as_str()), ("A. Lovelace", "ada@old.example"));
    assert_eq!((d.committer.as_str(), d.committer_email.as_str()), ("G. Hopper", "ghopper@old.example"));
    let _ = std::fs::remove_dir_all(&dir);
}
