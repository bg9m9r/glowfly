//! The mailmap's letter case, how entries combine and how lines are read, each as git does, and a
//! fixture history compared with `git log --use-mailmap`.

use super::tests::commit_as;
use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::collections::HashMap;

fn map(text: &str) -> Mailmap {
    MailmapText::from_text(text).open()
}

/// Asserts that `m` shows `who` (name, email) as `want`; None for as written.
fn shows(m: &Mailmap, who: (&str, &str), want: Option<(&str, &str)>) {
    let got = m.resolve(who.0, who.1);
    let want = want.map(|(n, e)| (n.to_owned(), e.to_owned()));
    assert_eq!(got, want, "{who:?}");
}

#[test]
fn an_address_matches_in_any_letter_case() {
    let m = map("Jane Doe <jane@new.example> <Jane@Example.com>\n");
    shows(&m, ("j", "jane@example.com"), Some(("Jane Doe", "jane@new.example")));
    shows(&m, ("j", "JANE@EXAMPLE.COM"), Some(("Jane Doe", "jane@new.example")));
    let m = map("Jane Doe <jane@new.example> <jane@example.com>\n");
    shows(&m, ("j", "JANE@Example.COM"), Some(("Jane Doe", "jane@new.example")));

    // A field the entry doesn't give stays as the commit writes it.
    let m = map("Jane Doe <Jane@Example.com>\n");
    shows(&m, ("j", "jane@example.com"), Some(("Jane Doe", "jane@example.com")));
    shows(&m, ("JANE DOE", "jane@example.com"), Some(("Jane Doe", "jane@example.com")));
    shows(&m, ("Jane Doe", "JANE@EXAMPLE.COM"), None);

    let m = map("<jane@new.example> <JANE@old.example>\n");
    shows(&m, ("Jay", "jane@OLD.example"), Some(("Jay", "jane@new.example")));
}

#[test]
fn a_name_and_email_entry_matches_a_differently_cased_name() {
    let m = map("Di Proper <di@new.example> Di Old <di@old.example>\n");
    shows(&m, ("DI OLD", "DI@old.example"), Some(("Di Proper", "di@new.example")));
    shows(&m, ("di old", "di@old.example"), Some(("Di Proper", "di@new.example")));
    shows(&m, ("Someone", "di@old.example"), None);
    shows(&m, ("Di  Old", "di@old.example"), None);
}

#[test]
fn of_two_entries_differing_only_in_case_the_later_wins() {
    let m = map("One <a@x.example>\nTwo <A@X.example>\n");
    shows(&m, ("q", "a@x.example"), Some(("Two", "a@x.example")));
    shows(&m, ("q", "A@X.EXAMPLE"), Some(("Two", "A@X.EXAMPLE")));

    let m = map("N1 <e1@x.example> Old <a@x.example>\n<e2@x.example> OLD <A@x.example>\n");
    shows(&m, ("Old", "a@x.example"), Some(("Old", "e2@x.example")));
}

#[test]
fn non_ascii_letters_are_compared_as_written() {
    let m = map("Rene <rene@x.example> René Müller <r@x.example>\n");
    let rene = Some(("Rene", "rene@x.example"));
    shows(&m, ("René Müller", "r@x.example"), rene);
    shows(&m, ("rené müller", "r@x.example"), rene);
    shows(&m, ("RENé MüLLER", "R@x.example"), rene);
    shows(&m, ("RENÉ MÜLLER", "r@x.example"), None);

    let m = map("Zed <zed@x.example> <zoë@x.example>\n");
    shows(&m, ("z", "ZOë@X.example"), Some(("Zed", "zed@x.example")));
    shows(&m, ("z", "ZOË@x.example"), None);

    // The Kelvin sign lower-cases to `k` in Unicode, not in git.
    let m = map("Kay <k@x.example>\n");
    shows(&m, ("n", "\u{212A}@x.example"), None);
}

#[test]
fn entries_for_one_address_combine_as_gits_do() {
    let ann = Some(("Ann", "new@x.example"));
    shows(&map("Ann <a@x.example>\n<new@x.example> <a@x.example>\n"), ("q", "a@x.example"), ann);
    shows(&map("<new@x.example> <a@x.example>\nAnn <a@x.example>\n"), ("q", "a@x.example"), ann);
    shows(&map("Ann <new@x.example> <a@x.example>\nBea <a@x.example>\n"), ("q", "a@x.example"), Some(("Bea", "new@x.example")));

    let m = map("Simple <simple@x.example> <a@x.example>\n<only@x.example> Old <a@x.example>\n");
    shows(&m, ("Old", "a@x.example"), Some(("Old", "only@x.example")));
    shows(&m, ("OLD", "A@x.example"), Some(("OLD", "only@x.example")));
    shows(&m, ("other", "a@x.example"), Some(("Simple", "simple@x.example")));

    shows(&map("N1 <e1@x.example> Old <a@x.example>\n"), ("Other", "a@x.example"), None);
}

