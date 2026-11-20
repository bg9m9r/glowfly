//! Counting other worktrees' changes: the index header, what a load leaves for the background
//! count (Pending, or Carried from the prior), how that count reads a worktree, and that git runs
//! there only when it may.

use super::*;
use crate::repo::status_cli_tests::marker_script;
use crate::repo::worktrees::wip_tests::{DIRTY, cleanup, counted_load, dirty, linked, two_files};
use crate::repo::{GraphCache, LoadOptions, Prior, load, load_reusing};
use crate::testutil::*;

#[test]
fn index_entries_reads_the_header() {
    let dir = two_files("wtcount-header");
    assert_eq!(index_entries(&dir.join(".git").join("index")), Some(2));
    assert_eq!(index_entries(&dir.join(".git").join("no-such-index")), None);
    assert_eq!(index_entries(&dir.join("a.txt")), None, "not an index");
    cleanup(&[&dir]);
}

#[test]
fn a_load_leaves_other_worktrees_pending_without_reading_them() {
    let dir = two_files("wtcount-load");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!((snap.worktrees[1].read, snap.worktrees[1].changes), (ChangesRead::Pending, ChangeCounts::default()));
    assert_eq!(snap.worktrees[0].read, ChangesRead::NotCounted, "the current worktree is never counted");
    assert!(snap.graph.worktree_wips.is_empty());

    let off = load(&dir, &LoadOptions { worktree_wips: false, ..LoadOptions::all(100) }).unwrap();
    assert!(off.worktrees.iter().all(|w| w.read == ChangesRead::NotCounted && w.changes.is_empty()));
    assert!(off.graph.worktree_wips.is_empty());
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_reload_carries_the_counts_of_an_unchanged_worktree() {
    let dir = two_files("wtcount-carry");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let opts = LoadOptions::all(100);
    let snap = counted_load(&dir, &opts);
    assert_eq!((snap.worktrees[1].read, snap.worktrees[1].changes), (ChangesRead::Counted, DIRTY));

    let reload = |prior: &Prior| load_reusing(&dir, &opts, &GraphCache::default(), Some(prior)).unwrap();
    let carried = reload(&snap.prior());
    assert_eq!((carried.worktrees[1].read, carried.worktrees[1].changes), (ChangesRead::Carried, DIRTY));
    assert_eq!(carried.graph.worktree_wips.len(), 1, "its row is in the load's own layout");
    assert_eq!(carried.graph.worktree_wips, snap.graph.worktree_wips);

    // Carried counts carry on to the next load, until a count replaces them.
    let again = reload(&carried.prior());
    assert_eq!((again.worktrees[1].read, again.worktrees[1].changes), (ChangesRead::Carried, DIRTY));

    // A commit there moves its HEAD: the old counts no longer describe it.
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "more"]);
    let moved = reload(&snap.prior());
    assert_eq!((moved.worktrees[1].read, moved.worktrees[1].changes), (ChangesRead::Pending, ChangeCounts::default()));
    assert!(moved.graph.worktree_wips.is_empty());

    // Held reads carry without counts; TooLarge and Pending reads are not carried.
    for (read, want) in [
        (ChangesRead::Held, ChangesRead::Carried),
        (ChangesRead::TooLarge, ChangesRead::Pending),
        (ChangesRead::Pending, ChangesRead::Pending),
    ] {
        let mut s = load(&dir, &opts).unwrap();
        s.worktrees[1].read = read;
        s.worktrees[1].changes = ChangeCounts::default();
        let next = reload(&s.prior());
        assert_eq!((next.worktrees[1].read, next.worktrees[1].changes), (want, ChangeCounts::default()), "{read:?}");
    }
    cleanup(&[&wt, &dir]);
}

#[test]
fn known_counts_takes_only_other_counted_worktrees() {
    let dir = two_files("wtcount-known");
    let wt = linked(&dir, "feat");
    let repo = Repository::open(&dir).unwrap();
    let base = super::super::worktrees::list(&repo);
    let reads = [
        ChangesRead::NotCounted,
        ChangesRead::Pending,
        ChangesRead::TooLarge,
        ChangesRead::Counted,
        ChangesRead::Held,
        ChangesRead::Carried,
    ];
    let mut list = vec![base[0].clone(), base[1].clone()];
    list[0].read = ChangesRead::Counted; // the current one
    list[1].missing = true;
    list[1].read = ChangesRead::Counted;
    for (i, read) in reads.into_iter().enumerate() {
        let mut w = base[1].clone();
        w.path = wt.join(i.to_string());
        w.read = read;
        w.changes = ChangeCounts { modified: i + 1, ..Default::default() };
        list.push(w);
    }
    let known = known_counts(&list);
    let paths: Vec<_> = known.iter().map(|k| k.path.clone()).collect();
    assert_eq!(paths, [wt.join("3"), wt.join("4"), wt.join("5")], "Counted, Held and Carried");
    assert_eq!(known[0].changes, ChangeCounts { modified: 4, ..Default::default() });
    drop(repo);
    cleanup(&[&wt, &dir]);
}

