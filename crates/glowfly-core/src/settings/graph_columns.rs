//! Which parts of the commit graph are shown (Preferences > Appearance > Commit Graph).
//! Files written before the COMMIT column had separate author and SHA toggles; either one on
//! shows the COMMIT column, which holds the author with the short SHA under it.

use super::DescriptionMode;
use serde::{Deserialize, Serialize};

/// Which parts of the commit graph are shown (Appearance > graph toggles).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "StoredColumns")]
pub struct GraphColumns {
    pub refs: bool,
    pub tree: bool,
    pub message: bool,
    pub description: DescriptionMode,
    pub changes: bool,
    /// The COMMIT column (the author, with the short SHA under it).
    pub commit: bool,
    /// The short SHA line under the author in the COMMIT column.
    pub commit_sha: bool,
    pub date: bool,
}

impl Default for GraphColumns {
    fn default() -> Self {
        Self {
            refs: true,
            tree: true,
            message: true,
            description: DescriptionMode::Always,
            changes: false,
            commit: true,
            commit_sha: true,
            date: true,
        }
    }
}

impl GraphColumns {
    pub fn compact() -> Self {
        Self {
            refs: true,
            tree: true,
            message: true,
            description: DescriptionMode::OnHover,
            changes: false,
            commit: false,
            commit_sha: true,
            date: false,
        }
    }
}

/// `GraphColumns` as any version wrote it: every key optional, plus the old author and SHA
/// toggles.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredColumns {
    refs: Option<bool>,
    tree: Option<bool>,
    message: Option<bool>,
    description: Option<DescriptionMode>,
    changes: Option<bool>,
    commit: Option<bool>,
    commit_sha: Option<bool>,
    date: Option<bool>,
    /// The old AUTHOR column toggle.
    author: Option<bool>,
    /// The old SHA column toggle.
    sha: Option<bool>,
}

impl From<StoredColumns> for GraphColumns {
    fn from(s: StoredColumns) -> Self {
        let d = GraphColumns::default();
        Self {
            refs: s.refs.unwrap_or(d.refs),
            tree: s.tree.unwrap_or(d.tree),
            message: s.message.unwrap_or(d.message),
            description: s.description.unwrap_or(d.description),
            changes: s.changes.unwrap_or(d.changes),
            commit: s.commit.unwrap_or(s.author.unwrap_or(true) || s.sha.unwrap_or(false)),
            commit_sha: s.commit_sha.unwrap_or(true),
            date: s.date.unwrap_or(d.date),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> GraphColumns {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn old_author_and_sha_toggles_map_to_the_commit_column() {
        let old = load(
            r#"{"refs":true,"tree":true,"sha":false,"message":true,"description":"Always","changes":false,"author":true,"date":true}"#,
        );
        assert_eq!((old.commit, old.commit_sha), (true, true));
        assert_eq!(old, GraphColumns::default());
        assert!(!load(r#"{"author":false,"sha":false}"#).commit, "both off hides the column");
        assert!(load(r#"{"author":false,"sha":true}"#).commit, "the SHA alone shows it");
        assert!(load(r#"{"sha":false}"#).commit, "the old author default was on");
        assert_eq!(load("{}"), GraphColumns::default());
        let off = load(r#"{"description":"Never","date":false,"commit":true,"commit_sha":false}"#);
        assert_eq!((off.description, off.date, off.commit_sha), (DescriptionMode::Never, false, false));
    }

    #[test]
    fn graph_columns_round_trip() {
        for c in [GraphColumns::default(), GraphColumns::compact()] {
            let json = serde_json::to_value(c).unwrap();
            let keys = json.as_object().unwrap();
            assert!(!keys.contains_key("author") && !keys.contains_key("sha"), "{json}");
            assert_eq!(serde_json::from_value::<GraphColumns>(json).unwrap(), c);
        }
    }
}
