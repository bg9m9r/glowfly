//! Pull request and issue filters: choosing one, the signed-in user, issue searches and their
//! replies, refreshes, and which calls are never sent.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, FilterCall, FilterResult, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{FilterMsg, ForgeMsg, Msg, RepoMsg};
use crate::state::forge_filters::{Hint, MeState, issue_view, pr_view};
use crate::state::{AppState, ForgeState, Status};
use glowfly_core::forge::{
    Auth, Builtin, Connection, FilterChoice, FilterKind, Issue, IssueFacets, Me, PrFacets, Provider, PullRequest, Target,
};
use glowfly_core::settings::{RepoTrust, Settings};
use std::path::PathBuf;
use std::time::Instant;

/// A repository whose origin is on GitHub, opened, with the service answering.
pub(super) fn open(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fs = st.session.forge.as_mut().expect("the service is detected");
    fs.host_status = Status::Ok;
    fs.tracker_status = Status::Ok;
    fs.loading = 0;
    (st, dir)
}

pub(super) fn send(st: &mut AppState, m: FilterMsg) -> Vec<Effect> {
    update(st, Msg::ForgeFilter(m), Instant::now())
}

/// The filter calls among `fx`, with where they go.
pub(super) fn filter_calls(fx: &[Effect]) -> Vec<(&Target, &FilterCall)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Forge { target, call: ForgeCall::Filter(c), .. } => Some((target, c)),
            _ => None,
        })
        .collect()
}

/// The settings the effects in `fx` save, if any.
pub(super) fn saved(fx: &[Effect]) -> Option<&Settings> {
    fx.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(&**s),
        _ => None,
    })
}

