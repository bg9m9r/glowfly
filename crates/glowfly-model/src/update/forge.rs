//! Hosting services: detection, refresh, replies, connections. Starting a pull request is in
//! update/push_pr.rs.

use super::pr_details;
use crate::effect::{AccountEffect, Effect, ForgeCall, ForgeReply, ForgeResult};
use crate::msg::ForgeMsg;
use crate::state::error_words::{hosting, hosting_log};
use crate::state::{AppState, FORGE_REFRESH_INTERVAL, ForgeState, ProjectKeys, Status, pr_repos, tracker_target};
use git2::Oid;
use glowfly_core::error::advice::Ask;
use glowfly_core::forge::{self, Account, Connection, Target};
use glowfly_core::repo::Snapshot;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: ForgeMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        ForgeMsg::ForgeRefresh { interactive } => {
            if let Some(fs) = st.session.forge.as_mut() {
                fs.host_status = Status::Idle;
                fs.tracker_status = Status::Idle;
                if interactive {
                    fs.tips.expire();
                    fs.details.expire();
                }
            }
            refresh(st, true, interactive, now, fx);
        }
        ForgeMsg::ForgeReplied(reply) => replied(st, reply, now, fx),
        ForgeMsg::TipsShown(oids) => super::build_status::shown(st, oids),
        ForgeMsg::CreatePrDialog { source, target } => super::push_pr::open_dialog(st, source, target, None, fx),
        ForgeMsg::CreatePrFrom { remote, source } => super::push_pr::open_dialog(st, source, None, Some(remote), fx),
        ForgeMsg::PickPrRepo { side, remote } => super::pr_repo_pick::pick(st, side, &remote),
        ForgeMsg::PushAndStartPr { branch, target } => super::push_pr::start(st, &branch, target, None, fx),
        ForgeMsg::PushToForPr { branch, remote, target } => super::push_pr::start(st, &branch, target, Some(remote), fx),
        ForgeMsg::PushToMyFork { branch, target } => super::fork_pr::open(st, branch, target, fx),
        ForgeMsg::CreatePr { pr, from, into } => super::push_pr::create(st, pr, &from, &into, fx),
        ForgeMsg::Connect { account, user, token } => {
            if let Some(p) = st.prefs.as_mut() {
                p.int_busy = true;
                p.int_error = None;
            }
            fx.push(Effect::Account(AccountEffect::Connect { account, user, token, cwd: st.any_dir() }));
        }
        ForgeMsg::Connected { account, method, token, result } => connected(st, account, method, token, result, now, fx),
        ForgeMsg::Disconnect(account) => disconnect(st, &account, fx),
        ForgeMsg::ForgetTokens { gcm } => {
            let accounts: Vec<Account> = st.settings.connections.iter().map(|c| c.account.clone()).collect();
            for a in &accounts {
                disconnect(st, a, fx);
            }
            if gcm {
                // Another profile's saved accounts keep their sign-in.
                let gcm_accounts =
                    accounts.iter().filter(|a| a.provider.supports_gcm() && !st.settings.account_in_use(a)).cloned().collect();
                fx.push(Effect::Account(AccountEffect::ForgetCredentialManager { accounts: gcm_accounts, cwd: st.any_dir() }));
            }
            let extra = if gcm { " and signed Git Credential Manager out of those hosts" } else { "" };
            st.log(true, format!("Removed {} connection{}{extra}.", accounts.len(), if accounts.len() == 1 { "" } else { "s" }));
        }
        ForgeMsg::SetTracker(choice) => {
            let Some(w) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
            st.settings.set_tracker(&w, choice);
            fx.extend(super::prefs::save_settings(st));
            if let Some(snap) = st.snap.take() {
                detect(st, &snap);
                st.snap = Some(snap);
            }
            super::issue::tracker_changed(st);
            refresh(st, true, false, now, fx);
            super::trello::want_boards(st, fx);
        }
        ForgeMsg::SetYearBranches(on) => super::issue::set_year_branches(st, on, fx),
        ForgeMsg::ShowTrackerPage => super::trello::show_page(st, fx),
        ForgeMsg::LoadTrelloBoards(account) => super::trello::load_boards(st, account, fx),
        ForgeMsg::TrelloBoardsLoaded { account, result } => super::trello::boards_loaded(st, account, result),
    }
}

