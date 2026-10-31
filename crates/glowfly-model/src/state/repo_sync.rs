//! Fetching or pulling many listed repositories at once (`AppState::repo_sync`): which
//! repositories a scope takes in, each one's progress and result, and the words the dialog
//! (ui/repo_sync.rs), the toasts and the palette share. Changed by update/repo_sync.rs; each
//! repository runs in the runner (`ReposEffect::Sync`), at most `MAX_RUNNING` at a time.

use super::AppState;
use super::repo_lists::{favorites, is_missing};
use glowfly_core::i18n::Lang;
use glowfly_core::repo_sync::Outcome;
use std::path::PathBuf;

/// How many repositories fetch or pull at the same time.
pub const MAX_RUNNING: usize = 4;

/// Fetch or pull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncKind {
    Fetch,
    Pull,
}

/// Which repositories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncScope {
    /// The favorites.
    Favorites,
    /// Every listed repository: the favorites, then the Recent list.
    Listed,
}

/// Where one repository stands.
#[derive(Clone, Debug, PartialEq)]
pub enum RowState {
    Waiting,
    Running,
    Done(Outcome),
    /// Stop was clicked before it started.
    Stopped,
}

/// One repository in the run.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncRow {
    pub path: PathBuf,
    pub name: String,
    pub state: RowState,
}

/// A fetch or pull across repositories, running or finished.
pub struct RepoSync {
    /// Incremented for every run started; results of older runs are ignored.
    pub generation: u64,
    pub kind: SyncKind,
    pub scope: SyncScope,
    pub rows: Vec<SyncRow>,
}

impl RepoSync {
    /// Whether a repository is still waiting or running.
    pub fn running(&self) -> bool {
        self.rows.iter().any(|r| matches!(r.state, RowState::Waiting | RowState::Running))
    }

    /// (finished or stopped, all).
    pub fn progress(&self) -> (usize, usize) {
        (self.rows.iter().filter(|r| matches!(r.state, RowState::Done(_) | RowState::Stopped)).count(), self.rows.len())
    }

    /// Whether Stop has anything to do: a repository still waits to start.
    pub fn can_stop(&self) -> bool {
        self.rows.iter().any(|r| r.state == RowState::Waiting)
    }

    /// After Stop: how many repositories still run (none waits, some were stopped).
    pub fn stopping(&self) -> Option<usize> {
        let count = |s: &RowState| self.rows.iter().filter(|r| r.state == *s).count();
        let running = count(&RowState::Running);
        (count(&RowState::Waiting) == 0 && running > 0 && count(&RowState::Stopped) > 0).then_some(running)
    }

    /// The dialog's title.
    pub fn title(&self) -> &'static str {
        match (self.kind, self.scope) {
            (SyncKind::Fetch, SyncScope::Favorites) => "Fetch favorites",
            (SyncKind::Fetch, SyncScope::Listed) => "Fetch all listed repositories",
            (SyncKind::Pull, SyncScope::Favorites) => "Pull favorites",
            (SyncKind::Pull, SyncScope::Listed) => "Pull all listed repositories",
        }
    }

    /// One line on how the finished run went, for the dialog and the toast.
    pub fn summary(&self) -> String {
        let count = |f: &dyn Fn(&Outcome) -> bool| self.rows.iter().filter(|r| matches!(&r.state, RowState::Done(o) if f(o))).count();
        let updated = count(&|o| matches!(o, Outcome::Updated { .. }));
        let current = count(&|o| matches!(o, Outcome::UpToDate));
        let nothing = count(&|o| matches!(o, Outcome::NoRemotes | Outcome::NoUpstream));
        let failed = count(&|o| matches!(o, Outcome::Failed { .. }));
        let skipped = count(&|o| matches!(o, Outcome::Untrusted));
        let stopped = self.rows.iter().filter(|r| r.state == RowState::Stopped).count();
        let (title, verb) = match self.kind {
            SyncKind::Fetch => ("Fetch", "fetch"),
            SyncKind::Pull => ("Pull", "pull"),
        };
        let with_nothing = format!("with nothing to {verb}");
        let mut parts = Vec::new();
        let counts = [
            (updated, "updated"),
            (current, "up to date"),
            (nothing, &with_nothing),
            (failed, "failed"),
            (skipped, "skipped"),
            (stopped, "stopped"),
        ];
        for (n, what) in counts {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        format!("{title} of {}: {}.", repositories(self.rows.len()), parts.join(", "))
    }
}

/// Whether row `r` can run again: it failed or was stopped.
pub fn can_retry(r: &SyncRow) -> bool {
    matches!(r.state, RowState::Done(Outcome::Failed { .. }) | RowState::Stopped)
}

/// Whether row `r` has finished, so its repository can be opened from the dialog.
pub fn can_open(r: &SyncRow) -> bool {
    matches!(r.state, RowState::Done(_) | RowState::Stopped)
}

