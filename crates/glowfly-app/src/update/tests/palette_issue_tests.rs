//! Headless tests for the palette's issue entries: Create issue… (and its reason while
//! blocked), View issue… with its sub-list over the listed issues, and the recent ids.

use super::issue_tests::{done, github, listed, open_repo};
use super::update;
use crate::effect::{Effect, IssueCall, IssueEffect};
use crate::msg::{Msg, PaletteMsg};
use crate::state::forge_filters::IssueList;
use crate::state::palette::{Mode, Palette, PaletteCommand, PickKind, shown};
use crate::state::{AppState, Modal, Status};
use glowfly_core::forge::{Connection, FilterChoice, Issue, Provider, Target};
use std::time::Instant;

fn send(st: &mut AppState, cmd: PaletteCommand) -> Vec<Effect> {
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now())
}

/// The entry labelled `label` and its detail, when listed.
fn entry(st: &AppState, label: &str) -> Option<Option<String>> {
    shown(st, "").into_iter().find(|s| s.entry.label == label).map(|s| s.entry.detail)
}

#[test]
fn create_issue_opens_the_dialog_or_says_why_not() {
    let (mut st, dir) = open_repo("palette-issue-create");
    assert_eq!(entry(&st, "Create issue…"), Some(None));
    let fx = send(&mut st, PaletteCommand::CreateIssue);
    assert!(matches!(st.modal, Some(Modal::CreateIssue(_))));
    assert!(fx.iter().any(|e| matches!(e, Effect::Issue(IssueEffect::Call(j)) if matches!(j.call, IssueCall::Options(_)))));
    assert_eq!(st.palette_recent.keys(), ["create-issue"]);
    st.modal = None;

    let trello = Target { provider: Provider::Trello, host: "trello.com".into(), ..github() };
    st.session.forge.as_mut().unwrap().tracker = Some(trello.clone());
    assert_eq!(entry(&st, "Create card…"), Some(Some("Connect Trello at trello.com in Preferences first".into())));
    st.palette = Some(Palette::default());
    send(&mut st, PaletteCommand::CreateIssue);
    assert!(st.modal.is_none());
    assert!(st.palette.is_some(), "Enter does nothing while blocked");
    st.settings.connections.push(Connection { account: trello.account(), user: "Ada".into(), method: "access token".into() });
    assert_eq!(entry(&st, "Create card…"), Some(None), "Trello cards once connected");
    st.settings.connections.clear();

    st.session.forge.as_mut().unwrap().tracker = None;
    assert!(entry(&st, "Create issue…").is_none(), "no tracker");
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    st.session.forge.as_mut().unwrap().host = Some(dc);
    let why = "Bitbucket Data Center has no issue tracker of its own; choose Jira under Repo-specific preferences > Issue tracker";
    assert_eq!(entry(&st, "Create issue…"), Some(Some(why.into())), "Bitbucket Data Center points to Jira");
    assert!(entry(&st, "View issue…").is_none());
    done(st, dir);
}

#[test]
fn view_issue_picks_from_the_listed_issues() {
    let (mut st, dir) = open_repo("palette-issue-view");
    let fs = st.session.forge.as_mut().unwrap();
    let searched = Issue { title: "Faster search".into(), ..listed("14") };
    fs.filters.issues = Some(IssueList {
        choice: FilterChoice::default(),
        conditions: Default::default(),
        items: vec![listed("12"), searched],
        status: Status::Ok,
    });
    assert_eq!(entry(&st, "View issue…"), Some(Some("2 listed".into())));
    send(&mut st, PaletteCommand::ChooseIssueToView);
    assert_eq!(st.palette_recent.keys(), ["view-issue"]);
    let Some(Palette { mode: Mode::Pick(pick), .. }) = st.palette.as_ref() else { panic!("the sub-list") };
    assert_eq!(pick.kind, PickKind::ViewIssue);
    assert_eq!((pick.label(0), pick.label(1), pick.label(2)), ("#12  Crash on start", "#14  Faster search", ""), "each issue once");
    assert_eq!(pick.detail(1).as_deref(), Some("open"));
    let cmd = pick.command(1).unwrap();
    assert_eq!(cmd, PaletteCommand::ViewIssue("14".into()));
    assert_eq!(cmd.recall_key(), None);
    let fx = send(&mut st, cmd);
    assert!(st.palette.is_none());
    assert_eq!(st.session.issue.view.as_ref().map(|v| v.key.as_str()), Some("14"));
    assert!(fx.iter().any(|e| matches!(e, Effect::Issue(IssueEffect::Call(j)) if matches!(&j.call, IssueCall::Load(k) if k == "14"))));
    assert_eq!(st.palette_recent.keys(), ["view-issue"], "the pick itself isn't recorded");

    let fs = st.session.forge.as_mut().unwrap();
    fs.my_issues.clear();
    fs.filters.issues = None;
    assert!(entry(&st, "View issue…").is_none(), "nothing listed");
    done(st, dir);
}
