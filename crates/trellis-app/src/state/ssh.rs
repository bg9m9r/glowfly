//! The SSH preferences page: the key pairs read in the background, the Generate New Key form,
//! the rules that decide whether Generate can run, and the texts its toasts and dialog show.
//! The key git uses and the agent are in `ssh_use`, adding a key to a service in `ssh_service`.

use super::ssh_service::UploadForm;
use super::ssh_use::{Choices, RepoTarget};
use std::path::{Path, PathBuf};
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{AgentStatus, LocationProblem, SshKeyPair, SshListing, comment_problem, default_location, resolve_location};

pub const COPIED: &str = "Public key copied to the clipboard";
pub const REPLACE_TITLE: &str = "Replace SSH key";
pub const REPLACE_BUTTON: &str = "Replace key";
pub const MISMATCH: &str = "The passphrases don't match.";
pub const TAKEN: &str = "A key already exists here; Generate will ask before replacing it.";
pub const NO_KEYGEN: &str = "ssh-keygen wasn't found, so fingerprints aren't shown and keys can't be generated. Install OpenSSH (or Git \
                             for Windows) and reload.";

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

    /// The first reason Generate can't run now.
    pub fn blocked(&self) -> Option<String> {
        if self.generating {
            return Some("A key is being generated.".into());
        }
        let Some(listing) = &self.listing else { return Some("Wait until your .ssh folder has been read.".into()) };
        if listing.keygen.is_none() {
            return Some("ssh-keygen wasn't found.".into());
        }
        if let Some(p) = self.location_problem() {
            return Some(p.message().into());
        }
        if let Some(p) = comment_problem(&self.comment) {
            return Some(p.into());
        }
        self.passphrase_mismatch().then(|| MISMATCH.into())
    }

    pub fn can_generate(&self) -> bool {
        self.blocked().is_none()
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

/// The Replace confirmation's text.
pub fn replace_body(path: &Path) -> String {
    format!(
        "{} already exists. Replacing it deletes the old private and public key; servers and hosting services that trust the old key \
         will stop accepting it.",
        path.display()
    )
}
