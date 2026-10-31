//! Preferences > Profile's Folders row and Open repository row: their texts, and each rule's
//! two lines. Pure.

use glowfly_core::paths;
use glowfly_core::settings::{FolderRule, Profile, shown_folder};
use std::path::Path;

pub const FOLDERS_LABEL: &str = "Folders";
pub const HERE_LABEL: &str = "Open repository";
const FOLDER_LINE: &str = "This folder and every repository in it";
pub const REMOVE_BUTTON: &str = "Remove";
pub const ADD_FOLDER_BUTTON: &str = "Add folder…";
pub const USE_REPO_BUTTON: &str = "Use for this repository";
pub const ALREADY_HERE: &str = "This repository already uses this profile";
const NEEDS_EMAIL: &str = "Set an author email for this profile first";
pub const NO_RULES: &str = "Not tied to any folder. Repositories in a folder you add here commit with this profile's identity, in Glowfly and on the command line.";
pub const RULES_HELP: &str = "Glowfly adds an includeIf entry for each folder to your global Git config, pointing at a file it writes for this profile. Your global identity stays as it is. An identity set in a repository's own config still comes first.";
pub const PICK_FOLDER_TITLE: &str = "Choose a folder for this profile";

/// A rule's path as shown, and its second line.
pub fn rule_line(rule: &FolderRule) -> (String, String) {
    let folder = shown_folder(rule);
    let second = if rule.repository {
        let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        format!("Repository {name}")
    } else {
        FOLDER_LINE.to_owned()
    };
    (glowfly_core::native_path::shown(&folder), second)
}

/// Whether `profile` already has a rule for the repository whose common git dir is `common`.
pub fn has_repository(profile: &Profile, common: &Path) -> bool {
    profile.folders.iter().any(|r| r.repository && paths::same_spelling(&paths::decode(&r.path), common))
}

/// Why the Add buttons are off for `profile`, if they are.
pub fn add_blocked(profile: &Profile) -> Option<&'static str> {
    profile.author_email.trim().is_empty().then_some(NEEDS_EMAIL)
}