pub(super) fn reply(st: &mut AppState, target: Target, r: FilterResult) {
    let reply = ForgeReply { target, auth: None, results: vec![ForgeResult::Filter(r)] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
}

fn choose(kind: FilterKind, b: Builtin) -> FilterMsg {
    FilterMsg::Choose { kind, choice: FilterChoice::Builtin(b) }
}

fn host(st: &AppState) -> Target {
    st.session.forge.as_ref().unwrap().host.clone().unwrap()
}

fn fs(st: &AppState) -> &ForgeState {
    st.session.forge.as_ref().unwrap()
}

pub(super) fn pr(id: u64, author: &str) -> PullRequest {
    PullRequest {
        id,
        title: format!("P{id}"),
        description: String::new(),
        source: format!("b{id}"),
        target: "main".into(),
        author: author.into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: PrFacets { author_id: author.into(), ..Default::default() },
    }
}

fn issue(key: &str) -> Issue {
    Issue {
        key: key.into(),
        title: format!("I{key}"),
        state: "open".into(),
        kind: "Issue".into(),
        assignee: None,
        url: String::new(),
        facets: IssueFacets { labels: vec!["bug".into()], milestone: None },
    }
}

fn ada() -> Me {
    Me { id: "ada".into(), name: "Ada".into(), ..Default::default() }
}

fn error(auth: bool) -> ForgeError {
    ForgeError { message: "boom".into(), auth }
}

#[test]
fn choosing_mine_saves_it_and_asks_who_i_am_once() {
    let (mut st, dir) = open("ff-mine");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let fx = send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    let settings = saved(&fx).expect("the choice is saved");
    assert_eq!(settings.forge_filters_for(&real).choice(FilterKind::Prs), FilterChoice::Builtin(Builtin::MyPrs));
    let calls = filter_calls(&fx);
    assert_eq!(calls.len(), 1);
    assert!(matches!(calls[0], (t, FilterCall::Me { .. }) if *t == host(&st)));
    assert_eq!(fs(&st).filters.me, MeState::Asking);
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::NeedsMyReview))).is_empty(), "already asking");

    st.session.forge.as_mut().unwrap().prs = vec![pr(1, "ada"), pr(2, "grace"), pr(3, "Ada")];
    let filters = st.settings.forge_filters_for(&real);
    assert!(matches!(pr_view(fs(&st), &filters).hint, Some(Hint::Faint(h)) if h == "Finding your GitHub account…"));
    let h = host(&st);
    reply(&mut st, h, FilterResult::Me { epoch: 0, result: Ok(ada()) });
    let view = pr_view(fs(&st), &filters);
    assert!(view.rows.is_empty(), "nobody asked Ada for a review");
    assert_eq!(view.empty.as_deref(), Some("No open pull requests match \u{201c}Needs my review\u{201d}"));
    let fx = send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    assert!(filter_calls(&fx).is_empty(), "known");
    let filters = st.settings.forge_filters_for(&real);
    let view = pr_view(fs(&st), &filters);
    assert_eq!(view.rows.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(view.hint, Some(Hint::Faint("Mine · 2 of 3".into())));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn team_review_requests_count_as_needs_my_review() {
    let (mut st, dir) = open("ff-teams");
    let fx = send(&mut st, choose(FilterKind::Prs, Builtin::NeedsMyReview));
    assert!(matches!(filter_calls(&fx)[..], [(t, FilterCall::Me { .. })] if *t == host(&st)));
    let with = |id, f: PrFacets| PullRequest { facets: PrFacets { author_id: "grace".into(), ..f }, ..pr(id, "grace") };
    st.session.forge.as_mut().unwrap().prs = vec![
        with(1, PrFacets { awaiting_team_ids: vec!["o/core".into()], ..Default::default() }),
        with(2, PrFacets { awaiting_team_ids: vec!["o/docs".into()], ..Default::default() }),
        with(3, PrFacets { awaiting_ids: vec!["ada".into()], ..Default::default() }),
    ];
    let h = host(&st);
    reply(&mut st, h, FilterResult::Me { epoch: 0, result: Ok(Me { teams: vec!["O/Core".into()], ..ada() }) });
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    let view = pr_view(fs(&st), &filters);
    assert_eq!(view.rows.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(view.hint, Some(Hint::Faint("Needs my review · 2 of 3".into())));
    // Signing in again forgets the teams with the user.
    st.session.forge.as_mut().unwrap().filters.account_changed(true, false);
    assert_eq!(fs(&st).filters.me, MeState::Unknown);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failed_lookup_shows_the_error_and_the_next_refresh_asks_again() {
    let (mut st, dir) = open("ff-me-failed");
    send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    let h = host(&st);
    reply(&mut st, h, FilterResult::Me { epoch: 0, result: Err(error(false)) });
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    assert_eq!(pr_view(fs(&st), &filters).hint, Some(Hint::Error("boom".into())));
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now());
    assert!(filter_calls(&fx).iter().any(|(_, c)| matches!(c, FilterCall::Me { .. })));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn issue_filters_search_the_tracker() {
    let (mut st, dir) = open("ff-issues");
    st.session.forge.as_mut().unwrap().filters.me = MeState::Known(ada());
    let fx = send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    let calls = filter_calls(&fx);
    let created = Builtin::IssuesCreatedByMe.conditions();
    assert!(
        matches!(calls[..], [(_, FilterCall::Issues { conditions, me: Some(m), .. })] if *conditions == created && m.id == "ada"),
        "{calls:?}"
    );
    let choice = FilterChoice::Builtin(Builtin::IssuesCreatedByMe);
    let tracker = fs(&st).tracker.clone().unwrap();
    reply(
        &mut st,
        tracker.clone(),
        FilterResult::Issues { choice, conditions: created.clone(), epoch: 0, me: None, result: Ok(vec![issue("7")]) },
    );
    let list = fs(&st).filters.issues.as_ref().unwrap();
    assert_eq!((list.items.len(), &list.status), (1, &Status::Ok));
    assert!(fs(&st).cache.contains_key("7"), "mention chips resolve");
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    let view = issue_view(fs(&st), &filters);
    assert_eq!((view.rows.len(), view.hint.clone()), (1, Hint::Faint("GitHub · Created by me".into())));

    // A newer choice wins over a late reply for the old one; loading still goes down.
    send(&mut st, choose(FilterKind::Issues, Builtin::IssuesMentioningMe));
    let loading = fs(&st).loading;
    reply(&mut st, tracker.clone(), FilterResult::Issues { choice, conditions: created, epoch: 0, me: None, result: Ok(vec![issue("8")]) });
    assert_eq!(fs(&st).loading, loading - 1);
    let list = fs(&st).filters.issues.as_ref().unwrap();
    assert_eq!((list.items.len(), &list.status), (0, &Status::Idle));

    let fx = send(&mut st, choose(FilterKind::Issues, Builtin::IssuesAssignedToMe));
    assert!(filter_calls(&fx).is_empty() && fs(&st).filters.issues.is_none(), "assigned issues are my_issues");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tracker_that_cannot_apply_the_filter_is_not_asked() {
    let (mut st, dir) = open("ff-gitlab");
    let gl = Target { provider: Provider::GitLab, host: "gitlab.com".into(), owner: "g".into(), project: String::new(), repo: "a".into() };
    st.session.forge = Some(ForgeState::new(Some(gl.clone()), "origin".into(), Some(gl)));
    let fx = send(&mut st, choose(FilterKind::Issues, Builtin::IssuesMentioningMe));
    assert!(filter_calls(&fx).is_empty());
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    assert!(matches!(issue_view(fs(&st), &filters).hint, Hint::Faint(h) if h.contains("GitLab can't")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_auth_error_drops_the_saved_sign_in() {
    let (mut st, dir) = open("ff-auth");
    let key = host(&st).account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    let (choice, conditions) = (FilterChoice::Builtin(Builtin::IssuesCreatedByMe), Builtin::IssuesCreatedByMe.conditions());
    let h = host(&st);
    reply(&mut st, h, FilterResult::Issues { choice, conditions, epoch: 0, me: None, result: Err(error(true)) });
    assert!(!fs(&st).auth.contains_key(&key));
    assert_eq!(fs(&st).filters.issues.as_ref().unwrap().status, Status::Error("boom".into()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_refresh_runs_the_shown_filters_again() {
    let (mut st, dir) = open("ff-refresh");
    send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    st.session.forge.as_mut().unwrap().filters.me = MeState::Unknown;
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Forge { call: ForgeCall::Refresh { with_issues: true, .. }, .. })));
    let calls = filter_calls(&fx);
    assert!(calls.iter().any(|(_, c)| matches!(c, FilterCall::Issues { .. })), "{calls:?}");
    assert!(calls.iter().any(|(_, c)| matches!(c, FilterCall::Me { .. })), "{calls:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn untrusted_repositories_and_unconnected_servers_get_no_filter_calls() {
    let (mut st, dir) = open("ff-held");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs))).is_empty());
    assert_eq!(fs(&st).filters.me, MeState::Unknown, "not left asking");
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe))).is_empty());

    let (mut st, dir2) = open("ff-ghe");
    let ghe = Target {
        provider: Provider::GitHubEnterprise,
        host: "github.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    st.session.forge = Some(ForgeState::new(Some(ghe.clone()), "origin".into(), Some(ghe.clone())));
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs))).is_empty());
    st.settings.connections.push(Connection { account: ghe.account(), user: "ada".into(), method: "access token".into() });
    assert_eq!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::NeedsMyReview))).len(), 1, "once connected");
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

#[test]
fn dropped_replies_are_asked_for_again() {
    let (mut st, dir) = open("ff-parked");
    send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    send(&mut st, FilterMsg::NewDialog(FilterKind::Prs));
    let f = st.session.forge.as_mut().unwrap();
    f.replies_dropped();
    assert_eq!(f.filters.me, MeState::Unknown);
    assert_eq!(f.filters.pr_options, crate::state::forge_filters::OptionsState::NotAsked);
    // A reply for an inactive tab is dropped whole.
    let other = Msg::Scoped {
        workdir: dir.join("elsewhere"),
        msg: Box::new(Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply {
            target: host(&st),
            auth: None,
            results: vec![ForgeResult::Filter(FilterResult::Me { epoch: 0, result: Ok(ada()) })],
        }))),
    };
    update(&mut st, other, Instant::now());
    assert_eq!(fs(&st).filters.me, MeState::Unknown);
    for m in [FilterMsg::Save, FilterMsg::Remove(1), FilterMsg::EditDialog(1), FilterMsg::Move { id: 1, up: true }] {
        assert!(Msg::ForgeFilter(m).on_inactive().is_none());
    }
    let _ = std::fs::remove_dir_all(dir);
}
