use super::*;
use crate::state::pull_request::{Page, PrAction, Tips};
use crate::state::pull_request_actions::tests::{detail, view};
use git2::Oid;
use trellis_core::forge::{EntryKind, ThreadComment};
use trellis_core::repo::{BranchChanges, Comparison, FileEntry};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn comment(id: Option<u64>, time: i64) -> ThreadComment {
    ThreadComment { id, author: "ada".into(), time, body: "Hm".into(), url: String::new(), pending: false }
}

pub(crate) fn thread(id: &str, path: &str, line: Option<u32>, time: i64) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: path.into(),
        side: LineSide::New,
        line,
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: String::new(),
        comments: vec![comment(Some(55), time)],
        more: 0,
    }
}

fn resolved(mut t: ReviewThread) -> ReviewThread {
    t.resolved = true;
    t.can_unresolve = true;
    t
}

fn outdated(mut t: ReviewThread) -> ReviewThread {
    t.outdated = true;
    t.original_line = t.line.take();
    t
}

fn entry(time: i64) -> Entry {
    Entry { author: "grace".into(), time, kind: EntryKind::Comment, body: String::new(), url: String::new() }
}

/// A loaded view whose files (`src/a.rs`, `src/b.rs`) are loaded from source tip 2, which the
/// service also reports as the head.
fn synced(threads: Vec<ReviewThread>) -> PrView {
    let mut d = detail();
    d.head_sha = oid(2).to_string().to_uppercase();
    let mut v = view(Some(d));
    if let Page::Loaded(p) = &mut v.page {
        p.threads = threads;
        p.entries = vec![entry(10), entry(30)];
    }
    v.tips = Tips { target: Some(oid(1)), source: Some(oid(2)) };
    let files = vec![FileEntry::new("src/a.rs", 'M'), FileEntry::new("src/b.rs", 'A')];
    v.files = Files::Loaded(BranchChanges { base: oid(3), comparison: Comparison { files, absent: Default::default() } });
    v
}

fn page(v: &PrView) -> &PrPage {
    v.loaded().unwrap()
}

#[test]
fn listed_and_open_defaults_and_overrides() {
    let mut tv = ThreadView::default();
    let (plain, done, old) =
        (thread("T1", "a", Some(1), 0), resolved(thread("T2", "a", Some(1), 0)), outdated(thread("T3", "a", Some(1), 0)));
    assert!(listed(&tv, &plain) && !listed(&tv, &done) && !listed(&tv, &old));
    assert!(is_open(&tv, &plain) && !is_open(&tv, &done) && !is_open(&tv, &old));
    tv.show_resolved = true;
    assert!(listed(&tv, &done) && !listed(&tv, &old));
    tv.show_outdated = true;
    assert!(listed(&tv, &old));
    let both = outdated(resolved(thread("T4", "a", Some(1), 0)));
    tv.show_resolved = false;
    assert!(!listed(&tv, &both), "hidden while either kind is hidden");
    tv.open.insert("T1".into(), false);
    tv.open.insert("T2".into(), true);
    assert!(!is_open(&tv, &plain) && is_open(&tv, &done));
}

#[test]
fn hidden_counts_and_text() {
    assert_eq!(hidden_text(1, 0).as_deref(), Some("1 resolved thread hidden"));
    assert_eq!(hidden_text(2, 1).as_deref(), Some("2 resolved and 1 outdated threads hidden"));
    assert_eq!(hidden_text(0, 3).as_deref(), Some("3 outdated threads hidden"));
    assert_eq!(hidden_text(0, 0), None);
    let v = synced(vec![
        resolved(thread("T1", "src/a.rs", Some(1), 0)),
        resolved(outdated(thread("T2", "src/a.rs", Some(2), 0))),
        outdated(thread("T3", "src/b.rs", Some(3), 0)),
        thread("T4", "src/b.rs", Some(4), 0),
    ]);
    let mut tv = ThreadView::default();
    assert_eq!(hidden(page(&v), &tv, None), (2, 1), "one both resolved and outdated counts as resolved");
    assert_eq!(hidden(page(&v), &tv, Some("src/b.rs")), (0, 1));
    tv.show_resolved = true;
    assert_eq!(hidden(page(&v), &tv, None), (0, 2));
    assert_eq!(kind_counts(page(&v), None), (2, 2));
    assert_eq!(kind_counts(page(&v), Some("src/a.rs")), (2, 1));
    assert_eq!(resolved_toggle(&tv, 2), "Hide resolved");
    assert_eq!(outdated_toggle(&tv, 2), "Show outdated (2)");
}

