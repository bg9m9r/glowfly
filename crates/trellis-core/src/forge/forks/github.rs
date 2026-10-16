//! Forks on GitHub and GitHub Enterprise Server (REST API v3).

use super::super::github::{api, me, repo_api};
use super::super::{Auth, Target, enc, get, page, post};
use super::{CreatedFork, Fork, ForkList, ForkLookup, Wait};
use crate::error::Result;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Name {
    full_name: String,
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
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    parent: Option<Name>,
    #[serde(default)]
    source: Option<Name>,
}

impl ApiRepo {
    fn into_fork(self) -> Fork {
        Fork {
            owner: self.owner.login,
            full_name: self.full_name,
            https_url: self.clone_url,
            ssh_url: self.ssh_url,
            web_url: self.html_url,
            private: self.private,
        }
    }

    /// Whether this is a fork of `owner/repo` (its parent, or the root of a fork chain).
    fn forks(&self, source: &str) -> bool {
        self.fork && [&self.parent, &self.source].into_iter().flatten().any(|n| n.full_name.eq_ignore_ascii_case(source))
    }
}

fn forks_url(t: &Target) -> String {
    format!("{}/forks?per_page=100&sort=newest", repo_api(t))
}

/// `GET /repos/{owner}/{name}` for a full name such as `ada/app`.
fn repo_by_name(t: &Target, full_name: &str) -> String {
    let (owner, name) = full_name.split_once('/').unwrap_or((full_name, ""));
    format!("{}/repos/{}/{}", api(&t.host, t.provider), enc(owner), enc(name))
}

pub(in crate::forge) fn list(t: &Target, auth: &Auth) -> Result<ForkList> {
    let repos: Vec<ApiRepo> = page::by_link(auth, forks_url(t))?;
    Ok(ForkList { forks: repos.into_iter().map(ApiRepo::into_fork).collect(), truncated: false })
}

pub(in crate::forge) fn my_fork(t: &Target, auth: &Auth) -> Result<ForkLookup> {
    let user = me(&t.host, t.provider, auth)?.login;
    if t.owner.eq_ignore_ascii_case(&user) {
        return Ok(ForkLookup { user, mine: None, own_repository: true });
    }
    let source = format!("{}/{}", t.owner, t.repo);
    // A fork usually keeps the repository's name; look there first.
    match get::<ApiRepo>(auth, &repo_by_name(t, &format!("{user}/{}", t.repo))) {
        Ok(r) if r.forks(&source) => return Ok(ForkLookup { user, mine: Some(r.into_fork()), own_repository: false }),
        Ok(_) => {}
        Err(e) if e.is_not_found() => {}
        Err(e) => return Err(e),
    }
    // A renamed fork: the newest forks, by owner.
    let newest: Vec<ApiRepo> = get(auth, &forks_url(t))?;
    let mine = newest.into_iter().find(|r| r.owner.login.eq_ignore_ascii_case(&user)).map(ApiRepo::into_fork);
    Ok(ForkLookup { user, mine, own_repository: false })
}

pub(in crate::forge) fn create(t: &Target, auth: &Auth, wait: &Wait) -> Result<CreatedFork> {
    // 202 with the new repository (or the fork the user already has); it fills in the background.
    let created: ApiRepo = post(auth, &format!("{}/forks", repo_api(t)), json!({}))?;
    let url = repo_by_name(t, &created.full_name);
    let mut ready = false;
    for attempt in 0..wait.attempts {
        if attempt > 0 {
            std::thread::sleep(wait.interval);
        }
        match get::<serde_json::Value>(auth, &url) {
            Ok(_) => {
                ready = true;
                break;
            }
            Err(e) if e.is_not_found() => {}
            Err(e) => return Err(e),
        }
    }
    Ok(CreatedFork { fork: created.into_fork(), ready })
}
