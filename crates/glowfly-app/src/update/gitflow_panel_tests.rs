//! Headless tests for the Git Flow panel (opening, closing, what closes it, parking it with its
//! tab) and for starting a flow branch from an issue (the message, the palette entries and their
//! issue sub-list).

use super::gitflow_tests::{cleanup, flow, git_job, initialized, last_log, open, run};
use super::tests::{drive, git, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::state::history::FileHistoryView;
use crate::state::palette::{Mode, PaletteCommand as C, PickKind, entries};
use crate::state::rebase::RebasePanel;
use crate::state::{AppState, ForgeState, Modal, Selection};
use glowfly_core::forge::{Issue, Provider, Target};
use glowfly_core::gitcli::args;
use glowfly_core::gitflow::FlowKind;
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{self, DiffTarget, FileDiff, LoadOptions};
use std::time::Instant;

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

fn head(st: &AppState) -> git2::Oid {
    st.snap.as_ref().unwrap().head_oid.unwrap()
}

#[test]
fn open_and_close() {
    let (mut st, dir) = initialized("panel-open");
    let h = head(&st);
    st.diff = Some((DiffTarget::Commit(h, "a.txt".into()), empty_diff().into()));
    st.history = Some(FileHistoryView::loading("a.txt".into(), None));
    flow(&mut st, GitFlowMsg::OpenPanel);
    assert!(st.gitflow_panel);
    assert!(st.diff.is_none() && st.history.is_none());
    flow(&mut st, GitFlowMsg::ClosePanel);
    assert!(!st.gitflow_panel);
    cleanup(&dir);

    let mut st = state();
    update(&mut st, Msg::GitFlow(GitFlowMsg::OpenPanel), Instant::now());
    assert!(!st.gitflow_panel, "nothing without a repository");
}

#[test]
fn rebase_panel_and_git_flow_panel_replace_each_other() {
    let (mut st, dir) = initialized("panel-rebase");
    let h = head(&st);
    st.rebase = Some(RebasePanel::loading(RebaseBase::FromCommit(h)));
    flow(&mut st, GitFlowMsg::OpenPanel);
    assert!(st.gitflow_panel && st.rebase.is_none());

    st.gitflow_panel = false;
    let mut starting = RebasePanel::loading(RebaseBase::FromCommit(h));
    starting.starting = true;
    st.rebase = Some(starting);
    flow(&mut st, GitFlowMsg::OpenPanel);
    assert!(!st.gitflow_panel && st.rebase.is_some(), "not while a rebase is starting");

    st.rebase = None;
    flow(&mut st, GitFlowMsg::OpenPanel);
    update(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(h))), Instant::now());
    assert!(st.rebase.is_some() && !st.gitflow_panel);
    cleanup(&dir);
}

#[test]
fn jumping_to_a_commit_closes_it() {
    let (mut st, dir) = initialized("panel-jump");
    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let tip = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "feature/x").and_then(|b| b.oid).unwrap();
    flow(&mut st, GitFlowMsg::OpenPanel);
    let fx = update(&mut st, Msg::Repo(RepoMsg::JumpTo(tip)), Instant::now());
    assert!(!st.gitflow_panel);
    assert_eq!(st.selection, Selection::Commit(tip));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid, .. } if *oid == tip)));
    cleanup(&dir);
}

#[test]
fn parked_with_its_tab() {
    let (mut st, dir) = initialized("panel-tabs");
    flow(&mut st, GitFlowMsg::OpenPanel);
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(!st.gitflow_panel, "the new tab shows no panel");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.gitflow_panel, "back on its tab");

    // A repository opened into this tab starts without it.
    let snap = Box::new(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(snap) });
    update(&mut st, m, Instant::now());
    assert!(!st.gitflow_panel);
    cleanup(&dir);
}

fn start_form(st: &AppState) -> Option<(FlowKind, String)> {
    match &st.modal {
        Some(Modal::GitFlowStart(f)) => Some((f.kind, f.name.clone())),
        _ => None,
    }
}

#[test]
fn start_from_issue_opens_the_dialog_with_its_name() {
    let (mut st, dir) = initialized("issue-start");
    let snap = st.snap.as_mut().unwrap();
    snap.tags = vec![("v0.1.0".into(), snap.head_oid.unwrap())];
    snap.gitflow.version_tag = Some("v".into());
    let from = |kind, name: &str| GitFlowMsg::StartFromIssue { kind, name: name.into() };
    flow(&mut st, from(FlowKind::Feature, "PROJ-12-crash-on-start"));
    assert_eq!(start_form(&st), Some((FlowKind::Feature, "PROJ-12-crash-on-start".into())));
    st.modal = None;
    flow(&mut st, from(FlowKind::Release, "PROJ-12-crash-on-start"));
    assert_eq!(start_form(&st), Some((FlowKind::Release, "PROJ-12-crash-on-start".into())), "not the version suggestion");

    st.modal = None;
    st.snap.as_mut().unwrap().locals.retain(|b| b.name != "develop");
    flow(&mut st, from(FlowKind::Feature, "x"));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "develop doesn't exist. Save the Git Flow settings to create it.");
    cleanup(&dir);

    let (mut st, dir) = open("issue-off");
    flow(&mut st, from(FlowKind::Feature, "x"));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), crate::state::gitflow::NOT_SET_UP);
    cleanup(&dir);
}

