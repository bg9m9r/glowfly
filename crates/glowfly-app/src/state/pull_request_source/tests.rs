//! The notes on where the pull request panel's files come from, their tone and buttons, and
//! the key a comparison is asked from.

use super::*;
use crate::state::pull_request::tests::{merged_detail, oid, pr, view};
use crate::state::pull_request::{Page, Tips};
use glowfly_core::forge::{PrPage, PrState};
use glowfly_core::repo::{BranchChanges, Comparison, RangeCommits, SourceFrom, SourceTip};

fn loaded(v: &mut PrView, source: Oid, from: SourceFrom) {
    let changes = BranchChanges { base: oid(3), comparison: Comparison { files: Vec::new(), absent: Default::default() } };
    v.set_loaded(changes, SourceTip { oid: source, from }, RangeCommits::default());
}

fn page(v: &mut PrView, f: impl FnOnce(&mut glowfly_core::forge::PrDetail)) {
    let mut detail = merged_detail();
    detail.state = PrState::Open;
    f(&mut detail);
    v.page = Page::Loaded(Box::new(PrPage { detail, entries: Vec::new(), threads: Vec::new(), threads_note: None }));
}

#[test]
fn out_of_sync_notes_each_problem() {
    let both = Tips { target: Some(oid(1)), source: Some(oid(2)) };
    assert_eq!(out_of_sync(&view(pr(None), Tips { target: None, ..both })), [OutOfSync::TargetMissing]);
    assert_eq!(out_of_sync(&view(pr(None), Tips { source: None, ..both })), [OutOfSync::SourceMissing]);
    assert_eq!(out_of_sync(&view(pr(None), Tips { target: None, source: None })), [OutOfSync::TargetMissing, OutOfSync::SourceMissing]);
    // No head from the service: nothing to compare with.
    assert!(out_of_sync(&view(pr(None), both)).is_empty());
    let local = oid(2).to_string();
    assert!(out_of_sync(&view(pr(Some(&local)), both)).is_empty());
    // A 12-character prefix (Bitbucket Cloud) matches.
    assert!(out_of_sync(&view(pr(Some(&local[..12])), both)).is_empty());
    let other = oid(9).to_string();
    let v = view(pr(Some(&other)), both);
    assert_eq!(out_of_sync(&v), [OutOfSync::SourceDiffers { local: oid(2), head: other.clone() }]);
    assert_eq!(
        out_of_sync(&v)[0].text(&v),
        "origin/feature is at 0202020, but the pull request is at 0909090. Fetch origin to see its latest changes."
    );
    assert_eq!(OutOfSync::TargetMissing.text(&v), "origin/main isn't in this repository. Fetch origin to compare.");
}

#[test]
fn a_merged_source_reads_as_deleted() {
    let both = Tips { target: Some(oid(1)), source: Some(oid(2)) };
    let mut v = view(pr(None), Tips { source: None, ..both });
    let mut detail = merged_detail();
    v.page = Page::Loaded(Box::new(PrPage { detail: detail.clone(), entries: Vec::new(), threads: Vec::new(), threads_note: None }));
    assert_eq!(out_of_sync(&v), [OutOfSync::SourceDeleted]);
    assert_eq!(OutOfSync::SourceDeleted.text(&v), "origin/feature was deleted after the merge.");
    detail.state = PrState::Open;
    v.page = Page::Loaded(Box::new(PrPage { detail, entries: Vec::new(), threads: Vec::new(), threads_note: None }));
    assert_eq!(out_of_sync(&v), [OutOfSync::SourceMissing]);
}

#[test]
fn nothing_is_noted_about_the_source_while_the_files_load() {
    let mut v = view(pr(None), Tips { target: None, source: None });
    v.files = Files::Loading;
    assert_eq!(out_of_sync(&v), [OutOfSync::TargetMissing]);
}

