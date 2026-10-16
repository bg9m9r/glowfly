//! The WORKTREES section's text and the Create worktree dialog's form and rules, shared by the
//! views and `update` so a disabled item or a refused request shows the same words. Pure: paths
//! are compared component by component, never resolved on disk.

use crate::worktree::NewWorktree;
use git2::Oid;
use std::path::Path;
use trellis_core::branch_ops::check_branch_name;
use trellis_core::paths::{is_within, same_spelling};
use trellis_core::repo::{Snapshot, WorktreeInfo};
use trellis_core::worktree_ops::{WorktreeBase, folder_slug, parse_folder};

/// What the Create worktree dialog is editing.
#[derive(Clone, Debug, PartialEq)]
pub struct WorktreeForm {
    pub base: WorktreeBase,
    pub folder: String,
    /// The user typed in the folder field or picked a parent, so it no longer follows the slug.
    pub folder_edited: bool,
    /// The new branch's name; empty checks out the base branch, or detaches at the commit.
    pub branch: String,
    /// Open the new worktree in a tab once it is created.
    pub open: bool,
    /// (folder text it was for, message) from the runner's disk check; shown only while the
    /// field holds that text.
    pub error: Option<(String, String)>,
    /// The request whose disk check is running. Only that check's result may create the
    /// worktree, and only while this dialog is still open.
    pub pending: Option<NewWorktree>,
}

impl WorktreeForm {
    pub fn new(snap: &Snapshot, base: WorktreeBase) -> Self {
        let mut form =
            Self { base, folder: String::new(), folder_edited: false, branch: String::new(), open: true, error: None, pending: None };
        form.folder = default_folder(snap, &form.slug());
        form
    }

    /// Whether the disk check is running.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// The disk check's error, while the folder field still holds the text it was for.
    pub fn current_error(&self) -> Option<&str> {
        self.error.as_ref().filter(|(text, _)| *text == self.folder).map(|(_, e)| e.as_str())
    }

    /// The new folder's name: from the new branch when one is typed, else the base's label.
    pub fn slug(&self) -> String {
        let typed = folder_slug(self.branch.trim());
        if typed.is_empty() { folder_slug(&self.base.label()) } else { typed }
    }
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Whether the section is drawn: the repository has a linked worktree.
pub fn section_shown(snap: &Snapshot) -> bool {
    snap.worktrees.iter().any(|w| !w.is_main())
}

/// The text at the right of a row: what is checked out, then whether it is locked or missing.
pub fn right_text(wt: &WorktreeInfo) -> String {
    let mut out = match (&wt.branch, wt.head) {
        (Some(b), _) => b.clone(),
        (None, Some(oid)) => format!("detached {}", short(oid)),
        (None, None) => "unknown".to_owned(),
    };
    if wt.locked.is_some() {
        out.push_str(" · locked");
    }
    if wt.missing {
        out.push_str(" · missing");
    }
    out
}

/// A row's hover text, one fact per line.
pub fn tooltip(wt: &WorktreeInfo) -> String {
    let mut lines = vec![match &wt.name {
        None => "Main worktree".to_owned(),
        Some(n) => format!("Linked worktree ({n})"),
    }];
    lines.push(wt.path.display().to_string());
    match (&wt.branch, wt.head) {
        (Some(b), _) => lines.push(format!("Branch: {b}")),
        (None, Some(oid)) => lines.push(format!("Detached HEAD at {}", short(oid))),
        (None, None) => {}
    }
    match wt.locked.as_deref() {
        Some("") => lines.push("Locked".to_owned()),
        Some(r) => lines.push(format!("Locked: {r}")),
        None => {}
    }
    if wt.missing {
        lines.push("The folder is missing. git can prune this record.".to_owned());
    }
    if wt.current {
        lines.push("Open in this tab.".to_owned());
    }
    lines.join("\n")
}

/// Why the worktree can't be opened in a new tab.
pub fn open_blocked(wt: &WorktreeInfo) -> Option<&'static str> {
    if wt.current {
        Some("This worktree is open in this tab.")
    } else if wt.missing {
        Some("Its folder is missing.")
    } else {
        None
    }
}

