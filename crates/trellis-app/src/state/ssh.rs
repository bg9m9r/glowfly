//! The SSH preferences page: the key pairs read in the background, the Generate New Key form,
//! the rules that decide whether Generate can run, and the texts its toasts and dialog show.

use crate::state::tabs::same_path;
use std::path::{Path, PathBuf};
use trellis_core::ssh_keys::{LocationProblem, SshListing, comment_problem, default_location, resolve_location};

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

    /// The location is one of the listed private keys.
    pub fn location_taken(&self) -> bool {
        let (Some(listing), Some(path)) = (self.listing.as_ref(), self.resolved()) else { return false };
        listing.keys.iter().any(|k| same_path(&k.private, &path))
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
