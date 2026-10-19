//! The Move worktree dialog's form and the rules and words for moving a worktree, shared by the
//! views and `update` so a disabled menu item and a refused request say the same thing. Pure:
//! the target is compared with the listed worktrees by spelling; the runner checks the disk.

use crate::worktree::MoveRequest;
use std::path::{Path, PathBuf};
use trellis_core::native_path::shown;
use trellis_core::paths::{FolderId, is_within, same_spelling};
use trellis_core::repo::{Snapshot, WorktreeInfo};
use trellis_core::worktree_ops::parse_folder;

/// What the Move worktree dialog is editing.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveForm {
    /// Where the worktree is now.
    pub path: PathBuf,
    pub name: String,
    /// The new folder, as typed.
    pub folder: String,
    /// The worktree's identity on disk, which finds the tabs showing it.
    pub folder_id: Option<FolderId>,
    /// (folder text it was for, message) from the runner; shown only while the field holds
    /// that text.
    pub error: Option<(String, String)>,
    /// Whether the folder was edited or a submit tried (its problem shows then).
    pub touched: super::touched::Touched,
}

impl MoveForm {
    /// The form for `wt`, its New folder field holding where it is now.
    pub fn new(wt: &WorktreeInfo) -> Self {
        let (path, name, folder) = (wt.path.clone(), wt.folder_name(), shown(&wt.path));
        Self { path, name, folder, folder_id: wt.folder, error: None, touched: Default::default() }
    }

    /// The runner's error, while the folder field still holds the text it was for.
    pub fn current_error(&self) -> Option<&str> {
        self.error.as_ref().filter(|(text, _)| *text == self.folder).map(|(_, e)| e.as_str())
    }

    /// Browse… picked `parent`: the worktree keeps its folder name inside it.
    pub fn parent_picked(&mut self, parent: &Path) {
        let name = self.path.file_name().map(PathBuf::from).unwrap_or_default();
        self.folder = parent.join(name).display().to_string();
    }
}

/// Why the worktree can't be moved.
pub fn move_refused(wt: &WorktreeInfo) -> Option<&'static str> {
    if wt.is_main() {
        Some("The main worktree can't be moved.")
    } else if wt.current {
        Some("This worktree is open in this tab.")
    } else if wt.missing {
        Some("Its folder is missing.")
    } else if wt.locked.is_some() {
        Some("It is locked. Unlock it first.")
    } else {
        None
    }
}

/// Why the folder can't be the new place: it is the current one (a prompt, not an error).
pub const UNMOVED: &str = "That is where it is now.";

/// What makes the typed folder `text` unusable as the new place of the worktree at `from`,
/// before the disk is checked.
pub fn target_problem(snap: &Snapshot, from: &Path, text: &str) -> Option<String> {
    let path = match parse_folder(text) {
        Ok(p) => p,
        Err(p) => return Some(p.message(text.trim())),
    };
    if same_spelling(&path, from) {
        return Some(UNMOVED.to_owned());
    }
    if is_within(&path, from) {
        return Some("A worktree can't be moved into its own folder.".to_owned());
    }
    super::worktrees::find(snap, &path).map(|w| format!("A worktree is already registered at {}.", shown(&w.path)))
}

/// The request the form describes, or the first problem with it.
pub fn move_request(snap: &Snapshot, form: &MoveForm) -> Result<MoveRequest, String> {
    if let Some(p) = target_problem(snap, &form.path, &form.folder) {
        return Err(p);
    }
    let to = parse_folder(&form.folder).map_err(|p| p.message(form.folder.trim()))?;
    Ok(MoveRequest { repo: snap.workdir.clone(), from: form.path.clone(), to, name: form.name.clone(), folder: form.folder_id })
}

pub fn moved_headline(name: &str, to: &Path) -> String {
    format!("Moved worktree {name} to {}", shown(to))
}
