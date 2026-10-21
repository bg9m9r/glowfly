//! The command lines of the journal's Restore, run through gitcli. The reflog reason of a moved
//! branch is fixed English text, like the auto-stash messages: git keeps it in the repository.

use crate::gitcli::args;
use git2::Oid;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Moves branch `name` (not checked out anywhere) from `from` to `to`; git's old-value check
/// refuses it when the branch moved meanwhile.
pub fn move_branch(name: &str, to: Oid, from: Oid) -> Vec<String> {
    let reason = format!("Glowfly: restore {name} to {}", short(to));
    args(&["update-ref", "-m", &reason, &format!("refs/heads/{name}"), &to.to_string(), &from.to_string()])
}

/// Moves the checked-out branch, the index and the working tree to `oid`.
pub fn reset_to(oid: Oid) -> Vec<String> {
    args(&["reset", "--hard", &oid.to_string()])
}

/// Checks out branch `branch`.
pub fn checkout_branch(branch: &str) -> Vec<String> {
    args(&["checkout", branch, "--"])
}

/// Detaches HEAD at `oid`.
pub fn checkout_detached(oid: Oid) -> Vec<String> {
    args(&["checkout", "--detach", &oid.to_string(), "--"])
}

/// Makes branch `name` again at `oid`.
pub fn create_branch(name: &str, oid: Oid) -> Vec<String> {
    args(&["branch", name, &oid.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines() {
        let to = Oid::from_str("1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b").unwrap();
        let from = Oid::from_str("9f8e7d6c5b4a9f8e7d6c5b4a9f8e7d6c5b4a9f8e").unwrap();
        let (t, f) = (to.to_string(), from.to_string());
        assert_eq!(
            move_branch("feature/x", to, from),
            args(&["update-ref", "-m", "Glowfly: restore feature/x to 1a2b3c4", "refs/heads/feature/x", &t, &f])
        );
        assert_eq!(reset_to(to), args(&["reset", "--hard", &t]));
        assert_eq!(checkout_branch("main"), args(&["checkout", "main", "--"]));
        assert_eq!(checkout_detached(to), args(&["checkout", "--detach", &t, "--"]));
        assert_eq!(create_branch("old", to), args(&["branch", "old", &t]));
    }
}
