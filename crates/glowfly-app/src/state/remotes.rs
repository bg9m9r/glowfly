//! The REMOTE section's text and the Add/Edit remote dialog's form and rules, shared by the
//! views and `update` so a disabled item or a refused request shows the same words. Pure.

use super::forks::{ForkAction, add_fork_label, fork_label, fork_target};
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::remote::RemoteMsg;
use glowfly_core::forge::{self, Connection};
use glowfly_core::remote_ops::{RemoteEdit, RemoteNameProblem, UrlProblem, add_cmds, check_remote_name, check_url, display_url, edit_cmds};
use glowfly_core::repo::{RemoteInfo, Snapshot};

/// What the Add/Edit remote dialog is editing.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteForm {
    /// The remote as it was when Edit opened; None when adding.
    pub original: Option<RemoteInfo>,
    pub name: String,
    pub url: String,
    pub push_url: String,
    /// Add only: fetch the new remote in the same job.
    pub fetch_after: bool,
    /// The fields edited and whether a submit was tried, so a problem waits for them.
    pub touched: super::touched::Touched,
}

impl RemoteForm {
    /// An empty Add form, named origin when the repository has no remote yet.
    pub fn add(snap: &Snapshot) -> Self {
        let name = if snap.remotes.is_empty() { "origin".to_owned() } else { String::new() };
        Self { original: None, name, url: String::new(), push_url: String::new(), fetch_after: true, touched: Default::default() }
    }

    /// An Edit form filled from `info` (its first fetch and push URLs).
    pub fn edit(info: &RemoteInfo) -> Self {
        Self {
            original: Some(info.clone()),
            name: info.name.clone(),
            url: info.fetch_url().to_owned(),
            push_url: info.push_url().unwrap_or("").to_owned(),
            fetch_after: false,
            touched: Default::default(),
        }
    }

    pub fn is_edit(&self) -> bool {
        self.original.is_some()
    }

    /// Whether Save would change anything; always true when adding.
    pub fn changed(&self) -> bool {
        let Some(o) = &self.original else { return true };
        self.name.trim() != o.name || self.url.trim() != o.fetch_url() || self.push_url.trim() != o.push_url().unwrap_or("")
    }

    /// Whether an edit changes the remote's name (letter case included).
    pub fn renaming(&self) -> bool {
        self.original.as_ref().is_some_and(|o| self.name.trim() != o.name)
    }
}

/// The configured remote called `name`.
pub fn find<'a>(snap: &'a Snapshot, name: &str) -> Option<&'a RemoteInfo> {
    snap.remote_info.iter().find(|r| r.name == name)
}

/// What is wrong with the form's name, against the remotes in `snap`.
pub fn name_problem(snap: Option<&Snapshot>, form: &RemoteForm) -> Option<RemoteNameProblem> {
    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    check_remote_name(form.name.trim(), &existing, form.original.as_ref().map(|o| o.name.as_str())).err()
}

/// What is wrong with the fetch URL.
pub fn url_problem(form: &RemoteForm) -> Option<UrlProblem> {
    check_url(form.url.trim()).err()
}

/// What is wrong with the push URL; an empty one is fine.
pub fn push_problem(form: &RemoteForm) -> Option<UrlProblem> {
    let p = form.push_url.trim();
    if p.is_empty() { None } else { check_url(p).err() }
}

/// The push URL repeats the fetch URL (allowed, but worth a note).
pub fn push_same_as_fetch(form: &RemoteForm) -> bool {
    let p = form.push_url.trim();
    !p.is_empty() && p == form.url.trim()
}

/// Push URLs in config after the first, which the dialog leaves alone.
pub fn extra_push_urls(form: &RemoteForm) -> usize {
    form.original.as_ref().map_or(0, |o| o.push_urls.len().saturating_sub(1))
}

/// A git job: its label and command lines.
pub type Job = (String, Vec<Vec<String>>);

/// The job for a submitted form against the current snapshot, or the reason it can't run.
/// Ok(None) means an edit with nothing changed.
pub fn request(snap: &Snapshot, form: &RemoteForm) -> Result<Option<Job>, String> {
    if let Some(p) = name_problem(Some(snap), form) {
        return Err(p.message());
    }
    if let Some(p) = url_problem(form) {
        return Err(p.message(false));
    }
    if let Some(p) = push_problem(form) {
        return Err(p.message(true));
    }
    let (name, url, push) = (form.name.trim(), form.url.trim(), form.push_url.trim());
    let push = (!push.is_empty()).then_some(push);
    let Some(original) = &form.original else {
        return Ok(Some((format!("Add remote {name}"), add_cmds(name, url, push, form.fetch_after))));
    };
    let Some(current) = find(snap, &original.name) else {
        return Err(gone_text(&original.name));
    };
    let cmds = edit_cmds(current, &RemoteEdit { name, url, push_url: push });
    Ok((!cmds.is_empty()).then(|| (format!("Edit remote {}", original.name), cmds)))
}