/// Works out the code host (from remotes, preferring origin) and the issue tracker.
pub(super) fn detect(st: &mut AppState, snap: &Snapshot) {
    let (remote, host) = match forge::dashboard::code_host(&snap.remote_urls, &st.settings.connections) {
        Some((r, t)) => (r, Some(t)),
        None => (String::new(), None),
    };
    let tracker = tracker_target(st.settings.tracker_for(&snap.real_workdir), host.as_ref());
    let pr_repos = host.as_ref().map(|h| pr_repos::list(&snap.remote_urls, &remote, h, &st.settings.connections)).unwrap_or_default();
    st.session.forge = match st.session.forge.take() {
        _ if host.is_none() && tracker.is_none() => None,
        // The same service: keep its loaded state, following a renamed remote.
        Some(mut existing) if existing.host == host && existing.tracker == tracker => {
            existing.remote = remote;
            existing.pr_repos = pr_repos;
            Some(existing)
        }
        _ => Some(ForgeState { pr_repos, ..ForgeState::new(host, remote, tracker) }),
    };
    st.session.forge.iter_mut().for_each(|f| f.year_branches = st.settings.year_branches_for(&snap.real_workdir));
    // An edited remote, a new connection or another profile can change the tracker.
    super::issue::tracker_detected(st);
}

/// Calls the service, unless it is a self-hosted server only guessed from the remote URL: those
/// are contacted (and sent credentials) only once the user connects them in Preferences.
pub(super) fn request(st: &mut AppState, target: Target, interactive: bool, call: ForgeCall, fx: &mut Vec<Effect>) {
    // Signing in (resolve_auth) may ask the repository's own credential helper.
    if !st.runs_git_unasked() {
        tracing::debug!("forge request held: repository not trusted");
        return;
    }
    let cwd = st.any_dir();
    let account = target.account();
    let connected = forge::is_connected(&account, &st.settings.connections);
    let Some(fs) = st.session.forge.as_mut() else { return };
    if !forge::may_contact(&account, &st.settings.connections) {
        let msg = format!("not connected to {} at {}", account.provider.name(), account.display());
        if fs.host.as_ref() == Some(&target) {
            fs.host_status = Status::Error(msg.clone());
        }
        if fs.tracker.as_ref() == Some(&target) {
            fs.tracker_status = Status::Error(msg);
        }
        return;
    }
    fs.loading += 1;
    let auth = if interactive { None } else { fs.auth.get(&account.key()).cloned() };
    fx.push(Effect::Forge { target, cwd, auth, interactive, connected, call });
}

/// Refreshes pull requests and issues; `force` ignores the interval and earlier failures.
pub(super) fn refresh(st: &mut AppState, force: bool, interactive: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(fs) = st.session.forge.as_mut() else { return };
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
    let project_keys = fs.needs_project_keys();
    let (host, tracker) = (fs.host.clone(), fs.tracker.clone());
    let same = host.is_some() && host == tracker;
    if let Some(h) = host {
        request(st, h, interactive, ForgeCall::Refresh { need_default, with_issues: same }, fx);
    }
    if let (Some(t), false) = (tracker, same) {
        request(st, t, interactive, ForgeCall::MyIssues { project_keys }, fx);
    }
    super::forge_filters::refresh(st, fx);
}

