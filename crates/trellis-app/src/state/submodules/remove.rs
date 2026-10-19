//! The Remove submodule confirmation: what it deletes, what would be lost (changes inside,
//! commits on no remote-tracking branch, stashes), why removing is unavailable, and the
//! messages for each way the removal can end.

use super::{count_commits, named_commits};
use git2::Oid;
use std::path::{Path, PathBuf};
use trellis_core::native_path::shown;
use trellis_core::repo::{Snapshot, SubmoduleInfo};
use trellis_core::submodule_ops::RemovalFacts;

/// The confirmation, filled from what the runner read just before it opened.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveForm {
    pub path: String,
    pub name: String,
    pub initialized: bool,
    /// The stored module, when it exists.
    pub module_dir: Option<PathBuf>,
    pub dirty: bool,
    /// Commits on no remote-tracking branch inside it, newest first.
    pub unpushed: Vec<(Oid, String)>,
    pub stashes: usize,
    /// The acknowledgement that the warnings' work is lost for good is ticked.
    pub acked: bool,
}

impl RemoveForm {
    pub fn from_facts(f: &RemovalFacts) -> Self {
        Self {
            path: f.info.path.clone(),
            name: f.info.name.clone(),
            initialized: f.info.is_initialized(),
            module_dir: f.module_dir.clone(),
            dirty: f.info.is_dirty(),
            unpushed: f.unpushed.clone(),
            stashes: f.stashes,
            acked: false,
        }
    }

    /// Whether the button waits for the acknowledgement: removing loses work for good.
    pub fn needs_ack(&self) -> bool {
        !self.warnings().is_empty()
    }

    /// The acknowledgement's label, naming what is lost: "Lose its uncommitted changes, 2 commits
    /// and 1 stash for good".
    pub fn ack_label(&self) -> Option<String> {
        let mut lost = Vec::new();
        if self.dirty {
            lost.push("its uncommitted changes".to_owned());
        }
        if !self.unpushed.is_empty() {
            lost.push(count_commits(self.unpushed.len()));
        }
        match self.stashes {
            0 => {}
            1 => lost.push("1 stash".to_owned()),
            n => lost.push(format!("{n} stashes")),
        }
        let last = lost.pop()?;
        let list = if lost.is_empty() { last } else { format!("{} and {last}", lost.join(", ")) };
        Some(format!("Lose {list} for good"))
    }

    /// The lines under "This deletes:".
    pub fn deleted_lines(&self) -> Vec<String> {
        let path = &self.path;
        let mut out = vec![
            if self.initialized {
                format!("the working folder {path} and everything in it")
            } else {
                format!("the empty folder {path} (not checked out)")
            },
            "its entry in .gitmodules and its gitlink in the index (both staged, so a commit records the removal)".to_owned(),
        ];
        if let Some(dir) = &self.module_dir {
            out.push(format!("the stored module {}, the submodule's own repository with all its history", shown(dir)));
        }
        out
    }

    /// What would be lost, one warning per kind; empty when nothing would be.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.dirty {
            out.push("It has uncommitted changes or untracked files; they are deleted.".to_owned());
        }
        if !self.unpushed.is_empty() {
            let (verb, them) = if self.unpushed.len() == 1 { ("is", "it") } else { ("are", "them") };
            out.push(format!(
                "{} in it {verb} on no remote-tracking branch; removing the submodule loses {them}:{}",
                count_commits(self.unpushed.len()),
                named_commits(&self.unpushed)
            ));
        }
        match self.stashes {
            0 => {}
            1 => out.push("It has 1 stash; it is deleted with the stored module.".to_owned()),
            n => out.push(format!("It has {n} stashes; they are deleted with the stored module.")),
        }
        out
    }
}

/// Why Remove submodule… is unavailable: git can't deinitialize a submodule without a
/// .gitmodules entry, and `git rm` refuses while .gitmodules has unstaged (or conflicted)
/// changes.
pub fn remove_blocked(sm: &SubmoduleInfo, snap: &Snapshot) -> Option<&'static str> {
    if sm.url.is_none() {
        Some(".gitmodules has no entry for it, so git can't deinitialize it.")
    } else if snap.unstaged.iter().any(|f| f.path == ".gitmodules") {
        Some("Stage or discard the changes to .gitmodules first.")
    } else {
        None
    }
}

