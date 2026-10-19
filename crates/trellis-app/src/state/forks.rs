//! The fork dialogs and View on service: Add a fork (pick one of the repository's forks on its
//! hosting service and add it as a remote) and Fork (fork the repository into the user's account,
//! then add and fetch it). Their forms and the rules shared by the views and `update`, so a
//! disabled item and a refused request say the same thing. Pure.

use super::remotes::Job;
use crate::effect::ForgeError;
use std::path::PathBuf;
use trellis_core::forge::{self, Connection, Fork, ForkList, Target};
use trellis_core::remote_ops::{add_cmds, check_remote_name, check_url};
use trellis_core::repo::{RemoteInfo, Snapshot};

/// The Add a fork dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkPicker {
    pub workdir: PathBuf,
    /// The remote whose repository's forks are listed.
    pub remote: String,
    pub target: Target,
    pub loading: bool,
    pub list: ForkList,
    pub error: Option<ForgeError>,
    pub filter: String,
    /// Index into `list.forks`.
    pub picked: Option<usize>,
    pub name: String,
    /// The last name Trellis suggested; a pick replaces `name` only while it still equals this.
    pub suggested: String,
    pub fetch_after: bool,
}

/// Where the Fork dialog is.
#[derive(Clone, Debug, PartialEq)]
pub enum ForkStep {
    /// Signing in and looking for a fork the user already has.
    Checking,
    /// The user already has `fork`.
    Mine { user: String, fork: Fork },
    /// The repository is in the user's own account.
    OwnRepo { user: String },
    /// Nothing found: the dialog asks to fork.
    Ready { user: String },
    /// Waiting for the service to create the fork.
    Creating { user: String },
}

/// The Fork dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkCreate {
    pub workdir: PathBuf,
    pub remote: String,
    pub target: Target,
    pub step: ForkStep,
    pub name: String,
    pub fetch_after: bool,
    pub error: Option<ForgeError>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForkAction {
    /// List the repository's forks (Add a fork).
    List,
    /// Fork it into the user's account.
    Create,
}

/// The hosting-service repository behind `info` for `action`, or why Trellis can't do it.
pub fn fork_target(info: &RemoteInfo, connections: &[Connection], action: ForkAction) -> Result<Target, String> {
    let url = info.fetch_url();
    if url.is_empty() {
        return Err("This remote has no fetch URL.".into());
    }
    let Some(t) = forge::parse_remote(url, connections) else {
        return Err("Trellis doesn't recognize a hosting service in this remote's URL.".into());
    };
    let p = t.provider;
    match action {
        ForkAction::List if !p.lists_forks() => return Err(format!("Listing forks on {} isn't supported yet.", p.name())),
        ForkAction::Create if !p.creates_forks() => return Err(format!("Forking on {} isn't supported yet.", p.name())),
        _ => {}
    }
    if !forge::may_contact(&t.account(), connections) {
        return Err(format!("Connect {} at {} in Preferences > Integrations first.", p.name(), t.host));
    }
    Ok(t)
}

/// `Fork origin (team/app) on GitHub…`, or a generic label when no service is recognized.
pub fn fork_label(info: &RemoteInfo, connections: &[Connection]) -> String {
    match forge::parse_remote(info.fetch_url(), connections) {
        Some(t) => format!("Fork {} ({}) on {}…", info.name, t.display(), t.provider.name()),
        None => format!("Fork {} on its hosting service…", info.name),
    }
}

/// The remote menu's Fork item: "Fork on GitHub…", or a generic label when no service is
/// recognized. The menu's header names the remote.
pub fn fork_menu_label(info: &RemoteInfo, connections: &[Connection]) -> String {
    match forge::parse_remote(info.fetch_url(), connections) {
        Some(t) => format!("Fork on {}…", t.provider.name()),
        None => "Fork on its hosting service…".into(),
    }
}

/// The palette's Add a fork entry, and where the fork-creation log points.
pub fn add_fork_label(name: &str) -> String {
    format!("Add a fork of {name}…")
}

/// The remote menu's Add a fork item.
pub const ADD_FORK_ITEM: &str = "Add a fork…";

