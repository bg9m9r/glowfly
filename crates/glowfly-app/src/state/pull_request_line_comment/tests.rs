use super::*;
use crate::state::pull_request::{PrAction, Tips};
use crate::state::pull_request_actions::tests::{detail, view};
use crate::state::{AppState, ForgeState};
use git2::Oid;
use glowfly_core::forge::{PrDetail, ReviewThread, Target, ThreadComment};
use glowfly_core::repo::{BranchChanges, Comparison, FileEntry, Hunk, SourceFrom, SourceTip};
use glowfly_core::settings::Settings;
use std::time::Instant;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// A loaded view whose files (f.txt) are compared from merge base 3 to source tip 2, which the
/// service also reports as the head, as `f` changes the detail.
fn synced(f: impl FnOnce(&mut PrDetail)) -> PrView {
    let mut d = detail();
    d.head_sha = oid(2).to_string();
    f(&mut d);
    let mut v = view(Some(d));
    v.tips = Tips { target: Some(oid(1)), source: Some(oid(2)) };
    let files = vec![FileEntry::new("f.txt", 'M')];
    let changes = BranchChanges { base: oid(3), comparison: Comparison { files, absent: Default::default() } };
    v.set_loaded(changes, SourceTip { oid: oid(2), from: SourceFrom::Branch }, Default::default());
    v
}

fn target() -> DiffTarget {
    DiffTarget::Between(oid(3), oid(2), "f.txt".into())
}

fn dl(origin: char, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { origin, old, new, text: text.into() }
}

/// [ctx 1/1, - old 2, + new 2, ctx 3/3], then a hunk at line 20.
fn diff() -> FileDiff {
    let one = Hunk {
        header: "@@ -1,3 +1,3 @@".into(),
        lines: vec![
            dl(' ', Some(1), Some(1), "a"),
            dl('-', Some(2), None, "b"),
            dl('+', None, Some(2), "B"),
            dl(' ', Some(3), Some(3), "c"),
        ],
    };
    let two = Hunk { header: "@@ -20,1 +20,2 @@".into(), lines: vec![dl(' ', Some(20), Some(20), "t"), dl('+', None, Some(21), "u")] };
    FileDiff { binary: false, hunks: vec![one, two], highlight: None, intraline: Default::default(), whole: None }
}

fn drafted(v: &mut PrView, first: usize, last: usize, text: &str) {
    let mut d = draft_at(v, &target(), &diff(), 0, first, last).unwrap();
    d.text = text.into();
    v.line_draft = Some(d);
}

fn anchor(line: u32, side: LineSide, start: Option<(u32, LineSide)>) -> LineAnchor {
    LineAnchor { path: "f.txt".into(), line, side, start }
}

const GH: Option<Provider> = Some(Provider::GitHub);

#[test]
fn where_text_names_the_lines_and_versions() {
    assert_eq!(where_text(&anchor(42, LineSide::New, None)), "line 42");
    assert_eq!(where_text(&anchor(42, LineSide::New, Some((40, LineSide::New)))), "lines 40\u{2013}42");
    assert_eq!(where_text(&anchor(12, LineSide::Old, None)), "deleted line 12");
    assert_eq!(where_text(&anchor(4, LineSide::Old, Some((3, LineSide::Old)))), "deleted lines 3\u{2013}4");
    assert_eq!(where_text(&anchor(5, LineSide::New, Some((3, LineSide::Old)))), "deleted line 3 to line 5");
}

#[test]
fn draft_at_anchors_the_span_and_copies_its_lines() {
    let v = synced(|_| {});
    let d = draft_at(&v, &target(), &diff(), 0, 2, 1).unwrap();
    assert_eq!((d.first, d.last, d.hunk), (1, 2, 0));
    assert_eq!(d.anchor, anchor(2, LineSide::New, Some((2, LineSide::Old))));
    assert_eq!(d.head, oid(2).to_string());
    assert_eq!(d.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["b", "B"]);
    let other = DiffTarget::Between(oid(3), oid(9), "f.txt".into());
    assert!(draft_at(&v, &other, &diff(), 0, 0, 0).is_err(), "a diff the panel doesn't show");
    assert!(draft_at(&v, &target(), &diff(), 7, 0, 0).is_err(), "no such hunk");
}

