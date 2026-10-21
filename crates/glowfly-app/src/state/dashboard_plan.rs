//! What a dashboard refresh reads and asks (pure): the repositories in order (open tabs,
//! favorites, recent), and from their scanned remotes the targets, grouped by account, with
//! the calls for the accounts Trellis may contact.

use super::AppState;
use super::dashboard::{AccountRow, AccountState, LocalRepo, MAX_REPOS, MAX_TARGETS, TargetData};
use super::forge::tracker_target;
use super::repo_lists;
use crate::effect::{AccountJob, TargetJob};
use glowfly_core::forge::dashboard::ScannedRepo;
use glowfly_core::forge::{self, Target};
use glowfly_core::paths::same_spelling;
use std::path::PathBuf;

/// The folders to read, in order (open tabs, favorites, recent), each folder once and at most
/// `MAX_REPOS`, and the listed folders last found missing (not read).
pub fn repos_to_read(st: &AppState) -> (Vec<PathBuf>, Vec<PathBuf>) {
    // Every spelling taken so far: a tab's path and where it really is.
    let mut seen: Vec<PathBuf> = Vec::new();
    let (mut read, mut missing) = (Vec::new(), Vec::new());
    let tabs = st.tabs.iter().filter_map(|t| t.path.clone().map(|p| (p, t.real.clone())));
    let lists = repo_lists::favorites(&st.settings).into_iter().chain(st.recent.iter().cloned()).map(|p| (p, None));
    for (path, real) in tabs.chain(lists) {
        if seen.iter().any(|s| st.same_folder(s, &path)) {
            continue;
        }
        seen.push(path.clone());
        seen.extend(real);
        if repo_lists::is_missing(st, &path) {
            missing.push(path);
        } else if read.len() < MAX_REPOS {
            read.push(path);
        }
    }
    (read, missing)
}

/// What a scan's results plan: the rows' targets and accounts, the calls to make, and what
/// can't be asked.
#[derive(Default)]
pub struct Plan {
    pub targets: Vec<TargetData>,
    pub accounts: Vec<AccountRow>,
    pub no_service: Vec<PathBuf>,
    pub unreadable: Vec<(PathBuf, String)>,
    /// Targets past `MAX_TARGETS`.
    pub over_limit: usize,
    /// One per account that may be contacted.
    pub jobs: Vec<AccountJob>,
}

fn entry<'a>(targets: &'a mut Vec<TargetData>, t: &Target) -> &'a mut TargetData {
    let i = match targets.iter().position(|d| d.target == *t) {
        Some(i) => i,
        None => {
            targets.push(TargetData::new(t.clone()));
            targets.len() - 1
        }
    };
    &mut targets[i]
}

fn add_local(data: &mut TargetData, local: LocalRepo) {
    match data.locals.iter_mut().find(|l| same_spelling(&l.path, &local.path)) {
        // The repository's tracker is also one of its remotes: keep the remote's row.
        Some(l) => l.is_host |= local.is_host,
        None => data.locals.push(local),
    }
}

/// Plans a refresh from `scans` (each folder asked, with what reading it found).
pub fn plan(st: &AppState, scans: &[(PathBuf, Result<ScannedRepo, String>)]) -> Plan {
    let mut p = Plan::default();
    let mut reals: Vec<&std::path::Path> = Vec::new();
    for (path, result) in scans {
        let s = match result {
            Ok(s) => s,
            Err(e) => {
                p.unreadable.push((path.clone(), e.clone()));
                continue;
            }
        };
        // Two spellings of one folder (a link, a short name) read as one repository.
        if reals.iter().any(|r| same_spelling(r, &s.real_workdir)) {
            continue;
        }
        reals.push(&s.real_workdir);
        let host = s.host.map(|i| &s.remotes[i].target);
        let tracker = tracker_target(st.settings.tracker_for(&s.real_workdir), host);
        if s.remotes.is_empty() && tracker.is_none() {
            p.no_service.push(path.clone());
            continue;
        }
        let local = |remote: &str, url: &str, is_host: bool| LocalRepo {
            path: path.clone(),
            real: s.real_workdir.clone(),
            name: s.name.clone(),
            remote: remote.to_owned(),
            remote_url: url.to_owned(),
            is_host,
        };
        for (i, r) in s.remotes.iter().enumerate() {
            let data = entry(&mut p.targets, &r.target);
            data.wants_prs |= !r.target.provider.is_tracker_only();
            add_local(data, local(&r.remote, &r.url, s.host == Some(i)));
        }
        if let Some(t) = tracker {
            let data = entry(&mut p.targets, &t);
            data.wants_issues = true;
            add_local(data, local("", "", false));
        }
    }
    p.over_limit = p.targets.len().saturating_sub(MAX_TARGETS);
    p.targets.truncate(MAX_TARGETS);
    let d = &st.dashboard;
    let connections = &st.settings.connections;
    for data in &mut p.targets {
        // Earlier answers stay on screen until the new ones arrive.
        if let Some(old) = d.targets.iter().find(|o| o.target == data.target) {
            data.prs = old.prs.clone().filter(|_| data.wants_prs);
            data.issues = old.issues.clone().filter(|_| data.wants_issues);
        }
        let account = data.target.account();
        let job = TargetJob { target: data.target.clone(), prs: data.wants_prs, issues: data.wants_issues };
        if let Some(j) = p.jobs.iter_mut().find(|j| j.account == account) {
            j.targets.push(job);
            continue;
        }
        if p.accounts.iter().any(|a| a.account == account) {
            continue; // not contacted
        }
        let me = d.account(&account).and_then(|a| a.me.clone());
        if !forge::may_contact(&account, connections) {
            p.accounts.push(AccountRow::new(account, AccountState::NotConnected));
            continue;
        }
        p.accounts.push(AccountRow { me: me.clone(), ..AccountRow::new(account.clone(), AccountState::Pending) });
        p.jobs.push(AccountJob {
            connected: forge::is_connected(&account, connections),
            auth: d.auth.get(&account.key()).cloned(),
            me,
            targets: vec![job],
            account,
        });
    }
    p
}
