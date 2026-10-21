//! The user's repositories on Bitbucket Cloud (API 2.0): the workspaces they can reach, then
//! each workspace's repositories, merged newest update first.

use super::super::bitbucket::{CLOUD, cloud_list};
use super::super::{Auth, enc, page};
use super::HostedRepo;
use crate::error::{Error, Result};
use serde::Deserialize;

/// The most workspaces whose repositories are listed.
const MAX_WORKSPACES: usize = 50;

#[derive(Deserialize)]
struct Slug {
    slug: String,
}

/// A workspace entry: `/user/workspaces` wraps it in an access record; a plain workspace is
/// read too.
#[derive(Deserialize)]
#[serde(untagged)]
enum WsEntry {
    Access { workspace: Slug },
    Plain { slug: String },
}

impl WsEntry {
    fn slug(self) -> String {
        match self {
            WsEntry::Access { workspace } => workspace.slug,
            WsEntry::Plain { slug } => slug,
        }
    }
}

#[derive(Deserialize)]
struct Href {
    href: String,
}

#[derive(Deserialize)]
struct CloneLink {
    name: String,
    href: String,
}

#[derive(Deserialize)]
struct Links {
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
    links: Links,
    #[serde(default)]
    updated_on: String,
}

fn href(links: &[CloneLink], name: &str) -> String {
    links.iter().find(|l| l.name == name).map(|l| l.href.clone()).unwrap_or_default()
}

/// The slugs of the workspaces the user can reach. Where `/user/workspaces` isn't available
/// (gone or not found), the older permissions list is asked instead.
fn workspaces(auth: &Auth) -> Result<Vec<String>> {
    let entries: Vec<WsEntry> = match cloud_list(auth, format!("{CLOUD}/user/workspaces?pagelen=100")) {
        Err(e) if e.is_not_found() || matches!(e, Error::Http { status: 410, .. }) => {
            cloud_list(auth, format!("{CLOUD}/user/permissions/workspaces?pagelen=100"))?
        }
        other => other?,
    };
    Ok(entries.into_iter().map(WsEntry::slug).take(MAX_WORKSPACES).collect())
}

pub(super) fn list(auth: &Auth) -> Result<Vec<HostedRepo>> {
    let mut repos: Vec<CloudRepo> = Vec::new();
    for slug in workspaces(auth)? {
        repos.extend(cloud_list::<CloudRepo>(auth, format!("{CLOUD}/repositories/{}?pagelen=100&sort=-updated_on", enc(&slug)))?);
        if repos.len() >= page::MAX_ITEMS {
            repos.truncate(page::MAX_ITEMS);
            break;
        }
    }
    // RFC 3339 times from one API compare as text.
    repos.sort_by(|a, b| b.updated_on.cmp(&a.updated_on));
    Ok(repos
        .into_iter()
        .map(|r| HostedRepo {
            owner: r.workspace.slug,
            full_name: r.full_name,
            https_url: href(&r.links.clone, "https"),
            ssh_url: href(&r.links.clone, "ssh"),
            web_url: r.links.html.href,
            private: r.is_private,
        })
        .collect())
}