/// Fetches issues that are mentioned but not cached yet.
pub(super) fn want_issues(st: &mut AppState, keys: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(fs) = st.session.forge.as_mut() else { return };
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
    let (Some(fs), Some(snap)) = (st.session.forge.as_mut(), st.snap.as_ref()) else { return };
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

fn replied(st: &mut AppState, reply: ForgeReply, now: Instant, fx: &mut Vec<Effect>) {
    let mut logs: Vec<(bool, String)> = Vec::new();
    let mut prs_changed = false;
    // Created pull requests' sources and whether a create failed, for a stack's run.
    let (mut created, mut create_failed) = (Vec::new(), false);
    let (t, provider) = (st.settings.lang(), reply.target.provider);
    let Some(fs) = st.session.forge.as_mut() else { return };
    let is_host = fs.host.as_ref() == Some(&reply.target);
    let is_tracker = fs.tracker.as_ref() == Some(&reply.target);
    // A pull request created on another PR repository (an upstream of a fork).
    let other_repo = !is_host && !is_tracker && fs.pr_repos.iter().any(|r| r.target == reply.target);
    if !is_host && !is_tracker && !other_repo {
        return; // the repository changed since the request
    }
    fs.loading = fs.loading.saturating_sub(1);
    let key = reply.target.account().key();
    if let Some(a) = reply.auth {
        fs.auth.insert(key.clone(), a);
    }
    for result in reply.results {
        match result {
            ForgeResult::Created(Ok(pr)) if other_repo => {
                logs.push((
                    true,
                    format!(
                        "Created {} {} {}
{}",
                        fs.pr_noun(),
                        fs.pr_label(pr.id),
                        pr.title,
                        pr.url
                    ),
                ));
                created.push(pr.source.clone());
            }
            ForgeResult::AuthFailed(e) if other_repo => {
                fs.auth.remove(&key);
                create_failed = true;
                logs.push((false, format!("Creating the {} failed: {e}", fs.pr_noun())));
            }
            ForgeResult::Created(Err(e)) if other_repo => {
                if e.auth {
                    fs.auth.remove(&key);
                }
                create_failed = true;
                let line = format!("Creating the {} failed: {}", fs.pr_noun(), e.message);
                logs.push((false, hosting_log(t, provider, Ask::Change, &e, line)));
            }
            // The rest belongs to the code host's and the tracker's repositories.
            _ if other_repo => {}
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
                pr_details::list_arrived(fs, now);
                fs.host_status = Status::Ok;
                prs_changed = true;
            }
            ForgeResult::Prs(Err(e)) => {
                if e.auth {
                    fs.auth.remove(&key);
                }
                fs.host_status = Status::Error(hosting(t, provider, Ask::Read, &e));
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
                fs.tracker_status = Status::Error(hosting(t, provider, Ask::Read, &e));
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
            ForgeResult::DefaultBranch(b) => fs.default_branch = b.or(fs.default_branch.take()),
            ForgeResult::Created(Ok(pr)) => {
                logs.push((true, format!("Created {} {} {}\n{}", fs.pr_noun(), fs.pr_label(pr.id), pr.title, pr.url)));
                created.push(pr.source.clone());
                fs.prs.insert(0, *pr);
                prs_changed = true;
            }
            ForgeResult::Created(Err(e)) => {
                create_failed = true;
                let line = format!("Creating the {} failed: {}", fs.pr_noun(), e.message);
                logs.push((false, hosting_log(t, provider, Ask::Change, &e, line)));
            }
            ForgeResult::TipChecks(asked, r) => super::build_status::replied(fs, &asked, r.map_err(drop), now),
            ForgeResult::PrDetails(r) => prs_changed |= pr_details::replied(fs, r, &key, now),
            ForgeResult::ProjectKeys(r) => match r {
                Ok(Some(keys)) => fs.jira_projects = ProjectKeys::Known(keys.into_iter().collect()),
                Ok(None) => fs.jira_projects = ProjectKeys::TooMany,
                // Left unknown, so the next refresh asks again.
                Err(e) => tracing::debug!("Jira project keys: {}", e.message),
            },
            ForgeResult::Filter(r) => super::forge_filters::replied(fs, r, &key, is_host, is_tracker),
        }
    }
    for (ok, text) in logs {
        st.toast(ok, text.lines().next().unwrap_or_default().to_owned(), now);
        st.log(ok, text);
    }
    if prs_changed {
        resolve_pr_heads(st);
        super::build_status::seed_from_prs(st, now);
        super::pull_request::prs_changed(st, fx);
        super::dashboard::prs_arrived(st, now, fx);
    }
    if create_failed {
        super::stack_prs::create_failed(st);
    }
    super::stack_prs::created(st, &created, fx);
    pr_details::send_next(st, fx);
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
                fx.push(Effect::Account(AccountEffect::SaveToken { account: account.clone(), user: u, token: t }));
            }
            st.settings.connections.retain(|c| c.account != account);
            st.settings.connections.push(Connection { account: account.clone(), user: user.clone(), method });
            fx.extend(super::prefs::save_settings(st));
            if let Some(p) = st.prefs.as_mut() {
                p.int_token.clear();
                p.int_adding = false;
            }
            st.log(true, format!("Connected to {} ({}) as {user}.", account.provider.name(), account.display()));
            // A new connection can make a self-hosted remote recognizable.
            if let Some(fs) = st.session.forge.as_mut() {
                fs.auth.remove(&account.key());
                let (host, tracker) = fs.uses_account(&account);
                fs.filters.account_changed(host, tracker);
                fs.host_status = Status::Idle;
                fs.tracker_status = Status::Idle;
            }
            if let Some(snap) = st.snap.take() {
                detect(st, &snap);
                st.snap = Some(snap);
            }
            refresh(st, true, false, now, fx);
            super::dashboard::account_changed(st, &account, fx);
        }
        Err(e) => {
            if let Some(p) = st.prefs.as_mut() {
                p.int_error = Some(e);
            }
        }
    }
}

/// Removes the connection; its stored token goes too unless another profile still lists the
/// account (sign-ins are shared per account).
fn disconnect(st: &mut AppState, account: &Account, fx: &mut Vec<Effect>) {
    st.settings.connections.retain(|c| &c.account != account);
    if !st.settings.account_in_use(account) {
        fx.push(Effect::Account(AccountEffect::DeleteToken(account.clone())));
    }
    fx.extend(super::prefs::save_settings(st));
    super::dashboard::account_changed(st, account, fx);
    if let Some(fs) = st.session.forge.as_mut() {
        fs.auth.remove(&account.key());
        let msg = format!("not connected to {}", account.provider.name());
        let (host, tracker) = fs.uses_account(account);
        fs.filters.account_changed(host, tracker);
        if host {
            fs.prs.clear();
            fs.details.clear();
            fs.pr_heads.clear();
            fs.tips = Default::default();
            fs.host_status = Status::Error(msg.clone());
        }
        if tracker {
            fs.my_issues.clear();
            fs.tracker_status = Status::Error(msg);
        }
    }
}
