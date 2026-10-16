//! Effects of the Clone dialog that need more than a git command: a sparse clone and listing the
//! user's repositories on a connected service. Both are unscoped: the clone ends the active job
//! and the list answers the dialog whichever tab is active.

use std::path::PathBuf;
use trellis_core::forge::Account;

#[derive(Debug, PartialEq)]
pub enum CloneEffect {
    /// Runs the clone `cmd` (with `--sparse`) in `parent`, then sets `folders` (cone mode) in the
    /// new repository `dest`. Reported as `CloneMsg::Finished`.
    Sparse { parent: PathBuf, cmd: Vec<String>, dest: PathBuf, folders: Vec<String> },
    /// The repositories the user can reach on `account`, signing in from `cwd`. Reported as
    /// `CloneMsg::ReposLoaded`.
    ListRepos { account: Account, cwd: PathBuf },
}

impl CloneEffect {
    pub fn name(&self) -> &'static str {
        match self {
            CloneEffect::Sparse { .. } => "sparse-clone",
            CloneEffect::ListRepos { .. } => "list-my-repos",
        }
    }
}
