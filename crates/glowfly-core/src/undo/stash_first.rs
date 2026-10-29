//! The stash "Stash changes and undo" makes first: its message carries a nonce for this one
//! run, so it is told apart (with its commit id) from an older one, or one another tab of the
//! same repository made meanwhile. Reading the stash list fails closed: when it can't be read,
//! nothing is stashed.

use super::UNDO_STASH_MESSAGE;
use crate::error::Result;
use crate::gitcli::args;
use git2::{ErrorCode, Oid, Repository};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

/// `UNDO_STASH_MESSAGE` with a nonce for one run ("Glowfly: changes before undo #1a2b3c4d").
pub fn undo_stash_message() -> String {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let n = nanos ^ std::process::id().rotate_left(16) ^ RUNS.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9e37_79b9);
    format!("{UNDO_STASH_MESSAGE} #{n:08x}")
}

/// The command that stashes local changes under `message` before "Stash changes and undo":
/// untracked files too only with `untracked` (an undo that puts the changes back afterwards
/// leaves them in place, so git refuses rather than write over one).
pub fn stash_first_command(message: &str, untracked: bool) -> Vec<String> {
    if untracked { args(&["stash", "push", "-u", "-m", message]) } else { args(&["stash", "push", "-m", message]) }
}

/// The newest stash in `workdir`'s list (`None`: the list is empty); an error when it can't be
/// read.
pub fn stash_top_at(workdir: &Path) -> Result<Option<Oid>> {
    let repo = Repository::open(workdir)?;
    match repo.find_reference("refs/stash") {
        Ok(r) => Ok(r.target()),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The stash `stash_first_command` just made with `message`, wherever it is in the list (another
/// tab may have stashed on top of it since): the one whose message ends with `message`, whose
/// nonce no other run has. `None` when git stashed nothing; an error when the list can't be read.
pub fn undo_stash_made(workdir: &Path, message: &str) -> Result<Option<Oid>> {
    let mut repo = Repository::open(workdir)?;
    let mut found = None;
    repo.stash_foreach(|_, m, id| {
        if m.lines().next().unwrap_or_default().trim_end().ends_with(message) {
            found = Some(*id);
        }
        found.is_none()
    })?;
    Ok(found)
}

/// The files the index marks intent-to-add (`git add -N`), which `git stash push` can't stash.
pub fn intent_to_add(workdir: &Path) -> Result<Vec<String>> {
    /// `GIT_INDEX_ENTRY_INTENT_TO_ADD`, in an entry's extended flags.
    const INTENT_TO_ADD: u16 = 1 << 13;
    let index = Repository::open(workdir)?.index()?;
    Ok(index.iter().filter(|e| e.flags_extended & INTENT_TO_ADD != 0).map(|e| String::from_utf8_lossy(&e.path).into_owned()).collect())
}

#[cfg(test)]
mod tests {
    use super::super::tests::commit;
    use super::*;
    use crate::testutil::*;

    #[test]
    fn each_run_has_its_own_message_and_finds_only_its_own_stash() {
        let (a, b) = (undo_stash_message(), undo_stash_message());
        assert!(a.starts_with(UNDO_STASH_MESSAGE) && b.starts_with(UNDO_STASH_MESSAGE));
        assert_ne!(a, b);
        assert_eq!(stash_first_command(&a, true), args(&["stash", "push", "-u", "-m", &a]));
        assert_eq!(stash_first_command(&a, false), args(&["stash", "push", "-m", &a]));

        let dir = scratch_repo("undo-stash-first");
        commit(&dir, "a.txt", "one\n", "c1");
        assert_eq!(stash_top_at(&dir).unwrap(), None);
        write(&dir, "a.txt", "two\n");
        git(&dir, &["stash", "push", "-q", "-m", &a]);
        let top = stash_top_at(&dir).unwrap();
        assert!(top.is_some());
        assert_eq!(undo_stash_made(&dir, &b).unwrap(), None, "another run's stash isn't ours");
        assert_eq!(undo_stash_made(&dir, &a).unwrap(), top);
        // Found under a newer stash too.
        write(&dir, "a.txt", "three\n");
        git(&dir, &["stash", "push", "-q", "-m", "another tab"]);
        assert_ne!(stash_top_at(&dir).unwrap(), top);
        assert_eq!(undo_stash_made(&dir, &a).unwrap(), top);

        write(&dir, "n.txt", "new\n");
        git(&dir, &["add", "-N", "n.txt"]);
        assert_eq!(intent_to_add(&dir).unwrap(), ["n.txt"]);

        // A list that can't be read is an error, not "empty".
        std::fs::write(dir.join(".git/refs/stash"), "not an id\n").unwrap();
        assert!(stash_top_at(&dir).is_err());
        assert!(undo_stash_made(&dir, &a).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
