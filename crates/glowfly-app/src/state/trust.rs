//! Repository trust: whether Glowfly may let git run the programs a repository's own config and
//! hooks name without asking (auto-fetch, hosting-service sign-in, git's own status read). The
//! decisions live in `Settings::repo_trust`, each Trusted one with the print of the scan it was
//! made on (`Settings::repo_trust_prints`); the scan comes with each full load
//! (`Snapshot::trust_scan`), and a scan the print doesn't cover asks again. A git job the user
//! starts while git is held waits here as the session's held job until they decide. Pure rules
//! only; update/trust.rs asks and records.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{RepoTrust, SavedTrust, Settings, repo_keys};
use glowfly_core::trust::{TrustPrint, TrustScan};
use std::path::{Path, PathBuf};

/// How a repository Glowfly is about to open came to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Created {
    /// Cloned by Glowfly: trusted when it first opens.
    Clone,
    /// Made by `git init` from Glowfly: trusted when it first opens, if git made a new one.
    Init,
    /// A linked worktree Glowfly created: it has its own decision, like any other repository,
    /// trusted when it was made from a trusted repository (update/trust_inherit.rs).
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
    /// Trusted earlier, but its settings or hooks gained something since; held like Undecided.
    Changed,
}

/// What this session remembers about trust.
#[derive(Default)]
pub struct TrustSession {
    /// Repositories (real paths) asked about this session: refreshes don't ask again.
    pub asked: Vec<PathBuf>,
    /// Repositories Glowfly just cloned or initialised, recorded as trusted when they load.
    pub created: Vec<PathBuf>,
    /// The job waiting for the open trust prompt.
    pub held: Option<HeldJob>,
    /// What the user asked for outside the job gate that waits for the open trust prompt, run
    /// once git runs in its repository unasked (update/trust.rs::hold_waiting).
    pub waiting: Option<Waiting>,
    /// What the running job makes from a trusted repository, to be trusted once it succeeds.
    pub inherit: Option<Inherit>,
    /// The running `git lfs install` job's label and its repository's real path: once it
    /// succeeds, that repository's next full load adopts its new print (`adopt_next`).
    pub lfs_install: Option<(String, PathBuf)>,
    /// A repository (real path) whose next full load adopts its new print without asking.
    pub adopt_next: Option<PathBuf>,
    /// The re-checks for effects pushed since the shell last paired them (update/trust_seal.rs).
    pub seals: Vec<crate::effect::Seal>,
}

/// Repositories a running job makes from the trusted repository `from` (update/trust_inherit.rs).
#[derive(Clone, Debug, PartialEq)]
pub struct Inherit {
    /// The job's label.
    pub label: String,
    pub kind: InheritKind,
    /// `Snapshot::real_workdir` of the repository they are made from.
    pub from: PathBuf,
    /// The folders that become repositories.
    pub paths: Vec<PathBuf>,
}

/// Something the user asked for that waits for the trust prompt (`TrustSession::waiting`).
pub struct Waiting {
    /// `Snapshot::real_workdir` of the repository it waits for.
    pub real: PathBuf,
    pub what: WaitingFor,
}

/// What waits.
pub enum WaitingFor {
    /// Open in external terminal: the launch.
    Terminal(crate::effect::Effect),
    /// A read whose git runs the repository's own programs (a conflict check: merge-tree runs
    /// its merge drivers).
    Read(crate::effect::Effect),
    /// A merge preview: its dialog opens and the preview runs.
    Preview(crate::effect::OpPreviewJob),
}

/// What kind of repository is made from a trusted one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InheritKind {
    /// A linked worktree: trusted only while what it runs is in the source's print.
    Worktree,
    /// A submodule checked out for the first time: trusted like a clone Glowfly made.
    Submodule,
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
    /// The job runs in a repository other than the active tab's: it may run whichever tab is
    /// active.
    pub elsewhere: bool,
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
    /// For a repository trusted earlier (`Standing::Changed`): what its scan gained since.
    pub gained: Option<TrustScan>,
    /// The built-in terminal's shells wait for this decision: Trust and run / Cancel.
    pub terminal: bool,
    /// What the commit the held job brings in has, where hooks follow the working tree, that
    /// the decision doesn't cover (update/trust_seal.rs); trusting covers it too.
    pub incoming: Option<TrustScan>,
    /// Why what the held job brings in couldn't be compared (a partial clone, a long replay, a
    /// revision Glowfly can't resolve as git does): Trust and run runs it without comparing.
    pub unchecked: Option<Box<crate::effect::Accepted>>,
}

