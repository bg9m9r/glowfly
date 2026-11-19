//! Which history view a repository shows: the commit graph or the Chapters list. Saved per
//! repository (`repo_keys`) under a stable name; no entry is the graph.

use super::{Settings, repo_keys};
use std::path::Path;

/// The history view of a repository.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryMode {
    /// Every commit, in the graph.
    #[default]
    Commits,
    /// The current branch's history grouped into chapters.
    Chapters,
}

impl HistoryMode {
    /// The name the choice is saved under: never rename these.
    pub fn id(self) -> &'static str {
        match self {
            HistoryMode::Commits => "commits",
            HistoryMode::Chapters => "chapters",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [HistoryMode::Commits, HistoryMode::Chapters].into_iter().find(|m| m.id() == id)
    }
}

impl Settings {
    /// The history view of the repository at `real`; a name this version doesn't know reads as
    /// the graph.
    pub fn history_mode_for(&self, real: &Path) -> HistoryMode {
        if self.repo_history_modes.is_empty() {
            return HistoryMode::Commits;
        }
        repo_keys::get(&self.repo_history_modes, real).and_then(|id| HistoryMode::from_id(id)).unwrap_or_default()
    }

    /// Saves `mode` for the repository at `real` (the graph removes the entry); returns whether
    /// the choice changed.
    pub fn set_history_mode(&mut self, real: &Path, mode: HistoryMode) -> bool {
        if self.history_mode_for(real) == mode {
            return false;
        }
        repo_keys::set(&mut self.repo_history_modes, real, (mode != HistoryMode::Commits).then(|| mode.id().to_owned()));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_graph_is_the_default_and_the_choice_is_per_repository() {
        let mut s = Settings::default();
        let (a, b) = (PathBuf::from("/w/a"), PathBuf::from("/w/b"));
        assert_eq!(s.history_mode_for(&a), HistoryMode::Commits);
        assert!(s.set_history_mode(&a, HistoryMode::Chapters));
        assert!(!s.set_history_mode(&PathBuf::from("/w/a/"), HistoryMode::Chapters), "nothing changed");
        assert_eq!(s.history_mode_for(&PathBuf::from("/w/a/")), HistoryMode::Chapters);
        assert_eq!(s.repo_history_modes.len(), 1, "one entry whatever the spelling");
        assert_eq!(s.history_mode_for(&b), HistoryMode::Commits);
    }

    #[test]
    fn choosing_the_graph_removes_the_entry() {
        let mut s = Settings::default();
        let a = PathBuf::from("/w/a");
        s.set_history_mode(&a, HistoryMode::Chapters);
        assert!(s.set_history_mode(&a, HistoryMode::Commits));
        assert!(s.repo_history_modes.is_empty());
        assert!(!s.set_history_mode(&a, HistoryMode::Commits));
    }

    #[test]
    fn an_unknown_name_reads_as_the_graph() {
        let mut s = Settings::default();
        let a = PathBuf::from("/w/a");
        s.repo_history_modes.insert(repo_keys::key(&a), "timeline".into());
        assert_eq!(s.history_mode_for(&a), HistoryMode::Commits);
    }

    #[test]
    fn the_field_is_optional_and_round_trips() {
        let old: Settings = serde_json::from_str(r#"{"theme": "Mist"}"#).unwrap();
        assert!(old.repo_history_modes.is_empty());
        let mut s = Settings::default();
        let a = PathBuf::from("/w/a");
        s.set_history_mode(&a, HistoryMode::Chapters);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.history_mode_for(&a), HistoryMode::Chapters);
        assert_eq!(HistoryMode::from_id("chapters"), Some(HistoryMode::Chapters));
        assert_eq!(HistoryMode::from_id("commits"), Some(HistoryMode::Commits));
    }
}
