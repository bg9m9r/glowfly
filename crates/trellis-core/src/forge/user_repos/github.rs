//! The user's repositories on GitHub and GitHub Enterprise Server (REST API v3).

use super::super::github::api;
use super::super::{Account, Auth, page};
use super::HostedRepo;
use crate::error::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct ApiRepo {
    owner: Login,
    full_name: String,
    #[serde(default)]
    clone_url: String,
    #[serde(default)]
    ssh_url: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    private: bool,
}

/// Repositories the user owns, collaborates on or reaches through an organization, most
/// recently pushed first.
pub(super) fn list(account: &Account, auth: &Auth) -> Result<Vec<HostedRepo>> {
    let url = format!(
        "{}/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member",
        api(&account.host, account.provider)
    );
    let repos: Vec<ApiRepo> = page::by_link(auth, url)?;
    Ok(repos
        .into_iter()
        .map(|r| HostedRepo {
            owner: r.owner.login,
            full_name: r.full_name,
            https_url: r.clone_url,
            ssh_url: r.ssh_url,
            web_url: r.html_url,
            private: r.private,
        })
        .collect())
}