#[test]
fn timeline_merges_by_time() {
    let v = synced(vec![
        thread("T1", "src/a.rs", Some(1), 30),
        thread("T2", "src/a.rs", Some(2), 5),
        resolved(thread("T3", "src/a.rs", Some(3), 20)),
    ]);
    let items: Vec<String> = timeline(page(&v), &ThreadView::default())
        .into_iter()
        .map(|i| match i {
            Item::Entry(e) => format!("e{}", e.time),
            Item::Thread(t) => t.id.clone(),
        })
        .collect();
    assert_eq!(items, ["T2", "e10", "e30", "T1"], "an entry first at the same time; the resolved thread hidden");
}

#[test]
fn file_threads_order() {
    let v = synced(vec![
        thread("L9", "src/a.rs", Some(9), 0),
        outdated(thread("OLD", "src/a.rs", Some(1), 0)),
        thread("FILE", "src/a.rs", None, 5),
        thread("L2", "src/a.rs", Some(2), 9),
        thread("B", "src/b.rs", Some(1), 0),
    ]);
    let tv = ThreadView { show_outdated: true, ..Default::default() };
    let ids: Vec<&str> = file_threads(page(&v), &tv, "src/a.rs").iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["FILE", "L2", "L9", "OLD"]);
    assert_eq!(file_threads(page(&v), &ThreadView::default(), "src/a.rs").len(), 3, "outdated hidden");
    assert_eq!(on_file(page(&v), "src/a.rs"), 4, "hidden ones count");
    assert_eq!(on_file(page(&v), "SRC/A.RS"), 0, "paths match exactly");
}

#[test]
fn placement_needs_the_head_locally() {
    let mut v = synced(Vec::new());
    assert_eq!(placement(&v), Placement::OnLines, "the head matched ignoring case");
    v.tips.source = Some(oid(4));
    let Placement::Elsewhere(note) = placement(&v) else { panic!("out of sync") };
    assert_eq!(
        note,
        format!("These comments are on {}; fetch origin to see them on their lines.", &oid(2).to_string().to_uppercase()[..7])
    );
    v.tips.source = None;
    assert!(matches!(placement(&v), Placement::Elsewhere(_)), "no source tip");
    let mut v = synced(Vec::new());
    v.files = Files::Loading;
    assert!(matches!(placement(&v), Placement::Elsewhere(_)), "files not loaded");
}

#[test]
fn marks_for_listed_current_threads() {
    let mut two = thread("T2", "src/a.rs", Some(4), 0);
    two.comments.push(comment(Some(56), 1));
    let mut left = thread("T5", "src/a.rs", Some(4), 0);
    left.side = LineSide::Old;
    let mut v = synced(vec![
        thread("T1", "src/a.rs", Some(4), 0),
        two,
        left,
        resolved(thread("T3", "src/a.rs", Some(8), 0)),
        outdated(thread("T4", "src/a.rs", Some(9), 0)),
        thread("F", "src/a.rs", None, 0),
        thread("B", "src/b.rs", Some(4), 0),
    ]);
    let m = marks(&v, "src/a.rs").unwrap();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!(m[&(LineSide::New, 4)], Mark { thread: "T1".into(), count: 3, resolved: false });
    assert_eq!(m[&(LineSide::Old, 4)].thread, "T5");
    v.threads.show_resolved = true;
    let m = marks(&v, "src/a.rs").unwrap();
    assert_eq!(m[&(LineSide::New, 8)], Mark { thread: "T3".into(), count: 1, resolved: true });
    v.threads.show_outdated = true;
    assert!(!marks(&v, "src/a.rs").unwrap().contains_key(&(LineSide::New, 9)), "outdated threads have no line");
    v.tips.source = Some(oid(9));
    assert_eq!(marks(&v, "src/a.rs"), None, "out of sync");
}

#[test]
fn where_text_forms() {
    let mut t = thread("T", "a", Some(42), 0);
    assert_eq!(where_text(&t), "line 42");
    t.start_line = Some(40);
    assert_eq!(where_text(&t), "lines 40\u{2013}42");
    t.start_line = Some(42);
    assert_eq!(where_text(&t), "line 42", "a range of one line");
    let mut d = thread("T", "a", Some(12), 0);
    d.side = LineSide::Old;
    assert_eq!(where_text(&d), "deleted line 12");
    assert_eq!(where_text(&thread("T", "a", None, 0)), "the file");
    assert_eq!(where_text(&outdated(thread("T", "a", Some(7), 0))), "line 7 of an earlier version");
}