/// The job's label, in the toast and the activity log.
pub fn job_label(path: &str) -> String {
    format!("Remove submodule {path}")
}

pub fn removed_headline(path: &str) -> String {
    format!("Removed submodule {path}. Commit to record the removal.")
}

pub fn rm_failed_headline(path: &str, last: &str) -> String {
    format!("Deinitialized {path}, but git rm failed: {last}. It is still in .gitmodules and the index.")
}

/// The headline when the git part worked but the stored module stays (`module_left_text`
/// follows it).
pub fn git_part_headline(path: &str) -> String {
    format!("Removed submodule {path} from the index and .gitmodules.")
}

pub fn module_left_text(path: &str, dir: &Path, error: &str) -> String {
    format!("Could not delete the stored module at {}: {error}. Delete that folder before adding a submodule at {path} again.", shown(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> RemoveForm {
        RemoveForm {
            path: "libs/lib".into(),
            name: "libs/lib".into(),
            initialized: true,
            module_dir: None,
            dirty: false,
            unpushed: Vec::new(),
            stashes: 0,
            acked: false,
        }
    }

    fn commits(n: usize) -> Vec<(Oid, String)> {
        (0..n).map(|i| (Oid::from_str(&format!("{:x}", i + 1).repeat(40)[..40]).unwrap(), format!("work {i}"))).collect()
    }

    #[test]
    fn lines_and_warnings() {
        let f = form();
        let lines = f.deleted_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("the working folder libs/lib") && lines[1].contains(".gitmodules"), "{lines:?}");
        assert!(f.warnings().is_empty());

        let dir = PathBuf::from("r").join(".git").join("modules").join("libs").join("lib");
        let f = RemoveForm { initialized: false, module_dir: Some(dir.clone()), dirty: true, unpushed: commits(7), stashes: 2, ..form() };
        let lines = f.deleted_lines();
        assert!(lines[0].contains("not checked out") && lines[2].contains(&shown(&dir)), "{lines:?}");
        let w = f.warnings();
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(w[1].starts_with("7 commits in it are on no remote-tracking branch"), "{}", w[1]);
        assert!(w[1].contains("\n    5555555 work 4") && w[1].ends_with("\n    and 2 more"), "{}", w[1]);
        assert_eq!(w[2], "It has 2 stashes; they are deleted with the stored module.");

        let one = RemoveForm { unpushed: commits(1), stashes: 1, ..form() };
        let w = one.warnings();
        assert!(w[0].starts_with("1 commit in it is on no remote-tracking branch; removing the submodule loses it:"), "{}", w[0]);
        assert_eq!(w[1], "It has 1 stash; it is deleted with the stored module.");
    }

    #[test]
    fn acknowledgement_names_what_is_lost() {
        assert!(!form().needs_ack());
        assert_eq!(form().ack_label(), None);
        let label = |f: RemoveForm| {
            assert!(f.needs_ack());
            f.ack_label().unwrap()
        };
        assert_eq!(label(RemoveForm { dirty: true, ..form() }), "Lose its uncommitted changes for good");
        assert_eq!(label(RemoveForm { unpushed: commits(1), ..form() }), "Lose 1 commit for good");
        assert_eq!(label(RemoveForm { stashes: 3, ..form() }), "Lose 3 stashes for good");
        let all = RemoveForm { dirty: true, unpushed: commits(2), stashes: 1, ..form() };
        assert_eq!(label(all), "Lose its uncommitted changes, 2 commits and 1 stash for good");
    }

    #[test]
    fn headlines() {
        assert_eq!(job_label("a"), "Remove submodule a");
        assert_eq!(removed_headline("a"), "Removed submodule a. Commit to record the removal.");
        assert!(rm_failed_headline("a", "boom").contains("git rm failed: boom"));
        assert!(module_left_text("a", Path::new("d"), "why").starts_with("Could not delete the stored module at d: why."));
    }
}
