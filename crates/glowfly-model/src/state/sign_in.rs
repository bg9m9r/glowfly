//! Remotes that auto-fetch skips because they need the user to sign in (or to confirm an SSH
//! host key), and the fetch job running now, whose result can pause or resume them. Global and
//! in memory only: keyed by workdir and remote name, across tabs.

use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::RemoteInfo;
use std::path::{Path, PathBuf};

/// A remote whose auto-fetch failed for want of a sign-in.
#[derive(Clone, Debug, PartialEq)]
pub struct SignInNeeded {
    pub workdir: PathBuf,
    pub remote: String,
    /// The fetch URL's host, for the banner.
    pub host: Option<String>,
    pub need: AuthNeed,
    /// "Not now": the banner line is hidden; auto-fetch still skips the remote.
    pub dismissed: bool,
}

/// The fetch job running now: its repository, the remotes it covers (name, fetch URL) and
/// whether it is the background auto-fetch.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchRun {
    pub workdir: PathBuf,
    pub remotes: Vec<(String, String)>,
    pub auto: bool,
}

#[derive(Default)]
pub struct SignInNotices {
    pub paused: Vec<SignInNeeded>,
    pub fetching: Option<FetchRun>,
}

impl SignInNotices {
    pub fn is_paused(&self, workdir: &Path, remote: &str) -> bool {
        self.paused.iter().any(|p| p.remote == remote && same_spelling(&p.workdir, workdir))
    }

    /// Pauses a remote; false when it already was.
    pub fn pause(&mut self, n: SignInNeeded) -> bool {
        if self.is_paused(&n.workdir, &n.remote) {
            return false;
        }
        self.paused.push(n);
        true
    }

    /// Ends the pauses of `names` in `workdir`, returning the names that were paused.
    pub fn resume<'a>(&mut self, workdir: &Path, names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut resumed = Vec::new();
        for name in names {
            if self.is_paused(workdir, name) {
                self.paused.retain(|p| !(p.remote == name && same_spelling(&p.workdir, workdir)));
                resumed.push(name.to_owned());
            }
        }
        resumed
    }

    /// "Not now": hides the remote's banner line.
    pub fn dismiss(&mut self, workdir: &Path, remote: &str) {
        for p in self.paused.iter_mut().filter(|p| p.remote == remote && same_spelling(&p.workdir, workdir)) {
            p.dismissed = true;
        }
    }

    /// The banner lines for `workdir`: paused, not dismissed, and still configured in `remotes`.
    pub fn shown<'a>(&'a self, workdir: &Path, remotes: &[RemoteInfo]) -> Vec<&'a SignInNeeded> {
        self.paused
            .iter()
            .filter(|p| !p.dismissed && same_spelling(&p.workdir, workdir) && remotes.iter().any(|r| r.name == p.remote))
            .collect()
    }
}

/// The remote, with its host when known.
fn who(n: &SignInNeeded) -> String {
    match &n.host {
        Some(h) => format!("{} ({h})", n.remote),
        None => n.remote.clone(),
    }
}

/// The banner's text for one paused remote.
pub fn banner_line(n: &SignInNeeded) -> String {
    let why = match n.need {
        AuthNeed::HostKey => ": its SSH host key needs confirming.",
        AuthNeed::Https | AuthNeed::Ssh => " needs you to sign in.",
    };
    format!("{}{why} Auto-fetch skips it until a fetch works.", who(n))
}

/// The activity log's line when a remote is paused.
pub fn log_line(n: &SignInNeeded) -> String {
    let why = match n.need {
        AuthNeed::HostKey => "its SSH host key needs confirming",
        AuthNeed::Https | AuthNeed::Ssh => "it needs you to sign in",
    };
    format!("Auto-fetch skipped {}: {why}.", n.remote)
}
