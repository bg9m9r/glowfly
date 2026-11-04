//! Headless tests for the commit panel's rail beside a tool view: which view counts (in the
//! central area's order), collapsing without saving, expanding for the open view only, Go to the
//! commit message expanding it, the toggle key hiding it, Close bringing the full panel back (the
//! pull request panel, the rebase editor and the conflict view too), a stopped rebase keeping
//! the full panel, and the rail's change count.

use super::issue_tests::open_repo;
use super::tests::{git, scratch_repo, state};
use super::update;
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{self, DiffTarget, FileDiff, LoadOptions};
use glowfly_model::effect::Effect;
use glowfly_model::effect::SaveEffect;
use glowfly_model::gitflow::GitFlowMsg;
use glowfly_model::keymap::KeyAction;
use glowfly_model::msg::{BlameMsg, ConflictMsg, HistoryMsg, IssueMsg, LayoutMsg, Msg, RebaseMsg};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::blame::BlameView;
use glowfly_model::state::conflict::ConflictView;
use glowfly_model::state::history::{FileHistoryView, HistoryKind};
use glowfly_model::state::layout::{DetailsShape, Panel, ToolView, details_shape, tool_view, wip_change_count};
use glowfly_model::state::rebase::RebasePanel;
use glowfly_model::state::{AppState, Selection};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_))))
}

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn done(st: AppState, dir: PathBuf) {
    drop(st);
    let _ = std::fs::remove_dir_all(dir);
}

fn blame() -> Option<BlameView> {
    Some(BlameView::loading("a.txt".into(), None))
}

fn pr() -> glowfly_core::forge::PullRequest {
    glowfly_core::forge::PullRequest {
        id: 7,
        title: "PR".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

fn head(st: &AppState) -> git2::Oid {
    st.snap.as_ref().unwrap().commits[0].oid
}

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

/// Any message that changes nothing else, so `update` looks at the views just set by hand.
fn nudge(st: &mut AppState) -> Vec<Effect> {
    send(st, Msg::Layout(LayoutMsg::ZoomReset))
}

#[test]
fn tool_view_follows_the_central_order() {
    let (mut st, dir) = open_repo("rail-order");
    st.session.blame.view = blame();
    st.session.history.view = Some(FileHistoryView::loading_kind("src".into(), None, HistoryKind::Folder));
    assert_eq!(tool_view(&st), Some(ToolView::Blame));
    st.session.blame.view = None;
    assert_eq!(tool_view(&st), Some(ToolView::FolderHistory));
    st.session.history.view = None;
    send(&mut st, Msg::Issue(IssueMsg::Open("12".into())));
    assert_eq!(tool_view(&st), Some(ToolView::Issue));
    let pr = glowfly_model::state::pull_request::PrView::new(pr(), st.session.forge.as_ref().unwrap(), st.snap.as_ref().unwrap());
    st.session.pull_request.view = Some(pr);
    assert_eq!(tool_view(&st), Some(ToolView::PullRequest), "the pull request panel covers the issue panel");
    st.session.pull_request.view = None;
    st.dashboard.open = true;
    assert_eq!(tool_view(&st), None);
    st.dashboard.open = false;
    st.session.issue.view = None;
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Unstaged("a.txt".into()), Arc::new(diff)));
    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(head(&st))));
    assert_eq!(tool_view(&st), None, "a diff covers the rebase editor, beside the full panel");
    st.session.diff.view = None;
    assert_eq!(tool_view(&st), Some(ToolView::Rebase));
    st.session.rebase.view = None;
    st.session.gitflow_panel = true;
    assert_eq!(tool_view(&st), Some(ToolView::GitFlow));
    st.session.conflict.view = Some(ConflictView::loading("a.txt".into()));
    assert_eq!(tool_view(&st), Some(ToolView::Conflict));
    st.snap = None;
    assert_eq!(tool_view(&st), None, "no repository");
    super::issue_tests::done(st, dir);
}

#[test]
fn details_shape_collapses_in_tool_views_without_saving() {
    let (mut st, dir) = opened("rail-collapse");
    assert_eq!(details_shape(&st), DetailsShape::Full);
    st.session.blame.view = blame();
    let fx = nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Rail);
    assert!(st.settings.show_details_panel);
    assert!(!saves(&fx));
    st.settings.show_details_panel = false;
    assert_eq!(details_shape(&st), DetailsShape::Hidden, "hidden wins over the rail");
    done(st, dir);
}

