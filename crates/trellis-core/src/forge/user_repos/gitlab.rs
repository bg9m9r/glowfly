//! The user's projects on GitLab.com and GitLab Self-Managed (REST API v4).

use super::super::{Account, Auth, page};
use super::HostedRepo;
use crate::error::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Namespace {
    full_path: String,
}

#[derive(Deserialize)]
struct ApiProject {
    path_with_namespace: String,
    namespace: Namespace,
    #[serde(default)]
    http_url_to_repo: String,
    #[serde(default)]
    ssh_url_to_repo: String,
    #[serde(default)]
    web_url: String,
    #[serde(default)]
    visibility: String,
}

/// Projects the user is a member of, most recently active first. Not `simple=true`: that leaves
/// out the visibility.
pub(super) fn list(account: &Account, auth: &Auth) -> Result<Vec<HostedRepo>> {
    let url = format!("https://{}/api/v4/projects?membership=true&per_page=100&order_by=last_activity_at&sort=desc", account.host);
    let projects: Vec<ApiProject> = page::by_link(auth, url)?;
    Ok(projects
        .into_iter()
        .map(|p| HostedRepo {
            owner: p.namespace.full_path,
            full_name: p.path_with_namespace,
            https_url: p.http_url_to_repo,
            ssh_url: p.ssh_url_to_repo,
            web_url: p.web_url,
            private: p.visibility != "public",
        })
        .collect())
}