/// What hovering a failed row shows: git's whole output, or the reason when there is no more;
/// after the row's plain-words sentence and a blank line when it has one (the row can be cut).
pub fn failure_tip(t: Lang, o: &Outcome) -> Option<String> {
    let Outcome::Failed { reason, detail } = o else { return None };
    let raw = if detail.is_empty() { reason } else { detail };
    Some(match super::error_words::sync_failure(t, raw) {
        Some(sentence) => format!("{sentence}\n\n{raw}"),
        None => raw.clone(),
    })
}

/// The verb of `kind`, lower case.
pub fn verb(kind: SyncKind) -> &'static str {
    match kind {
        SyncKind::Fetch => "fetch",
        SyncKind::Pull => "pull",
    }
}

/// What a finished repository's row says, and whether it is bad news. A failure reaching the
/// remote, signing in, a diverged pull or a lock file is said in plain words
/// (`error_words::sync_failure`), others with
/// git's last line.
pub fn outcome_text(t: Lang, kind: SyncKind, o: &Outcome) -> (String, bool) {
    match o {
        Outcome::Updated { .. } if kind == SyncKind::Pull => ("Pulled".into(), false),
        Outcome::Updated { changed: 1 } => ("Fetched: 1 branch or tag moved".into(), false),
        Outcome::Updated { changed } => (format!("Fetched: {changed} branches or tags moved"), false),
        Outcome::UpToDate => ("Up to date".into(), false),
        Outcome::NoRemotes => (format!("Nothing to {}: no remotes", verb(kind)), false),
        Outcome::NoUpstream => ("Nothing to pull: the branch has no upstream".into(), false),
        Outcome::Untrusted => ("Skipped: not trusted. Open it to review its trust.".into(), true),
        Outcome::Failed { reason, detail } => {
            let raw = if detail.is_empty() { reason } else { detail };
            let sentence = super::error_words::sync_failure(t, raw);
            (format!("Failed: {}", sentence.as_deref().unwrap_or(reason)), true)
        }
    }
}

/// `n` repositories, in words.
fn repositories(n: usize) -> String {
    if n == 1 { "1 repository".to_owned() } else { format!("{n} repositories") }
}

/// The repositories `scope` takes in, each folder once, leaving out those the last check found
/// missing.
pub fn targets(st: &AppState, scope: SyncScope) -> Vec<PathBuf> {
    let listed = match scope {
        SyncScope::Favorites => favorites(&st.settings),
        SyncScope::Listed => favorites(&st.settings).into_iter().chain(st.recent.iter().cloned()).collect(),
    };
    let mut out: Vec<PathBuf> = Vec::new();
    for p in listed {
        if !is_missing(st, &p) && !out.iter().any(|o| st.same_folder(o, &p)) {
            out.push(p);
        }
    }
    out
}