/// The prompt for the loaded repository `snap`.
pub fn prompt_for(s: &Settings, snap: &Snapshot) -> TrustPrompt {
    let standing = standing(s, snap);
    let gained = match (standing, saved_for(s, snap).and_then(|t| t.print)) {
        (Standing::Changed, Some(print)) => Some(snap.trust_scan.gained_since(&print)),
        _ => None,
    };
    TrustPrompt {
        name: snap.name.clone(),
        workdir: snap.workdir.clone(),
        real: snap.real_workdir.clone(),
        scan: snap.trust_scan.clone(),
        refused: standing == Standing::NotTrusted,
        held: None,
        gained,
        terminal: false,
        incoming: None,
        unchecked: None,
    }
}

/// The decision saved for the repository at `path`, under any spelling of it. Never under
/// another case: on a volume that tells case apart that is another repository, so a decision is
/// only shared by paths spelled the same.
pub fn recorded(s: &Settings, path: &Path) -> Option<RepoTrust> {
    repo_keys::get(&s.repo_trust, path).copied()
}

/// Saves `t` for the repository at `real`, replacing a decision saved under another spelling,
/// with no print: a Trusted one covers only what runs nothing (`SavedTrust::covers`), so the
/// next scan that finds anything asks again.
pub fn record(s: &mut Settings, real: &Path, t: RepoTrust) {
    s.set_trust(real, Some(SavedTrust { trust: t, print: None }));
}

/// Saves Trusted for the repository at `real`, made on what a scan of it finds now, for tests.
#[cfg(test)]
pub fn record_as_found(s: &mut Settings, real: &Path) {
    let print = glowfly_core::trust::scan_at(real).map(|scan| scan.print()).unwrap_or_default();
    record_trusted(s, real, print);
}

/// Saves Trusted for the repository at `real`, made on a scan whose print is `print`.
pub fn record_trusted(s: &mut Settings, real: &Path, print: TrustPrint) {
    s.set_trust(real, Some(SavedTrust::trusted(Some(print))));
}

/// The decision saved for the loaded repository `snap`, under its real path or as opened.
pub(crate) fn saved_for(s: &Settings, snap: &Snapshot) -> Option<SavedTrust> {
    s.saved_trust(&snap.real_workdir).or_else(|| s.saved_trust(&snap.workdir))
}

/// Where the loaded repository `snap` stands.
pub fn standing(s: &Settings, snap: &Snapshot) -> Standing {
    let saved = saved_for(s, snap);
    let covered = saved.as_ref().is_some_and(|t| t.covers(&snap.trust_print));
    standing_of(saved.map(|t| t.trust), snap.trust_scan.is_empty(), covered)
}