#[test]
fn the_plan_follows_size_and_trust() {
    let large = super::super::status::LARGE_INDEX as u32 + 1;
    let cases = [
        // (entries, git_ok, wants_git) -> plan
        ((10, true, true), Plan::Git),
        ((10, true, false), Plan::Libgit2),
        ((10, false, true), Plan::Libgit2),
        ((large, true, false), Plan::Git),
        ((large, false, false), Plan::Libgit2),
        ((NO_GIT_COUNT_INDEX, false, true), Plan::Libgit2),
        ((NO_GIT_COUNT_INDEX + 1, false, true), Plan::TooLarge),
        ((NO_GIT_COUNT_INDEX + 1, true, false), Plan::Git),
    ];
    for ((entries, git_ok, wants_git), want) in cases {
        assert_eq!(plan(entries, git_ok, wants_git), want, "{entries} {git_ok} {wants_git}");
    }
}

#[test]
fn an_untrusted_worktree_is_counted_without_running_its_hooks() {
    let dir = two_files("wtcount-untrusted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);

    let count = count_worktree(&wt, &dir, None, COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes, &count.path), (ChangesRead::Counted, DIRTY, &wt));
    assert!(!marker.exists(), "no hook ran");
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_trusted_worktree_is_counted_through_git() {
    let dir = two_files("wtcount-trusted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    git(&dir, &["config", "core.untrackedCache", "true"]);
    let count = count_worktree(&wt, &dir, Some(&SavedTrust::trusted(None)), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));

    let plain = crate::paths::temp_dir().join(format!("glowfly-wtcount-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    let none = count_worktree(&plain, &dir, Some(&SavedTrust::trusted(None)), COUNT_TIMEOUT);
    assert_eq!((none.read, none.changes), (ChangesRead::Held, ChangeCounts::default()), "not a repository for now: asked again");
    // Once it is one, a recount counts it.
    git(&plain, &["init", "-q"]);
    assert_eq!(count_worktree(&plain, &dir, Some(&SavedTrust::trusted(None)), COUNT_TIMEOUT).read, ChangesRead::Counted);
    cleanup(&[&wt, &dir, &plain]);
}

#[test]
fn a_worktree_not_trusted_never_runs_git() {
    let dir = two_files("wtcount-refused");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);
    let count = count_worktree(&wt, &dir, Some(&SavedTrust { trust: RepoTrust::NotTrusted, print: None }), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    assert!(!marker.exists(), "no hook ran");

    // Trusting it lets git read it, so its hook runs.
    let print = crate::trust::scan_at(&wt).unwrap().print();
    let count = count_worktree(&wt, &dir, Some(&SavedTrust::trusted(Some(print))), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    assert!(marker.exists(), "git read it");
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_trusted_worktree_that_gained_a_hook_since_is_counted_without_git() {
    let dir = two_files("wtcount-stale");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let before = crate::trust::scan_at(&wt).unwrap().print();
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);
    let stale = SavedTrust::trusted(Some(before));
    let count = count_worktree(&wt, &dir, Some(&stale), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    assert!(!marker.exists(), "the decision doesn't cover the new hook, so git didn't run");

    let now = SavedTrust::trusted(Some(crate::trust::scan_at(&wt).unwrap().print()));
    let count = count_worktree(&wt, &dir, Some(&now), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    assert!(marker.exists(), "git read it");
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_worktree_whose_git_file_names_a_network_place_is_never_opened() {
    let dir = two_files("wtcount-offsite");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let repo = Repository::open(&dir).unwrap();
    let real = std::fs::read_to_string(wt.join(".git")).unwrap();
    // Its folder and record are local; only the folder's own `.git` file names the place.
    write(&wt, ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("wt/.git"))));
    let mut list = super::super::worktrees::list(&repo);
    leave_for_later(&mut list, repo.commondir(), &[]);
    assert_eq!((list[1].read, list[1].changes), (ChangesRead::NotCounted, ChangeCounts::default()), "{:?}", list[1]);
    let known = [KnownCount { path: list[1].path.clone(), head: list[1].head, changes: DIRTY }];
    leave_for_later(&mut list, repo.commondir(), &known);
    assert_eq!(list[1].read, ChangesRead::NotCounted, "never carried either");
    let count = count_worktree(&wt, &dir, Some(&SavedTrust::trusted(None)), COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::NotCounted, ChangeCounts::default()), "as the load leaves it");
    write(&wt, ".git", &real);
    drop(repo);
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_worktree_whose_record_names_a_network_place_is_listed_and_never_counted() {
    let dir = two_files("wtcount-far-record");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let admin = dir.join(".git/worktrees").join(wt.file_name().unwrap());
    let place = net_place().join("wt");
    write(&admin, "gitdir", &format!("{}\n", git_spelling(&place.join(".git"))));
    let repo = Repository::open(&dir).unwrap();
    let mut list = super::super::worktrees::list(&repo);
    leave_for_later(&mut list, repo.commondir(), &[]);
    let row = list.iter().find(|w| !w.current).expect("listed from its admin dir");
    assert!(on_net_place(&row.path), "{:?}", row.path);
    assert_eq!((row.read, row.changes, row.missing), (ChangesRead::NotCounted, ChangeCounts::default(), false));
    // Asked for by its listed folder, from the work tree it is listed in: never opened.
    for decision in [None, Some(SavedTrust::trusted(None))] {
        let count = count_worktree(&row.path, &dir, decision.as_ref(), COUNT_TIMEOUT);
        assert_eq!((count.read, count.changes), (ChangesRead::NotCounted, ChangeCounts::default()));
    }
    drop(repo);
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_worktree_of_a_repository_with_a_conditional_include_is_still_counted() {
    let dir = two_files("wtcount-conditional");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let config = dir.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(dir.join(".git/extra.cfg"), "[core]\n\tautocrlf = false\n").unwrap();
    std::fs::write(&config, format!("{text}[includeIf \"gitdir:**\"]\n\tpath = extra.cfg\n")).unwrap();

    let count = count_worktree(&wt, &dir, None, COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    let repo = Repository::open(&dir).unwrap();
    let mut list = super::super::worktrees::list(&repo);
    leave_for_later(&mut list, repo.commondir(), &[]);
    let row = list.iter().find(|w| !w.current).expect("the linked worktree");
    assert_eq!((row.read, row.changes), (ChangesRead::Pending, ChangeCounts::default()), "it can be opened");
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_worktree_of_a_repository_with_many_includes_is_still_counted() {
    let dir = two_files("wtcount-many-includes");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let config = dir.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    let at = dir.join(".git").to_string_lossy().replace('\\', "/");
    let many: String = (0..=crate::config_includes::MAX_FILES).map(|i| format!("\tpath = {at}/many{i}.cfg\n")).collect();
    std::fs::write(&config, format!("{text}[include]\n{many}")).unwrap();
    let count = count_worktree(&wt, &dir, None, COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_worktree_whose_gitattributes_is_linked_away_is_not_read() {
    let dir = two_files("wtcount-linked-away");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let untrusted = SavedTrust { trust: RepoTrust::NotTrusted, print: None };
    assert_eq!(count_worktree(&wt, &dir, Some(&untrusted), COUNT_TIMEOUT).changes, DIRTY);
    // In the other worktree's own folder; the opened repository's files are plain.
    if link_to(&net_place().join("attributes"), &wt.join(".gitattributes"), false) {
        let held = super::super::status::libgit2_status(&Repository::open(&wt).unwrap());
        assert!(matches!(held, Err(crate::error::Error::LinkedFile(f)) if f == ".gitattributes"));
        let count = count_worktree(&wt, &dir, Some(&untrusted), COUNT_TIMEOUT);
        assert_eq!((count.read, count.changes), (ChangesRead::Held, ChangeCounts::default()));
        assert!(super::super::status::libgit2_status(&Repository::open(&dir).unwrap()).is_ok());
        // Plain again: counted.
        std::fs::remove_file(wt.join(".gitattributes")).unwrap();
        assert_eq!(count_worktree(&wt, &dir, Some(&untrusted), COUNT_TIMEOUT).read, ChangesRead::Counted);
    }
    cleanup(&[&wt, &dir]);
}