/// The toast shown when `scope` has no repository to fetch or pull.
pub fn nothing_to_sync(kind: SyncKind, scope: SyncScope) -> String {
    let lists = match scope {
        SyncScope::Favorites => "no favorite",
        SyncScope::Listed => "no listed repository",
    };
    format!("No repositories to {}: {lists} can be found.", verb(kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;
    use std::time::Instant;

    fn row(state: RowState) -> SyncRow {
        SyncRow { path: PathBuf::from("/w/a"), name: "a".into(), state }
    }

    fn failed(reason: &str, detail: &str) -> Outcome {
        Outcome::Failed { reason: reason.into(), detail: detail.into() }
    }

    fn run(states: &[RowState]) -> RepoSync {
        let rows = states.iter().cloned().map(row).collect();
        RepoSync { generation: 1, kind: SyncKind::Fetch, scope: SyncScope::Favorites, rows }
    }

    #[test]
    fn stop_and_stopping_follow_the_rows() {
        let waiting = run(&[RowState::Running, RowState::Waiting]);
        assert!(waiting.can_stop() && waiting.running());
        assert_eq!(waiting.stopping(), None, "nothing stopped yet");
        let stopping = run(&[RowState::Running, RowState::Running, RowState::Stopped]);
        assert!(!stopping.can_stop() && stopping.running());
        assert_eq!(stopping.stopping(), Some(2));
        assert_eq!(stopping.progress(), (1, 3));
        let ended = run(&[RowState::Done(Outcome::UpToDate), RowState::Stopped]);
        assert!(!ended.can_stop() && !ended.running());
        assert_eq!(ended.stopping(), None);
        let plain = run(&[RowState::Running]);
        assert_eq!(plain.stopping(), None, "running without a Stop isn't stopping");
    }

    #[test]
    fn retry_open_and_the_failure_tip() {
        assert!(can_retry(&row(RowState::Done(failed("x", "")))));
        assert!(can_retry(&row(RowState::Stopped)));
        for s in [RowState::Waiting, RowState::Running, RowState::Done(Outcome::UpToDate), RowState::Done(Outcome::Untrusted)] {
            assert!(!can_retry(&row(s)));
        }
        assert!(can_open(&row(RowState::Stopped)) && can_open(&row(RowState::Done(Outcome::Untrusted))));
        assert!(!can_open(&row(RowState::Waiting)) && !can_open(&row(RowState::Running)));
        let tip = |o: &Outcome| failure_tip(Lang::En, o);
        assert_eq!(tip(&failed("boom", "fatal: x\nboom")).as_deref(), Some("fatal: x\nboom"));
        assert_eq!(tip(&failed("The folder can't be found.", "")).as_deref(), Some("The folder can't be found."));
        assert_eq!(tip(&Outcome::UpToDate), None);
        let raw = "fatal: unable to access 'https://example.com/r/': Could not resolve host: example.com";
        let rewritten = tip(&failed("fatal: unable to access …", raw)).unwrap();
        assert!(rewritten.starts_with("Couldn't reach example.com. ") && rewritten.ends_with(&format!(".\n\n{raw}")), "{rewritten}");
    }

    #[test]
    fn targets_take_each_folder_once_without_missing_ones() {
        let settings = Settings { favorite_repos: vec!["/w/a".into(), "/w/b".into()], ..Settings::default() };
        let mut st = AppState::new(settings, vec![PathBuf::from("/w/b/"), PathBuf::from("/w/c"), PathBuf::from("/w/d")], Instant::now());
        st.missing_repos = vec![PathBuf::from("/w/d")];
        assert_eq!(targets(&st, SyncScope::Favorites), [PathBuf::from("/w/a"), PathBuf::from("/w/b")]);
        assert_eq!(targets(&st, SyncScope::Listed), [PathBuf::from("/w/a"), PathBuf::from("/w/b"), PathBuf::from("/w/c")]);
    }

    #[test]
    fn the_summary_counts_each_outcome() {
        let rows = vec![
            row(RowState::Done(Outcome::Updated { changed: 2 })),
            row(RowState::Done(Outcome::UpToDate)),
            row(RowState::Done(Outcome::UpToDate)),
            row(RowState::Done(failed("no", ""))),
            row(RowState::Done(Outcome::Untrusted)),
            row(RowState::Stopped),
        ];
        let sync = RepoSync { generation: 1, kind: SyncKind::Fetch, scope: SyncScope::Listed, rows };
        assert!(!sync.running());
        assert_eq!(sync.progress(), (6, 6), "a stopped row counts as finished");
        assert_eq!(sync.summary(), "Fetch of 6 repositories: 1 updated, 2 up to date, 1 failed, 1 skipped, 1 stopped.");
        let one = RepoSync {
            generation: 1,
            kind: SyncKind::Pull,
            scope: SyncScope::Favorites,
            rows: vec![row(RowState::Done(Outcome::UpToDate))],
        };
        assert_eq!(one.summary(), "Pull of 1 repository: 1 up to date.");
        assert_eq!(one.title(), "Pull favorites");
    }

    #[test]
    fn rows_say_how_each_went() {
        let text = |kind, o: &Outcome| outcome_text(Lang::En, kind, o);
        assert_eq!(text(SyncKind::Fetch, &Outcome::Updated { changed: 1 }).0, "Fetched: 1 branch or tag moved");
        assert_eq!(text(SyncKind::Pull, &Outcome::Updated { changed: 3 }).0, "Pulled");
        assert_eq!(text(SyncKind::Fetch, &Outcome::NoRemotes).0, "Nothing to fetch: no remotes");
        assert!(text(SyncKind::Pull, &Outcome::Untrusted).1);
        assert_eq!(text(SyncKind::Pull, &failed("boom", "fatal: x\nboom")), ("Failed: boom".into(), true));
        assert_eq!(
            text(SyncKind::Fetch, &failed("Could not read from remote repository.", "")).0,
            "Failed: Couldn't read from the remote repository. Check the remote's URL, your network and your access, then try again."
        );
        let resolve = "fatal: unable to access 'https://example.com/r/': Could not resolve host: example.com";
        assert!(text(SyncKind::Fetch, &failed("fatal: unable to access …", resolve)).0.starts_with("Failed: Couldn't reach example.com. "));
        let prompts = "fatal: could not read Username for 'https://example.com': terminal prompts disabled";
        assert!(
            text(SyncKind::Pull, &failed("terminal prompts disabled", prompts)).0.starts_with("Failed: The server needs you to sign in.")
        );
        let diverged = "fatal: Not possible to fast-forward, aborting.";
        let said = text(SyncKind::Pull, &failed(diverged, diverged)).0;
        assert!(said.starts_with("Failed: This branch and its upstream have both moved on"), "{said}");
        assert_eq!(nothing_to_sync(SyncKind::Fetch, SyncScope::Favorites), "No repositories to fetch: no favorite can be found.");
    }
}
