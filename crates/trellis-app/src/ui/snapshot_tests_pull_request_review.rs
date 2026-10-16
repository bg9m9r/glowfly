//! The pull request panel's Overview tab: the conversation, the review form and the merge form,
//! as a reviewer, as the author, once merged, and on a service without in-app reviews; and the
//! messages its tabs and buttons send.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Page, PrTab, PrView};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use trellis_core::forge::{Entry, EntryKind, MergeMethod, MergeState, PrDetail, PrPage, PrState, Provider, Target, Verdict};
use trellis_core::settings::ThemeChoice;

fn detail() -> PrDetail {
    PrDetail {
        state: PrState::Open,
        draft: false,
        locked: false,
        mergeable: Some(true),
        merge_state: MergeState::Clean,
        head_sha: "h1".into(),
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

fn entry(author: &str, day: i64, kind: EntryKind, body: &str) -> Entry {
    // Days in January 2026, at 10:00 UTC.
    Entry { author: author.into(), time: 1_767_225_600 + (day - 1) * 86_400 + 36_000, kind, body: body.into(), url: String::new() }
}

/// The panel's page loaded by hand with `d` and `entries`, the forms fitted to it.
fn load(st: &mut AppState, d: PrDetail, entries: Vec<Entry>) {
    let v = st.pull_request.as_mut().unwrap();
    crate::state::pull_request_actions::prefill(&mut v.forms, &d);
    v.page = Page::Loaded(Box::new(PrPage { detail: d, entries }));
}

#[test]
fn pull_request_overview_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-overview", true);
    let entries = vec![
        entry("linus", 12, EntryKind::Comment, "Could the results be paged?"),
        entry("grace", 13, EntryKind::Review(Verdict::Approved), "Reads well. Paging can come later."),
        entry("linus", 14, EntryKind::Review(Verdict::ChangesRequested), "Please add a test for an empty result."),
    ];
    load(&mut st, detail(), entries);
    let v = st.pull_request.as_mut().unwrap();
    v.forms.merge_method = Some(MergeMethod::Squash);
    v.forms.merge_title = "Search results page (#12)".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_overview_dark");
    assert!(h.query_by_label("No conflicts with main").is_some());
    assert!(h.query_by_label("opened this pull request").is_some());
    assert!(h.query_by_label("requested changes").is_some());
    assert!(h.get_by_role_and_label(Role::Button, "Comment").is_disabled(), "nothing written yet");
    assert!(!h.get_by_label("Merge pull request").is_disabled());
    assert!(h.get_by_label("Rebase and merge").is_disabled(), "not allowed by the repository");

    // Files changed shows that tab.
    h.get_by_label("Files changed").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::Tab(PrTab::Files)))));
    // Merge asks first.
    h.state_mut().1.clear();
    h.get_by_label("Merge pull request").click();
    h.run();
    let confirm = h.state().1.iter().find_map(|m| match m {
        Msg::Confirm { body, then, .. } => Some((body.clone(), matches!(**then, Msg::PullRequest(PullRequestMsg::Merge)))),
        _ => None,
    });
    let (body, merges) = confirm.expect("a confirmation");
    assert!(merges && body.contains("by squashing its commits"), "{body}");
    // A written comment can be posted.
    h.state_mut().1.clear();
    h.state_mut().0.pull_request.as_mut().unwrap().forms.comment = "Thanks!".into();
    h.run();
    h.get_by_role_and_label(Role::Button, "Comment").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::PostComment))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_author_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-author", true);
    let mut d = detail();
    d.me = "ada".into();
    d.merge_state = MergeState::Conflicts;
    d.mergeable = Some(false);
    load(&mut st, d, vec![entry("grace", 13, EntryKind::Review(Verdict::Commented), "")]);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_author_light");
    assert!(h.get_by_label("Approve").is_disabled());
    assert!(h.get_by_label("Request changes").is_disabled());
    assert!(h.get_by_label("Merge pull request").is_disabled());
    assert!(h.query_by_label("Conflicts with main must be resolved first").is_some());
    assert!(h.query_by_label("Left comments on the code.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_merged_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-merged", true);
    let mut d = detail();
    d.state = PrState::Merged;
    load(&mut st, d, vec![entry("grace", 13, EntryKind::Review(Verdict::Approved), "")]);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_merged_light");
    assert!(h.query_by_label("Merged into main.").is_some());
    h.get_by_label("Delete search on origin").click();
    h.run();
    let then = h.state().1.iter().find_map(|m| match m {
        Msg::Confirm { title, then, .. } if title == "Delete remote branch" => Some(matches!(**then, Msg::Job(_))),
        _ => None,
    });
    assert_eq!(then, Some(true));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_gitlab_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-gitlab", true);
    let fs = st.forge.as_mut().unwrap();
    fs.host = Some(Target {
        provider: Provider::GitLab,
        host: "gitlab.com".into(),
        owner: "acme".into(),
        project: String::new(),
        repo: "demo".into(),
    });
    let view = PrView::new(fs.prs[0].clone(), fs, st.snap.as_ref().unwrap());
    assert_eq!(view.page, Page::Unsupported);
    st.pull_request = Some(view);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_gitlab_light");
    assert!(h.query_by_label_contains("work with GitHub for now").is_some());
    assert!(h.query_by_label("Your review").is_none() && h.query_by_label("Write a comment").is_none());
    drop(h);
    remove_fixture(&dir);
}
