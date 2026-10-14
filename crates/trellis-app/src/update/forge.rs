//! Hosting services: detection, refresh, replies, pull request creation, connections.

use crate::effect::{Effect, ForgeCall, ForgeReply, ForgeResult};
use crate::msg::Msg;
use crate::state::{AppState, FORGE_REFRESH_INTERVAL, ForgeState, Modal, Status};
use git2::Oid;
use std::time::Instant;
use trellis_core::forge::{self, Account, Connection, Target};
use trellis_core::repo::Snapshot;
use trellis_core::settings::TrackerChoice;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::ForgeRefresh { interactive } => {
            if let Some(fs) = st.forge.as_mut() {
                fs.host_status = Status::Idle;
                fs.tracker_status = Status::Idle;
            }
            refresh(st, true, interactive, now, fx);
        }
        Msg::ForgeReplied(reply) => replied(st, reply, now),
        Msg::CreatePrDialog(source) => open_create_pr(st, source, fx),
        Msg::CreatePr(pr) => {
            if let Some(target) = st.forge.as_ref().and_then(|f| f.host.clone()) {
                request(st, target, false, ForgeCall::Create(pr), fx);
            }
        }
        Msg::Connect { account, user, token } => {
            if let Some(p) = st.prefs.as_mut() {
                p.int_busy = true;
                p.int_error = None;
            }
            fx.push(Effect::Connect { account, user, token, cwd: st.any_dir() });
        }
        Msg::Connected { account, method, token, result } => connected(st, account, method, token, result, now, fx),
        Msg::Disconnect(account) => disconnect(st, &account, fx),
        Msg::ForgetTokens { gcm } => {
            let accounts: Vec<Account> = st.settings.connections.iter().map(|c| c.account.clone()).collect();
            for a in &accounts {
                disconnect(st, a, fx);
            }
            if gcm {
                let gcm_accounts = accounts.iter().filter(|a| a.provider.supports_gcm()).cloned().collect();
                fx.push(Effect::ForgetCredentialManager { accounts: gcm_accounts, cwd: st.any_dir() });
            }
            let extra = if gcm { " and signed Git Credential Manager out of those hosts" } else { "" };
            st.log(true, format!("Removed {} connection{}{extra}.", accounts.len(), if accounts.len() == 1 { "" } else { "s" }));
        }
        Msg::SetTracker(choice) => {
            let Some(w) = st.workdir() else { return };
            st.settings.repo_trackers.insert(w.display().to_string(), choice);
            fx.push(super::prefs::save_settings(st));
            if let Some(snap) = st.snap.take() {
                detect(st, &snap);
                st.snap = Some(snap);
            }
            refresh(st, true, false, now, fx);
        }
        _ => {}
    }
}

/// Works out the code host (from remotes, preferring origin) and the issue tracker.
pub(super) fn detect(st: &mut AppState, snap: &Snapshot) {
    let host = snap
        .remote_urls
        .iter()
        .filter_map(|(name, url)| forge::parse_remote(url, &st.settings.connections).map(|t| (name.clone(), t)))
        .min_by_key(|(name, _)| name != "origin");
    let (remote, host) = match host {
        Some((r, t)) => (r, Some(t)),
        None => (String::new(), None),
    };
    let tracker = match st.settings.tracker_for(&snap.workdir) {
        TrackerChoice::Auto => host.clone().filter(|t| t.provider.has_issues()),
        TrackerChoice::None => None,
        TrackerChoice::Jira { account, project } => Some(Target::jira(&account, &project)),
    };
    if host.is_none() && tracker.is_none() {
        st.forge = None;
        return;
    }
    st.forge = match st.forge.take() {
        // The same service: keep its loaded state, following a renamed remote.
        Some(mut existing) if existing.host == host && existing.tracker == tracker => {
            existing.remote = remote;
            Some(existing)
        }
        _ => Some(ForgeState::new(host, remote, tracker)),
    };
}

fn request(st: &mut AppState, target: Target, interactive: bool, call: ForgeCall, fx: &mut Vec<Effect>) {
    let cwd = st.any_dir();
    let Some(fs) = st.forge.as_mut() else { return };
    fs.loading += 1;
    let auth = if interactive { None } else { fs.auth.get(&target.account().key()).cloned() };
    fx.push(Effect::Forge { target, cwd, auth, interactive, call });
}

