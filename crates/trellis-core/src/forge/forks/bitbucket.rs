//! Forks on Bitbucket Cloud (API 2.0) and Bitbucket Data Center (REST 1.0). Only listing: finding
//! the user's own fork and forking are not built for Bitbucket yet.

use super::super::bitbucket::{cloud_list, cloud_repo};
use super::super::bitbucket_dc::{dc_list, dc_repo};
use super::super::{Auth, Target};
use super::{Fork, ForkList};
use crate::error::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Href {
    href: String,
}

/// A clone link: Cloud names them `https` and `ssh`, Data Center `http` and `ssh`.
#[derive(Deserialize)]
struct CloneLink {
    name: String,
    href: String,
}

fn clone_href(links: &[CloneLink], names: &[&str]) -> String {
    links.iter().find(|l| names.contains(&l.name.as_str())).map(|l| l.href.clone()).unwrap_or_default()
}

#[derive(Deserialize)]
struct Slug {
    slug: String,
}

#[derive(Deserialize)]
struct CloudLinks {
    #[serde(default)]
    clone: Vec<CloneLink>,
    html: Href,
}

#[derive(Deserialize)]
struct CloudRepo {
    full_name: String,
    #[serde(default)]
    is_private: bool,
    workspace: Slug,
    links: CloudLinks,
}

pub(in crate::forge) fn cloud(t: &Target, auth: &Auth) -> Result<ForkList> {
    let repos: Vec<CloudRepo> = cloud_list(auth, format!("{}/forks?pagelen=100", cloud_repo(t)))?;
    let forks = repos
        .into_iter()
        .map(|r| Fork {
            owner: r.workspace.slug,
            full_name: r.full_name,
            https_url: clone_href(&r.links.clone, &["https"]),
            ssh_url: clone_href(&r.links.clone, &["ssh"]),
            web_url: r.links.html.href,
            private: r.is_private,
        })
        .collect();
    Ok(ForkList { forks, truncated: false })
}

#[derive(Deserialize)]
struct DcProject {
    key: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    owner: Option<Slug>,
}

#[derive(Deserialize)]
struct DcLinks {
    #[serde(default)]
    clone: Vec<CloneLink>,
    #[serde(rename = "self", default)]
    this: Vec<Href>,
}

#[derive(Deserialize)]
struct DcRepo {
    slug: String,
    #[serde(default)]
    public: bool,
    project: DcProject,
    links: DcLinks,
}

pub(in crate::forge) fn data_center(t: &Target, auth: &Auth) -> Result<ForkList> {
    let repos: Vec<DcRepo> = dc_list(auth, format!("{}/forks?limit=100", dc_repo(t)))?;
    let forks = repos
        .into_iter()
        .map(|r| {
            // A personal repository's project is the user (key `~ADA`); name the fork after them.
            let personal = r.project.kind == "PERSONAL";
            let owner = r.project.owner.filter(|_| personal).map(|o| o.slug).unwrap_or_else(|| r.project.key.clone());
            Fork {
                owner,
                full_name: format!("{}/{}", r.project.key, r.slug),
                https_url: clone_href(&r.links.clone, &["http", "https"]),
                ssh_url: clone_href(&r.links.clone, &["ssh"]),
                web_url: r.links.this.into_iter().next().map(|h| h.href).unwrap_or_default(),
                private: !r.public,
            }
        })
        .collect();
    Ok(ForkList { forks, truncated: false })
}