#[test]
fn expand_lasts_for_the_open_view_only() {
    let (mut st, dir) = opened("rail-expand");
    st.session.blame.view = blame();
    let fx = send(&mut st, Msg::Layout(LayoutMsg::ExpandDetails));
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.view.details_expanded_for, Some(ToolView::Blame));
    assert_eq!(details_shape(&st), DetailsShape::Full);
    send(&mut st, Msg::Blame(BlameMsg::Close));
    assert_eq!(details_shape(&st), DetailsShape::Full);
    assert_eq!(st.view.details_expanded_for, None);
    st.session.blame.view = blame();
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Rail, "the next blame starts collapsed");
    st.session.blame.view = None;

    st.session.history.view = Some(FileHistoryView::loading("a.txt".into(), None));
    send(&mut st, Msg::Layout(LayoutMsg::ExpandDetails));
    assert_eq!(details_shape(&st), DetailsShape::Full);
    st.session.history.view = Some(FileHistoryView::loading_kind(String::new(), None, HistoryKind::Folder));
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Rail, "another tool view collapses it again");
    send(&mut st, Msg::Layout(LayoutMsg::ExpandDetails));
    st.session.conflict.view = Some(ConflictView::loading("a.txt".into()));
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Rail);
    assert!(st.settings.show_details_panel);
    done(st, dir);
}

#[test]
fn expand_without_a_tool_view_does_nothing() {
    let (mut st, dir) = opened("rail-expand-graph");
    let fx = send(&mut st, Msg::Layout(LayoutMsg::ExpandDetails));
    assert!(fx.is_empty());
    assert_eq!(st.view.details_expanded_for, None);
    assert_eq!(details_shape(&st), DetailsShape::Full);
    done(st, dir);
}

#[test]
fn go_to_commit_message_expands_the_rail() {
    let (mut st, dir) = opened("rail-focus-message");
    st.session.blame.view = blame();
    st.session.selection = Selection::None;
    let fx = send(&mut st, Msg::Shortcut(KeyAction::FocusMessage));
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.view.focus_message);
    assert_eq!(details_shape(&st), DetailsShape::Full);
    assert!(st.settings.show_details_panel);
    assert!(!saves(&fx), "the panel was shown already");
    // Hidden: it is shown (and saved), and in full.
    let mut st2 = state();
    st2.snap = st.snap.take();
    st2.session.blame.view = blame();
    st2.settings.show_details_panel = false;
    let fx = send(&mut st2, Msg::Shortcut(KeyAction::FocusMessage));
    assert!(st2.settings.show_details_panel);
    assert!(saves(&fx));
    assert_eq!(details_shape(&st2), DetailsShape::Full);
    done(st2, dir);
}

#[test]
fn toggle_key_hides_from_the_rail() {
    let (mut st, dir) = opened("rail-toggle");
    st.session.blame.view = blame();
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Rail);
    let fx = send(&mut st, Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)));
    assert!(!st.settings.show_details_panel);
    assert!(saves(&fx));
    assert_eq!(details_shape(&st), DetailsShape::Hidden);
    done(st, dir);
}

/// A tool view's name, how to open it, and the message that closes it.
type Case = (&'static str, fn(&mut AppState), Msg);

#[test]
fn closing_each_tool_view_restores_the_panel() {
    let (mut st, dir) = open_repo("rail-close");
    let cases: [Case; 4] = [
        (
            "history",
            |st| st.session.history.view = Some(FileHistoryView::loading("a.txt".into(), None)),
            Msg::History(HistoryMsg::CloseFileHistory),
        ),
        (
            "conflict",
            |st| st.session.conflict.view = Some(ConflictView::loading("a.txt".into())),
            Msg::Conflict(ConflictMsg::CloseConflict),
        ),
        (
            "issue",
            |st| {
                update(st, Msg::Issue(IssueMsg::Open("12".into())), Instant::now());
            },
            Msg::Issue(IssueMsg::Close),
        ),
        ("git flow", |st| st.session.gitflow_panel = true, Msg::GitFlow(GitFlowMsg::ClosePanel)),
    ];
    for (name, open, close) in cases {
        open(&mut st);
        nudge(&mut st);
        assert_eq!(details_shape(&st), DetailsShape::Rail, "{name}");
        send(&mut st, close);
        assert_eq!(details_shape(&st), DetailsShape::Full, "{name}");
    }
    super::issue_tests::done(st, dir);
}

/// Each mode that collapses the panel, how to open it, and the message that leaves it.
fn modes() -> [Case; 3] {
    [
        (
            "pull request",
            |st| {
                let pr =
                    glowfly_model::state::pull_request::PrView::new(pr(), st.session.forge.as_ref().unwrap(), st.snap.as_ref().unwrap());
                st.session.pull_request.view = Some(pr);
            },
            Msg::PullRequest(PullRequestMsg::Close),
        ),
        (
            "rebase",
            |st| st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(head(st)))),
            Msg::Rebase(RebaseMsg::CloseRebase),
        ),
        (
            "conflict",
            |st| st.session.conflict.view = Some(ConflictView::loading("a.txt".into())),
            Msg::Conflict(ConflictMsg::CloseConflict),
        ),
    ]
}

