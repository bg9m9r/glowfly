//! The Preferences save model. App settings apply at once; pages that write repository or git
//! config keep their edits until the Save / Revert bar saves or reverts them (`unsaved` tells
//! whether the open page has any), and leaving such a page asks first (update/prefs_leave.rs).
//! Either way a short "Saved" tick shows beside the page title (`SavedFlash`).

use super::PrefPage;
use super::prefs::PrefsState;
use std::time::{Duration, Instant};
use trellis_core::repo::Snapshot;
use trellis_core::settings::Settings;

/// How long the "Saved" tick stays at full strength, then how long it fades.
pub const SHOW_FOR: Duration = Duration::from_millis(1600);
pub const FADE_FOR: Duration = Duration::from_millis(400);

/// The "Saved" tick beside the page title. `mark` asks for it; the next `tick` (once per frame)
/// stamps the time, since update handlers that save don't all know it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedFlash {
    /// Marked since the last tick.
    pub pending: bool,
    /// When it started showing; None while hidden.
    pub since: Option<Instant>,
    /// Its strength now, 1 at full and 0 hidden.
    pub alpha: f32,
}

impl SavedFlash {
    pub fn mark(&mut self) {
        self.pending = true;
    }

    /// Advances to `now`. With `reduce_motion` it stays at full strength and then vanishes.
    pub fn tick(&mut self, now: Instant, reduce_motion: bool) {
        if std::mem::take(&mut self.pending) {
            self.since = Some(now);
            self.alpha = 1.0;
        }
        let Some(since) = self.since else { return };
        let elapsed = now.saturating_duration_since(since);
        if elapsed >= SHOW_FOR + FADE_FOR {
            self.since = None;
            self.alpha = 0.0;
        } else if elapsed > SHOW_FOR && !reduce_motion {
            let left = (SHOW_FOR + FADE_FOR - elapsed).as_secs_f32();
            self.alpha = left / FADE_FOR.as_secs_f32();
        } else {
            self.alpha = 1.0;
        }
    }

    pub fn showing(&self) -> bool {
        self.since.is_some()
    }
}

/// The pages that write the repository's config: leaving them also covers switching tabs.
pub fn repo_page(page: PrefPage) -> bool {
    matches!(page, PrefPage::RepoIdentity | PrefPage::RepoHooks | PrefPage::RepoGitFlow | PrefPage::RepoSparse)
}

/// Whether the open page has edits the Save / Revert bar would save.
pub fn unsaved(p: &PrefsState, settings: &Settings, snap: Option<&Snapshot>) -> bool {
    let differ = |a: &str, b: &str| a.trim() != b.trim();
    let git = p.git.as_ref().zip(p.git_saved.as_ref());
    match p.page {
        PrefPage::General => git.is_some_and(|(g, s)| g.longpaths != s.longpaths || g.autocrlf != s.autocrlf),
        PrefPage::RepoIdentity => git.is_some_and(|(g, s)| differ(&g.repo_name, &s.repo_name) || differ(&g.repo_email, &s.repo_email)),
        PrefPage::RepoHooks => git.is_some_and(|(g, s)| differ(&g.hooks_path, &s.hooks_path)),
        PrefPage::Profile => p.profiles.edited(settings),
        PrefPage::RepoGitFlow => snap.is_some_and(|s| s.gitflow.is_initialized()) && p.gitflow.edited(),
        PrefPage::RepoSparse => p.sparse.status.as_ref().is_some_and(|s| s.enabled) && p.sparse.edited(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "prefs_save_tests.rs"]
mod tests;