/// The remote menu's View item ("View on GitHub", "View in browser") and, when it is disabled,
/// why.
pub fn view_item(info: &RemoteInfo, connections: &[Connection]) -> (String, Option<String>) {
    match forge::web_page(info.fetch_url(), connections) {
        Ok(p) if !p.service.is_empty() => (format!("View on {}", p.service), None),
        Ok(_) => ("View in browser".into(), None),
        Err(e) => ("View in browser".into(), Some(e.message())),
    }
}

/// The remote (origin first) and service for the Add remote dialog's fork link.
pub fn fork_source(snap: &Snapshot, connections: &[Connection]) -> Option<(String, String)> {
    let origin = snap.remote_info.iter().filter(|r| r.name == "origin");
    origin.chain(snap.remote_info.iter().filter(|r| r.name != "origin")).find_map(|r| {
        let t = fork_target(r, connections, ForkAction::List).ok()?;
        Some((r.name.clone(), t.provider.name().to_owned()))
    })
}

/// The configured remote that already points at `fork` (by fetch or push URL).
pub fn added_as(snap: &Snapshot, fork: &Fork, connections: &[Connection]) -> Option<String> {
    let urls = [&fork.https_url, &fork.ssh_url];
    let points = |u: &String| urls.iter().any(|f| forge::same_repository(u, f, connections));
    snap.remote_info.iter().find(|r| r.urls.iter().chain(&r.push_urls).any(points)).map(|r| r.name.clone())
}

/// Indices of the forks whose name contains the filter (any letter case).
pub fn visible(p: &ForkPicker) -> Vec<usize> {
    let filter = p.filter.trim().to_lowercase();
    (0..p.list.forks.len()).filter(|i| p.list.forks[*i].full_name.to_lowercase().contains(&filter)).collect()
}

/// A failed request's text, with how to fix a sign-in failure.
pub fn error_text(e: &ForgeError) -> String {
    if e.auth { format!("{} Sign in again in Preferences > Integrations.", e.message) } else { e.message.clone() }
}

/// A valid, unused remote name for a fork owned by `owner`.
pub fn suggest(snap: &Snapshot, owner: &str) -> String {
    let existing: Vec<&str> = snap.remote_info.iter().map(|r| r.name.as_str()).collect();
    forge::remote_name_for(owner, &existing)
}

/// The job that adds `fork` as remote `name` (and fetches it), checked against `snap`. The URL
/// follows the style of `source_url`, the remote the fork came from.
pub fn add_job(snap: &Snapshot, source_url: &str, fork: &Fork, name: &str, fetch: bool, connections: &[Connection]) -> Result<Job, String> {
    let name = name.trim();
    let existing: Vec<&str> = snap.remote_info.iter().map(|r| r.name.as_str()).collect();
    check_remote_name(name, &existing, None).map_err(|p| p.message())?;
    if let Some(r) = added_as(snap, fork, connections) {
        return Err(format!("{} is already the remote {r}.", fork.full_name));
    }
    let url = fork.url_like(source_url);
    check_url(url).map_err(|p| p.message(false))?;
    Ok((format!("Add remote {name}"), add_cmds(name, url, None, fetch)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(url: &str) -> RemoteInfo {
        RemoteInfo { name: "origin".into(), urls: vec![url.into()], push_urls: Vec::new(), skip_fetch_all: false }
    }

    #[test]
    fn menu_labels_leave_the_remote_to_the_header() {
        assert_eq!(fork_menu_label(&remote("https://github.com/team/app.git"), &[]), "Fork on GitHub…");
        assert_eq!(fork_menu_label(&remote("https://example.com/team/app.git"), &[]), "Fork on its hosting service…");
        assert_eq!(view_item(&remote("https://github.com/team/app.git"), &[]), ("View on GitHub".into(), None));
        assert_eq!(view_item(&remote("https://example.com/team/app.git"), &[]).0, "View in browser");
        let folder = trellis_core::paths::temp_dir().join("app.git");
        let (label, why) = view_item(&remote(&folder.to_string_lossy()), &[]);
        assert_eq!(label, "View in browser");
        assert!(why.is_some(), "a folder has no web page");
    }
}
