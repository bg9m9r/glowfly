//! Where git pushes a branch when no remote is named (`PushRemotes`): `branch.<name>.pushRemote`,
//! then `remote.pushDefault`, and what it pushes there by `push.default` (`PushMode`). Read with
//! libgit2 from the repository's whole config (system, global, local and the files they
//! include), the last value of a key winning as in git. Only remote names are read; whether the
//! remote exists is up to the caller (`push_target`). Also whether a plain `git pull` rebases
//! (`pull_rebases`), which the Pull button's tooltip names.

use git2::Repository;

/// `push.default`: what a plain `git push` sends. Its values are compared case-sensitively,
/// as git does; any other value (or none, in a key without one) is a config error git refuses.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PushMode {
    Nothing,
    Current,
    /// `upstream`, or its old name `tracking`.
    Upstream,
    #[default]
    Simple,
    Matching,
    /// A value git doesn't accept, as written.
    Invalid(String),
}

impl PushMode {
    /// The value as git spells it, for messages.
    pub fn config_value(&self) -> &str {
        match self {
            PushMode::Nothing => "nothing",
            PushMode::Current => "current",
            PushMode::Upstream => "upstream",
            PushMode::Simple => "simple",
            PushMode::Matching => "matching",
            PushMode::Invalid(v) => v,
        }
    }

    /// The mode a `push.default` value sets.
    fn of(value: &str) -> PushMode {
        match value {
            "nothing" => PushMode::Nothing,
            "current" => PushMode::Current,
            "upstream" | "tracking" => PushMode::Upstream,
            "simple" => PushMode::Simple,
            "matching" => PushMode::Matching,
            v => PushMode::Invalid(v.to_owned()),
        }
    }
}

/// The configured push remotes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PushRemotes {
    /// `remote.pushDefault`.
    pub default: Option<String>,
    /// (branch, `branch.<branch>.pushRemote`), by branch name.
    pub branches: Vec<(String, String)>,
    /// `push.default`, Simple when unset.
    pub mode: PushMode,
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

