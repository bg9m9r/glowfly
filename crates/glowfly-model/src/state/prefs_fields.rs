//! What Preferences' pages write besides the settings (`PrefsFields`): the search, the git
//! config values, the sign-in form, Git LFS's new pattern, Git Flow's form, the sparse paths,
//! the SSH key's upload title, the profile fields and forms and the Keyboard and Trusted repositories filters. The view
//! draws them from a copy each frame and sends a changed copy back as `EditMsg::Prefs`; update
//! takes it back (`PrefsState::apply`), clearing the error a page shows for a field that
//! changed. The secrets are in `ViewState::secrets`, and Generate SSH key's fields in
//! `ssh::KeygenFields`: that dialog draws in the same frame as the page, so each edit covers
//! fields the other doesn't.

use super::prefs::{GitConfigValues, PrefsState};
use super::profiles::{AddForm, RenameForm};
use glowfly_core::gitflow::GitFlowSettings;
use glowfly_core::sparse_ops::SparseMode;

/// Preferences' own fields, grouped by the page that writes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrefsFields {
    /// The navigation's search field, and whether it takes the keyboard on the next frame.
    pub search: String,
    pub focus_search: bool,
    /// General's Git section, Signing, Profiles (read), This repository's Identity and Hooks;
    /// None until read.
    pub git: Option<GitConfigValues>,
    /// Integrations' sign-in form and the tracker page's Jira project.
    pub int_host: String,
    pub int_user: String,
    pub jira_project: String,
    /// Git LFS's new pattern field.
    pub lfs_pattern: String,
    /// Git Flow's branch and prefix fields, and the branch Finish acts on.
    pub gitflow: GitFlowSettings,
    pub finish_branch: String,
    /// Sparse checkout's paths and mode.
    pub sparse_paths: String,
    pub sparse_mode: SparseMode,
    /// The SSH key's Add to service form's title, while that form is open.
    pub ssh_upload_title: Option<String>,
    pub profile: ProfileFields,
    /// Keyboard's filter field.
    pub keys_filter: String,
    /// Trusted repositories' filter field.
    pub trust_filter: String,
}

/// Profiles' identity fields and its open rename and New profile forms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileFields {
    pub author_name: String,
    pub author_email: String,
    pub signing_key: String,
    pub renaming: Option<RenameForm>,
    pub adding: Option<AddForm>,
}

impl PrefsState {
    /// The copy the pages draw from.
    pub fn fields(&self) -> PrefsFields {
        let pr = &self.profiles;
        PrefsFields {
            search: self.search.clone(),
            focus_search: self.focus_search,
            git: self.git.clone(),
            int_host: self.int_host.clone(),
            int_user: self.int_user.clone(),
            jira_project: self.jira_project.clone(),
            lfs_pattern: self.lfs.new_pattern.clone(),
            gitflow: self.gitflow.form.clone(),
            finish_branch: self.gitflow.finish_branch.clone(),
            sparse_paths: self.sparse.paths.clone(),
            sparse_mode: self.sparse.mode,
            ssh_upload_title: self.ssh.upload.as_ref().map(|u| u.title.clone()),
            profile: ProfileFields {
                author_name: pr.author_name.clone(),
                author_email: pr.author_email.clone(),
                signing_key: pr.signing_key.clone(),
                renaming: pr.renaming.clone(),
                adding: pr.adding.clone(),
            },
            keys_filter: self.keys.filter.clone(),
            trust_filter: self.trust.filter.clone(),
        }
    }

    /// Takes back what the pages wrote. The git values replace only values that were read; a
    /// changed LFS pattern, Git Flow form or sparse paths or mode clears that page's error; the
    /// upload title lands only while its form is open.
    pub fn apply(&mut self, f: PrefsFields) {
        self.search = f.search;
        self.focus_search = f.focus_search;
        if let (Some(git), Some(edited)) = (self.git.as_mut(), f.git) {
            *git = edited;
        }
        self.int_host = f.int_host;
        self.int_user = f.int_user;
        self.jira_project = f.jira_project;
        if self.lfs.new_pattern != f.lfs_pattern {
            self.lfs.new_pattern = f.lfs_pattern;
            self.lfs.pattern_error = None;
        }
        if self.gitflow.form != f.gitflow {
            self.gitflow.form = f.gitflow;
            self.gitflow.error = None;
        }
        self.gitflow.finish_branch = f.finish_branch;
        if self.sparse.paths != f.sparse_paths || self.sparse.mode != f.sparse_mode {
            self.sparse.paths = f.sparse_paths;
            self.sparse.mode = f.sparse_mode;
            self.sparse.paths_error = None;
        }
        if let (Some(upload), Some(title)) = (self.ssh.upload.as_mut(), f.ssh_upload_title) {
            upload.title = title;
        }
        let pr = &mut self.profiles;
        pr.author_name = f.profile.author_name;
        pr.author_email = f.profile.author_email;
        pr.signing_key = f.profile.signing_key;
        pr.renaming = f.profile.renaming;
        pr.adding = f.profile.adding;
        self.keys.filter = f.keys_filter;
        self.trust.filter = f.trust_filter;
    }
}
