//! Messages of the SUBMODULES section (state/submodules.rs, update/submodules.rs,
//! ui/submodules.rs).

use trellis_core::repo::SubmoduleInfo;

#[derive(Clone, Debug, PartialEq)]
pub enum SubmoduleMsg {
    /// Open the initialized submodule at `path` (repo-relative) as a repository in a tab.
    Open(String),
    /// `git submodule update --init -- <path>` as a job; out of sync asks first (`confirmed: false`).
    Update { path: String, confirmed: bool },
    /// The submodule at `path` read again before asking to update it (`Effect::CheckSubmodule`);
    /// None when it is gone.
    Checked { path: String, result: Result<Option<SubmoduleInfo>, String> },
}