#[test]
fn start_from_issue_end_to_end() {
    let (mut st, dir) = initialized("issue-e2e");
    let name = "PROJ-12-crash-on-start";
    flow(&mut st, GitFlowMsg::StartFromIssue { kind: FlowKind::Feature, name: name.into() });
    let (kind, typed) = start_form(&st).expect("the Start dialog");
    let fx = run(&mut st, Msg::GitFlow(GitFlowMsg::Start { kind, name: typed.clone() }));
    let branch = format!("feature/{name}");
    assert_eq!(git_job(&fx), Some(vec![args(&["checkout", "-b", &branch, "refs/heads/develop"])]));
    super::tests::finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some(branch.as_str()));
    cleanup(&dir);
}

#[test]
fn palette_opens_the_panel_once_set_up() {
    let (st, dir) = open("palette-panel-off");
    assert!(!entries(&st).iter().any(|e| e.command == C::OpenGitFlowPanel));
    cleanup(&dir);

    let (mut st, dir) = initialized("palette-panel");
    assert!(entries(&st).iter().any(|e| e.command == C::OpenGitFlowPanel && e.label == "Open Git Flow panel"));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::OpenGitFlowPanel)));
    assert!(st.gitflow_panel);
    assert_eq!(st.palette_recent.keys(), ["gitflow-panel"]);
    cleanup(&dir);
}

/// A GitHub forge with one assigned issue, #12 "Crash on start".
fn with_issue(st: &mut AppState) {
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let mut f = ForgeState::new(Some(target.clone()), "origin".into(), Some(target));
    f.my_issues = vec![Issue {
        key: "12".into(),
        title: "Crash on start".into(),
        state: "open".into(),
        kind: "Issue".into(),
        assignee: Some("ada".into()),
        url: "https://github.com/o/r/issues/12".into(),
        facets: Default::default(),
    }];
    st.forge = Some(f);
}

fn issue_entries(st: &AppState) -> Vec<(C, String, Option<String>)> {
    entries(st).into_iter().filter(|e| matches!(e.command, C::ChooseGitFlowIssue(_))).map(|e| (e.command, e.label, e.detail)).collect()
}

#[test]
fn palette_starts_from_an_assigned_issue() {
    let (mut st, dir) = open("palette-issue-off");
    with_issue(&mut st);
    assert!(issue_entries(&st).is_empty(), "not without Git Flow");
    cleanup(&dir);

    let (mut st, dir) = initialized("palette-issue");
    assert!(issue_entries(&st).is_empty(), "not without issues");
    with_issue(&mut st);
    let one = Some("1 assigned".to_owned());
    assert_eq!(
        issue_entries(&st),
        [
            (C::ChooseGitFlowIssue(FlowKind::Feature), "Start Git Flow feature from issue…".into(), one.clone()),
            (C::ChooseGitFlowIssue(FlowKind::Release), "Start Git Flow release from issue…".into(), one.clone()),
            (C::ChooseGitFlowIssue(FlowKind::Hotfix), "Start Git Flow hotfix from issue…".into(), one),
        ]
    );
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseGitFlowIssue(FlowKind::Feature))));
    assert_eq!(st.palette_recent.keys(), ["gitflow-issue-feature"]);
    let Some(Mode::Pick(pick)) = st.palette.as_ref().map(|p| &p.mode) else { panic!("the issue step") };
    assert_eq!(pick.kind, PickKind::GitFlowIssue(FlowKind::Feature));
    assert_eq!(pick.label(0), "#12  Crash on start");
    assert_eq!(pick.detail(0).as_deref(), Some("open"));
    let cmd = pick.command(0).unwrap();
    assert_eq!(cmd, C::StartGitFlowFromIssue { kind: FlowKind::Feature, name: "12-crash-on-start".into() });
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
    assert!(st.palette.is_none());
    assert_eq!(start_form(&st), Some((FlowKind::Feature, "12-crash-on-start".into())));
    assert_eq!(st.palette_recent.keys(), ["gitflow-issue-feature"], "the pick itself is not recorded");
    cleanup(&dir);
}
