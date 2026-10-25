//! The cross-repository dashboard's work (`Effect::Dashboard`): reading the known repositories'
//! remotes with libgit2 (nothing in them runs), then signing in to each account outside every
//! repository and asking its targets, a few calls at a time.

use super::execute::{Cx, Execute};
use crate::effect::{AccountJob, AccountReply, DashboardEffect, FetchJob, FetchReply, ForgeError, TargetJob, TargetReply};
use crate::msg::{DashboardMsg, Msg};
use glowfly_core::error::Error;
use glowfly_core::forge::dashboard::{CONCURRENCY, fetch_target, read_repo, run_bounded, unreadable_reason};
use glowfly_core::forge::{self, Auth, filters};
use std::path::Path;

pub(super) fn run(e: DashboardEffect) -> Msg {
    match e {
        DashboardEffect::Scan { generation, repos, connections } => {
            let repos = run_bounded(repos, CONCURRENCY, |p| {
                let r = read_repo(&p, &connections).map_err(|e| unreadable_reason(&e));
                (p, r)
            });
            Msg::Dashboard(DashboardMsg::Scanned { generation, repos })
        }
        DashboardEffect::Fetch(job) => Msg::Dashboard(DashboardMsg::Fetched(Box::new(fetch(*job)))),
    }
}

impl Execute for DashboardEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}

/// A sign-in failure as the user reads it: "not signed in to GitHub", not "authentication failed: …".
fn auth_text(e: Error) -> String {
    match e {
        Error::Auth(m) => m,
        e => e.to_string(),
    }
}

/// Signs in to one account and, when its pull requests are wanted and the user isn't known yet,
/// looks the user up. Returns the account's reply (its targets still to fill) and its targets.
fn sign_in(job: AccountJob, cwd: &Path, interactive: bool) -> (AccountReply, Vec<TargetJob>) {
    let auth = match job.auth {
        Some(a) => Ok(a),
        None => forge::resolve_auth(&job.account, cwd, interactive, job.connected).map_err(auth_text),
    };
    let me = match (&auth, &job.me, job.targets.iter().find(|t| t.prs)) {
        (Ok(a), None, Some(t)) => Some(filters::me(&t.target, a).map_err(ForgeError::from)),
        _ => None,
    };
    (AccountReply { account: job.account, auth, me, targets: Vec::new() }, job.targets)
}

fn fetch(job: FetchJob) -> FetchReply {
    let FetchJob { generation, cwd, interactive, partial, accounts } = job;
    let signed = run_bounded(accounts, CONCURRENCY, |a| sign_in(a, &cwd, interactive));
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
