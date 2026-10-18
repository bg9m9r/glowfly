//! Repository trust: whether Trellis may let git run the programs a repository's own config and
//! hooks name without asking (auto-fetch, hosting-service sign-in, git's own status read). The
//! decisions live in `Settings::repo_trust`; the scan comes with each full load
//! (`Snapshot::trust_scan`). A git job the user starts while git is held waits here as the
//! session's held job until they decide. Pure rules only; update/trust.rs asks and records.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use std::path::{Path, PathBuf};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{RepoTrust, Settings, repo_keys};
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
    /// The job waiting for the open trust prompt.
    pub held: Option<HeldJob>,
}

/// A job the user started while git was held in its repository: it runs if they trust it
/// (update/trust.rs).
pub struct HeldJob {
    /// `Snapshot::real_workdir` of the repository it waits for.
    pub real: PathBuf,
    /// The repository's name, for the log.
    pub name: String,
    pub job: super::RunningJob,
    pub effect: crate::effect::Effect,
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
    /// The saved decision is Don't trust (the prompt was brought back).
    pub refused: bool,
    /// The label of the job the user started that waits for this decision.
    pub held: Option<String>,
}

/// The prompt for the loaded repository `snap`.
pub fn prompt_for(s: &Settings, snap: &Snapshot) -> TrustPrompt {
    TrustPrompt {
        name: snap.name.clone(),
        workdir: snap.workdir.clone(),
        real: snap.real_workdir.clone(),
        scan: snap.trust_scan.clone(),
        refused: standing(s, snap) == Standing::NotTrusted,
        held: None,
    }
}

/// The decision saved for the repository at `path`, under any spelling of it. Never under
/// another case: on a volume that tells case apart that is another repository, so a decision is
/// only shared by paths spelled the same.
pub fn recorded(s: &Settings, path: &Path) -> Option<RepoTrust> {
    repo_keys::get(&s.repo_trust, path).copied()
}

/// Saves `t` for the repository at `real`, replacing a decision saved under another spelling.
pub fn record(s: &mut Settings, real: &Path, t: RepoTrust) {
    repo_keys::set(&mut s.repo_trust, real, Some(t));
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
    /// Where the active repository stands; None without a repository.
    pub fn trust_standing(&self) -> Option<Standing> {
        self.snap.as_ref().map(|snap| standing(&self.settings, snap))
    }

    /// Whether git is held in the active repository (Undecided or NotTrusted): the toolbar
    /// shows the Untrusted chip and the palette offers to review it.
    pub fn held(&self) -> bool {
        matches!(self.trust_standing(), Some(Standing::Undecided | Standing::NotTrusted))
    }

    /// Whether Trellis may run git in the active repository without being asked (Trusted or
    /// Clean). True without a repository: there is nothing of its own to run.
    pub fn runs_git_unasked(&self) -> bool {
        !self.held()
    }

    /// Whether the user trusts the repository at `workdir` (as given, or where the active tab
    /// found it on disk).
    pub fn trusts(&self, workdir: &Path) -> bool {
        let tab_real = self.tabs.get(self.active_tab).filter(|t| t.is(workdir)).and_then(|t| t.real.as_deref());
        let snap_real = self.snap.as_ref().filter(|s| super::tabs::is_repo_at(s, workdir)).map(|s| s.real_workdir.as_path());
        [Some(workdir), tab_real, snap_real].into_iter().flatten().any(|p| recorded(&self.settings, p) == Some(RepoTrust::Trusted))
    }

    /// The decision saved for the repository at `workdir` itself, as given or where a tab
    /// showing it found it on disk. Unlike `trusts`, a NotTrusted record counts too.
    pub fn decision_for(&self, workdir: &Path) -> Option<RepoTrust> {
        let reals = self.tabs.iter().filter(|t| t.is(workdir)).filter_map(|t| t.real.as_deref());
        std::iter::once(workdir).chain(reals).find_map(|p| recorded(&self.settings, p))
    }
}

/// What the Untrusted chip's tooltip says is paused, and why. Only what Trellis really holds:
/// background fetch, hosting-service calls and git's status read wait, and git commands the
/// user starts ask first. Empty for Trusted and Clean.
pub fn paused_note(standing: Standing) -> &'static str {
    match standing {
        Standing::Undecided => {
            "This repository's own settings or hooks make git run programs, and you haven't said whether to trust it. \
             Until you do, Trellis won't fetch in the background, contact its hosting service or let git read its status, \
             and it asks before running Pull, Push, Commit or any other git command you start. Click to review."
        }
        Standing::NotTrusted => {
            "You chose not to trust this repository. Trellis won't fetch in the background, contact its hosting service \
             or let git read its status, and it asks again before running Pull, Push, Commit or any other git command you \
             start. Click to review."
        }
        Standing::Trusted | Standing::Clean => "",
    }
}

/// Hidden words that find Review repository trust… in the palette.
const KEYWORDS: &str = "trust untrusted security safe hooks config settings allow prompt paused";

/// Review repository trust…, while git is held in the active repository.
pub(crate) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    let detail = match st.trust_standing() {
        Some(Standing::Undecided) => "undecided",
        Some(Standing::NotTrusted) => "not trusted",
        _ => return,
    };
    out.push(with_detail(entry(PaletteCommand::ReviewTrust, "Review repository trust…", KEYWORDS), Some(detail.into())));
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
        // Another case is another repository as far as the saved decisions go: on a volume that
        // tells case apart it is one, and trust is never shared by guessing.
        assert_eq!(recorded(&s, Path::new("work/Tools")), None);
        #[cfg(windows)]
        {
            record(&mut s, Path::new(r"\\?\C:\x"), RepoTrust::Trusted);
            assert_eq!(recorded(&s, Path::new(r"c:\x")), Some(RepoTrust::Trusted));
            assert_eq!(recorded(&s, Path::new(r"c:\X")), None);
            // A new decision replaces the one saved under the older spelling.
            record(&mut s, Path::new(r"c:\x\"), RepoTrust::NotTrusted);
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
    fn the_paused_note_says_what_is_held() {
        let (undecided, refused) = (paused_note(Standing::Undecided), paused_note(Standing::NotTrusted));
        assert_ne!(undecided, refused);
        for note in [undecided, refused] {
            assert!(note.contains("background"), "{note}");
            assert!(!note.contains("  "), "one space between sentences: {note}");
            assert!(note.contains("Pull, Push, Commit or any other git command you start"), "{note}");
            assert!(!note.contains("still run"), "{note}");
        }
        assert!(undecided.contains("settings or hooks make git run programs"));
        assert!(refused.starts_with("You chose not to trust"));
        assert!(paused_note(Standing::Trusted).is_empty() && paused_note(Standing::Clean).is_empty());
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