#[test]
fn start_blocked_in_order() {
    let v = synced(|_| {});
    assert_eq!(start_blocked(GH, &v), None);
    assert_eq!(start_blocked(Some(Provider::GitHubEnterprise), &v), None);
    assert_eq!(start_blocked(Some(Provider::GitLab), &v).as_deref(), Some(UNSUPPORTED));
    assert_eq!(start_blocked(None, &v).as_deref(), Some(UNSUPPORTED));
    assert_eq!(start_blocked(GH, &view(None)).as_deref(), Some("The pull request hasn't loaded yet"));
    let mut busy = synced(|_| {});
    busy.busy = Some(PrAction::Comment);
    assert_eq!(start_blocked(GH, &busy).as_deref(), Some(WAITING));
    assert_eq!(start_blocked(GH, &synced(|d| d.locked = true)).as_deref(), Some(LOCKED));
    let elsewhere = synced(|d| d.head_sha = "abc1234def".into());
    assert_eq!(start_blocked(GH, &elsewhere).as_deref(), Some("The service's head is abc1234; fetch origin to comment on its lines"));
}

#[test]
fn post_blocked_adds_the_drafts_own_reasons() {
    let mut v = synced(|_| {});
    drafted(&mut v, 2, 2, "Rename this");
    assert_eq!(post_blocked(GH, &v, false), None);
    assert_eq!(post_blocked(GH, &v, true), None);
    v.line_draft.as_mut().unwrap().head = oid(7).to_string();
    assert_eq!(post_blocked(GH, &v, false).as_deref(), Some(MOVED), "the files were compared again since");
    let mut v = synced(|d| d.pending_review = Some(9));
    drafted(&mut v, 2, 2, "Rename this");
    assert_eq!(post_blocked(GH, &v, false).as_deref(), Some(PENDING));
    assert_eq!(post_blocked(GH, &v, true), None);
    v.line_draft.as_mut().unwrap().text = "  \n".into();
    assert_eq!(post_blocked(GH, &v, true).as_deref(), Some(EMPTY));
    v.line_draft.as_mut().unwrap().text = "x".repeat(MAX_BODY + 1);
    assert_eq!(post_blocked(GH, &v, true).as_deref(), Some(TOO_LONG));
    v.line_draft.as_mut().unwrap().posting = Some(true);
    assert_eq!(post_blocked(GH, &v, true).as_deref(), Some(WAITING), "while it is being posted");
}

#[test]
fn suggestions_are_for_new_lines_only() {
    let mut v = synced(|_| {});
    drafted(&mut v, 2, 3, "");
    let d = v.line_draft.as_ref().unwrap();
    assert_eq!(suggest_blocked(d), None);
    assert_eq!(with_suggestion("", d), "```suggestion\nB\nc\n```");
    assert_eq!(with_suggestion("Like this:\n", d), "Like this:\n```suggestion\nB\nc\n```");
    assert_eq!(with_suggestion("Like this:", d), "Like this:\n\n```suggestion\nB\nc\n```", "a blank line before the block");
    drafted(&mut v, 1, 2, "");
    assert_eq!(suggest_blocked(v.line_draft.as_ref().unwrap()), Some(DELETED), "starts on a deleted line");
    drafted(&mut v, 0, 1, "");
    assert_eq!(suggest_blocked(v.line_draft.as_ref().unwrap()), Some(DELETED), "ends on a deleted line");
}

#[test]
fn the_main_action_follows_the_pending_review() {
    let v = synced(|_| {});
    assert_eq!((review_label(&v), primary_is_review(&v)), ("Start a review", false));
    let v = synced(|d| d.pending_review = Some(9));
    assert_eq!((review_label(&v), primary_is_review(&v)), ("Add review comment", true));
}

