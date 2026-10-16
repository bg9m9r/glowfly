//! `trellis://` links: the link being opened and what it waits for, the dialogs a link may need,
//! the scheme registration's status, and the pure rules (which remote a copied link names,
//! where to look for the linked repository, which match wins, the messages).

use super::AppState;
use crate::deep_link::LinkTarget;
use std::path::PathBuf;
use trellis_core::link::find::{Found, TargetState, rank};
use trellis_core::link::{self, Link};
use trellis_core::paths::same_spelling;
use trellis_core::repo::Snapshot;
use trellis_core::url_scheme::SchemeStatus;

/// The Clone… button's hover text when the link carries no URL.
pub const NO_CLONE_URL: &str = "The link doesn't include a remote URL to clone from";
/// The toast for a link that arrives while another dialog is open.
pub const LINK_WAITING: &str = "A Trellis link is waiting. Close the open dialog, then open it.";
/// The toast for a linked commit past the loaded history.
pub const OLDER_THAN_LOADED: &str = "This commit is older than the loaded history; Load more to see it in the graph.";
/// Why a link can't be made for a repository.
pub const NOTHING_TO_LINK: &str = "this repository has no commits and no remote yet";
/// The longest part of an invalid link the dialog shows.
pub const SHOWN_LINK_CHARS: usize = 200;

#[derive(Default)]
pub struct DeepLinkState {
    /// Incremented for every link opened: results of an older search are dropped.
    pub seq: u64,
    /// The link being opened, until its target is selected or it is given up.
    pub pending: Option<Pending>,
    /// Which program opens links (Preferences > General).
    pub scheme: SchemeView,
}

pub struct Pending {
    pub link: Link,
    pub awaiting: Awaiting,
    /// The matches the Choose a repository dialog lists.
    pub chosen: Vec<Found>,
    /// The folder Locate on disk… is checking.
    pub located: Option<PathBuf>,
}

impl Pending {
    pub fn new(link: Link) -> Self {
        Pending { link, awaiting: Awaiting::Search, chosen: Vec::new(), located: None }
    }
}

/// What a pending link waits for.
#[derive(Clone, Debug, PartialEq)]
pub enum Awaiting {
    /// The search over the open, favorite and recent repositories.
    Search,
    /// The user, in a deep-link dialog.
    Dialog,
    /// The repository at `workdir` to be the active tab, to select `target` there.
    Ready { workdir: PathBuf, target: TargetState, remote: Option<String> },
    /// The target is missing from `workdir`; a toast offers Fetch.
    Offered { workdir: PathBuf, remote: Option<String> },
    /// The fetch the toast started in `workdir`.
    Fetch { workdir: PathBuf, remote: Option<String> },
    /// A clone from `url` to open.
    Clone { url: String },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SchemeView {
    #[default]
    Unknown,
    Checking,
    Known(SchemeStatus),
    Failed(String),
}

/// The deep-link dialogs (`Modal::DeepLink`, ui/deep_link_dialog.rs).
#[derive(Clone, Debug, PartialEq)]
pub enum LinkDialog {
    /// The text isn't a link Trellis can open: why, and the text as shown.
    Invalid { text: String, reason: String },
    /// No copy of the repository was found. `what` is the target ("branch feature/x"), `repo`
    /// what the link names the repository by, `problem` why the located folder didn't match.
    NotFound { what: String, repo: String, can_clone: bool, problem: Option<String> },
    /// Several copies: the folders, and the one picked.
    Choose { what: String, paths: Vec<PathBuf>, picked: usize },
}

/// The URL of remote `name` in `snap` (libgit2's, with url.insteadOf applied), without any
/// password or token in it, since a link is made to be shared.
fn url_of(snap: &Snapshot, name: &str) -> Option<String> {
    snap.remote_urls.iter().find(|(n, _)| n == name).map(|(_, u)| link::shareable_url(u)).filter(|u| !u.is_empty())
}

/// The remote a link names when nothing else says which: the checked-out branch's upstream
/// remote, else `origin`, else the first.
pub fn preferred_remote(snap: &Snapshot) -> Option<&str> {
    let has = |n: &str| snap.remote_urls.iter().any(|(r, _)| r == n);
    let upstream = snap.head_info().and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote()).map(|t| t.remote.as_str());
    upstream.filter(|r| has(r)).or_else(|| has("origin").then_some("origin")).or_else(|| snap.remote_urls.first().map(|(r, _)| r.as_str()))
}

/// The URL, target and description ("branch feature/x") of the link for `t`, or why there can be
/// none. A local branch tracking a remote branch links that one, under its name there.
pub fn link_request(snap: &Snapshot, t: &LinkTarget) -> Result<(Option<String>, link::Target, String), &'static str> {
    let preferred = || preferred_remote(snap).and_then(|r| url_of(snap, r));
    let (url, target) = match t {
        LinkTarget::Repository { remote } => (remote.as_deref().and_then(|r| url_of(snap, r)).or_else(preferred), link::Target::Repository),
        LinkTarget::LocalBranch(name) => {
            let upstream = snap.locals.iter().find(|b| &b.name == name).and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote());
            match upstream.and_then(|t| url_of(snap, &t.remote).map(|u| (u, t.merge.clone()))) {
                Some((url, merge)) => (Some(url), link::Target::Branch(merge)),
                None => (preferred(), link::Target::Branch(name.clone())),
            }
        }
        LinkTarget::RemoteBranch { remote, name } => (url_of(snap, remote).or_else(preferred), link::Target::Branch(name.clone())),
        LinkTarget::Commit(oid) => (preferred(), link::Target::Commit(oid.to_string())),
        LinkTarget::Tag(name) => (preferred(), link::Target::Tag(name.clone())),
    };
    if url.is_none() && snap.head_oid.is_none() {
        return Err(NOTHING_TO_LINK);
    }
    let what = Link { url: None, root: None, target: target.clone() }.describe();
    Ok((url, target, what))
}

