//! Settings saved per repository (graph filters, pull request and issue filters, the issue
//! tracker, trust decisions) are keyed by the repository's resolved working directory
//! (`Snapshot::real_workdir`), saved losslessly (`paths::encode`). One set of helpers reads and
//! writes every such map, so a repository opened by another spelling (a link, a short 8.3 name,
//! libgit2's `C:/x/` form) finds its settings, a new entry replaces one saved under another
//! spelling of the same path, and entries earlier versions saved under libgit2's spelling move
//! to the resolved key when the repository loads.

use super::{GraphFilter, Settings, TrackerChoice};
use crate::paths;
use std::collections::HashMap;
use std::path::Path;

/// The key a repository's settings are saved under.
pub fn key(real: &Path) -> String {
    paths::encode(real)
}

/// Whether the saved key `k` is a spelling of `path`.
fn names(k: &str, path: &Path) -> bool {
    paths::same_spelling(&paths::decode(k), path)
}

/// The entry for the repository at `real`.
pub fn get<'a, V>(map: &'a HashMap<String, V>, real: &Path) -> Option<&'a V> {
    map.get(&key(real)).or_else(|| map.iter().find(|(k, _)| names(k, real)).map(|(_, v)| v))
}

/// Saves `value` for the repository at `real` (None removes it), replacing entries saved under
/// other spellings of the same path.
pub fn set<V>(map: &mut HashMap<String, V>, real: &Path, value: Option<V>) {
    map.retain(|k, _| !names(k, real));
    if let Some(v) = value {
        map.insert(key(real), v);
    }
}

/// Moves entries saved under `aliases` (other names the repository at `real` was seen by), or
/// under another spelling of `real`, to `real`'s key. An entry already under that key wins over
/// the old ones. Returns whether anything changed.
pub fn adopt<V>(map: &mut HashMap<String, V>, real: &Path, aliases: &[&Path]) -> bool {
    let exact = key(real);
    let old: Vec<String> =
        map.keys().filter(|k| **k != exact && (names(k, real) || aliases.iter().any(|a| names(k, a)))).cloned().collect();
    if old.is_empty() {
        return false;
    }
    let mut moved = None;
    for k in old {
        let v = map.remove(&k);
        moved = moved.or(v);
    }
    if let Some(v) = moved {
        map.entry(exact).or_insert(v);
    }
    true
}

impl Settings {
    /// The graph filter saved for the repository at `real` (empty when there is none).
    pub fn graph_filter_for(&self, real: &Path) -> GraphFilter {
        get(&self.graph_filters, real).cloned().unwrap_or_default()
    }

    /// Saves `f` for the repository at `real`; an empty filter removes the entry.
    pub fn set_graph_filter(&mut self, real: &Path, f: GraphFilter) {
        set(&mut self.graph_filters, real, (!f.is_empty()).then_some(f));
    }

    /// The issue tracker chosen for the repository at `real`.
    pub fn tracker_for(&self, real: &Path) -> TrackerChoice {
        get(&self.repo_trackers, real).cloned().unwrap_or_default()
    }

    /// Saves the issue tracker for the repository at `real`.
    pub fn set_tracker(&mut self, real: &Path, choice: TrackerChoice) {
        set(&mut self.repo_trackers, real, Some(choice));
    }

    /// Whether the slow-status note was shown for the repository at `real`.
    pub fn status_tip_shown_for(&self, real: &Path) -> bool {
        get(&self.status_tip_shown, real).copied().unwrap_or(false)
    }

    /// Remembers that the slow-status note was shown for the repository at `real`.
    pub fn mark_status_tip_shown(&mut self, real: &Path) {
        set(&mut self.status_tip_shown, real, Some(true));
    }