/// Refreshes pull requests and issues; `force` ignores the interval and earlier failures.
pub(super) fn refresh(st: &mut AppState, force: bool, interactive: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    if !force && fs.last_refresh.is_some_and(|t| now.duration_since(t) < FORGE_REFRESH_INTERVAL) {
        return;
    }
    // Don't keep retrying in the background after a failed sign-in.
    let failed = matches!(fs.host_status, Status::Error(_)) || matches!(fs.tracker_status, Status::Error(_));
    if !interactive && !force && failed {
        return;
    }
    fs.last_refresh = Some(now);
    let need_default = fs.default_branch.is_none();
    let (host, tracker) = (fs.host.clone(), fs.tracker.clone());
    let same = host.is_some() && host == tracker;
    if let Some(h) = host {
        request(st, h, interactive, ForgeCall::Refresh { need_default, with_issues: same }, fx);
    }
    if let (Some(t), false) = (tracker, same) {
        request(st, t, interactive, ForgeCall::MyIssues, fx);
    }
}

/// Fetches issues that are mentioned but not cached yet.
pub(super) fn want_issues(st: &mut AppState, keys: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    let Some(tracker) = fs.tracker.clone() else { return };
    if matches!(fs.tracker_status, Status::Error(_)) {
        return;
    }
    let missing: Vec<String> = keys.into_iter().filter(|k| !fs.cache.contains_key(k) && !fs.pending.contains(k)).collect();
    if missing.is_empty() {
        return;
    }
    fs.pending.extend(missing.iter().cloned());
    request(st, tracker, false, ForgeCall::Issues(missing), fx);
}

/// Maps each pull request's head SHA (or SHA prefix) to a commit in the graph.
pub(super) fn resolve_pr_heads(st: &mut AppState) {
    let (Some(fs), Some(snap)) = (st.forge.as_mut(), st.snap.as_ref()) else { return };
    fs.pr_heads.clear();
    for pr in &fs.prs {
        let Some(head) = pr.head.as_deref() else { continue };
        let oid = Oid::from_str(head)
            .ok()
            .filter(|o| head.len() == 40 && snap.row_of.contains_key(o))
            .or_else(|| snap.commits.iter().find(|c| pr.head_matches(&c.oid.to_string())).map(|c| c.oid));
        if let Some(o) = oid {
            fs.pr_heads.insert(pr.id, o);
        }
    }
}

fn replied(st: &mut AppState, reply: ForgeReply, now: Instant) {
    let mut logs: Vec<(bool, String)> = Vec::new();
    let mut prs_changed = false;
    let Some(fs) = st.forge.as_mut() else { return };
    let is_host = fs.host.as_ref() == Some(&reply.target);
    let is_tracker = fs.tracker.as_ref() == Some(&reply.target);
    if !is_host && !is_tracker {
        return; // the repository changed since the request
    }
    fs.loading = fs.loading.saturating_sub(1);
    let key = reply.target.account().key();
    if let Some(a) = reply.auth {
        fs.auth.insert(key.clone(), a);
    }
    for result in reply.results {
        match result {
            ForgeResult::AuthFailed(e) => {
                fs.auth.remove(&key);
                if is_host {
                    fs.host_status = Status::Error(e.clone());
                }
                if is_tracker {
                    fs.tracker_status = Status::Error(e);
                }
            }
            ForgeResult::Prs(Ok(prs)) => {
                fs.prs = prs;
                fs.host_status = Status::Ok;
                prs_changed = true;
            }
            ForgeResult::Prs(Err(e)) => {
                if e.auth {
                    fs.auth.remove(&key);
                }
                fs.host_status = Status::Error(e.message);
            }
            ForgeResult::MyIssues(Ok(items)) => {
                for i in &items {
                    fs.cache.insert(i.key.clone(), i.clone());
                }
                fs.my_issues = items;
                fs.tracker_status = Status::Ok;
            }
            ForgeResult::MyIssues(Err(e)) => {
                if e.auth {
                    fs.auth.remove(&key);
                }
                fs.tracker_status = Status::Error(e.message);
            }
            ForgeResult::Issues(keys, result) => {
                if let Ok(items) = result {
                    for i in items {
                        fs.cache.insert(i.key.clone(), i);
                    }
                    // Keys that came back missing stay "pending" so they aren't refetched.
                    for k in keys.iter().filter(|k| fs.cache.contains_key(*k)) {
                        fs.pending.remove(k);
                    }
                }
            }
            ForgeResult::DefaultBranch(b) => {
                if b.is_some() {
                    fs.default_branch = b;
                }
            }
            ForgeResult::Created(Ok(pr)) => {
                logs.push((true, format!("Created {} {} {}\n{}", fs.pr_noun(), fs.pr_label(pr.id), pr.title, pr.url)));
                fs.prs.insert(0, pr);
                prs_changed = true;
            }
            ForgeResult::Created(Err(e)) => logs.push((false, format!("Creating the {} failed: {}", fs.pr_noun(), e.message))),
        }
    }
    for (ok, text) in logs {
        st.toast(ok, text.lines().next().unwrap_or_default().to_owned(), now);
        st.log(ok, text);
    }
    if prs_changed {
        resolve_pr_heads(st);
    }
}

