//! Messages of the SUBMODULES section (state/submodules.rs, update/submodules.rs,
//! ui/submodules.rs).

#[derive(Clone, Debug, PartialEq)]
pub enum SubmoduleMsg {
    /// Open the initialized submodule at `path` (repo-relative) as a repository in a tab.
    Open(String),
    /// `git submodule update --init -- <path>` as a job; out of sync asks first (`confirmed: false`).
    Update { path: String, confirmed: bool },
}
