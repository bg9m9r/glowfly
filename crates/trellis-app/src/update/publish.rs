//! Publishing a repository that has no remote: create a GitHub repository, add it as `origin`
//! and push the current branch to it.

use super::ops::{JobSpec, start_job};
use crate::effect::{AccountEffect, Effect};
use crate::msg::PublishMsg;
use crate::state::publish::PublishForm;
use crate::state::{AppState, Modal};
use trellis_core::forge::{self, Account, NewRepo, Provider};
use trellis_core::gitcli::args;
use trellis_core::paths::same_spelling;

pub(super) fn handle(st: &mut AppState, msg: PublishMsg, fx: &mut Vec<Effect>) {
    match msg {
        PublishMsg::PublishDialog => open(st, fx),
        PublishMsg::PublishAccount(i) => {
            let Some(f) = form(st) else { return };
            if i == f.account || i >= f.accounts.len() {
                return;
            }
            f.account = i;
            f.user = None;
            f.owners.clear();
            f.owner.clear();
            load_owners(st, fx);
        }
        PublishMsg::PublishOwners { account, result } => {
            let Some(f) = form(st).filter(|f| f.account() == Some(&account)) else { return };
            f.loading = false;
            match result {
                Ok(owners) => {
                    f.user = owners.first().cloned();
                    if !owners.contains(&f.owner) {
                        f.owner = owners.first().cloned().unwrap_or_default();
                    }
                    f.owners = owners;
                    f.error = None;
                }
                Err(e) => f.error = Some(format!("Couldn't sign in to {}: {e}", account.display())),
            }
        }
        PublishMsg::Publish => {
            let idle = st.job.is_none();
            let Some(f) = form(st) else { return };
            let (Some(account), Some(user)) = (f.account().cloned(), f.user.clone()) else { return };
            let name = f.name.trim().to_owned();
            if !idle || f.creating || !forge::valid_repo_name(&name) || f.owner.is_empty() {
                return;
            }
            f.creating = true;
            f.error = None;
            let repo = NewRepo { owner: f.owner.clone(), name, description: f.description.trim().to_owned(), private: f.private };
            fx.push(Effect::Account(AccountEffect::CreateRepo { account, cwd: f.workdir.clone(), user, repo, branch: f.branch.clone() }));
        }
        PublishMsg::Published { workdir, branch, result } => match result {
            Ok(repo) => {
                if form(st).is_some_and(|f| same_spelling(&f.workdir, &workdir)) {
                    st.modal = None;
                }
                st.log(true, format!("Created {} ({}).", repo.full_name, repo.html_url));
                let cmds = vec![args(&["remote", "add", "origin", &repo.clone_url]), args(&["push", "-u", "origin", &branch])];
                start_job(st, JobSpec { cwd: Some(workdir.clone()), ..JobSpec::new("Publish to GitHub".into(), cmds) }, fx);
                // The job belongs to the published repository even if another tab is active now.
                if let Some(job) = st.job.as_mut() {
                    job.workdir = Some(workdir);
                }
            }
            Err(e) => match form(st).filter(|f| same_spelling(&f.workdir, &workdir)) {
                Some(f) => {
                    f.creating = false;
                    f.error = Some(e);
                }
                None => st.log(false, format!("Creating the GitHub repository failed: {e}")),
            },
        },
    }
}

fn form(st: &mut AppState) -> Option<&mut PublishForm> {
    match st.modal.as_mut() {
        Some(Modal::Publish(f)) => Some(f),
        _ => None,
    }
}

/// Opens the dialog for the active repository, if it has a branch with commits to push.
pub(super) fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let Some(branch) = snap.head_branch.clone() else {
        st.log(false, "Check out a branch before publishing.");
        return;
    };
    if snap.commits.is_empty() {
        st.log(false, "Make a commit before publishing: there is nothing to push yet.");
        return;
    }
    let mut accounts: Vec<Account> = st
        .settings
        .connections
        .iter()
        .map(|c| c.account.clone())
        .filter(|a| matches!(a.provider, Provider::GitHub | Provider::GitHubEnterprise))
        .collect();
    // github.com first; without a connection, sign in the way `git push` does.
    accounts.sort_by_key(|a| a.provider != Provider::GitHub);
    if accounts.is_empty() {
        accounts.push(Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() });
    }
    st.modal = Some(Modal::Publish(Box::new(PublishForm {
        workdir: snap.workdir.clone(),
        branch,
        accounts,
        account: 0,
        user: None,
        owners: Vec::new(),
        owner: String::new(),
        name: forge::repo_name_for(&snap.name),
        description: String::new(),
        private: true,
        loading: false,
        creating: false,
        error: None,
        touched: Default::default(),
    })));
    load_owners(st, fx);
}

fn load_owners(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(f) = form(st) else { return };
    let Some(account) = f.account().cloned() else { return };
    f.loading = true;
    f.error = None;
    fx.push(Effect::Account(AccountEffect::Owners { account, cwd: f.workdir.clone() }));
}
