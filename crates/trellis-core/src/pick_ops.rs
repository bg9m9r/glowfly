//! Command lines for cherry-picking and reverting commits from the graph. The app runs them
//! through gitcli so hooks, config and commit signing apply.

use git2::Oid;

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