#[test]
fn go_to_reasons() {
    let v = synced(Vec::new());
    let t = thread("T", "src/a.rs", Some(4), 0);
    let (target, side, n) = go_to(&v, &t).unwrap();
    assert_eq!(target, DiffTarget::Between(oid(3), oid(2), "src/a.rs".into()));
    assert_eq!((side, n), (LineSide::New, 4));
    assert_eq!(go_to(&v, &thread("T", "src/a.rs", None, 0)).unwrap().2, 0, "a file comment");
    assert_eq!(go_to(&v, &outdated(t.clone())).unwrap_err(), "This comment is on an earlier version of the file");
    assert_eq!(go_to(&v, &thread("T", "src/A.rs", Some(1), 0)).unwrap_err(), "src/A.rs isn't among the files changed here");
    let mut away = v.clone();
    away.tips.source = Some(oid(5));
    assert!(go_to(&away, &t).unwrap_err().starts_with("These comments are on"));
}

#[test]
fn reply_blocked_reasons_in_order() {
    let t = thread("T1", "src/a.rs", Some(4), 0);
    let mut v = synced(vec![t.clone()]);
    v.forms.replies.insert("T1".into(), "Fixed".into());
    assert_eq!(reply_blocked(&v, &t), None);

    let mut loading = v.clone();
    loading.page = Page::Loading;
    assert_eq!(reply_blocked(&loading, &t).unwrap(), "The pull request hasn't loaded yet");
    let mut busy = v.clone();
    busy.busy = Some(PrAction::Comment);
    assert_eq!(reply_blocked(&busy, &t).unwrap(), "Waiting for the service…");
    let mut cant = t.clone();
    cant.can_reply = false;
    assert_eq!(reply_blocked(&v, &cant).unwrap(), "You can't reply to this thread");
    let mut locked = v.clone();
    if let Page::Loaded(p) = &mut locked.page {
        p.detail.locked = true;
    }
    assert_eq!(reply_blocked(&locked, &cant).unwrap(), "The conversation is locked on the service");
    let mut pending = v.clone();
    if let Page::Loaded(p) = &mut pending.page {
        p.detail.pending_review = Some(9);
    }
    assert!(reply_blocked(&pending, &t).unwrap().starts_with("Finish your review in progress first"));
    let mut lost = t.clone();
    lost.comments[0].id = None;
    assert_eq!(reply_blocked(&v, &lost).unwrap(), "This thread's first comment can't be found, so there is nothing to reply to");
    let mut empty = v.clone();
    empty.forms.replies.insert("T1".into(), "  \n".into());
    assert_eq!(reply_blocked(&empty, &t).unwrap(), "Write a reply first");
    assert_eq!(reply_blocked(&synced(Vec::new()), &t).unwrap(), "Write a reply first", "no draft yet");
    let mut long = v.clone();
    long.forms.replies.insert("T1".into(), "x".repeat(MAX_BODY + 1));
    assert_eq!(reply_blocked(&long, &t).unwrap(), "Replies can have at most 65,536 characters");
}

#[test]
fn resolve_blocked_and_label() {
    let t = thread("T1", "src/a.rs", Some(4), 0);
    let v = synced(vec![t.clone()]);
    assert_eq!(resolve_blocked(&v, &t), None);
    assert_eq!(resolve_label(&t), "Resolve");
    let mut no = t.clone();
    no.can_resolve = false;
    assert_eq!(resolve_blocked(&v, &no).unwrap(), "Only the author and people with write access can resolve threads");
    let mut done = resolved(t.clone());
    assert_eq!(resolve_label(&done), "Unresolve");
    assert_eq!(resolve_blocked(&v, &done), None);
    done.can_unresolve = false;
    assert_eq!(resolve_blocked(&v, &done).unwrap(), "You can't unresolve this thread");
    let mut busy = v.clone();
    busy.busy = Some(PrAction::Reply("T9".into()));
    assert_eq!(resolve_blocked(&busy, &t).unwrap(), "Waiting for the service…");
    let mut merged = v.clone();
    if let Page::Loaded(p) = &mut merged.page {
        p.detail.state = trellis_core::forge::PrState::Merged;
    }
    assert_eq!(resolve_blocked(&merged, &t), None, "merged pull requests aren't blocked");
}