/// Reads the push remotes and `push.default` from `repo`'s config; none when it can't be read.
/// Empty remote names are left out (git treats them as unset).
pub fn push_remotes(repo: &Repository) -> PushRemotes {
    let read = || -> Result<PushRemotes, git2::Error> {
        let cfg = repo.config()?.snapshot()?;
        let mut out = PushRemotes::default();
        let mut entries = cfg.entries(Some(r"^(branch\..*\.pushremote|remote\.pushdefault|push\.default)$"))?;
        while let Some(e) = entries.next() {
            let e = e?;
            if e.name().is_some_and(|n| n.eq_ignore_ascii_case("push.default")) {
                // A value-less key has no value bytes to read (`value_bytes` would panic).
                out.mode = if e.has_value() {
                    PushMode::of(String::from_utf8_lossy(e.value_bytes()).trim())
                } else {
                    PushMode::Invalid(String::new())
                };
                continue;
            }
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

/// Whether a plain `git pull` on `branch` rebases: `branch.<branch>.rebase`, else
/// `pull.rebase`. As git reads them: a key with no value (`[pull] rebase`) is true; false, no,
/// off, an empty value and an integer 0 (`00` too) merge; any other value (true, a non-zero
/// integer, `merges`, `interactive`) rebases. Unset, or unreadable config, merges.
pub fn pull_rebases(repo: &Repository, branch: Option<&str>) -> bool {
    let Ok(cfg) = repo.config().and_then(|mut c| c.snapshot()) else { return false };
    let keys = branch.map(|b| format!("branch.{b}.rebase")).into_iter().chain(["pull.rebase".to_owned()]);
    for key in keys {
        if let Ok(e) = cfg.get_entry(&key) {
            if !e.has_value() {
                return true;
            }
            return rebase_value(&String::from_utf8_lossy(e.value_bytes()));
        }
    }
    false
}

/// Whether a `pull.rebase` value that is set rebases (see `pull_rebases`).
fn rebase_value(v: &str) -> bool {
    let v = v.trim().to_ascii_lowercase();
    match v.parse::<i64>() {
        Ok(n) => n != 0,
        Err(_) => !matches!(v.as_str(), "false" | "no" | "off" | ""),
    }
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
    fn push_default_reads_each_value() {
        let dir = scratch_repo("push-default");
        let read = || push_remotes(&Repository::open(&dir).unwrap()).mode;
        // Set here first, so the user's own global setting can't decide the rest.
        set_config(&dir, &[("push.default", "simple")]);
        assert_eq!(read(), PushMode::Simple);
        let modes = [
            ("nothing", PushMode::Nothing),
            ("current", PushMode::Current),
            ("upstream", PushMode::Upstream),
            ("tracking", PushMode::Upstream),
            ("matching", PushMode::Matching),
            ("Simple", PushMode::Invalid("Simple".into())),
            ("", PushMode::Invalid(String::new())),
        ];
        for (value, mode) in modes {
            set_config(&dir, &[("push.default", value)]);
            assert_eq!(read(), mode, "push.default = {value:?}");
        }
        assert_eq!(PushMode::Upstream.config_value(), "upstream");
        // A key without a value is an error to git.
        let local = dir.join(".git/config");
        let text = std::fs::read_to_string(&local).unwrap();
        std::fs::write(&local, format!("{text}\n[push]\n\tdefault\n")).unwrap();
        assert_eq!(read(), PushMode::Invalid(String::new()), "value-less");
        // The last value wins, through an include too.
        std::fs::write(dir.join(".git/push-mode.cfg"), "[push]\n\tdefault = current\n").unwrap();
        set_config(&dir, &[("include.path", "push-mode.cfg")]);
        assert_eq!(read(), PushMode::Current, "the included value comes last");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unset_push_default_is_simple() {
        assert_eq!(PushRemotes::default().mode, PushMode::Simple);
    }

    #[test]
    fn pull_rebases_follows_the_branch_then_pull_rebase() {
        let dir = scratch_repo("pull-rebases");
        let open = || Repository::open(&dir).unwrap();
        // Set here, so the user's own global setting can't decide it.
        set_config(&dir, &[("pull.rebase", "false")]);
        assert!(!pull_rebases(&open(), Some("main")), "false merges");
        set_config(&dir, &[("pull.rebase", "true")]);
        assert!(pull_rebases(&open(), Some("main")));
        assert!(pull_rebases(&open(), None), "a detached HEAD reads pull.rebase");
        set_config(&dir, &[("branch.main.rebase", "false")]);
        assert!(!pull_rebases(&open(), Some("main")), "the branch's own setting wins");
        set_config(&dir, &[("pull.rebase", "merges")]);
        assert!(pull_rebases(&open(), Some("other")), "merges rebases");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pull_rebases_reads_values_as_git_does() {
        let dir = scratch_repo("pull-rebases-values");
        let open = || Repository::open(&dir).unwrap();
        for (value, rebases) in [("0", false), ("00", false), ("1", true), ("2", true), ("off", false), ("Yes", true), ("", false)] {
            set_config(&dir, &[("pull.rebase", value)]);
            assert_eq!(pull_rebases(&open(), Some("main")), rebases, "pull.rebase = {value:?}");
        }
        // A key without a value is true in git: `[pull] rebase` rebases.
        let local = dir.join(".git/config");
        let text = std::fs::read_to_string(&local).unwrap();
        std::fs::write(
            &local,
            format!(
                "{text}
[branch \"main\"]
	rebase
"
            ),
        )
        .unwrap();
        assert!(pull_rebases(&open(), Some("main")), "a value-less branch.main.rebase");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rebase_values() {
        assert!(!rebase_value("00") && !rebase_value(" FALSE ") && rebase_value("interactive") && rebase_value("-1"));
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
