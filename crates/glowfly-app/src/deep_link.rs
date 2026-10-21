//! Messages and effects of `glowfly://` links: copying a link to a branch, commit, tag or the
//! repository, opening one (from the command line, a second Glowfly process or a toast), the
//! dialogs a link may need, and registering the scheme with the OS (state/deep_link.rs,
//! update/deep_link.rs, update/deep_link_open.rs, runner/deep_link.rs, ui/deep_link_dialog.rs,
//! ui/prefs/links.rs).

use git2::Oid;
use glowfly_core::link::find::Found;
use glowfly_core::link::{self, Link};
use glowfly_core::url_scheme::SchemeStatus;
use std::path::PathBuf;

/// What a Copy link item points at, as the menus know it.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkTarget {
    /// The repository, with the remote whose URL goes in the link (None: the preferred one).
    Repository {
        remote: Option<String>,
    },
    LocalBranch(String),
    RemoteBranch {
        remote: String,
        name: String,
    },
    Commit(Oid),
    Tag(String),
}

/// Where a search for the linked repository came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FindOrigin {
    /// The open tabs, favorites and recent repositories.
    Search,
    /// The folder the user located on disk.
    Located,
    /// The repository again, after the Fetch the missing-target toast offered.
    AfterFetch,
    /// The repository just cloned from the link's URL.
    AfterClone,
}

#[derive(Debug)]
pub enum DeepLinkMsg {
    /// A Copy link item or palette entry.
    Copy(LinkTarget),
    /// From the runner: the link to copy (`what` names it for the toast), or why there is none.
    Built {
        what: String,
        result: Result<String, String>,
    },
    /// A link to open (the command line, another Glowfly process, a toast's Open link).
    Open(String),
    /// From the runner: the copies of the linked repository among the candidates, for search
    /// `seq`.
    Found {
        seq: u64,
        origin: FindOrigin,
        result: Result<Vec<Found>, String>,
    },
    /// The Repository not found dialog's Locate on disk….
    Locate,
    /// The Repository not found dialog's Clone….
    Clone,
    /// The Choose a repository dialog's radio row `i`.
    Choose(usize),
    /// The Choose a repository dialog's Open.
    OpenChosen,
    /// The missing-target toast's Fetch.
    Fetch,
    /// Read which program opens links (Preferences opened).
    CheckScheme,
    Register,
    Unregister,
    /// From the runner: which program opens links now.
    SchemeChecked(Result<SchemeStatus, String>),
}

impl DeepLinkMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. Link effects are unscoped, so their results never arrive scoped.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Copy(_) | Self::Built { .. } | Self::Open(_) | Self::Found { .. } => None,
            Self::Locate | Self::Clone | Self::Choose(_) | Self::OpenChosen | Self::Fetch => None,
            Self::CheckScheme | Self::Register | Self::Unregister | Self::SchemeChecked(_) => None,
        }
    }
}

/// Work for links outside the UI thread (runner/deep_link.rs). All unscoped: the clipboard and
/// a search answer whatever tab is active.
#[derive(Debug)]
pub enum DeepLinkEffect {
    /// Works out `workdir`'s root commit and builds the link.
    Build {
        workdir: PathBuf,
        url: Option<String>,
        target: link::Target,
        what: String,
    },
    /// Checks each candidate folder for the linked repository (libgit2 reads only).
    Find {
        link: Link,
        candidates: Vec<PathBuf>,
        seq: u64,
        origin: FindOrigin,
    },
    SchemeStatus,
    Register,
    Unregister,
}

impl DeepLinkEffect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Build { .. } => "build-link",
            Self::Find { .. } => "find-linked-repo",
            Self::SchemeStatus => "link-scheme-status",
            Self::Register => "register-link-scheme",
            Self::Unregister => "unregister-link-scheme",
        }
    }

    /// None for every variant (see the enum).
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            Self::Build { .. } | Self::Find { .. } | Self::SchemeStatus | Self::Register | Self::Unregister => None,
        }
    }
}
