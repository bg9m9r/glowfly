//! Which left-panel sections are expanded.

/// Each left-panel section's open state; views toggle them directly.
pub struct Sections {
    pub local: bool,
    pub remote: bool,
    pub prs: bool,
    pub issues: bool,
    pub tags: bool,
    pub stashes: bool,
    pub submodules: bool,
    pub worktrees: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self { local: true, remote: true, prs: true, issues: true, tags: false, stashes: true, submodules: true, worktrees: true }
    }
}