/// The listed worktree at `path`.
pub fn find<'a>(snap: &'a Snapshot, path: &Path) -> Option<&'a WorktreeInfo> {
    snap.worktrees.iter().find(|w| same_spelling(&w.path, path))
}

/// The worktree that has `branch` checked out.
pub fn checked_out_in<'a>(snap: &'a Snapshot, branch: &str) -> Option<&'a WorktreeInfo> {
    snap.worktrees.iter().find(|w| w.branch.as_deref() == Some(branch))
}

/// `<parent of the main worktree>/<its folder name>-worktrees/<slug>`; the open workdir stands
/// in for the main worktree when none is listed (a bare main repository).
pub fn default_folder(snap: &Snapshot, slug: &str) -> String {
    let main = snap.worktrees.iter().find(|w| w.is_main()).map(|w| w.path.as_path()).unwrap_or(&snap.workdir);
    let name = main.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".to_owned());
    let parent = main.parent().unwrap_or(main);
    parent.join(format!("{name}-worktrees")).join(slug).display().to_string()
}

/// What makes the typed folder unusable, before the disk is checked.
pub fn folder_problem(snap: &Snapshot, text: &str) -> Option<String> {
    let path = match parse_folder(text) {
        Ok(p) => p,
        Err(p) => return Some(p.message(text.trim())),
    };
    find(snap, &path).map(|w| format!("A worktree is already registered at {}.", w.path.display()))
}

/// A caution, not a refusal: the folder is inside a listed worktree.
pub fn folder_warning(snap: &Snapshot, text: &str) -> Option<String> {
    let path = parse_folder(text).ok()?;
    let host = snap
        .worktrees
        .iter()
        .filter(|w| is_within(&path, &w.path) && !same_spelling(&path, &w.path))
        .max_by_key(|w| w.path.components().count())?;
    Some(format!(
        "This folder is inside the worktree at {}. Its files will show up there as untracked unless .gitignore covers them.",
        host.path.display()
    ))
}

/// What makes the base and new branch name unusable.
pub fn branch_problem(snap: &Snapshot, base: &WorktreeBase, new_branch: &str) -> Option<String> {
    let new_branch = new_branch.trim();
    if let WorktreeBase::Branch(b) = base {
        match snap.locals.iter().find(|l| l.name == *b) {
            None => return Some(format!("Branch {b} no longer exists.")),
            Some(l) if l.oid.is_none() => return Some(format!("{b} has no commits yet.")),
            Some(_) => {}
        }
    }
    if !new_branch.is_empty() {
        let existing: Vec<&str> = snap.locals.iter().map(|l| l.name.as_str()).collect();
        return check_branch_name(new_branch, &existing, None).err().map(|p| p.message(new_branch));
    }
    let WorktreeBase::Branch(b) = base else { return None };
    let wt = checked_out_in(snap, b)?;
    Some(if wt.current {
        format!("{b} is checked out in this worktree. Enter a name for a new branch.")
    } else {
        format!("{b} is checked out in the worktree at {}. Enter a name for a new branch.", wt.path.display())
    })
}

/// The request the form describes, or the first problem with it.
pub fn request(snap: &Snapshot, form: &WorktreeForm) -> Result<NewWorktree, String> {
    if let Some(p) = folder_problem(snap, &form.folder).or_else(|| branch_problem(snap, &form.base, &form.branch)) {
        return Err(p);
    }
    let folder = parse_folder(&form.folder).map_err(|p| p.message(form.folder.trim()))?;
    let branch = Some(form.branch.trim().to_owned()).filter(|b| !b.is_empty());
    Ok(NewWorktree { repo: snap.workdir.clone(), base: form.base.clone(), folder, branch, open: form.open })
}

/// The same rules on a request built earlier, against a newer snapshot.
pub fn recheck(snap: &Snapshot, req: &NewWorktree) -> Option<String> {
    folder_problem(snap, &req.folder.display().to_string())
        .or_else(|| branch_problem(snap, &req.base, req.branch.as_deref().unwrap_or_default()))
}
