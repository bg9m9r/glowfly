//! Command lines for cherry-picking and reverting commits from the graph. The app runs them
//! through gitcli so hooks, config and commit signing apply. Carrying a run past the commits
//! that change nothing is in `pick_empty.rs`.

use git2::Oid;

#[path = "pick_empty.rs"]
mod pick_empty;
pub use pick_empty::*;

/// Which of the two commands a job runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Picking {
    #[default]
    CherryPick,
    Revert,
}

impl Picking {
    /// Skips the commit a stopped run is at and carries on with the rest.
    pub fn skip(self) -> &'static [&'static str] {
        match self {
            Picking::CherryPick => CHERRY_PICK_SKIP,
            Picking::Revert => REVERT_SKIP,
        }
    }

    /// Ends a stopped run, putting the branch back where it started.
    pub fn abort(self) -> &'static [&'static str] {
        match self {
            Picking::CherryPick => &["cherry-pick", "--abort"],
            Picking::Revert => &["revert", "--abort"],
        }
    }

    /// The ref git writes while a commit of the run waits to be committed.
    fn head_ref(self) -> &'static str {
        match self {
            Picking::CherryPick => "CHERRY_PICK_HEAD",
            Picking::Revert => "REVERT_HEAD",
        }
    }
}

/// Skips the commit a stopped cherry-pick sequence is at and carries on with the rest.
pub const CHERRY_PICK_SKIP: &[&str] = &["cherry-pick", "--skip"];

/// Skips the commit a stopped revert sequence is at and carries on with the rest.
pub const REVERT_SKIP: &[&str] = &["revert", "--skip"];

/// `cherry-pick [-m N] <oids…>`: applies each commit in turn, in the order given. `mainline`
/// picks a merge commit's changes relative to that parent (1-based).
pub fn cherry_pick_cmd(oids: &[Oid], mainline: Option<u32>) -> Vec<String> {
    with_commits(vec!["cherry-pick".to_owned()], oids, mainline)
}

/// `revert --no-edit [-m N] <oids…>`: one revert commit per commit, in the order given (newest
/// first undoes them cleanly), each with git's default message.
pub fn revert_cmd(oids: &[Oid], mainline: Option<u32>) -> Vec<String> {
    with_commits(vec!["revert".to_owned(), "--no-edit".to_owned()], oids, mainline)
}

fn with_commits(mut argv: Vec<String>, oids: &[Oid], mainline: Option<u32>) -> Vec<String> {
    if let Some(n) = mainline {
        argv.extend(["-m".to_owned(), n.to_string()]);
    }
    argv.extend(oids.iter().map(Oid::to_string));
    argv
}

#[cfg(test)]
#[path = "pick_ops_tests.rs"]
mod tests;