/// Where to look for a link's repository, in order: the active tab, the other tabs, the
/// favorites, then the recent list; each folder once, and none the last check found missing.
pub fn candidates(st: &AppState) -> Vec<PathBuf> {
    let active = st.snap.as_ref().map(|s| s.workdir.clone()).or_else(|| st.tabs.get(st.active_tab).and_then(|t| t.path.clone()));
    let others = st.tabs.iter().enumerate().filter(|(i, _)| *i != st.active_tab).filter_map(|(_, t)| t.path.clone());
    let all = active.into_iter().chain(others).chain(super::repo_lists::favorites(&st.settings)).chain(st.recent.iter().cloned());
    let mut out: Vec<PathBuf> = Vec::new();
    for p in all {
        if !super::repo_lists::is_missing(st, &p) && !out.iter().any(|o| st.same_folder(o, &p)) {
            out.push(p);
        }
    }
    out
}

/// What the search found, once ranked.
#[derive(Debug, PartialEq)]
pub enum Best {
    None,
    One(Found),
    Many(Vec<Found>),
}

/// The best of `matches`: only the top rank counts; worktrees of one repository (one common
/// dir) are one match, preferring one open in a tab, then the main worktree; and among several,
/// the only one open in a tab wins.
pub fn best(matches: Vec<Found>, st: &AppState) -> Best {
    let Some(top) = matches.iter().map(rank).max() else { return Best::None };
    let open = |f: &Found| st.tab_showing(&f.workdir).is_some();
    let mut kept: Vec<Found> = Vec::new();
    for f in matches.into_iter().filter(|f| rank(f) == top) {
        match kept.iter().position(|k| same_spelling(&k.common_dir, &f.common_dir)) {
            Some(i) if (open(&f), f.main_worktree) > (open(&kept[i]), kept[i].main_worktree) => kept[i] = f,
            Some(_) => {}
            None => kept.push(f),
        }
    }
    if kept.len() > 1 {
        let in_tabs: Vec<usize> = (0..kept.len()).filter(|&i| open(&kept[i])).collect();
        if let [i] = in_tabs[..] {
            return Best::One(kept.swap_remove(i));
        }
    }
    match kept.len() {
        0 => Best::None,
        1 => Best::One(kept.remove(0)),
        _ => Best::Many(kept),
    }
}

/// "Branch feature/x isn't in app. Fetching origin may bring it."
pub fn missing_text(link: &Link, repo_name: &str, remote: Option<&str>, has_remotes: bool) -> String {
    let described = link.describe();
    let mut chars = described.chars();
    let what: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    match (remote, has_remotes) {
        (Some(r), _) => format!("{what} isn't in {repo_name}. Fetching {r} may bring it."),
        (None, true) => format!("{what} isn't in {repo_name}. Fetching its remotes may bring it."),
        (None, false) => format!("{what} isn't in {repo_name}, which has no remotes to fetch it from."),
    }
}

/// The Trellis Links row's status line, and whether it is an error.
pub fn status_text(view: &SchemeView) -> (String, bool) {
    match view {
        SchemeView::Unknown | SchemeView::Checking => ("Checking…".to_owned(), false),
        SchemeView::Known(SchemeStatus::Registered) => ("trellis:// links open this copy of Trellis".to_owned(), false),
        SchemeView::Known(SchemeStatus::OtherProgram(p)) => (format!("trellis:// links open another program: {p}"), false),
        SchemeView::Known(SchemeStatus::NotRegistered) => ("trellis:// links aren't registered".to_owned(), false),
        SchemeView::Known(SchemeStatus::Unsupported(why)) => ((*why).to_owned(), false),
        SchemeView::Failed(e) => (e.clone(), true),
    }
}

/// An invalid link's text as the dialog shows it: printable, at most `SHOWN_LINK_CHARS`.
pub fn shown_link(text: &str) -> String {
    let safe = trellis_core::text::printable(text);
    match safe.char_indices().nth(SHOWN_LINK_CHARS) {
        Some((i, _)) => format!("{}…", &safe[..i]),
        None => safe,
    }
}

/// Whether Unregister can change anything: Trellis's own registration (this copy's or another
/// copy's), not another program's desktop file.
pub fn can_unregister(view: &SchemeView) -> bool {
    match view {
        SchemeView::Known(SchemeStatus::Registered) => true,
        SchemeView::Known(SchemeStatus::OtherProgram(p)) => !(p.ends_with(".desktop") && p != trellis_core::url_scheme::DESKTOP_FILE),
        _ => false,
    }
}
