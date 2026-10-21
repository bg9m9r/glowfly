//! The WIP panel's Conflicted Files and Resolved Files lists: the conflict markers left in each
//! conflicted file, the files resolved so far (git's resolve-undo record), and the rules for
//! stepping to the next conflicted file, for which resolved files can be marked conflicted
//! again, and for the "markers left" and Mark conflicted confirmations.

use super::AppState;
use super::path_hazards::hazard_of;
use super::tool_compare::merging;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Hazard, ResolvedFile, Snapshot};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct ConflictFiles {
    /// Complete marker regions in each conflicted working file; `None` when unknown (binary,
    /// deleted, a link, too large).
    pub counts: HashMap<String, Option<usize>>,
    /// The files git can bring the conflict back for, in git's order.
    pub resolved: Vec<ResolvedFile>,
    /// Numbers the summary reads; older results are dropped.
    pub generation: u64,
}

impl ConflictFiles {
    /// The note after a conflicted row: its marker count, or "no markers left" (green: ready to
    /// mark resolved). `None` when unknown or not read yet.
    pub fn count_note(&self, t: Lang, path: &str) -> Option<(String, bool)> {
        match (*self.counts.get(path)?)? {
            0 => Some((t.tr("conflict.files.no_markers").into(), true)),
            n => Some((t.trn("conflict.files.count", n, &[]), false)),
        }
    }

    /// The Resolved Files rows: resolved files that aren't conflicted again. None once the
    /// operation is over (state clean, nothing conflicted).
    pub fn resolved_rows<'a>(&'a self, snap: &Snapshot) -> Vec<&'a ResolvedFile> {
        let conflicted: Vec<&str> = snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
        if snap.state == RepositoryState::Clean && conflicted.is_empty() {
            return Vec::new();
        }
        self.resolved.iter().filter(|r| !conflicted.contains(&r.entry.path.as_str())).collect()
    }
}

/// The conflicted paths Glowfly can name to git (not `Hazard::NotUtf8`), in list order.
pub fn conflicted_paths(snap: &Snapshot) -> Vec<String> {
    snap.unstaged.iter().filter(|f| f.status == '!' && !matches!(f.hazard, Some(Hazard::NotUtf8(_)))).map(|f| f.path.clone()).collect()
}

/// The conflicted file to open after `after` was resolved: the first one after it in list
/// order, wrapping to the first.
pub fn next_conflicted(snap: &Snapshot, after: &str) -> Option<String> {
    let paths = conflicted_paths(snap);
    paths.iter().find(|p| p.as_str() > after).or(paths.first()).filter(|p| p.as_str() != after).cloned()
}

/// The Resolved Files rows git can bring the conflict back for: both sides recorded, a name
/// that can be given to git safely, and not open in the merge tool.
pub fn restorable_paths(st: &AppState) -> Vec<String> {
    let Some(snap) = &st.snap else { return Vec::new() };
    st.conflict_files
        .resolved_rows(snap)
        .into_iter()
        .filter(|r| r.both_sides && r.entry.hazard.is_none())
        .map(|r| &r.entry.path)
        .filter(|p| hazard_of(st, p).is_none() && !merging(st, p))
        .cloned()
        .collect()
}

/// The question before marking `paths` conflicted again.
pub fn mark_conflicted_body(t: Lang, paths: &[String]) -> String {
    let path = paths.first().map_or("", String::as_str);
    t.trn("conflict.files.mark_conflicted.body", paths.len(), &[("path", &path)])
}

/// How many files the "markers left" warning names before "and N more".
const NAMED: usize = 5;

/// The body of the warning before marking files with markers left resolved: `left` holds each
/// such path with its number of marker regions.
pub fn markers_warning(left: &[(String, usize)]) -> String {
    let regions = |n: usize| if n == 1 { "1 conflict marker region".to_owned() } else { format!("{n} conflict marker regions") };
    match left {
        [(path, n)] => format!("{path} still has {}. Marking it resolved stages the markers as they are.", regions(*n)),
        _ => {
            let mut named: Vec<String> = left.iter().take(NAMED).map(|(p, n)| format!("{p} ({n})")).collect();
            if left.len() > NAMED {
                named.push(format!("and {} more", left.len() - NAMED));
            }
            format!(
                "{} files still have conflict markers: {}. Marking them resolved stages the markers as they are.",
                left.len(),
                named.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_warning_names_files_with_their_counts() {
        assert_eq!(
            markers_warning(&[("a.txt".into(), 2)]),
            "a.txt still has 2 conflict marker regions. Marking it resolved stages the markers as they are."
        );
        assert!(markers_warning(&[("a.txt".into(), 1)]).starts_with("a.txt still has 1 conflict marker region."));
        let many: Vec<(String, usize)> = (1..=7).map(|i| (format!("f{i}.txt"), i)).collect();
        assert_eq!(
            markers_warning(&many),
            "7 files still have conflict markers: f1.txt (1), f2.txt (2), f3.txt (3), f4.txt (4), f5.txt (5), and 2 more. \
             Marking them resolved stages the markers as they are."
        );
    }

    #[test]
    fn mark_conflicted_asks_about_one_or_several() {
        let one = ["a.txt".to_owned()];
        assert_eq!(
            mark_conflicted_body(Lang::En, &one),
            "Mark a.txt conflicted again? git rewrites the file with conflict markers from both sides. The staged resolution and \
             any edits to the file are lost."
        );
        let three: Vec<String> = ["a", "b", "c"].map(String::from).into();
        assert_eq!(
            mark_conflicted_body(Lang::En, &three),
            "Mark 3 files conflicted again? git rewrites each with conflict markers from both sides. Their staged resolutions and \
             any edits to them are lost."
        );
        let de = mark_conflicted_body(Lang::De, &one);
        assert!(de.contains("a.txt") && de != mark_conflicted_body(Lang::En, &one), "{de}");
        assert!(mark_conflicted_body(Lang::De, &three).contains('3'));
    }

    #[test]
    fn count_notes() {
        let files = ConflictFiles {
            counts: [("a".to_owned(), Some(2)), ("b".to_owned(), Some(1)), ("c".to_owned(), Some(0)), ("d".to_owned(), None)].into(),
            ..Default::default()
        };
        let en = Lang::En;
        assert_eq!(files.count_note(en, "a"), Some(("2 conflicts".into(), false)));
        assert_eq!(files.count_note(en, "b"), Some(("1 conflict".into(), false)));
        assert_eq!(files.count_note(en, "c"), Some(("no markers left".into(), true)));
        assert_eq!((files.count_note(en, "d"), files.count_note(en, "e")), (None, None));
        assert_ne!(files.count_note(Lang::De, "a"), files.count_note(en, "a"));
        assert_ne!(files.count_note(Lang::De, "c"), files.count_note(en, "c"));
    }
}