#[test]
fn whole_file_lines_outside_the_hunks_are_refused() {
    let normal = diff();
    assert_eq!(line_blocked(&normal, &dl('+', None, Some(2), "B")), None);
    assert_eq!(line_blocked(&normal, &dl(' ', Some(3), Some(3), "c")), None);
    assert_eq!(line_blocked(&normal, &dl(' ', Some(10), Some(10), "far")), Some(OUTSIDE));
    assert_eq!(line_blocked(&normal, &dl('>', None, None, "")), None, "a marker has no button at all");
}

#[test]
fn edited_drafts_are_confirmed_before_discarding() {
    let mut v = synced(|_| {});
    drafted(&mut v, 2, 2, " \n");
    assert!(!edited(v.line_draft.as_ref().unwrap()));
    drafted(&mut v, 2, 2, "Rename");
    let d = v.line_draft.as_ref().unwrap();
    assert!(edited(d));
    match discard_confirm(d) {
        Modal::ConfirmDanger { title, body, button, then, .. } => {
            assert_eq!((title.as_str(), button.as_str()), ("Discard comment", "Discard"));
            assert_eq!(body, "Discard your comment on line 2 of f.txt? It hasn't been posted.");
            assert!(matches!(*then, Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Discard))));
        }
        _ => panic!("not a danger confirmation"),
    }
}

fn state(provider: Provider, v: PrView) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let t = Target { provider, host: "example.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.session.forge = Some(ForgeState::new(Some(t), "origin".into(), None));
    st.session.pull_request.view = Some(v);
    st
}

#[test]
fn diff_comments_only_for_a_shown_file_on_github() {
    let mut v = synced(|_| {});
    drafted(&mut v, 1, 2, "");
    let st = state(Provider::GitHub, v.clone());
    let c = diff_comments(&st, &target()).expect("a pull request file on GitHub");
    assert_eq!(c.blocked, None);
    assert_eq!(c.end, Some((LineSide::New, 2)));
    assert_eq!(c.draft_hunk, Some(0));
    assert_eq!(c.draft, HashSet::from([(LineSide::Old, 2), (LineSide::New, 2)]));
    assert!(esc_cancels(&st));
    let v = st.session.pull_request.view.as_ref().unwrap();
    assert!(draft_on(v, "f.txt") && !draft_on(v, "g.txt"));
    assert!(diff_comments(&st, &DiffTarget::Between(oid(3), oid(9), "f.txt".into())).is_none(), "not the panel's diff");
    assert!(diff_comments(&state(Provider::GitLab, v.clone()), &target()).is_none(), "not on GitLab");
}

fn thread(id: &str, first: u64) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: "f.txt".into(),
        side: LineSide::New,
        line: Some(2),
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: String::new(),
        comments: vec![ThreadComment {
            id: Some(first),
            author: "me".into(),
            time: 1,
            body: "x".into(),
            url: String::new(),
            pending: false,
        }],
        more: 0,
    }
}

#[test]
fn a_reloaded_page_adopts_the_new_threads_state() {
    let mut v = synced(|_| {});
    v.forms.replies.insert("new:901".into(), "Also this".into());
    v.forms.replies.insert("T_other".into(), "Kept".into());
    v.threads.open.insert("new:901".into(), true);
    v.threads.focus = Some("new:901".into());
    let page =
        PrPage { detail: detail(), entries: Vec::new(), threads: vec![thread("PRRT_9", 901), thread("T_other", 5)], threads_note: None };
    adopt_new_threads(&mut v, &page);
    assert_eq!(v.forms.replies.get("PRRT_9").map(String::as_str), Some("Also this"));
    assert!(!v.forms.replies.contains_key("new:901"));
    assert_eq!(v.forms.replies["T_other"], "Kept");
    assert_eq!(v.threads.open.get("PRRT_9"), Some(&true));
    assert_eq!(v.threads.focus.as_deref(), Some("PRRT_9"));
}