#[test]
fn lines_are_read_as_git_reads_them() {
    let m = map(concat!(
        "# Hidden <a@x.example>\n",
        " #Indented <b@x.example>\n",
        "Tail <c@x.example> # note\n",
        "Tail2 <d2@x.example> <d@x.example> # note <z@x.example>\n",
        "Tail3 <e@x.example> # see <bug@x.example>\n",
        "  \tPadded Name\t <p@x.example>\r\n",
        "Sp <new@x.example> < spaced@x.example >\r\n",
        "NoMail <new@x.example> <>\n",
        "Ignored <>\n",
        "Ignored2 <> <i@x.example>\n",
        "Broken <k@x.example\n",
        "Last <l@x.example>",
    ));
    shows(&m, ("q", "a@x.example"), None);
    shows(&m, ("q", "b@x.example"), Some(("#Indented", "b@x.example")));
    shows(&m, ("q", "c@x.example"), Some(("Tail", "c@x.example")));
    shows(&m, ("q", "d@x.example"), Some(("Tail2", "d2@x.example")));
    shows(&m, ("q", "z@x.example"), None);
    shows(&m, ("# see", "bug@x.example"), Some(("Tail3", "e@x.example")));
    shows(&m, ("q", "e@x.example"), None);
    shows(&m, ("q", "p@x.example"), Some(("Padded Name", "p@x.example")));
    shows(&m, ("q", " spaced@x.example "), Some(("Sp", "new@x.example")));
    shows(&m, ("q", "spaced@x.example"), None);
    shows(&m, ("q", ""), Some(("NoMail", "new@x.example")));
    shows(&m, ("q", "i@x.example"), None);
    shows(&m, ("q", "k@x.example"), None);
    shows(&m, ("q", "l@x.example"), Some(("Last", "l@x.example")));

    assert!(map("# one\n# two <a@x.example>\n").is_none(), "comments only");
    assert!(!m.is_none());
}

#[test]
fn an_empty_name_is_mapped_by_its_address() {
    let m = map("Proper <other@example.com>\n");
    shows(&m, ("", "other@example.com"), Some(("Proper", "other@example.com")));
    shows(&m, ("Name", "OTHER@example.com"), Some(("Proper", "OTHER@example.com")));
}

#[test]
fn the_graph_and_the_details_show_what_git_log_shows() {
    let dir = scratch_repo("mailmap-case-git");
    let repo = Repository::open(&dir).unwrap();
    let who = [
        ("jane", "jane@example.com"),
        ("JANE DOE", "JANE@EXAMPLE.COM"),
        ("DI OLD", "Di@Old.Example"),
        ("Someone", "di@old.example"),
        ("René Müller", "r@x.example"),
        ("RENé MüLLER", "R@x.example"),
        ("RENÉ MÜLLER", "r@x.example"),
        ("q", "ED@x.example"),
        ("Old", "merge@x.example"),
        ("other", "MERGE@x.example"),
        ("Nobody", "nobody@example.com"),
    ];
    let mut parent = None;
    let mut written: HashMap<Oid, (&str, &str)> = HashMap::new();
    for (i, w) in who.into_iter().enumerate() {
        let oid = commit_as(&repo, parent, w, 1_700_000_000 + i as i64);
        written.insert(oid, w);
        parent = Some(oid);
    }
    write(
        &dir,
        ".mailmap",
        concat!(
            "# people\n",
            "Jane Doe <Jane@Example.com>\n",
            "Di Proper <di@new.example> Di Old <di@old.example>\n",
            "Rene <rene@x.example> René Müller <r@x.example>\n",
            "Ed One <ed@x.example>\n",
            "Ed Two <Ed@X.example>\n",
            "<ed@new.example> <ed@x.example>\n",
            "Simple <simple@x.example> <merge@x.example>\n",
            "<only@x.example> OLD <Merge@x.example>\n",
        ),
    );

    type Four = [String; 4];
    let two = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    let git = crate::gitcli::output(
        &dir,
        &crate::gitcli::utf8_args(&["log", "--use-mailmap", "--format=%H%x09%aN%x09%aE%x09%cN%x09%cE"]),
        "",
        &[],
        None,
    );
    assert!(git.ok, "{}", git.stderr);
    let mut theirs: HashMap<Oid, Four> = HashMap::new();
    for line in git.stdout.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 5, "{line:?}");
        theirs.insert(Oid::from_str(f[0]).unwrap(), [f[1], f[2], f[3], f[4]].map(str::to_owned));
    }
    assert_eq!(theirs.len(), 11);

    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!(snap.commits.len(), 11);
    let mut changed = 0;
    let mut shown: HashMap<Oid, (String, String)> = HashMap::new();
    for c in snap.commits.iter() {
        let want = &theirs[&c.oid];
        let got = two(c.author.name(), c.author.email());
        assert_eq!(got, two(&want[0], &want[1]), "graph author of {:?}", written[&c.oid]);
        let d = crate::repo::commit_detail(&dir, c.oid).unwrap();
        assert_eq!((d.author.as_str(), d.email.as_str()), (want[0].as_str(), want[1].as_str()), "details author");
        assert_eq!((d.committer.as_str(), d.committer_email.as_str()), (want[2].as_str(), want[3].as_str()), "details committer");
        let w = written[&c.oid];
        changed += usize::from(got != two(w.0, w.1));
        shown.insert(c.oid, got);
    }
    assert!(changed >= 7, "only {changed} rows differ from what was written");

    let by_written = |name: &str, email: &str| {
        let oid = written.iter().find(|(_, w)| **w == (name, email)).map(|(o, _)| *o).unwrap();
        shown[&oid].clone()
    };
    assert_eq!(by_written("JANE DOE", "JANE@EXAMPLE.COM"), two("Jane Doe", "JANE@EXAMPLE.COM"));
    assert_eq!(by_written("q", "ED@x.example"), two("Ed Two", "ed@new.example"));
    assert_eq!(by_written("RENÉ MÜLLER", "r@x.example"), two("RENÉ MÜLLER", "r@x.example"));
    let _ = std::fs::remove_dir_all(&dir);
}
