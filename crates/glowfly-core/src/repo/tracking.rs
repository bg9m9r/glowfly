//! A branch's configured upstream (`Tracking`), as `BranchInfo` holds it.

/// A branch's configured upstream (`branch.<name>.remote` and `.merge`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tracking {
    /// The remote name, or "." when the upstream is a local branch.
    pub remote: String,
    /// The upstream branch name on that remote, without refs/heads/.
    pub merge: String,
}

impl Tracking {
    /// Whether the upstream is on a remote rather than a local branch.
    pub fn is_remote(&self) -> bool {
        self.remote != "."
    }

    /// "remote/merge", or just the branch name for a local upstream.
    pub fn display(&self) -> String {
        if self.is_remote() { format!("{}/{}", self.remote, self.merge) } else { self.merge.clone() }
    }
}
