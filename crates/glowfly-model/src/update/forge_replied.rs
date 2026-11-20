//! Hosting-service replies: a call's results, and the credentials its lookup found
//! (state/auth_cache.rs).

use super::pull_request::details;
use crate::effect::{Effect, ForgeReply, ForgeResult, FoundAuth};
use crate::state::error_words::{hosting, hosting_log};
use crate::state::{AppState, ProjectKeys, Status};
use glowfly_core::error::advice::Ask;
use glowfly_core::forge;
use std::time::Instant;

/// A lookup found a credential: keep it for every tab and the dashboard, unless the account's
/// connection changed since the lookup started.
pub(super) fn auth_found(st: &mut AppState, f: FoundAuth) {
    if forge::is_connected(&f.account, &st.settings.connections) != f.connected {
        return;
    }
    st.auth_cache.store(f.account.key(), &f.dir, f.auth, f.asked || f.connected);
}

#[expect(clippy::too_many_lines, reason = "one arm per forge reply; split by reply kind")]
pub(super) fn replied(st: &mut AppState, reply: ForgeReply, now: Instant, fx: &mut Vec<Effect>) {
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
    // Set when a call failed to sign in or the service rejected the credential: what the call
    // could have carried is forgotten (`AuthCache::forget_for`).
    let mut rejected = false;
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
                rejected = true;
                create_failed = true;
                logs.push((false, format!("Creating the {} failed: {e}", fs.pr_noun())));
            }
            ForgeResult::Created(Err(e)) if other_repo => {
                rejected |= e.auth;
                create_failed = true;
                let line = format!("Creating the {} failed: {}", fs.pr_noun(), e.message);
                logs.push((false, hosting_log(t, provider, Ask::Change, &e, line)));
            }
            // The rest belongs to the code host's and the tracker's repositories.
            _ if other_repo => {}
            ForgeResult::AuthFailed(e) => {
                rejected = true;
                let e = forge::sign_in_status(e);
                if is_host {
                    fs.host_status = Status::Error(e.clone());
                }
                if is_tracker {
                    fs.tracker_status = Status::Error(e);
                }
            }
            ForgeResult::Prs(Ok(prs)) => {
                fs.prs = prs;
                details::list_arrived(fs, now);
                fs.host_status = Status::Ok;
                prs_changed = true;
            }
            ForgeResult::Prs(Err(e)) => {
                rejected |= e.auth;
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
                rejected |= e.auth;
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
            ForgeResult::PrDetails(r) => {
                let (changed, failed_auth) = details::replied(fs, r, now);
                prs_changed |= changed;
                rejected |= failed_auth;
            }
            ForgeResult::ProjectKeys(r) => match r {
                Ok(Some(keys)) => fs.jira_projects = ProjectKeys::Known(keys.into_iter().collect()),
                Ok(None) => fs.jira_projects = ProjectKeys::TooMany,
                // Left unknown, so the next refresh asks again.
                Err(e) => tracing::debug!("Jira project keys: {}", e.message),
            },
            ForgeResult::Filter(r) => rejected |= super::forge_filters::replied(fs, r, is_host, is_tracker),
        }
    }
    if rejected {
        let dir = st.any_dir();
        st.auth_cache.forget_for(&key, &dir);
    }
    for (ok, text) in logs {
        st.toast(ok, text.lines().next().unwrap_or_default().to_owned(), now);
        st.log(ok, text);
    }
    if prs_changed {
        super::pull_request::heads::resolve_pr_heads(st);
        super::build_status::seed_from_prs(st, now);
        super::pull_request::prs_changed(st, fx);
        super::dashboard::prs_arrived(st, now, fx);
    }
    if create_failed {
        super::stack_prs::create_failed(st);
    }
    super::stack_prs::created(st, &created, fx);
    details::send_next(st, fx);
}
