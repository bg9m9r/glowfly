//! Repository trust: whether Trellis may let git run the programs a repository's own config and
//! hooks name without asking (auto-fetch, hosting-service sign-in, git's own status read). The
//! decisions live in `Settings::repo_trust`; the scan comes with each full load
//! (`Snapshot::trust_scan`). Pure rules only; update/trust.rs asks and records.

use super::AppState;
use super::tabs::same_path;
use std::path::{Path, PathBuf};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{RepoTrust, Settings};
use trellis_core::trust::TrustScan;

/// How a repository Trellis is about to open came to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Created {
    /// Cloned by Trellis: trusted when it first opens.
    Clone,
    /// Made by `git init` from Trellis: trusted when it first opens, if git made a new one.
    Init,
    /// A linked worktree Trellis created: it has its own decision, like any other repository.
    Worktree,
}

/// Where a repository stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Standing {
    /// The user trusts it.
    Trusted,
    /// No decision, and nothing in it makes git run programs.
    Clean,
    /// No decision yet, and its config or hooks make git run programs.
    Undecided,
    /// The user said not to trust it.
    NotTrusted,
}

/// What this session remembers about trust.
#[derive(Default)]
pub struct TrustSession {
    /// Repositories (real paths) asked about this session: refreshes don't ask again.
    pub asked: Vec<PathBuf>,
    /// Repositories Trellis just cloned or initialised, recorded as trusted when they load.
    pub created: Vec<PathBuf>,
}

/// The "Trust this repository?" dialog.
#[derive(Clone, Debug)]
pub struct TrustPrompt {
    pub name: String,
    /// The working directory as shown.
    pub workdir: PathBuf,
    /// Where it really is on disk (`Snapshot::real_workdir`): the key the decision is saved under.
    pub real: PathBuf,
    pub scan: TrustScan,
}

/// The decision saved for the repository at `path`, under any spelling of it.
pub fn recorded(s: &Settings, path: &Path) -> Option<RepoTrust> {
    s.repo_trust.iter().find(|(k, _)| same_path(Path::new(k.as_str()), path)).map(|(_, t)| *t)
}

/// Saves `t` for the repository at `real`, replacing a decision saved under another spelling.
pub fn record(s: &mut Settings, real: &Path, t: RepoTrust) {
    s.repo_trust.retain(|k, _| !same_path(Path::new(k.as_str()), real));
    s.repo_trust.insert(real.display().to_string(), t);
}

/// Where the loaded repository `snap` stands.
pub fn standing(s: &Settings, snap: &Snapshot) -> Standing {
    let saved = recorded(s, &snap.real_workdir).or_else(|| recorded(s, &snap.workdir));
    standing_of(saved, snap.trust_scan.is_empty())
}

/// `standing` from the saved decision and whether the scan found nothing.
pub fn standing_of(saved: Option<RepoTrust>, clean: bool) -> Standing {
    match saved {
        Some(RepoTrust::Trusted) => Standing::Trusted,
        Some(RepoTrust::NotTrusted) => Standing::NotTrusted,
        None if clean => Standing::Clean,
        None => Standing::Undecided,
    }
}

impl AppState {
    /// Whether Trellis may run git in the active repository without being asked (Trusted or
    /// Clean). True without a repository: there is nothing of its own to run.
    pub fn runs_git_unasked(&self) -> bool {
        self.snap.as_ref().is_none_or(|snap| matches!(standing(&self.settings, snap), Standing::Trusted | Standing::Clean))
    }

    /// Whether the user trusts the repository at `workdir` (as given, or where the active tab
    /// found it on disk).
    pub fn trusts(&self, workdir: &Path) -> bool {
        let tab_real = self.tabs.get(self.active_tab).filter(|t| t.is(workdir)).and_then(|t| t.real.as_deref());
        let snap_real = self.snap.as_ref().filter(|s| same_path(&s.workdir, workdir)).map(|s| s.real_workdir.as_path());
        [Some(workdir), tab_real, snap_real].into_iter().flatten().any(|p| recorded(&self.settings, p) == Some(RepoTrust::Trusted))
    }
}

/// Shortens `dir` for the dialog: relative to `workdir` when inside it.
pub fn hooks_label(dir: &Path, workdir: &Path) -> String {
    match dir.strip_prefix(workdir) {
        Ok(rel) if !rel.as_os_str().is_empty() => {
            rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
        }
        _ => dir.display().to_string(),
    }
}

/// The most characters of a setting's value the dialog shows.
pub const VALUE_CHARS: usize = 80;

/// A setting's value for the dialog: control characters dropped, cut to `VALUE_CHARS` with "…".
pub fn shown_value(value: &str) -> String {
    let text = trellis_core::text::printable(value);
    if text.chars().count() <= VALUE_CHARS {
        return text;
    }
    let mut cut: String = text.chars().take(VALUE_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_match_any_spelling_of_the_folder() {
        let mut s = Settings::default();
        record(&mut s, Path::new("work/tools"), RepoTrust::Trusted);
        assert_eq!(recorded(&s, Path::new("work/./tools/")), Some(RepoTrust::Trusted));
        assert_eq!(recorded(&s, Path::new("work/other")), None);
        #[cfg(windows)]
        {
            record(&mut s, Path::new(r"\\?\C:\x"), RepoTrust::Trusted);
            assert_eq!(recorded(&s, Path::new(r"c:\X")), Some(RepoTrust::Trusted));
            // A new decision replaces the one saved under the older spelling.
            record(&mut s, Path::new(r"c:\X"), RepoTrust::NotTrusted);
            assert_eq!(recorded(&s, Path::new(r"\\?\C:\x")), Some(RepoTrust::NotTrusted));
            assert_eq!(s.repo_trust.len(), 2);
        }
        record(&mut s, Path::new("work/tools/"), RepoTrust::NotTrusted);
        assert_eq!(recorded(&s, Path::new("work/tools")), Some(RepoTrust::NotTrusted));
        assert_eq!(s.repo_trust.keys().filter(|k| k.contains("tools")).count(), 1);
    }

    #[test]
    fn standing_follows_the_decision_then_the_scan() {
        assert_eq!(standing_of(Some(RepoTrust::Trusted), false), Standing::Trusted);
        assert_eq!(standing_of(Some(RepoTrust::NotTrusted), true), Standing::NotTrusted, "no stays no");
        assert_eq!(standing_of(None, true), Standing::Clean);
        assert_eq!(standing_of(None, false), Standing::Undecided);
    }

    #[test]
    fn labels_and_values_are_shortened() {
        assert_eq!(hooks_label(Path::new("C:/work/tools/.git/hooks"), Path::new("C:/work/tools")), ".git/hooks");
        assert_eq!(hooks_label(Path::new("D:/shared/hooks"), Path::new("C:/work/tools")), "D:/shared/hooks");
        assert_eq!(shown_value("a\u{1b}b"), "ab");
        let long = "x".repeat(200);
        let shown = shown_value(&long);
        assert_eq!(shown.chars().count(), VALUE_CHARS + 1);
        assert!(shown.ends_with('…'));
    }
}
