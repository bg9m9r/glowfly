//! The cross-repository dashboard's work (`Effect::Dashboard`): reading the known repositories'
//! remotes with libgit2 (nothing in them runs), then signing in to each account outside every
//! repository and asking its targets, a few calls at a time.

use super::execute::{Cx, Execute};
use super::sign_ins::{self, Ask, SignInFlights, auth_text};
use glowfly_core::error::ReadFailed;
use glowfly_core::forge::dashboard::{CONCURRENCY, fetch_target, read_repo, run_bounded};
use glowfly_core::forge::{Auth, Connection, filters};
use glowfly_model::effect::{AccountJob, AccountReply, DashboardEffect, FetchJob, FetchReply, ForgeError, TargetJob, TargetReply};
use glowfly_model::msg::{DashboardMsg, Msg};
use std::path::{Path, PathBuf};

impl Execute for DashboardEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            DashboardEffect::Scan { generation, repos, connections } => scan(generation, repos, &connections),
            DashboardEffect::Fetch(job) => Msg::Dashboard(DashboardMsg::Fetched(Box::new(fetch(*job, cx)))),
        })
    }
}

fn scan(generation: u64, repos: Vec<PathBuf>, connections: &[Connection]) -> Msg {
    let repos = run_bounded(repos, CONCURRENCY, |p| {
        let r = read_repo(&p, connections).map_err(ReadFailed::from);
        (p, r)
    });
    Msg::Dashboard(DashboardMsg::Scanned { generation, repos })
}

/// Signs in to one account and, when its pull requests are wanted and the user isn't known yet,
/// looks the user up. Returns the account's reply (its targets still to fill), its targets and
/// the credential it found, for the model's cache.
fn sign_in(job: AccountJob, cwd: &Path, interactive: bool, flights: &SignInFlights) -> (AccountReply, Vec<TargetJob>, Option<Msg>) {
    let ask = Ask { account: job.account.clone(), cwd, interactive, connected: job.connected, env_only: false, asked: interactive };
    let (auth, found) = match job.auth {
        Some(a) => (Ok(a), None),
        None => match flights.lookup(&ask) {
            Ok(a) => {
                let found = sign_ins::found(&ask, &a);
                (Ok(a), Some(found))
            }
            Err(e) => (Err(auth_text(e)), None),
        },
    };
    let me = match (&auth, &job.me, job.targets.iter().find(|t| t.prs)) {
        (Ok(a), None, Some(t)) => Some(filters::me(&t.target, a).map_err(ForgeError::from)),
        _ => None,
    };
    (AccountReply { account: job.account, auth, me, targets: Vec::new() }, job.targets, found)
}

fn fetch(job: FetchJob, cx: &Cx<'_>) -> FetchReply {
    let FetchJob { generation, cwd, interactive, partial, accounts } = job;
    let signed = run_bounded(accounts, CONCURRENCY, |a| sign_in(a, &cwd, interactive, cx.sign_ins));
    // The credentials found go to the model before the calls that use them.
    let signed: Vec<(AccountReply, Vec<TargetJob>)> = signed
        .into_iter()
        .map(|(reply, targets, found)| {
            found.into_iter().for_each(|m| (cx.send)(m));
            (reply, targets)
        })
        .collect();
    // Every target of the accounts that signed in, as one list so the limit spans accounts.
    let calls: Vec<(usize, TargetJob, Auth)> = signed
        .iter()
        .enumerate()
        .filter_map(|(i, (reply, targets))| reply.auth.as_ref().ok().map(|auth| (i, targets, auth)))
        .flat_map(|(i, targets, auth)| targets.iter().map(move |t| (i, t.clone(), auth.clone())))
        .collect();
    let answers = run_bounded(calls, CONCURRENCY, |(i, t, auth)| {
        let f = fetch_target(&t.target, &auth, t.prs, t.issues);
        let reply = TargetReply {
            target: t.target,
            prs: f.prs.map(|r| r.map_err(ForgeError::from)),
            issues: f.issues.map(|r| r.map_err(ForgeError::from)),
        };
        (i, reply)
    });
    let mut replies: Vec<AccountReply> = signed.into_iter().map(|(reply, _)| reply).collect();
    for (i, t) in answers {
        replies[i].targets.push(t);
    }
    FetchReply { generation, partial, fetched_at: chrono::Utc::now().timestamp(), accounts: replies }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_folder_scans_as_not_a_repository() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-dash-plain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let msg = scan(2, vec![dir.clone()], &[]);
        let _ = std::fs::remove_dir_all(&dir);
        let Msg::Dashboard(DashboardMsg::Scanned { generation: 2, repos }) = msg else { panic!("a scan result") };
        assert!(
            matches!(&repos[..], [(p, Err(ReadFailed::NotARepository { .. }))] if *p == dir),
            "{:?}",
            repos.iter().map(|(_, r)| r.as_ref().err()).collect::<Vec<_>>()
        );
    }
}