    /// Moves the graph filter, pull request and issue filters, issue tracker and slow-status note
    /// saved under `aliases` (libgit2's spelling, the path it was opened by) to the repository's
    /// resolved key `real`. Trust decisions were always saved under the resolved key and are left
    /// alone. Returns whether anything moved.
    pub fn adopt_repo_keys(&mut self, real: &Path, aliases: &[&Path]) -> bool {
        let filters = adopt(&mut self.graph_filters, real, aliases);
        let forge_filters = adopt(&mut self.repo_forge_filters, real, aliases);
        let tips = adopt(&mut self.status_tip_shown, real, aliases);
        adopt(&mut self.repo_trackers, real, aliases) | filters | forge_filters | tips
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn one_entry_per_repository_whatever_the_spelling() {
        let mut m: HashMap<String, u32> = HashMap::new();
        set(&mut m, Path::new("/w/repo"), Some(1));
        assert_eq!(get(&m, Path::new("/w/./repo/")), Some(&1));
        set(&mut m, Path::new("/w/repo/"), Some(2));
        assert_eq!(m.len(), 1);
        assert_eq!(get(&m, Path::new("/w/repo")), Some(&2));
        // Another case is another repository unless the disk says otherwise.
        assert_eq!(get(&m, Path::new("/w/Repo")), None);
        set(&mut m, Path::new("/w/repo"), None);
        assert!(m.is_empty());
    }

    #[test]
    fn old_spellings_move_to_the_resolved_key() {
        let real = PathBuf::from("/real/repo");
        let mut m: HashMap<String, u32> = HashMap::new();
        m.insert("/link/repo/".into(), 7);
        m.insert("/other".into(), 9);
        assert!(adopt(&mut m, &real, &[Path::new("/link/repo")]));
        assert_eq!(get(&m, &real), Some(&7));
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(!adopt(&mut m, &real, &[Path::new("/link/repo")]), "nothing left to move");
        // What is already saved for the resolved path wins.
        m.insert("/link/repo".into(), 8);
        assert!(adopt(&mut m, &real, &[Path::new("/link/repo")]));
        assert_eq!(get(&m, &real), Some(&7));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn settings_adopt_filters_and_trackers() {
        let mut s = Settings::default();
        let f = GraphFilter { hidden: vec!["refs/heads/x".into()], solo: vec![] };
        s.graph_filters.insert("C:/x/repo/".into(), f.clone());
        s.repo_trackers.insert("C:/x/repo/".into(), TrackerChoice::None);
        let real = PathBuf::from(r"C:\x\repo");
        if cfg!(windows) {
            // libgit2's `C:/x/repo/` is a spelling of the resolved path on Windows.
            assert_eq!(s.graph_filter_for(&real), f);
        }
        assert!(s.adopt_repo_keys(&real, &[Path::new("C:/x/repo/")]));
        assert_eq!(s.graph_filter_for(&real), f);
        assert_eq!(s.tracker_for(&real), TrackerChoice::None);
        assert_eq!(s.graph_filters.keys().collect::<Vec<_>>(), [&key(&real)]);

        s.status_tip_shown.insert("C:/x/repo/".into(), true);
        assert!(s.adopt_repo_keys(&real, &[Path::new("C:/x/repo/")]));
        assert!(s.status_tip_shown_for(&real));
        assert_eq!(s.status_tip_shown.keys().collect::<Vec<_>>(), [&key(&real)]);
    }

    #[test]
    fn the_slow_status_note_is_remembered_per_repository() {
        let mut s = Settings::default();
        let (a, b) = (PathBuf::from("/w/a"), PathBuf::from("/w/b"));
        assert!(!s.status_tip_shown_for(&a));
        s.mark_status_tip_shown(&a);
        assert!(s.status_tip_shown_for(&a) && !s.status_tip_shown_for(&b));
        s.mark_status_tip_shown(&PathBuf::from("/w/a/"));
        assert_eq!(s.status_tip_shown.len(), 1, "one entry whatever the spelling");
    }

    #[test]
    fn trello_tracker_choices_round_trip() {
        use crate::forge::{Account, Provider};
        let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
        let choice = TrackerChoice::Trello { account: account.clone(), board: "5f1a2b3c4d5e6f7a8b9c0d1e".into(), board_name: "Web".into() };
        let back: TrackerChoice = serde_json::from_str(&serde_json::to_string(&choice).unwrap()).unwrap();
        assert_eq!(back, choice);
        // Saved without a board: all boards.
        let old: TrackerChoice = serde_json::from_str(r#"{"Trello":{"account":{"provider":"Trello","host":"trello.com"}}}"#).unwrap();
        assert_eq!(old, TrackerChoice::Trello { account, board: String::new(), board_name: String::new() });
    }
}
