//! A profile's folder rules (Preferences > Profile > Folders): adding and removing them (a
//! folder belongs to one profile at most, and only a profile with an author email gets one),
//! and which rule git applies to a git dir, mirrored from git's `includeIf.gitdir` matching so
//! Trellis knows what to expect (the deepest folder wins, as the entries are written
//! shallow-first and git's last include wins). Pure: the rules were resolved on disk when added
//! (`profile_includes::check_folder`).

use super::{FolderRule, ProfileId, Settings};
use crate::paths;
use std::path::{Path, PathBuf};

/// Why a folder rule was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleProblem {
    Missing,
    /// The profile already has this folder.
    Duplicate,
    /// Another profile (named) already has this folder.
    Taken(String),
    /// The profile has no author email, so the rule would give no identity.
    NoEmail,
}

impl RuleProblem {
    pub fn message(&self) -> String {
        match self {
            RuleProblem::Missing => "That profile no longer exists.".into(),
            RuleProblem::Duplicate => "This profile already has this folder.".into(),
            RuleProblem::Taken(name) => format!("The profile {name} already has this folder."),
            RuleProblem::NoEmail => "Set an author email for this profile first.".into(),
        }
    }
}

impl Settings {
    /// Adds `rule` to the profile `id`, unless a profile already has its folder (by spelling;
    /// both were resolved when added) or the profile has no author email.
    pub fn add_folder_rule(&mut self, id: ProfileId, rule: FolderRule) -> Result<(), RuleProblem> {
        let Some(profile) = self.profile(id) else { return Err(RuleProblem::Missing) };
        if profile.author_email.trim().is_empty() {
            return Err(RuleProblem::NoEmail);
        }
        let path = paths::decode(&rule.path);
        for p in &self.profiles {
            if p.folders.iter().any(|r| paths::same_spelling(&paths::decode(&r.path), &path)) {
                return Err(if p.id == id { RuleProblem::Duplicate } else { RuleProblem::Taken(p.name.clone()) });
            }
        }
        if let Some(p) = self.profile_mut(id) {
            p.folders.push(rule);
        }
        Ok(())
    }

    /// Removes the profile's rule for the folder spelled `path`; returns whether it had one.
    pub fn remove_folder_rule(&mut self, id: ProfileId, path: &str) -> bool {
        let Some(p) = self.profile_mut(id) else { return false };
        let path = paths::decode(path);
        let before = p.folders.len();
        p.folders.retain(|r| !paths::same_spelling(&paths::decode(&r.path), &path));
        p.folders.len() != before
    }

    pub fn has_folder_rules(&self) -> bool {
        self.profiles.iter().any(|p| !p.folders.is_empty())
    }

    /// Every profile's rules, profile by profile.
    pub fn rules(&self) -> Vec<(ProfileId, FolderRule)> {
        self.profiles.iter().flat_map(|p| p.folders.iter().map(move |r| (p.id, r.clone()))).collect()
    }
}

/// Whether git applies `rule` to a repository whose git dir is `git_dir`: the git dir is the
/// rule's folder or inside it, as `gitdir:<folder>/` matches. An `ignore_case` rule (`gitdir/i`)
/// compares ASCII letters without case, as git does; mirroring git is the one place Trellis
/// folds case itself.
pub fn rule_matches(rule: &FolderRule, git_dir: &Path) -> bool {
    let folder = paths::decode(&rule.path);
    if paths::is_within(git_dir, &folder) {
        return true;
    }
    let lower = |p: &Path| PathBuf::from(p.to_string_lossy().to_ascii_lowercase());
    rule.ignore_case && paths::is_within(&lower(git_dir), &lower(&folder))
}

/// The rule git applies to `git_dir`: the deepest matching folder (most path parts), then the
/// last by text, which is the order `profile_includes::plan` writes the entries in.
pub fn rule_for_git_dir<'a>(rules: &'a [(ProfileId, FolderRule)], git_dir: &Path) -> Option<&'a (ProfileId, FolderRule)> {
    rules.iter().filter(|(_, r)| rule_matches(r, git_dir)).max_by(|(_, a), (_, b)| order_key(a).cmp(&order_key(b)))
}

/// The profile whose rule git applies to `git_dir` (see `rule_for_git_dir`).
pub fn profile_for_git_dir(rules: &[(ProfileId, FolderRule)], git_dir: &Path) -> Option<ProfileId> {
    rule_for_git_dir(rules, git_dir).map(|(id, _)| *id)
}

/// Where a rule's entry goes among the others (`profile_includes::plan`): shallow first, then by
/// path text.
fn order_key(rule: &FolderRule) -> (usize, String) {
    (paths::decode(&rule.path).components().count(), rule.path.clone())
}

/// The folder a rule shows: a repository rule's common git dir named `.git` shows the
/// repository's folder.
pub fn shown_folder(rule: &FolderRule) -> PathBuf {
    let path = paths::decode(&rule.path);
    if rule.repository && path.file_name().is_some_and(|n| n == ".git") {
        return path.parent().map(Path::to_path_buf).unwrap_or(path);
    }
    path
}

#[cfg(test)]
#[path = "profile_rules_tests.rs"]
mod tests;
