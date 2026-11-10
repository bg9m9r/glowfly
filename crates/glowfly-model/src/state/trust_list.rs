//! Preferences > Trusted repositories: every saved trust decision as a row (its folder, the
//! decision, when it was made, the repository it was passed on from and how many were passed on
//! from it), whether the last folder check found its folder missing, and the filter.

use glowfly_core::paths::same_spelling;
use glowfly_core::settings::{RepoTrust, Settings};
use std::path::{Path, PathBuf};

/// The list shows its filter field from this many decisions on.
pub const FILTER_FROM: usize = 8;

/// The page's own state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrustPage {
    /// The folders the last check found missing; None until it reports (or while it runs).
    pub missing: Option<Vec<PathBuf>>,
    /// The filter field's text.
    pub filter: String,
}

/// One decision as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustRow {
    pub path: PathBuf,
    /// The folder's name (the whole path when it has none).
    pub name: String,
    pub trust: RepoTrust,
    /// When it was decided (Unix seconds); None for decisions saved before times were kept.
    pub at: Option<i64>,
    /// The folder name of the repository it was passed on from, if inherited.
    pub from_name: Option<String>,
    /// How many decisions were passed on from it, transitively.
    pub heirs: usize,
    /// The last check found its folder missing.
    pub missing: bool,
}

/// A folder's name, or the whole path when it has none (a drive root).
pub fn folder_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Every saved decision, sorted by path, marked missing where the last check said so.
pub fn rows(s: &Settings, page: &TrustPage) -> Vec<TrustRow> {
    s.trust_decisions()
        .into_iter()
        .map(|d| TrustRow {
            name: folder_name(&d.path),
            trust: d.trust,
            at: d.at,
            from_name: d.from.as_deref().map(folder_name),
            heirs: d.heirs,
            missing: page.missing.as_ref().is_some_and(|m| m.iter().any(|p| same_spelling(p, &d.path))),
            path: d.path,
        })
        .collect()
}

/// The rows `filter` matches, ignoring case, in the path or the source's name; all of them
/// when it is blank.
pub fn shown<'a>(rows: &'a [TrustRow], filter: &str) -> Vec<&'a TrustRow> {
    let want = filter.trim().to_lowercase();
    rows.iter()
        .filter(|r| {
            want.is_empty()
                || r.path.display().to_string().to_lowercase().contains(&want)
                || r.from_name.as_ref().is_some_and(|f| f.to_lowercase().contains(&want))
        })
        .collect()
}

/// The folders of the rows marked missing.
pub fn missing_paths(rows: &[TrustRow]) -> Vec<PathBuf> {
    rows.iter().filter(|r| r.missing).map(|r| r.path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::SavedTrust;

    #[test]
    fn rows_mark_missing_folders_and_filter_by_path_or_source() {
        let mut s = Settings::default();
        let base = glowfly_core::paths::temp_dir().join("glowfly-trust-rows");
        let (app, lib) = (base.join("alpha-tool"), base.join("Beta-lib"));
        s.set_trust(&app, Some(SavedTrust::trusted(None)));
        s.set_trust(&lib, Some(SavedTrust::trusted(None)));
        s.stamp_trust(&lib, Some(5), Some(&app));
        let page = TrustPage { missing: Some(vec![lib.clone()]), filter: String::new() };
        let rows = rows(&s, &page);
        assert_eq!(
            rows.iter().map(|r| (r.name.as_str(), r.heirs, r.missing)).collect::<Vec<_>>(),
            [("alpha-tool", 1, false), ("Beta-lib", 0, true)]
        );
        assert_eq!(rows[1].from_name.as_deref(), Some("alpha-tool"));
        assert_eq!(missing_paths(&rows), vec![lib]);
        assert_eq!(shown(&rows, " beta-LIB ").len(), 1);
        assert_eq!(shown(&rows, "ALPHA-TOOL").len(), 2, "the inherited row matches by its source too");
        assert!(shown(&rows, "nothing").is_empty());
    }
}
