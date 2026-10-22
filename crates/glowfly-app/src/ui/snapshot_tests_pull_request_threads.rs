//! The pull request panel's code comment threads: cards in the conversation (open, resolved,
//! outdated, pending), the filter bar and hidden-count note, and the pane and line markers
//! under a file's diff in Files changed; and the messages their buttons and links send.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Page, PrTab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::{Entry, EntryKind, MergeMethod, MergeState, PrDetail, PrPage, PrState, ReviewThread, ThreadComment, Verdict};
use glowfly_core::repo::{self, DiffTarget, FileDiff};
use glowfly_core::settings::ThemeChoice;
use std::sync::Arc;

/// Unix seconds on day `day` of January 2026, at 10:00 UTC.
fn day(day: i64) -> i64 {
    1_767_225_600 + (day - 1) * 86_400 + 36_000
}

fn detail(head: String) -> PrDetail {
    PrDetail {
        state: PrState::Open,
        draft: false,
        locked: false,
        mergeable: Some(true),
        merge_state: MergeState::Clean,
        head_sha: head,
        base_sha: "b1".into(),
        same_repo: true,
        author: "ada".into(),
        me: "grace".into(),
        can_merge: Some(true),
        methods: vec![MergeMethod::Merge, MergeMethod::Squash],
        deletes_merged_branches: false,
        pending_review: None,
    }
}

fn comment(id: u64, author: &str, on: i64, body: &str, pending: bool) -> ThreadComment {
    ThreadComment { id: Some(id), author: author.into(), time: day(on), body: body.into(), url: String::new(), pending }
}

fn thread(id: &str, line: u32, comments: Vec<ThreadComment>) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: "search.txt".into(),
        side: LineSide::New,
        line: Some(line),
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: "@@ -0,0 +1 @@\n+search".into(),
        comments,
        more: 0,
    }
}

/// The search.txt diff, its files loaded in the panel; returns the diff and its first added
/// line's number.
fn files_loaded(st: &mut AppState, dir: &std::path::Path) -> (DiffTarget, FileDiff, u32) {
    super::snapshot_tests_pull_request::compare(st, dir);
    let target = st.pull_request.as_ref().unwrap().diff_target("search.txt").unwrap();
    let d = repo::load_diff(dir, &target).unwrap();
    let added = d.hunks.iter().flat_map(|h| &h.lines).find(|l| l.origin == '+').and_then(|l| l.new).expect("an added line");
    (target, d, added)
}

/// T1 open on the added line (two comments, the second pending), T2 resolved and T3 outdated.
fn threads(line: u32) -> Vec<ReviewThread> {
    let t1 = thread(
        "T1",
        line,
        vec![
            comment(55, "linus", 13, "Should this be case-insensitive?", false),
            comment(56, "grace", 14, "Agreed, and trim the query too.", true),
        ],
    );
    let mut t2 = thread("T2", line, vec![comment(57, "grace", 15, "Typo in the name.", false)]);
    t2.resolved = true;
    t2.resolved_by = Some("ada".into());
    t2.can_resolve = false;
    t2.can_unresolve = true;
    let mut t3 = thread("T3", line, vec![comment(58, "linus", 11, "This line moved.", false)]);
    t3.outdated = true;
    t3.original_line = t3.line.take();
    vec![t1, t2, t3]
}

fn load(st: &mut AppState, d: PrDetail, threads: Vec<ReviewThread>) {
    let entries = vec![
        Entry {
            author: "linus".into(),
            time: day(12),
            kind: EntryKind::Comment,
            body: "Could the results be paged?".into(),
            url: String::new(),
        },
        Entry {
            author: "grace".into(),
            time: day(16),
            kind: EntryKind::Review(Verdict::Approved),
            body: "Reads well.".into(),
            url: String::new(),
        },
    ];
    let v = st.pull_request.as_mut().unwrap();
    crate::state::pull_request_actions::prefill(&mut v.forms, &d);
    v.page = Page::Loaded(Box::new(PrPage { detail: d, entries, threads, threads_note: None }));
}

fn sent(h: &egui_kittest::Harness<'static, (AppState, Vec<Msg>)>, f: impl Fn(&PullRequestMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(p) if f(p)))
}

