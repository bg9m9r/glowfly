//! The file and folder pickers Preferences and the dialogs open, and where each picked path goes.

use crate::effect::{Effect, OsEffect};
use crate::msg::{FilePurpose, FolderPurpose};
use crate::state::{AppState, Modal};
use glowfly_core::signing::SigningFormat;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) fn pick_file(purpose: FilePurpose, fx: &mut Vec<Effect>) {
    let title = match purpose {
        FilePurpose::SigningProgram => "Signing program",
        FilePurpose::SshAllowedSigners => "Allowed signers file",
    };
    fx.push(Effect::Os(OsEffect::PickFile { title: title.into(), purpose }));
}

/// Only fills the field: the Save / Revert bar writes it.
pub(super) fn file_picked(st: &mut AppState, purpose: FilePurpose, path: &Path, fx: &mut Vec<Effect>) {
    let value = path.display().to_string();
    let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) else { return };
    match purpose {
        FilePurpose::SigningProgram => match SigningFormat::from_config(&git.signing_format) {
            SigningFormat::OpenPgp => git.gpg_program = value,
            SigningFormat::Ssh => git.ssh_program = value,
            SigningFormat::X509 => git.x509_program = value,
        },
        FilePurpose::SshAllowedSigners => git.ssh_allowed_signers = value,
    }
    super::prefs_signing::load_keys(st, fx);
}

pub(super) fn pick_folder(purpose: FolderPurpose, fx: &mut Vec<Effect>) {
    let title = match purpose {
        FolderPurpose::OpenRepo => "Open a Git repository",
        FolderPurpose::InitRepo => "New repository in…",
        FolderPurpose::CloneParent => "Clone into folder",
        FolderPurpose::HooksPath => "Hooks folder",
        FolderPurpose::WorktreeParent => "Create the worktree in…",
        FolderPurpose::LocateLinkedRepo => "Locate the linked repository",
        FolderPurpose::ProjectFolder => crate::state::repo_scan::PICK_TITLE,
        FolderPurpose::ProfileFolder(_) => crate::state::profile_rules::PICK_FOLDER_TITLE,
    };
    fx.push(Effect::Os(OsEffect::PickFolder { title: title.into(), purpose }));
}

pub(super) fn folder_picked(st: &mut AppState, purpose: FolderPurpose, path: PathBuf, fx: &mut Vec<Effect>) {
    match purpose {
        FolderPurpose::OpenRepo => super::tabs::open(st, path, fx),
        FolderPurpose::WorktreeParent => super::worktrees::parent_picked(st, path),
        FolderPurpose::LocateLinkedRepo => super::deep_link::located(st, path, fx),
        FolderPurpose::ProjectFolder => super::repo_scan::folder_picked(st, path, fx),
        FolderPurpose::ProfileFolder(id) => super::profile_rules::folder_picked(st, id, path, fx),
        FolderPurpose::InitRepo => {
            if let Some(Modal::Init(form)) = st.modal.as_mut() {
                form.folder = glowfly_core::native_path::shown(&path);
            }
            super::init::check_folder(st, Duration::ZERO, fx);
        }
        FolderPurpose::CloneParent => {
            if let Some(Modal::Clone(form)) = st.modal.as_mut() {
                form.parent = path.display().to_string();
            }
            super::clone::check_parent(st, Duration::ZERO, fx);
        }
        // Only fills the field: the Save / Revert bar writes it.
        FolderPurpose::HooksPath => {
            if let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) {
                git.hooks_path = path.display().to_string();
            }
        }
    }
}