/// The message for a remote that is no longer configured.
pub fn gone_text(name: &str) -> String {
    format!("Remote {name} no longer exists.")
}

/// Up to three items, then "and N more": "a, b and c", "a, b, c and 2 more".
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        _ if items.len() <= 3 => format!("{} and {}", items[..items.len() - 1].join(", "), items[items.len() - 1]),
        _ => format!("{} and {} more", items[..3].join(", "), items.len() - 3),
    }
}

/// The Remove remote confirmation's text: what goes with the remote and which local branches
/// stop tracking it.
pub fn remove_body(snap: &Snapshot, name: &str) -> String {
    let branches: Vec<String> = snap
        .remotes
        .iter()
        .find(|(r, _)| r == name)
        .map(|(_, b)| b.iter().map(|(b, _)| format!("{name}/{b}")).collect())
        .unwrap_or_default();
    let locals: Vec<String> =
        snap.locals.iter().filter(|b| b.tracking.as_ref().is_some_and(|t| t.remote == name)).map(|b| b.name.clone()).collect();
    let deleted = match branches.len() {
        0 => None,
        1 => Some(format!("Its remote-tracking branch {} is deleted with it", branches[0])),
        n => Some(format!("Its {n} remote-tracking branches ({}) are deleted with it", list(&branches))),
    };
    let untracked = match locals.len() {
        0 => None,
        1 => Some(format!("the local branch {} stops tracking it", locals[0])),
        _ => Some(format!("the local branches {} stop tracking it", list(&locals))),
    };
    let middle = match (deleted, untracked) {
        (Some(d), Some(u)) => format!(" {d}, and {u}."),
        (Some(d), None) => format!(" {d}."),
        (None, Some(u)) => format!(" {}{}.", u[..1].to_uppercase(), &u[1..]),
        (None, None) => String::new(),
    };
    format!("Removes {name} from this repository's settings.{middle} Nothing changes on the server.")
}

/// Why Fetch <remote> is disabled.
pub fn fetch_blocked(info: &RemoteInfo, idle: bool) -> Option<&'static str> {
    if !idle {
        Some("Another operation is still running.")
    } else if info.fetch_url().is_empty() {
        Some("This remote has no fetch URL.")
    } else {
        None
    }
}

/// A remote row's tooltip: its fetch URL and a separate push URL, passwords hidden.
pub fn tooltip(info: &RemoteInfo) -> String {
    let fetch = if info.fetch_url().is_empty() { "(none)".to_owned() } else { display_url(info.fetch_url()) };
    match info.separate_push_url() {
        Some(p) => format!("Fetch: {fetch}\nPush: {}", display_url(p)),
        None => format!("Fetch: {fetch}"),
    }
}

/// Add remote…, and for each remote with a fetch URL: Fetch, View on its service, Add a fork
/// and Fork, the last three only where they can run.
pub(crate) fn palette_entries(snap: &Snapshot, connections: &[Connection], out: &mut Vec<Entry>) {
    let add = PaletteCommand::Remote(RemoteMsg::AddDialog);
    out.push(entry(add, "Add remote…", "new remote url origin upstream fork mirror"));
    let cmd = PaletteCommand::Remote;
    for r in snap.remote_info.iter().filter(|r| !r.fetch_url().is_empty()) {
        let e = entry(cmd(RemoteMsg::Fetch(r.name.clone())), format!("Fetch {}", r.name), "remote update download");
        out.push(with_detail(e, Some(display_url(r.fetch_url()))));
        if let Ok(page) = forge::web_page(r.fetch_url(), connections) {
            let label = if page.service.is_empty() {
                format!("View {} in browser", r.name)
            } else {
                format!("View {} on {}", r.name, page.service)
            };
            out.push(with_detail(entry(cmd(RemoteMsg::View(r.name.clone())), label, "browser web open site page"), Some(page.url)));
        }
        if fork_target(r, connections, ForkAction::List).is_ok() {
            out.push(entry(cmd(RemoteMsg::ForkDialog(r.name.clone())), add_fork_label(&r.name), "fork teammate contributor remote"));
        }
        if fork_target(r, connections, ForkAction::Create).is_ok() {
            out.push(entry(cmd(RemoteMsg::ForkOnService(r.name.clone())), fork_label(r, connections), "fork copy account"));
        }
    }
}