fn open_create_pr(st: &mut AppState, source: String, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.host.is_none() {
        return;
    }
    let remote_branches: Vec<String> =
        snap.remotes.iter().find(|(r, _)| *r == fs.remote).map(|(_, b)| b.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default();
    let target = fs
        .default_branch
        .clone()
        .filter(|b| *b != source)
        .or_else(|| {
            ["main", "master", "develop"].iter().find(|b| remote_branches.iter().any(|r| r == *b) && **b != source).map(|b| b.to_string())
        })
        .unwrap_or_else(|| "main".into());
    let local_tip = snap.locals.iter().find(|b| b.name == source).and_then(|b| b.oid);
    let title = local_tip.and_then(|o| snap.row_of.get(&o)).map(|&r| snap.commits[r].summary.clone()).unwrap_or_else(|| source.clone());
    // Issues mentioned on the branch: from its name and from commits not yet on the target.
    let mut keys: Vec<String> = fs.branch_issue(&source).into_iter().collect();
    let tip = |name: &str, remote_first: bool| {
        let remote =
            snap.remotes.iter().find(|(r, _)| *r == fs.remote).and_then(|(_, b)| b.iter().find(|(n, _)| n == name)).map(|(_, o)| *o);
        let local = snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid);
        if remote_first { remote.or(local) } else { local.or(remote) }
    };
    if let Some(from) = tip(&source, false) {
        for c in snap.commits_between(from, tip(&target, true)) {
            for k in fs.issue_refs(&c.summary).into_iter().chain(fs.issue_refs(&c.body)) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
    }
    let items = keys.join(", ");
    let candidates = if remote_branches.is_empty() { snap.locals.iter().map(|b| b.name.clone()).collect() } else { remote_branches };
    let mut targets: Vec<String> = candidates.into_iter().filter(|b| *b != source).collect();
    targets.sort();
    st.modal = Some(Modal::CreatePr { source, target, targets, title, description: String::new(), draft: false, items });
    want_issues(st, keys, fx);
}

fn connected(
    st: &mut AppState,
    account: Account,
    method: String,
    token: Option<(Option<String>, String)>,
    result: Result<String, String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if let Some(p) = st.prefs.as_mut() {
        p.int_busy = false;
    }
    match result {
        Ok(user) => {
            if let Some((u, t)) = token {
                fx.push(Effect::SaveToken { account: account.clone(), user: u, token: t });
            }
            st.settings.connections.retain(|c| c.account != account);
            st.settings.connections.push(Connection { account: account.clone(), user: user.clone(), method });
            fx.push(super::prefs::save_settings(st));
            if let Some(p) = st.prefs.as_mut() {
                p.int_token.clear();
            }
            st.log(true, format!("Connected to {} ({}) as {user}.", account.provider.name(), account.display()));
            // A new connection can make a self-hosted remote recognizable.
            if let Some(fs) = st.forge.as_mut() {
                fs.auth.remove(&account.key());
                fs.host_status = Status::Idle;
                fs.tracker_status = Status::Idle;
            }
            if let Some(snap) = st.snap.take() {
                detect(st, &snap);
                st.snap = Some(snap);
            }
            refresh(st, true, false, now, fx);
        }
        Err(e) => {
            if let Some(p) = st.prefs.as_mut() {
                p.int_error = Some(e);
            }
        }
    }
}

fn disconnect(st: &mut AppState, account: &Account, fx: &mut Vec<Effect>) {
    fx.push(Effect::DeleteToken(account.clone()));
    st.settings.connections.retain(|c| &c.account != account);
    fx.push(super::prefs::save_settings(st));
    if let Some(fs) = st.forge.as_mut() {
        fs.auth.remove(&account.key());
        let msg = format!("not connected to {}", account.provider.name());
        if fs.host.as_ref().is_some_and(|t| t.account() == *account) {
            fs.prs.clear();
            fs.pr_heads.clear();
            fs.host_status = Status::Error(msg.clone());
        }
        if fs.tracker.as_ref().is_some_and(|t| t.account() == *account) {
            fs.my_issues.clear();
            fs.tracker_status = Status::Error(msg);
        }
    }
}
