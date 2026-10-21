//! Forks on GitLab.com and GitLab Self-Managed (REST API v4).

use super::super::gitlab::project_api;
use super::super::{Auth, Target, get, page, post};
use super::{CreatedFork, Fork, ForkList, ForkLookup, Wait};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Namespace {
    full_path: String,
}

#[derive(Deserialize)]
struct ApiProject {
    id: u64,
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
    #[serde(default)]
    import_status: Option<String>,
}

impl ApiProject {
    fn into_fork(self) -> Fork {
        Fork {
            owner: self.namespace.full_path,
            full_name: self.path_with_namespace,
            https_url: self.http_url_to_repo,
            ssh_url: self.ssh_url_to_repo,
            web_url: self.web_url,
            private: self.visibility != "public",
        }
    }
}

pub(in crate::forge) fn list(t: &Target, auth: &Auth) -> Result<ForkList> {
    let url = format!("{}/forks?per_page=100&order_by=created_at&sort=desc", project_api(t));
    let projects: Vec<ApiProject> = page::by_link(auth, url)?;
    Ok(ForkList { forks: projects.into_iter().map(ApiProject::into_fork).collect(), truncated: false })
}

pub(in crate::forge) fn my_fork(t: &Target, auth: &Auth) -> Result<ForkLookup> {
    #[derive(Deserialize)]
    struct User {
        username: String,
    }
    let user = get::<User>(auth, &format!("https://{}/api/v4/user", t.host))?.username;
    if t.owner.eq_ignore_ascii_case(&user) {
        return Ok(ForkLookup { user, mine: None, own_repository: true });
    }
    let owned: Vec<ApiProject> = get(auth, &format!("{}/forks?owned=true&per_page=100", project_api(t)))?;
    // In the user's own namespace first, else one in a group they own.
    let at = owned.iter().position(|p| p.namespace.full_path.eq_ignore_ascii_case(&user)).unwrap_or(0);
    let mine = owned.into_iter().nth(at).map(ApiProject::into_fork);
    Ok(ForkLookup { user, mine, own_repository: false })
}

pub(in crate::forge) fn create(t: &Target, auth: &Auth, wait: &Wait) -> Result<CreatedFork> {
    let created: ApiProject = post(auth, &format!("{}/fork", project_api(t)), json!({}))?;
    let url = format!("https://{}/api/v4/projects/{}", t.host, created.id);
    let mut ready = false;
    for attempt in 0..wait.attempts {
        if attempt > 0 {
            std::thread::sleep(wait.interval);
        }
        let p: ApiProject = get(auth, &url)?;
        match p.import_status.as_deref() {
            None | Some("finished" | "none") => {
                ready = true;
                break;
            }
            Some("failed") => return Err(Error::Other("GitLab couldn't create the fork: import failed".into())),
            Some(_) => {}
        }
    }
    Ok(CreatedFork { fork: created.into_fork(), ready })
}
