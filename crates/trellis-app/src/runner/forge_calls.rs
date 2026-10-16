//! Calls to hosting services: pull requests and issues (`Effect::Forge`), connecting an
//! account (`Effect::Connect`) and a Trello account's boards (`Effect::TrelloBoards`).

use crate::effect::{ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg};
use std::path::Path;
use trellis_core::forge::{self, Account, Auth, Target};

/// Signs in to `account` with a saved or entered token, or through Git Credential Manager,
/// and checks that the sign-in works.
pub(super) fn connect(account: Account, user: Option<String>, token: Option<String>, cwd: &Path) -> Msg {
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
pub(super) fn trello_boards(account: Account, cwd: &Path) -> Msg {
    let result =
        forge::resolve_auth(&account, cwd, false, true).and_then(|a| forge::trello_boards(&account, &a)).map_err(|e| e.to_string());
    Msg::Forge(ForgeMsg::TrelloBoardsLoaded { account, result })
}

pub(super) fn call(target: Target, cwd: &Path, auth: Option<Auth>, interactive: bool, connected: bool, call: ForgeCall) -> ForgeReply {
    let auth = match auth {
        Some(a) => a,
        None => match forge::resolve_auth(&target.account(), cwd, interactive, connected) {
            Ok(a) => a,
            Err(e) => return ForgeReply { target, auth: None, results: vec![ForgeResult::AuthFailed(e.to_string())] },
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
        ForgeCall::MyIssues => vec![ForgeResult::MyIssues(forge::my_issues(t, &auth).map_err(ForgeError::from))],
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
    };
    ForgeReply { target, auth: Some(auth), results }
}
