//! Fetching or pulling many listed repositories at once (`AppState::repo_sync`): which
//! repositories a scope takes in, each one's progress and result, and the words the dialog
//! (ui/repo_sync.rs), the toasts and the palette share. Changed by update/repo_sync.rs; each
//! repository runs in the runner (`ReposEffect::Sync`), at most `MAX_RUNNING` at a time.

use super::AppState;
use super::repo_lists::{favorites, is_missing};
use std::path::PathBuf;
use trellis_core::repo_sync::Outcome;

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
        self.rows.iter().any(|r| !matches!(r.state, RowState::Done(_)))
    }

    /// (finished, all).
    pub fn progress(&self) -> (usize, usize) {
        (self.rows.iter().filter(|r| matches!(r.state, RowState::Done(_))).count(), self.rows.len())
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
        let failed = count(&|o| matches!(o, Outcome::Failed(_)));
        let skipped = count(&|o| matches!(o, Outcome::Untrusted));
        let (title, verb) = match self.kind {
            SyncKind::Fetch => ("Fetch", "fetch"),
            SyncKind::Pull => ("Pull", "pull"),
        };
        let with_nothing = format!("with nothing to {verb}");
        let mut parts = Vec::new();
        for (n, what) in [(updated, "updated"), (current, "up to date"), (nothing, &with_nothing), (failed, "failed"), (skipped, "skipped")]
        {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        format!("{title} of {}: {}.", repositories(self.rows.len()), parts.join(", "))
    }
}

/// The verb of `kind`, lower case.
pub fn verb(kind: SyncKind) -> &'static str {
    match kind {
        SyncKind::Fetch => "fetch",
        SyncKind::Pull => "pull",
    }
}

/// What a finished repository's row says, and whether it is bad news.
pub fn outcome_text(kind: SyncKind, o: &Outcome) -> (String, bool) {
    match o {
        Outcome::Updated { .. } if kind == SyncKind::Pull => ("Pulled".into(), false),
        Outcome::Updated { changed: 1 } => ("Fetched: 1 branch or tag moved".into(), false),
        Outcome::Updated { changed } => (format!("Fetched: {changed} branches or tags moved"), false),
        Outcome::UpToDate => ("Up to date".into(), false),
        Outcome::NoRemotes => (format!("Nothing to {}: no remotes", verb(kind)), false),
        Outcome::NoUpstream => ("Nothing to pull: the branch has no upstream".into(), false),
        Outcome::Untrusted => ("Skipped: not trusted. Open it to review its trust.".into(), true),
        Outcome::Failed(why) => (format!("Failed: {why}"), true),
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
    use std::time::Instant;
    use trellis_core::settings::Settings;

    fn row(state: RowState) -> SyncRow {
        SyncRow { path: PathBuf::from("/w/a"), name: "a".into(), state }
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
            row(RowState::Done(Outcome::Failed("no".into()))),
            row(RowState::Done(Outcome::Untrusted)),
        ];
        let sync = RepoSync { generation: 1, kind: SyncKind::Fetch, scope: SyncScope::Listed, rows };
        assert!(!sync.running());
        assert_eq!(sync.progress(), (5, 5));
        assert_eq!(sync.summary(), "Fetch of 5 repositories: 1 updated, 2 up to date, 1 failed, 1 skipped.");
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
        assert_eq!(outcome_text(SyncKind::Fetch, &Outcome::Updated { changed: 1 }).0, "Fetched: 1 branch or tag moved");
        assert_eq!(outcome_text(SyncKind::Pull, &Outcome::Updated { changed: 3 }).0, "Pulled");
        assert_eq!(outcome_text(SyncKind::Fetch, &Outcome::NoRemotes).0, "Nothing to fetch: no remotes");
        assert!(outcome_text(SyncKind::Pull, &Outcome::Untrusted).1);
        assert_eq!(outcome_text(SyncKind::Pull, &Outcome::Failed("boom".into())), ("Failed: boom".into(), true));
        assert_eq!(nothing_to_sync(SyncKind::Fetch, SyncScope::Favorites), "No repositories to fetch: no favorite can be found.");
    }
}
