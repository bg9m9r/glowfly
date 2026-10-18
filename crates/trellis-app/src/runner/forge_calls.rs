//! Calls to hosting services: pull requests and issues (`Effect::Forge`), and connected
//! accounts (`Effect::Account`): signing in, tokens, owners, creating a repository and a Trello
//! account's boards.

use crate::effect::{AccountEffect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, PublishMsg};
use std::path::Path;
use trellis_core::forge::{self, Account, Auth, Target};

/// Runs one account effect; the token saves and drops answer only when they fail.
pub(super) fn account(e: AccountEffect) -> Option<Msg> {
    let err = |e: trellis_core::error::Error| e.to_string();
    match e {
        AccountEffect::Owners { account, cwd } => {
            let result = forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::repo_owners(&account, &a)).map_err(err);
            Some(Msg::Publish(PublishMsg::PublishOwners { account, result }))
        }
        AccountEffect::CreateRepo { account, cwd, workdir, user, repo, branch } => {
            let result =
                forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::create_repo(&account, &a, &user, &repo)).map_err(err);
            Some(Msg::Publish(PublishMsg::Published { workdir, branch, result }))
        }
        AccountEffect::Connect { account, user, token, cwd } => Some(connect(account, user, token, &cwd)),
        AccountEffect::TrelloBoards { account, cwd } => Some(trello_boards(account, &cwd)),
        AccountEffect::SaveToken { account, user, token } => forge::save_token(&account, user.as_deref(), &token)
            .err()
            .map(|e| Msg::Logged { ok: false, text: format!("Connected, but the token could not be saved: {e}") }),
        AccountEffect::DeleteToken(account) => forge::delete_token(&account).err().map(|e| Msg::Logged { ok: false, text: e.to_string() }),
        AccountEffect::ForgetCredentialManager { accounts, cwd } => {
            let failures: Vec<String> = accounts
                .iter()
                .filter_map(|a| forge::forget_credential_manager(a, &cwd).err().map(|e| format!("{}: {e}", a.display())))
                .collect();
            (!failures.is_empty()).then(|| Msg::Logged { ok: false, text: failures.join("\n") })
        }
    }
}

/// Signs in to `account` with a saved or entered token, or through Git Credential Manager,
/// and checks that the sign-in works.
fn connect(account: Account, user: Option<String>, token: Option<String>, cwd: &Path) -> Msg {
    let (method, auth, saved) = match token {
        Some(t) => {
            let auth = Auth::from_token(account.provider, user.as_deref(), &t, "saved token");
            ("access token".to_owned(), Ok(auth), Some((user, t)))
        }
        None => ("Git Credential Manager".to_owned(), forge::from_credential_manager(&account, cwd, true), None),
    };
    let result = auth.and_then(|a| forge::verify(&account, &a)).map_err(|e| e.to_string());
    Msg::Forge(ForgeMsg::Connected { account, method, token: saved, result })
}

/// The open boards of a connected Trello account, for the Issue Tracker page's Board picker.
/// `connected` is true: update sends this only for a connected account.
fn trello_boards(account: Account, cwd: &Path) -> Msg {
    let result =
        forge::resolve_auth(&account, cwd, false, true).and_then(|a| forge::trello_boards(&account, &a)).map_err(|e| e.to_string());
    Msg::Forge(ForgeMsg::TrelloBoardsLoaded { account, result })
}

pub(super) fn call(target: Target, cwd: &Path, auth: Option<Auth>, interactive: bool, connected: bool, call: ForgeCall) -> ForgeReply {
    let auth = match auth {
        Some(a) => a,
        None => match forge::resolve_auth(&target.account(), cwd, interactive, connected) {
            Ok(a) => a,
            Err(e) => return ForgeReply { target, auth: None, results: auth_failed(&call, e.to_string()) },
        },
    };
    let t = &target;
    let results = match call {
        ForgeCall::Refresh { need_default, with_issues } => {
            let mut r = vec![ForgeResult::Prs(forge::pull_requests(t, &auth).map_err(ForgeError::from))];
            if need_default {
                r.push(ForgeResult::DefaultBranch(forge::default_branch(t, &auth).ok().flatten()));
            }
            if with_issues {
                r.push(ForgeResult::MyIssues(forge::my_issues(t, &auth).map_err(ForgeError::from)));
            }
            r
        }
        ForgeCall::MyIssues { project_keys } => {
            let mut r = vec![ForgeResult::MyIssues(forge::my_issues(t, &auth).map_err(ForgeError::from))];
            if project_keys {
                r.push(ForgeResult::ProjectKeys(forge::project_keys(t, &auth).map_err(ForgeError::from)));
            }
            r
        }
        ForgeCall::Issues(keys) => {
            let r = forge::issues(t, &auth, &keys).map_err(ForgeError::from);
            vec![ForgeResult::Issues(keys, r)]
        }
        ForgeCall::Create(pr) => {
            vec![ForgeResult::Created(forge::create_pull_request(t, &auth, &pr).map(Box::new).map_err(ForgeError::from))]
        }
        ForgeCall::TipChecks(tips) => {
            let r = forge::commit_checks(t, &auth, &tips).map_err(ForgeError::from);
            vec![ForgeResult::TipChecks(tips, r)]
        }
        ForgeCall::PrDetails(prs) => vec![ForgeResult::PrDetails(forge::pr_details(t, &auth, &prs).map_err(ForgeError::from))],
        ForgeCall::Filter(c) => vec![ForgeResult::Filter(super::forge_filter_calls::call(t, &auth, c))],
    };
    ForgeReply { target, auth: Some(auth), results }
}

/// A failed sign-in, answered in the shape the call expects where that matters: the second pass
/// over pull requests stops on its own failure and leaves the host's status alone.
fn auth_failed(call: &ForgeCall, message: String) -> Vec<ForgeResult> {
    match call {
        ForgeCall::PrDetails(_) => vec![ForgeResult::PrDetails(Err(ForgeError { message, auth: true }))],
        _ => vec![ForgeResult::AuthFailed(message)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_sign_in_for_pull_request_details_answers_as_details() {
        let r = auth_failed(&ForgeCall::PrDetails(Vec::new()), "no token".into());
        assert!(matches!(&r[..], [ForgeResult::PrDetails(Err(ForgeError { auth: true, .. }))]));
        let r = auth_failed(&ForgeCall::Issues(Vec::new()), "no token".into());
        assert!(matches!(&r[..], [ForgeResult::AuthFailed(m)] if m == "no token"));
    }
}
