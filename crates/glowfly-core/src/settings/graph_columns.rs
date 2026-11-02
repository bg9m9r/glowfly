//! Which parts of the commit graph are shown (Preferences > Appearance > Commit Graph). The
//! AUTHOR and COMMIT (short SHA) columns have their own toggles, as the oldest files did. Files
//! from between those had one COMMIT column for both (the author over the SHA): it now shows
//! AUTHOR, and COMMIT unless its SHA line was off.

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
    /// The AUTHOR column: the author's name.
    pub author: bool,
    /// The COMMIT column: the short SHA.
    pub commit: bool,
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
            author: true,
            commit: true,
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
            author: false,
            commit: false,
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
    /// The SHA line of the COMMIT column that held the author too.
    commit_sha: Option<bool>,
    date: Option<bool>,
    author: Option<bool>,
    /// The oldest files' SHA column toggle.
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
            // The combined COMMIT column (no `author` key): AUTHOR follows it, COMMIT too
            // unless its SHA line was off.
            author: s.author.or(s.commit).unwrap_or(d.author),
            commit: match (s.author, s.commit) {
                (None, Some(both)) => both && s.commit_sha.unwrap_or(true),
                _ => s.commit.or(s.sha).unwrap_or(d.commit),
            },
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
    fn old_toggles_map_to_the_author_and_commit_columns() {
        // The oldest files: AUTHOR and SHA toggles of their own.
        let old = load(
            r#"{"refs":true,"tree":true,"sha":false,"message":true,"description":"Always","changes":false,"author":true,"date":true}"#,
        );
        assert_eq!((old.author, old.commit), (true, false));
        assert_eq!((load(r#"{"author":false,"sha":true}"#).author, load(r#"{"author":false,"sha":true}"#).commit), (false, true));
        // The combined COMMIT column: AUTHOR follows it, COMMIT unless its SHA line was off.
        let both = load(r#"{"commit":true,"commit_sha":true}"#);
        assert_eq!((both.author, both.commit), (true, true));
        let no_sha = load(r#"{"commit":true,"commit_sha":false}"#);
        assert_eq!((no_sha.author, no_sha.commit), (true, false));
        let off = load(r#"{"description":"Never","date":false,"commit":false}"#);
        assert_eq!((off.description, off.date, off.author, off.commit), (DescriptionMode::Never, false, false, false));
        assert_eq!(load("{}"), GraphColumns::default());
    }

    #[test]
    fn graph_columns_round_trip() {
        for c in [GraphColumns::default(), GraphColumns::compact()] {
            let json = serde_json::to_value(c).unwrap();
            let keys = json.as_object().unwrap();
            assert!(!keys.contains_key("commit_sha") && !keys.contains_key("sha"), "{json}");
            assert_eq!(serde_json::from_value::<GraphColumns>(json).unwrap(), c);
        }
    }
}
