//! The New file and Rename file dialogs' form: the typed path, the last error and whether a
//! request is running; what status says about a path being deleted, the trash's name, the
//! file manager's show-in-folder label, and the offer of the home Trash (`TrashOffer`).

use glowfly_core::i18n::Lang;

/// Why Open and Show in folder are off for a file the working tree doesn't have.
pub const NOT_IN_WORKDIR: &str = "Not in your working tree (deleted, renamed or never checked out)";
/// The platform's name for where deleted files go.
pub const TRASH: &str = if cfg!(windows) { "Recycle Bin" } else { "Trash" };
/// The show-in-file-manager label for this platform.
pub const SHOW_IN_FOLDER: &str = if cfg!(windows) {
    "Show in Explorer"
} else if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else {
    "Open containing folder"
};

/// `NOT_IN_WORKDIR` in the language (the English const stays for the file history's views).
pub fn not_in_workdir(t: Lang) -> &'static str {
    t.tr("files.not_in_workdir")
}

/// `SHOW_IN_FOLDER` in the language.
pub fn show_in_folder(t: Lang) -> &'static str {
    if cfg!(windows) {
        t.tr("files.menu.show_in_folder.windows")
    } else if cfg!(target_os = "macos") {
        t.tr("files.menu.show_in_folder.macos")
    } else {
        t.tr("files.menu.show_in_folder.linux")
    }
}

/// The trash on `path`'s drive can't be used (Linux): `body` asks whether to move it to the home
/// Trash (`op` in `DeleteMode::HomeTrash`) or delete it permanently instead. For a file Undo
/// can't bring back, `confirm` is the body and acknowledgement of the confirmation Delete
/// permanently opens first.
#[derive(Clone, Debug, PartialEq)]
pub struct TrashOffer {
    pub op: crate::effect::FileOp,
    pub path: String,
    pub body: String,
    pub confirm: Option<(String, String)>,
}

/// A path field with the outcome of its last submission.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathForm {
    pub text: String,
    /// (the text it was produced for, the message): shown only while the field still holds that
    /// text, so editing the path hides it without anyone clearing it.
    pub error: Option<(String, String)>,
    /// A create or rename is running; submitting again is off until it reports back.
    pub busy: bool,
    /// Whether the path was edited or a submit tried (its problem shows then).
    pub touched: super::touched::Touched,
}

impl PathForm {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    /// The error for the text now in the field, if any.
    pub fn current_error(&self) -> Option<&str> {
        self.error.as_ref().filter(|(t, _)| *t == self.text).map(|(_, e)| e.as_str())
    }
}

/// The New file dialog's text for a sibling of `path`: its folder with a trailing '/', or ""
/// for a file at the repository root.
pub fn folder_prefill(path: &str) -> String {
    path.rfind('/').map(|i| path[..=i].to_owned()).unwrap_or_default()
}

/// Whether `path` has a stage-0 index entry as status shows it: staged with anything but a
/// deletion, or unstaged as modified, type-changed or renamed. Untracked files, and files
/// removed from the index (`rm --cached`: staged 'D' plus untracked), don't count.
pub fn tracked(snap: &glowfly_core::repo::Snapshot, path: &str) -> bool {
    snap.staged.iter().any(|f| f.path == path && f.status != 'D')
        || snap.unstaged.iter().any(|f| f.path == path && matches!(f.status, 'M' | 'T' | 'R'))
}

/// Whether `path` is staged as a new file (staged 'A'): not in the last commit, or there is no
/// commit yet. Deleting it then also takes it out of the index. Staged renames are covered by
/// `staged_rename`.
pub fn staged_new(snap: &glowfly_core::repo::Snapshot, path: &str) -> bool {
    snap.staged.iter().any(|f| f.path == path && f.status == 'A')
}

/// Whether `path` is the new path of a staged rename (staged 'R'). Status lists a staged rename
/// once, on its new path; the path it was renamed from isn't listed.
pub fn staged_rename(snap: &glowfly_core::repo::Snapshot, path: &str) -> bool {
    snap.staged.iter().any(|f| f.path == path && f.status == 'R')
}

/// Whether deleting `path` also takes it out of the index: it is staged as new or as a staged
/// rename's new path.
pub fn unstages_on_delete(snap: &glowfly_core::repo::Snapshot, path: &str) -> bool {
    staged_new(snap, path) || staged_rename(snap, path)
}

/// Whether status lists `path` (unstaged or staged) as a file that is still in the working tree,
/// so a new file can't take its place. A path equal to `except` ignoring case (a rename's source)
/// doesn't count. Core checks the disk again; this only catches what the snapshot shows.
pub fn listed(snap: &glowfly_core::repo::Snapshot, path: &str, except: Option<&str>) -> bool {
    if except.is_some_and(|e| e.eq_ignore_ascii_case(path)) {
        return false;
    }
    snap.unstaged.iter().chain(&snap.staged).any(|f| f.path == path) && !snap.missing_from_workdir(path)
}