#[test]
fn a_head_used_is_noted_as_info() {
    let both = Tips { target: Some(oid(1)), source: Some(oid(2)) };
    let head = oid(4).to_string();
    // The branch is missing.
    let mut v = view(pr(Some(&head)), Tips { source: None, ..both });
    loaded(&mut v, oid(4), SourceFrom::Head);
    let notes = out_of_sync(&v);
    assert_eq!(notes, [OutOfSync::HeadUsed { branch: None, head: oid(4), fork: false }]);
    assert_eq!(notes[0].text(&v), "origin/feature isn't in this repository, so this shows the pull request's head commit 0404040.");
    assert_eq!((tone(&notes), offers_fetch(&notes)), (NoteTone::Info, false));
    // The branch is elsewhere.
    let mut v = view(pr(Some(&head)), both);
    loaded(&mut v, oid(4), SourceFrom::Head);
    let notes = out_of_sync(&v);
    assert_eq!(
        notes[0].text(&v),
        "origin/feature is at 0202020; this shows the pull request's head commit 0404040, which is already here."
    );
    // A fork, known from the page: the same-named branch isn't asked for.
    page(&mut v, |d| d.same_repo = false);
    assert!(fork_known(&v));
    assert_eq!(files_key(&v), FilesKey { target: Some(oid(1)), branch: None, head: Some(head.clone()) });
    let notes = out_of_sync(&v);
    assert_eq!(notes, [OutOfSync::HeadUsed { branch: None, head: oid(4), fork: true }]);
    assert_eq!(notes[0].text(&v), "This pull request comes from a fork, so this shows its head commit 0404040.");
    // From the branch: no note.
    let mut v = view(pr(Some(&oid(2).to_string())), both);
    loaded(&mut v, oid(2), SourceFrom::Branch);
    assert!(out_of_sync(&v).is_empty());
}

#[test]
fn head_fetch_notes_and_buttons() {
    let head = oid(4).to_string();
    let mut v = view(pr(Some(&head)), Tips { target: Some(oid(1)), source: None });
    // Without a head ref, a fork's branch can't be fetched.
    assert!(out_of_sync(&v)[0].text(&v).ends_with("A branch on a fork can't be fetched from origin."));
    assert!(!offers_head_fetch(&v, &out_of_sync(&v)));
    v.head_ref = Some("refs/pull/7/head".into());
    let notes = out_of_sync(&v);
    assert_eq!(notes[0].text(&v), "origin/feature isn't in this repository. Fetch #7's head from origin to compare.");
    assert!(offers_head_fetch(&v, &notes) && offers_fetch(&notes));
    assert_eq!(head_button(&v), "Fetch #7's head");

    v.head_fetch = HeadFetch::Running;
    let notes = out_of_sync(&v);
    assert_eq!(notes, [OutOfSync::HeadFetching]);
    assert_eq!(notes[0].text(&v), "Fetching #7's head from origin…");
    assert_eq!((tone(&notes), offers_fetch(&notes), offers_head_fetch(&v, &notes)), (NoteTone::Progress, false, false));

    v.head_fetch = HeadFetch::Failed("could not read Username".into());
    let notes = out_of_sync(&v);
    assert_eq!(notes[0].text(&v), "Couldn't fetch #7's head from origin: could not read Username");
    assert_eq!((tone(&notes), offers_fetch(&notes), offers_head_fetch(&v, &notes)), (NoteTone::Warning, true, true));

    v.head_fetch = HeadFetch::Done;
    let notes = out_of_sync(&v);
    assert_eq!(notes, [OutOfSync::HeadNotSent { head: head.clone() }]);
    assert_eq!(
        notes[0].text(&v),
        "Fetched #7's head from origin, but its commit 0404040 wasn't in it: the pull request may have changed since the list was read."
    );
    assert!(offers_head_fetch(&v, &notes));
    // An unknown or malformed head can't be fetched.
    v.pr.head = Some("zzzzzzz".into());
    assert!(!head_fetchable(&v));
}
