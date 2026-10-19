//! The cross-repository dashboard: opening and closing it, refreshing (a libgit2 scan of the
//! known repositories, then one round of service calls with credentials resolved outside any
//! repository), its results, and the row actions that open a repository or a pull request in a
//! tab. A pull request whose tab hasn't listed it yet waits as `pending_pr` for that list.

use crate::effect::{AccountJob, DashboardEffect, Effect, FetchJob, FetchReply, TargetJob};
use crate::msg::DashboardMsg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::dashboard::{AccountRow, AccountState, Basis, PendingPr, Phase, pr_label};
use crate::state::dashboard_plan::{plan, repos_to_read};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::forge::{self, Account, Target};

#[cfg(test)]
#[path = "dashboard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dashboard_open_tests.rs"]
mod open_tests;

pub(super) fn handle(st: &mut AppState, msg: DashboardMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        DashboardMsg::Open => open(st, now, fx),
        DashboardMsg::Close => st.dashboard.open = false,
        DashboardMsg::Refresh => start(st, fx),
        DashboardMsg::Show(s) => st.dashboard.show = s,
        DashboardMsg::ClearFilter => st.dashboard.filter.clear(),
        DashboardMsg::SignIn(account) => sign_in(st, account, fx),
        DashboardMsg::OpenRepo(path) => super::tabs::open(st, path, fx),
        DashboardMsg::OpenPr { path, target, id } => open_pr(st, path, target, id, now, fx),
        DashboardMsg::Scanned { generation, repos } => {
            if generation != st.dashboard.generation {
                return; // a newer refresh started
            }
            for (path, r) in &repos {
                if let Ok(s) = r
                    && let Some(id) = s.folder
                {
                    st.known.learn(path, id);
                    st.known.learn(&s.real_workdir, id);
                }
            }
            let p = plan(st, &repos);
            let d = &mut st.dashboard;
            d.targets = p.targets;
            d.accounts = p.accounts;
            d.no_service = p.no_service;
            d.unreadable.extend(p.unreadable);
            d.over_limit = p.over_limit;
            if p.jobs.is_empty() {
                // Nothing to ask: the scan is the whole refresh.
                d.phase = Phase::Idle;
                d.fetched_at = Some(st.view.now);
                d.fetched = Some(now);
                return;
            }
            d.phase = Phase::Fetching;
            d.in_flight = 1;
            let job = FetchJob { generation, cwd: AppState::outside_dir(), interactive: false, partial: false, accounts: p.jobs };
            fx.push(Effect::Dashboard(DashboardEffect::Fetch(Box::new(job))));
        }
        DashboardMsg::Fetched(reply) => fetched(st, *reply, now),
    }
}

fn basis(st: &AppState) -> (Basis, Vec<PathBuf>, Vec<PathBuf>) {
    let (repos, missing) = repos_to_read(st);
    let all = repos.iter().chain(&missing).cloned().collect();
    (Basis { repos: all, connections: st.settings.connections.clone() }, repos, missing)
}

fn open(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    st.dashboard.open = true;
    st.dashboard.pending_pr = None;
    let (b, repos, missing) = basis(st);
    if repos.is_empty() && missing.is_empty() {
        // Nothing known any more: forget what an earlier refresh found.
        let d = &mut st.dashboard;
        (d.basis, d.targets, d.accounts, d.no_service, d.unreadable, d.over_limit) = (None, vec![], vec![], vec![], vec![], 0);
        return;
    }
    if !st.dashboard.is_busy() && st.dashboard.is_stale(&b, now) {
        start(st, fx);
    }
}

/// Starts a refresh: reads every known repository again, then asks the services.
fn start(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (b, repos, missing) = basis(st);
    let connections = b.connections.clone();
    let d = &mut st.dashboard;
    d.generation += 1;
    d.phase = Phase::Scanning;
    d.in_flight = 0;
    d.basis = Some(b);
    d.unreadable = missing.into_iter().map(|p| (p, "missing".to_owned())).collect();
    fx.push(Effect::Dashboard(DashboardEffect::Scan { generation: d.generation, repos, connections }));
}

/// Sign in: asks one account's targets again, letting Git Credential Manager show its window.
fn sign_in(st: &mut AppState, account: Account, fx: &mut Vec<Effect>) {
    let connections = &st.settings.connections;
    if !forge::may_contact(&account, connections) {
        return;
    }
    let connected = forge::is_connected(&account, connections);
    let d = &mut st.dashboard;
    let targets: Vec<TargetJob> = d
        .targets
        .iter()
        .filter(|t| t.target.account() == account)
        .map(|t| TargetJob { target: t.target.clone(), prs: t.wants_prs, issues: t.wants_issues })
        .collect();
    if targets.is_empty() {
        return;
    }
    d.auth.remove(&account.key());
    let me = d.account(&account).and_then(|a| a.me.clone());
    if let Some(row) = d.account_mut(&account) {
        row.state = AccountState::Pending;
    }
    d.phase = Phase::Fetching;
    d.in_flight += 1;
    let job = AccountJob { account, connected, auth: None, me, targets };
    let job = FetchJob { generation: d.generation, cwd: AppState::outside_dir(), interactive: true, partial: true, accounts: vec![job] };
    fx.push(Effect::Dashboard(DashboardEffect::Fetch(Box::new(job))));
}

