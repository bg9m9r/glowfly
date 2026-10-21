//! Creating a repository on a hosting service, for publishing a local repository that has no
//! remote yet. GitHub and GitHub Enterprise Server for now.

use super::github::api;
use super::{Account, Auth, Provider, get, page, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;

/// What to create.
#[derive(Clone, Debug, PartialEq)]
pub struct NewRepo {
    /// The user's own login, or an organization the user belongs to.
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
}

/// The repository as created.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CreatedRepo {
    pub full_name: String,
    /// HTTPS clone URL, added as the `origin` remote (signs in like `git push` does).
    pub clone_url: String,
    pub html_url: String,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

fn check(account: &Account) -> Result<()> {
    match account.provider {
        Provider::GitHub | Provider::GitHubEnterprise => Ok(()),
        p => Err(Error::Other(format!("Creating repositories on {} isn't supported yet", p.name()))),
    }
}

/// Where the signed-in user can create a repository: their own login first, then their
/// organizations (sorted).
pub fn repo_owners(account: &Account, auth: &Auth) -> Result<Vec<String>> {
    check(account)?;
    let base = api(&account.host, account.provider);
    let me: Login = get(auth, &format!("{base}/user"))?;
    // Listing organizations needs the read:org scope; without it, offer only the user.
    let mut orgs: Vec<String> = page::by_link::<Login>(auth, format!("{base}/user/orgs?per_page=100"))
        .map(|o| o.into_iter().map(|l| l.login).collect())
        .unwrap_or_default();
    orgs.sort_by_key(|o| o.to_lowercase());
    Ok(std::iter::once(me.login).chain(orgs).collect())
}

/// Creates an empty repository (no README, so the local history pushes cleanly). `user` is the
/// signed-in login, which decides between the user and organization endpoints.
pub fn create_repo(account: &Account, auth: &Auth, user: &str, repo: &NewRepo) -> Result<CreatedRepo> {
    check(account)?;
    let url = create_url(&api(&account.host, account.provider), user, &repo.owner);
    let body = json!({ "name": repo.name, "description": repo.description, "private": repo.private, "auto_init": false });
    post(auth, &url, body)
}

/// The user's own repositories are created under /user, an organization's under /orgs/{org}.
fn create_url(base: &str, user: &str, owner: &str) -> String {
    if owner.eq_ignore_ascii_case(user) { format!("{base}/user/repos") } else { format!("{base}/orgs/{}/repos", super::enc(owner)) }
}

/// A repository name GitHub accepts, from a folder name: letters, digits, `.`, `-` and `_`,
/// with anything else (spaces, punctuation) turned into `-`.
pub fn repo_name_for(folder: &str) -> String {
    let mapped: String = folder.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '-' }).collect();
    let trimmed = mapped.trim_matches('-');
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." { "repository".into() } else { trimmed.to_owned() }
}

/// Whether GitHub would accept `name` as a repository name.
pub fn valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_folders() {
        assert_eq!(repo_name_for("my project"), "my-project");
        assert_eq!(repo_name_for("trellis"), "trellis");
        assert_eq!(repo_name_for("Café (old)"), "Caf---old");
        assert_eq!(repo_name_for("###"), "repository");
        assert!(valid_repo_name("api-server_2.0"));
        assert!(!valid_repo_name("has space"));
        assert!(!valid_repo_name(".."));
        assert!(!valid_repo_name(""));
    }

    #[test]
    fn user_and_organization_endpoints() {
        let base = "https://api.github.com";
        assert_eq!(create_url(base, "ada", "ada"), "https://api.github.com/user/repos");
        assert_eq!(create_url(base, "ada", "Ada"), "https://api.github.com/user/repos");
        assert_eq!(create_url(base, "ada", "acme"), "https://api.github.com/orgs/acme/repos");
    }

    #[test]
    fn created_repo_parses() {
        let r: CreatedRepo = serde_json::from_str(
            r#"{"id":1,"full_name":"ada/demo","clone_url":"https://github.com/ada/demo.git","html_url":"https://github.com/ada/demo","private":true}"#,
        )
        .unwrap();
        assert_eq!(r.clone_url, "https://github.com/ada/demo.git");
    }

    #[test]
    fn other_providers_are_refused() {
        let gitlab = Account { provider: Provider::GitLab, host: "gitlab.com".into(), org: String::new() };
        let auth = Auth::from_token(Provider::GitLab, None, "t", "test");
        assert!(repo_owners(&gitlab, &auth).is_err());
    }
}
