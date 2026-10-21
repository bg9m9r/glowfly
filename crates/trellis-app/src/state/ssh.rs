//! The SSH preferences page: the key pairs read in the background, the Generate SSH key form
//! (drawn in its dialog, `Modal::SshKeygen`, with its fields kept here so closing the dialog
//! keeps them), the rules that decide whether Generate can run, what Use existing last refused,
//! and the texts its toasts and confirmation show. The key git uses and the agent are in
//! `ssh_use`, the marks on a key card, the agent-only keys and the agent pill in `ssh_marks`,
//! pointing git at Windows' OpenSSH in `ssh_windows`, adding a key to a service in
//! `ssh_service`.

use super::ssh_service::UploadForm;
use super::ssh_use::{Choices, RepoTarget};
use std::path::{Path, PathBuf};
use trellis_core::i18n::Lang;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{
    AgentStatus, KeyFileProblem, LocationProblem, SshKeyPair, SshListing, comment_problem, default_location, resolve_location,
};

pub const COPIED: &str = "Public key copied to the clipboard";
pub const REPLACE_TITLE: &str = "Replace SSH key";
pub const REPLACE_BUTTON: &str = "Replace key";

#[derive(Default)]
pub struct SshPage {
    /// None while ~/.ssh is read, or when reading it failed (`error`).
    pub listing: Option<SshListing>,
    pub error: Option<String>,
    /// Bumped by each read; an older read's result is ignored.
    pub generation: u64,
    /// The Save Location field (a view text buffer).
    pub location: String,
    /// The Comment field.
    pub comment: String,
    /// The comment was filled from user.email once; it isn't filled again.
    pub comment_prefilled: bool,
    pub passphrase: String,
    /// The Confirm Passphrase field.
    pub confirm: String,
    /// Which of the dialog's fields were edited, and whether a submit was tried.
    pub touched: super::touched::Touched,
    /// ssh-keygen is running.
    pub generating: bool,
    /// Why the last Generate was refused, or what ssh-keygen said when it failed.
    pub form_error: Option<String>,
    /// core.sshCommand at each level; None while it is read.
    pub choice: Option<Choices>,
    /// Bumped by each read of the choice; an older read's result is ignored.
    pub choice_generation: u64,
    /// Why the last Use for git or Stop using was refused, or what git said when it failed.
    pub choice_error: Option<String>,
    /// A core.sshCommand write is running.
    pub choice_busy: bool,
    /// The key the running write chooses (None: it unsets the choice).
    pub choice_key: Option<PathBuf>,
    /// The level the running write changes: the repository's (true) or all repositories'.
    pub choice_repo: bool,
    /// The repository open when the page was read, for its own core.sshCommand.
    pub repo: Option<RepoTarget>,
    /// What the last agent check found; None while it runs.
    pub agent: Option<AgentStatus>,
    pub agent_generation: u64,
    /// The key ssh-add is adding.
    pub agent_adding: Option<PathBuf>,
    /// Why the last Add to agent was refused, or what ssh-add said when it failed.
    pub agent_error: Option<String>,
    /// The Add to service form, under the key it is for.
    pub upload: Option<UploadForm>,
    /// Why the last file picked with Use existing isn't a key pair, with its file name.
    pub pick_error: Option<(String, KeyFileProblem)>,
    /// Why the last Use Windows' OpenSSH (or its undo) was refused, or what git said when it
    /// failed.
    pub windows_error: Option<String>,
}

impl SshPage {
    /// What is wrong with the typed location; None until the listing (with the home folder)
    /// arrives.
    pub fn location_problem(&self) -> Option<LocationProblem> {
        let listing = self.listing.as_ref()?;
        resolve_location(&self.location, &listing.home).err()
    }

    /// The private key path Generate would write, if the location is usable.
    pub fn resolved(&self) -> Option<PathBuf> {
        let listing = self.listing.as_ref()?;
        resolve_location(&self.location, &listing.home).ok()
    }

    /// The listed pair whose private key is `private`.
    pub fn key(&self, private: &Path) -> Option<&SshKeyPair> {
        self.listing.as_ref()?.keys.iter().find(|k| same_spelling(&k.private, private))
    }

    /// The location is one of the listed private keys.
    pub fn location_taken(&self) -> bool {
        let (Some(listing), Some(path)) = (self.listing.as_ref(), self.resolved()) else { return false };
        listing.keys.iter().any(|k| same_spelling(&k.private, &path))
    }

    pub fn passphrase_mismatch(&self) -> bool {
        self.passphrase != self.confirm
    }

    /// Why the Generate… button can't open the dialog now, in `t`.
    pub fn open_blocked(&self, t: Lang) -> Option<&'static str> {
        let Some(listing) = &self.listing else { return Some(t.tr("prefs.ssh.generate.reading.warn")) };
        listing.keygen.is_none().then(|| t.tr("prefs.ssh.generate.no_keygen.warn"))
    }

    /// The part of `blocked` that isn't about a field: a key is being generated, or the dialog
    /// couldn't open now (`open_blocked`). The dialog's footer shows only this.
    pub fn form_wait(&self, t: Lang) -> Option<&'static str> {
        if self.generating {
            return Some(t.tr("prefs.ssh.generate.busy.warn"));
        }
        self.open_blocked(t)
    }

    /// The first reason Generate can't run now, in `t`.
    pub fn blocked(&self, t: Lang) -> Option<&'static str> {
        if let Some(wait) = self.form_wait(t) {
            return Some(wait);
        }
        if let Some(p) = self.location_problem() {
            return Some(location_text(t, p));
        }
        if let Some(p) = self.comment_problem(t) {
            return Some(p);
        }
        self.passphrase_mismatch().then(|| t.tr("prefs.ssh.generate.mismatch.warn"))
    }

    /// Why the typed comment can't be used, in `t`.
    pub fn comment_problem(&self, t: Lang) -> Option<&'static str> {
        comment_problem(&self.comment).map(|_| t.tr("prefs.ssh.generate.comment.control.warn"))
    }

    pub fn can_generate(&self) -> bool {
        // Only whether there is a reason matters, not its language.
        self.blocked(Lang::En).is_none()
    }

    /// Fills the comment with the user's email, once, unless something is typed already.
    pub fn prefill_comment(&mut self, email: &str) {
        if !self.comment_prefilled && self.comment.is_empty() && !email.is_empty() {
            self.comment = email.to_owned();
            self.comment_prefilled = true;
        }
    }

    /// Fills an empty location with the default key path, once the home folder is known.
    pub fn fill_location(&mut self) {
        if let Some(listing) = &self.listing
            && self.location.is_empty()
        {
            self.location = default_location(&listing.home).display().to_string();
        }
    }
}

/// What is wrong with a typed save location, in `t`.
pub fn location_text(t: Lang, p: LocationProblem) -> &'static str {
    match p {
        LocationProblem::Empty => t.tr("prefs.ssh.generate.location.empty.warn"),
        LocationProblem::Relative => t.tr("prefs.ssh.generate.location.relative.warn"),
        LocationProblem::PublicFile => t.tr("prefs.ssh.generate.location.public.warn"),
        LocationProblem::NotAFile => t.tr("prefs.ssh.generate.location.folder.warn"),
        LocationProblem::WindowsName => t.tr("prefs.ssh.generate.location.windows_name.warn"),
    }
}

/// The Replace confirmation's text.
pub fn replace_body(path: &Path) -> String {
    format!(
        "{} already exists. Replacing it deletes the old private and public key; servers and hosting services that trust the old key \
         will stop accepting it.",
        path.display()
    )
}