/// `standing` from the saved decision, whether the scan found nothing, and whether the
/// decision covers the scan (`SavedTrust::covers`).
pub fn standing_of(saved: Option<RepoTrust>, clean: bool, covered: bool) -> Standing {
    match saved {
        Some(RepoTrust::Trusted) if !covered => Standing::Changed,
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

    /// Whether git is held in the active repository (Undecided, NotTrusted or Changed): the
    /// toolbar shows the Untrusted chip and the palette offers to review it.
    pub fn held(&self) -> bool {
        matches!(self.trust_standing(), Some(Standing::Undecided | Standing::NotTrusted | Standing::Changed))
    }

    /// Whether Glowfly may run git in the active repository without being asked (Trusted or
    /// Clean). True without a repository: there is nothing of its own to run.
    pub fn runs_git_unasked(&self) -> bool {
        !self.held()
    }

    /// Whether the user trusts the repository at `workdir` (as given, or where the active tab
    /// found it on disk). False while the active repository is that one and gained settings or
    /// hooks since it was trusted (`Standing::Changed`).
    pub fn trusts(&self, workdir: &Path) -> bool {
        let tab_real = self.tabs.get(self.active_tab).filter(|t| t.is(workdir)).and_then(|t| t.real.as_deref());
        let snap = self.snap.as_ref().filter(|s| super::tabs::is_repo_at(s, workdir));
        if snap.is_some_and(|s| standing(&self.settings, s) == Standing::Changed) {
            return false;
        }
        let snap_real = snap.map(|s| s.real_workdir.as_path());
        [Some(workdir), tab_real, snap_real].into_iter().flatten().any(|p| recorded(&self.settings, p) == Some(RepoTrust::Trusted))
    }

    /// The decision saved for the repository at `workdir` itself, with its print, as given or
    /// where a tab showing it found it on disk. Unlike `trusts`, a NotTrusted record counts too.
    pub fn saved_trust_for(&self, workdir: &Path) -> Option<SavedTrust> {
        let reals = self.tabs.iter().filter(|t| t.is(workdir)).filter_map(|t| t.real.as_deref());
        std::iter::once(workdir).chain(reals).find_map(|p| self.settings.saved_trust(p))
    }
}

/// What the Untrusted chip's tooltip says is paused, and why. Only what Glowfly really holds:
/// background fetch, hosting-service calls and git's status read wait, and git commands the
/// user starts ask first. Empty for Trusted and Clean.
pub fn paused_note(standing: Standing) -> &'static str {
    match standing {
        Standing::Undecided => {
            "This repository's own settings or hooks make git run programs, and you haven't said whether to trust it. \
             Until you do, Glowfly won't fetch in the background, contact its hosting service or let git read its status, \
             and it asks before running Pull, Push, Commit or any other git command you start. Click to review."
        }
        Standing::NotTrusted => {
            "You chose not to trust this repository. Glowfly won't fetch in the background, contact its hosting service \
             or let git read its status, and it asks again before running Pull, Push, Commit or any other git command you \
             start. Click to review."
        }
        Standing::Changed => {
            "Since you trusted this repository, it gained settings or hooks that make git run programs. Until you review \
             them, Glowfly won't fetch in the background, contact its hosting service or let git read its status, and it \
             asks before running Pull, Push, Commit or any other git command you start. Click to review."
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
        Some(Standing::Changed) => "changed",
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
    let text = glowfly_core::text::printable(value);
    if text.chars().count() <= VALUE_CHARS {
        return text;
    }
    let mut cut: String = text.chars().take(VALUE_CHARS).collect();
    cut.push('…');
    cut
}

/// Why what a held job brings in couldn't be compared, for the prompt and the job's output.
pub fn unchecked_text(t: glowfly_core::i18n::Lang, u: &glowfly_core::trust::incoming::Unchecked) -> String {
    use glowfly_core::text::printable;
    use glowfly_core::trust::incoming::Unchecked;
    match u {
        Unchecked::Unresolved(rev) => t.trf("trust.unchecked.unresolved", &[("rev", &printable(rev))]),
        Unchecked::Unreadable(e) => t.trf("trust.unchecked.unreadable", &[("error", &printable(e))]),
        Unchecked::Patched { path, .. } => t.trf("trust.unchecked.patched", &[("path", &printable(path))]),
        Unchecked::Conflicted(p) => t.trf("trust.unchecked.conflicted", &[("path", &printable(p))]),
        Unchecked::Unmodeled(what) => t.trf("trust.unchecked.unmodeled", &[("what", &printable(what))]),
    }
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
        assert_eq!(standing_of(Some(RepoTrust::Trusted), false, true), Standing::Trusted);
        assert_eq!(standing_of(Some(RepoTrust::Trusted), false, false), Standing::Changed, "gained something since");
        assert_eq!(standing_of(Some(RepoTrust::NotTrusted), true, true), Standing::NotTrusted, "no stays no");
        assert_eq!(standing_of(Some(RepoTrust::NotTrusted), false, false), Standing::NotTrusted);
        assert_eq!(standing_of(None, true, true), Standing::Clean);
        assert_eq!(standing_of(None, false, true), Standing::Undecided);
    }

    #[test]
    fn the_paused_note_says_what_is_held() {
        let (undecided, refused) = (paused_note(Standing::Undecided), paused_note(Standing::NotTrusted));
        let changed = paused_note(Standing::Changed);
        assert_ne!(undecided, refused);
        let start = |n: &str| n.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        assert!(start(changed) != start(undecided) && start(changed) != start(refused), "{changed}");
        assert!(changed.contains("gained settings or hooks"), "{changed}");
        for note in [undecided, refused, changed] {
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