#[test]
fn pull_request_threads_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-threads", true);
    let (_, _, line) = files_loaded(&mut st, &dir);
    let head = st.pull_request.as_ref().unwrap().source_tip().unwrap().to_string();
    load(&mut st, detail(head), threads(line));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_threads_light");
    assert!(h.query_by_label("Pending").is_some());
    assert!(h.query_by_label("1 resolved and 1 outdated threads hidden").is_some());
    assert!(h.query_by_label("Typo in the name.").is_none(), "the resolved thread is hidden");
    assert!(h.get_by_role_and_label(Role::Button, "Reply").is_disabled(), "nothing written yet");

    h.get_by_label("Show resolved (1)").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::ShowResolved(true))));
    h.get_by_role_and_label(Role::Button, "Resolve").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::Resolve { thread, resolve: true } if thread == "T1")));
    h.get_by_label("search.txt").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::GoToThread(id) if id == "T1")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_threads_pending_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-threads-pending", true);
    let (_, _, line) = files_loaded(&mut st, &dir);
    let head = st.pull_request.as_ref().unwrap().source_tip().unwrap().to_string();
    let mut d = detail(head);
    d.pending_review = Some(9);
    load(&mut st, d, threads(line));
    let v = st.pull_request.as_mut().unwrap();
    v.threads.show_resolved = true;
    v.threads.show_outdated = true;
    v.threads.open.insert("T3".into(), true);
    v.forms.replies.insert("T1".into(), "Done, thanks.".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_threads_pending_dark");
    assert!(h.query_by_label("Outdated").is_some());
    assert!(h.query_by_label("This line moved.").is_some(), "the outdated card is opened");
    assert!(h.query_by_label("Hide resolved").is_some() && h.query_by_label("Hide outdated").is_some());
    assert!(h.query_by_label("Typo in the name.").is_none(), "the resolved card is collapsed");
    let reply = h.get_all_by_role_and_label(Role::Button, "Reply").next().expect("T1's Reply");
    assert!(reply.is_disabled(), "a review is in progress");
    reply.hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("Finish your review in progress first").is_some());

    // The resolved card (the second in time) opens with its Show button.
    let show = h.get_all_by_role_and_label(Role::Button, "Show").next().expect("the collapsed card's Show");
    show.click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::ToggleThread(id) if id == "T2")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_file_threads_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-file-threads", true);
    let (target, d, line) = files_loaded(&mut st, &dir);
    let head = st.pull_request.as_ref().unwrap().source_tip().unwrap().to_string();
    let mut t1 = threads(line).remove(0);
    t1.comments[1].pending = false;
    load(&mut st, detail(head), vec![t1]);
    let v = st.pull_request.as_mut().unwrap();
    v.tab = PrTab::Files;
    v.threads.focus = Some("T1".into());
    assert!(v.shows(&target));
    st.diff = Some((target, Arc::new(d)));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_file_threads_dark");
    assert!(h.query_by_label("Comments on this file  1").is_some());
    assert!(h.query_by_label_contains("fetch origin").is_none(), "the head is checked out locally");
    h.get_by_role_and_label(Role::Button, "2 comments on this line").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::FocusThread(id) if id == "T1")));
    // Links read as labels: their text can be selected.
    h.get_by_label(&format!("line {line}")).click();
    h.run();
    assert!(sent(&h, |m| matches!(m, PullRequestMsg::GoToThread(id) if id == "T1")));
    drop(h);
    remove_fixture(&dir);
}

/// Command+Enter in a thread's reply box posts the reply, once and without a newline.
#[test]
fn command_enter_posts_a_reply() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-threads-key", true);
    let (_, _, line) = files_loaded(&mut st, &dir);
    let head = st.pull_request.as_ref().unwrap().source_tip().unwrap().to_string();
    load(&mut st, detail(head), threads(line));
    let mut h = harness(st, false);
    h.run();
    h.get_all_by_label("Reply").find(|n| n.role() == Role::MultilineTextInput).expect("T1's reply box").type_text("Done.");
    h.run();
    h.press_key_modifiers(eframe::egui::Modifiers::COMMAND, eframe::egui::Key::Enter);
    h.run();
    let replies = h.state().1.iter().filter(|m| matches!(m, Msg::PullRequest(PullRequestMsg::Reply(id)) if id == "T1")).count();
    assert_eq!(replies, 1);
    assert_eq!(h.state().0.pull_request.as_ref().unwrap().forms.replies["T1"], "Done.", "no newline added");
    drop(h);
    remove_fixture(&dir);
}