fn fetched(st: &mut AppState, r: FetchReply, now: Instant) {
    let d = &mut st.dashboard;
    if r.generation != d.generation {
        return;
    }
    for a in r.accounts {
        let key = a.account.key();
        if d.account(&a.account).is_none() {
            d.accounts.push(AccountRow::new(a.account.clone(), AccountState::Pending));
        }
        let mut drop_auth = false;
        for t in a.targets {
            let Some(data) = d.targets.iter_mut().find(|x| x.target == t.target) else { continue };
            if let Some(p) = t.prs {
                drop_auth |= p.as_ref().is_err_and(|e| e.auth);
                data.prs = Some(p.map_err(|e| e.message));
            }
            if let Some(i) = t.issues {
                drop_auth |= i.as_ref().is_err_and(|e| e.auth);
                data.issues = Some(i.map_err(|e| e.message));
            }
        }
        let Some(row) = d.accounts.iter_mut().find(|x| x.account == a.account) else { continue };
        match a.auth {
            Ok(auth) => {
                row.state = AccountState::Ok;
                d.auth.insert(key.clone(), auth);
            }
            Err(e) => {
                row.state = AccountState::SignInFailed(e);
                d.auth.remove(&key);
            }
        }
        match a.me {
            Some(Ok(me)) => (row.me, row.me_error) = (Some(me), None),
            Some(Err(e)) => row.me_error = Some(e.message),
            None => {}
        }
        if drop_auth {
            d.auth.remove(&key);
        }
    }
    if !r.partial {
        d.fetched_at = Some(r.fetched_at);
        d.fetched = Some(now);
    }
    d.in_flight = d.in_flight.saturating_sub(1);
    if d.in_flight == 0 {
        d.phase = Phase::Idle;
    }
}

/// Whether the active tab shows the clone at `path` with `target` as its code host.
fn showing(st: &AppState, path: &Path, target: &Target) -> bool {
    let here = st.snap.as_ref().is_some_and(|s| st.names_repo(s, path));
    here && st.forge.as_ref().is_some_and(|f| f.host.as_ref() == Some(target))
}

fn listed(st: &AppState, id: u64) -> bool {
    st.forge.as_ref().is_some_and(|f| f.prs.iter().any(|p| p.id == id))
}

fn open_pr(st: &mut AppState, path: PathBuf, target: Target, id: u64, now: Instant, fx: &mut Vec<Effect>) {
    super::tabs::open(st, path.clone(), fx);
    if showing(st, &path, &target) && listed(st, id) {
        super::pull_request::handle(st, PullRequestMsg::Open(id), now, fx);
        return;
    }
    let loaded = showing(st, &path, &target);
    st.dashboard.pending_pr = Some(PendingPr { path, target, id });
    if loaded {
        // The tab's list is older than this pull request: ask for it now rather than at the
        // next refresh.
        super::forge::refresh(st, true, false, now, fx);
    }
}

/// A pull request list arrived for the active tab: shows the pull request the dashboard asked
/// for, when it is that tab's. Any other repository's list drops the request.
pub(super) fn prs_arrived(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(p) = st.dashboard.pending_pr.take() else { return };
    if !showing(st, &p.path, &p.target) {
        return;
    }
    if listed(st, p.id) {
        super::pull_request::handle(st, PullRequestMsg::Open(p.id), now, fx);
    } else {
        st.log(false, format!("{} is no longer open on {}.", pr_label(&p.target, p.id), p.target.provider.name()));
    }
}

/// An account was connected or disconnected: its cached credential and signed-in user go (the
/// new connection may be someone else), and the results are stale (asked again at once while
/// the dashboard is open).
pub(super) fn account_changed(st: &mut AppState, account: &Account, fx: &mut Vec<Effect>) {
    let d = &mut st.dashboard;
    d.auth.remove(&account.key());
    if let Some(row) = d.account_mut(account) {
        (row.me, row.me_error) = (None, None);
    }
    stale(st, fx);
}

/// Another profile's connections were loaded: every cached credential and signed-in user goes.
pub(super) fn connections_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let d = &mut st.dashboard;
    d.auth.clear();
    for row in &mut d.accounts {
        (row.me, row.me_error) = (None, None);
    }
    stale(st, fx);
}

fn stale(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.dashboard.fetched = None;
    if st.dashboard.open {
        start(st, fx);
    }
}

/// Selecting, cycling to, opening or reopening a tab, or opening a repository any way, shows
/// that tab instead of the dashboard.
pub(super) fn leave(st: &mut AppState) {
    st.dashboard.open = false;
}
