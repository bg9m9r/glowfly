//! CI status of branch tips: which tips are asked about, how often, and what the answers do.

use super::tests::{drive, git, scratch_repo};
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, RepoMsg, TabsMsg};
use crate::state::{AppState, Status};
use git2::Oid;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::forge::{CheckState, Checks, PullRequest, TipQuery};
use trellis_core::gitcli::{args, run};
use trellis_core::settings::RepoTrust;

/// A repository whose `main` tracks `origin/main` on GitHub, plus a local-only `solo` branch on
/// another commit, opened with the host reachable. Returns the state, folder, main's and solo's tips.
fn open(name: &str) -> (AppState, PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    let (ok, out) = run(&dir, &args(&["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "solo"]));
    assert!(ok, "{out}");
    let solo = Oid::from_str(out.trim()).unwrap();
    git(&dir, &["branch", "solo", &solo.to_string()]);
    let mut st = crate::update::tests::state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let main = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.forge.as_mut().expect("the service is detected").host_status = Status::Ok;
    (st, dir, main, solo)
}

/// The tips asked about by the effects of `msg`.
fn asked(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Vec<TipQuery>> {
    update(st, msg, now)
        .into_iter()
        .filter_map(|e| match e {
            Effect::Forge { call: ForgeCall::TipChecks(q), .. } => Some(q),
            _ => None,
        })
        .collect()
}

fn tick(st: &mut AppState, now: Instant) -> Vec<Vec<TipQuery>> {
    asked(st, Msg::Tick(now), now)
}

fn reply(st: &mut AppState, tips: Vec<TipQuery>, result: Result<HashMap<String, Checks>, ForgeError>, now: Instant) {
    let target = st.forge.as_ref().unwrap().host.clone().unwrap();
    let reply = ForgeReply { target, auth: None, results: vec![ForgeResult::TipChecks(tips, result)] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), now);
}

fn checks(state: CheckState) -> Checks {
    Checks { state, ..Checks::from_counts(1, 0, 0, true) }
}

fn answer(oid: Oid, state: CheckState) -> Result<HashMap<String, Checks>, ForgeError> {
    Ok(HashMap::from([(oid.to_string(), checks(state))]))
}

fn tips_state(st: &AppState, oid: Oid) -> Option<CheckState> {
    st.forge.as_ref().unwrap().tips.checks(oid).map(|c| c.state)
}

#[test]
fn tips_are_asked_for_visible_upstream_tips() {
    let (mut st, dir, main, solo) = open("tips-asked");
    let t0 = Instant::now();
    update(&mut st, Msg::Forge(ForgeMsg::TipsShown(vec![main, solo])), t0);
    let want = vec![TipQuery { sha: main.to_string(), branch: "main".into() }];
    assert_eq!(tick(&mut st, t0), std::slice::from_ref(&want), "only the tip on the host's remote");
    assert!(tick(&mut st, t0).is_empty(), "one request in flight");

    reply(&mut st, want.clone(), answer(main, CheckState::Pending), t0);
    assert_eq!(tips_state(&st, main), Some(CheckState::Pending));
    assert!(st.forge.as_ref().unwrap().host_status == Status::Ok);
    assert!(tick(&mut st, t0 + Duration::from_secs(20)).is_empty(), "pending is fresh for a minute");
    let t1 = t0 + Duration::from_secs(61);
    assert_eq!(tick(&mut st, t1), std::slice::from_ref(&want), "pending is read again");

    reply(&mut st, want.clone(), answer(main, CheckState::Passed), t1);
    assert!(tick(&mut st, t1 + Duration::from_secs(120)).is_empty(), "finished results stay fresh");
    assert_eq!(tick(&mut st, t1 + Duration::from_secs(301)), [want]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn errors_back_off_until_a_manual_refresh() {
    let (mut st, dir, main, _) = open("tips-backoff");
    let t0 = Instant::now();
    update(&mut st, Msg::Forge(ForgeMsg::TipsShown(vec![main])), t0);
    let want = tick(&mut st, t0).pop().unwrap();
    reply(&mut st, want.clone(), Err(ForgeError { message: "boom".into(), auth: false }), t0);
    assert_eq!(st.forge.as_ref().unwrap().host_status, Status::Ok, "tip errors leave the host's status alone");
    assert!(tick(&mut st, t0 + Duration::from_secs(299)).is_empty());
    assert_eq!(tick(&mut st, t0 + Duration::from_secs(300)), std::slice::from_ref(&want));
    reply(&mut st, want.clone(), Err(ForgeError { message: "boom".into(), auth: false }), t0 + Duration::from_secs(300));

    let t1 = t0 + Duration::from_secs(320);
    update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }), t1);
    st.forge.as_mut().unwrap().host_status = Status::Ok;
    assert_eq!(tick(&mut st, t1), [want]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nothing_is_asked_while_the_host_failed_or_the_repository_is_untrusted() {
    let (mut st, dir, main, _) = open("tips-held");
    let t0 = Instant::now();
    update(&mut st, Msg::Forge(ForgeMsg::TipsShown(vec![main])), t0);
    st.forge.as_mut().unwrap().host_status = Status::Error("not signed in".into());
    assert!(tick(&mut st, t0).is_empty());

    st.forge.as_mut().unwrap().host_status = Status::Ok;
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(!st.runs_git_unasked());
    assert!(tick(&mut st, t0).is_empty());
    assert!(st.forge.as_ref().unwrap().tips.in_flight.is_empty(), "nothing is left waiting");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tips_are_seeded_from_pull_requests() {
    let (mut st, dir, main, _) = open("tips-seeded");
    let pr = PullRequest {
        id: 3,
        title: "Speed up".into(),
        description: String::new(),
        source: "main".into(),
        target: "release".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: Some(main.to_string()),
        merge_status: None,
        checks: Some(Checks { url: "https://ci.example.com/3".into(), ..checks(CheckState::Failed) }),
        url: String::new(),
    };
    let target = st.forge.as_ref().unwrap().host.clone().unwrap();
    let reply = ForgeReply { target, auth: None, results: vec![ForgeResult::Prs(Ok(vec![pr.clone()]))] };
    let t0 = Instant::now();
    update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), t0);
    assert_eq!(st.forge.as_ref().unwrap().tips.checks(main), pr.checks.as_ref());
    // Shown, it needs no call of its own.
    update(&mut st, Msg::Forge(ForgeMsg::TipsShown(vec![main])), t0);
    assert!(tick(&mut st, t0).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_restored_tab_asks_again_and_disconnecting_clears_the_cache() {
    let (mut st, dir, main, _) = open("tips-restore");
    let t0 = Instant::now();
    update(&mut st, Msg::Forge(ForgeMsg::TipsShown(vec![main])), t0);
    assert_eq!(tick(&mut st, t0).len(), 1);
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let fs = st.forge.as_mut().unwrap();
    assert!(fs.tips.in_flight.is_empty(), "the reply in flight was dropped with the tab");
    assert_eq!(fs.loading, 0);
    fs.host_status = Status::Ok;
    fs.tips.visible = vec![main];
    let want = tick(&mut st, t0 + Duration::from_secs(16)).pop().expect("asked again");
    reply(&mut st, want, answer(main, CheckState::Passed), t0 + Duration::from_secs(16));
    assert_eq!(tips_state(&st, main), Some(CheckState::Passed));

    let account = st.forge.as_ref().unwrap().host.as_ref().unwrap().account();
    update(&mut st, Msg::Forge(ForgeMsg::Disconnect(account)), t0);
    let fs = st.forge.as_ref().unwrap();
    assert!(fs.tips.by_oid.is_empty() && fs.tips.visible.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
