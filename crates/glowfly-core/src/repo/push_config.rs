//! Where git pushes a branch when no remote is named (`PushRemotes`): `branch.<name>.pushRemote`,
//! then `remote.pushDefault`. Read with libgit2 from the repository's whole config (system,
//! global, local and the files they include), the last value of a key winning as in git. Only
//! remote names are read; whether the remote exists is up to the caller.

use git2::Repository;

/// The configured push remotes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PushRemotes {
    /// `remote.pushDefault`.
    pub default: Option<String>,
    /// (branch, `branch.<branch>.pushRemote`), by branch name.
    pub branches: Vec<(String, String)>,
}

impl PushRemotes {
    /// `branch.<branch>.pushRemote`.
    pub fn of_branch(&self, branch: &str) -> Option<&str> {
        self.branches.iter().find(|(b, _)| b == branch).map(|(_, r)| r.as_str())
    }

    /// The remote `git push` picks for `branch` from config, with the key it came from:
    /// `branch.<branch>.pushRemote`, else `remote.pushDefault`.
    pub fn for_branch(&self, branch: &str) -> Option<(String, &str)> {
        match self.of_branch(branch) {
            Some(r) => Some((format!("branch.{branch}.pushRemote"), r)),
            None => self.default.as_deref().map(|r| ("remote.pushDefault".to_owned(), r)),
        }
    }
}

/// Reads the push remotes from `repo`'s config; none when it can't be read. Empty values are
/// left out (git treats them as unset).
pub fn push_remotes(repo: &Repository) -> PushRemotes {
    let read = || -> Result<PushRemotes, git2::Error> {
        let cfg = repo.config()?.snapshot()?;
        let mut out = PushRemotes::default();
        let mut entries = cfg.entries(Some(r"^(branch\..*\.pushremote|remote\.pushdefault)$"))?;
        while let Some(e) = entries.next() {
            let e = e?;
            let (Some(name), true) = (e.name(), e.has_value()) else { continue };
            let value = String::from_utf8_lossy(e.value_bytes()).trim().to_owned();
            let set = (!value.is_empty()).then_some(value);
            if name.eq_ignore_ascii_case("remote.pushdefault") {
                out.default = set;
                continue;
            }
            // `branch.<name>.pushremote`: the branch name keeps its case and may hold dots.
            let Some(branch) = name.get(7..name.len().saturating_sub(11)).filter(|b| !b.is_empty()) else { continue };
            out.branches.retain(|(b, _)| b != branch);
            if let Some(r) = set {
                out.branches.push((branch.to_owned(), r));
            }
        }
        Ok(out)
    };
    read().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn branch_push_remote_wins_over_push_default() {
        let dir = scratch_repo("push-remotes");
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(push_remotes(&repo), PushRemotes::default(), "unset");
        set_config(&dir, &[("remote.pushDefault", "mine"), ("branch.feat.x.pushRemote", "fork"), ("branch.Topic.pushremote", "other")]);
        let p = push_remotes(&Repository::open(&dir).unwrap());
        assert_eq!(p.for_branch("feat.x"), Some(("branch.feat.x.pushRemote".to_owned(), "fork")), "dots in the name");
        assert_eq!(p.of_branch("Topic"), Some("other"), "the name keeps its case");
        assert_eq!(p.for_branch("main"), Some(("remote.pushDefault".to_owned(), "mine")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_through_includes_with_the_last_value_winning() {
        let dir = scratch_repo("push-remotes-include");
        std::fs::write(dir.join(".git/push.cfg"), "[remote]\n\tpushDefault = included\n[branch \"main\"]\n\tpushRemote = fork\n").unwrap();
        set_config(&dir, &[("remote.pushDefault", "first"), ("include.path", "push.cfg")]);
        let p = push_remotes(&Repository::open(&dir).unwrap());
        assert_eq!((p.default.as_deref(), p.of_branch("main")), (Some("included"), Some("fork")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