#[test]
fn each_mode_collapses_and_leaving_it_restores_the_panel() {
    let (mut st, dir) = open_repo("rail-modes");
    for (name, open, close) in modes() {
        open(&mut st);
        let fx = nudge(&mut st);
        assert_eq!(details_shape(&st), DetailsShape::Rail, "{name}");
        assert!(st.settings.show_details_panel && !saves(&fx), "{name}: the setting is untouched");
        // Expanded, it stays in full until the mode ends.
        send(&mut st, Msg::Layout(LayoutMsg::ExpandDetails));
        nudge(&mut st);
        assert_eq!(details_shape(&st), DetailsShape::Full, "{name}: expanded");
        send(&mut st, close);
        assert_eq!(details_shape(&st), DetailsShape::Full, "{name}: left");
        assert_eq!(st.view.details_expanded_for, None, "{name}: the expansion ends with the mode");
        // The next time starts collapsed again.
        open(&mut st);
        nudge(&mut st);
        assert_eq!(details_shape(&st), DetailsShape::Rail, "{name}: again");
        let (_, _, close) = modes().into_iter().find(|(n, ..)| *n == name).unwrap();
        send(&mut st, close);
    }
    super::issue_tests::done(st, dir);
}

#[test]
fn the_toggle_key_still_works_in_each_mode() {
    let (mut st, dir) = open_repo("rail-modes-toggle");
    for (name, open, close) in modes() {
        open(&mut st);
        nudge(&mut st);
        send(&mut st, Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)));
        assert_eq!(details_shape(&st), DetailsShape::Hidden, "{name}: hidden");
        send(&mut st, Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)));
        assert_eq!(details_shape(&st), DetailsShape::Rail, "{name}: shown again, as the rail");
        send(&mut st, close);
        // Hidden in the mode stays hidden after it: the person's own choice.
        send(&mut st, Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)));
        assert_eq!(details_shape(&st), DetailsShape::Hidden, "{name}: the saved choice outlives the mode");
        send(&mut st, Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)));
    }
    super::issue_tests::done(st, dir);
}

#[test]
fn a_stopped_rebase_keeps_the_full_panel() {
    let dir = super::rebase_edit_commit_tests::stopped_at_edit("rail-stopped-rebase");
    let mut st = state();
    super::tests::drive(&mut st, Msg::Repo(glowfly_model::msg::RepoMsg::Open(dir.clone())));
    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(head(&st))));
    nudge(&mut st);
    assert_eq!(tool_view(&st), Some(ToolView::Rebase));
    assert_eq!(details_shape(&st), DetailsShape::Full, "Edit remaining steps… keeps the steps beside it");
    st.session.rebase.view = None;
    st.session.conflict.view = Some(ConflictView::loading("a.txt".into()));
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Full, "the conflict view too");
    st.session.conflict.view = None;
    st.session.blame.view = blame();
    nudge(&mut st);
    assert_eq!(details_shape(&st), DetailsShape::Full, "and every other view");
    done(st, dir);
}

#[test]
fn wip_change_count_counts_paths_once() {
    let dir = scratch_repo("rail-count");
    // a.txt staged, then modified again; b.txt untracked.
    git(&dir, &["add", "a.txt"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    let snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!((snap.staged.len(), snap.unstaged.len()), (1, 2));
    assert_eq!(wip_change_count(&snap), 2);
    let _ = std::fs::remove_dir_all(dir);
}
