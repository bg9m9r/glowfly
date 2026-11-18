//! The rules of Take the trivial picks in all files (update/conflict_trivial.rs): which
//! conflicted files it is sent for and which it holds back (the one open in the conflict view
//! with unsaved choices or a save running, and those open in the merge tool), why it is off,
//! and the words of its toast and its log line.

use crate::state::AppState;
use crate::state::conflict_files::{conflicted_paths, unmerged};
use crate::state::error_words::typed;
use crate::state::tool_compare::merging;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Snapshot, TrivialSkip};

/// How many skipped files the log line names before "and N more".
const NAMED: usize = 10;

/// Why a file is held back before the action is sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    /// The merge tool has it open.
    InTool,
    /// It is open in the conflict view with choices that aren't saved, or a save is running.
    Unsaved,
}

/// What the action is sent for: the files, and those held back with why.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Request {
    pub paths: Vec<String>,
    pub held: Vec<(String, Held)>,
}

/// The conflicted files with a name git can be given (`conflicted_paths`), split into those to
/// send and those held back.
pub fn request(st: &AppState) -> Request {
    let Some(snap) = &st.snap else { return Request::default() };
    let mut out = Request::default();
    for p in conflicted_paths(snap) {
        let open = st.session.conflict.view.as_ref().filter(|c| c.path == p);
        if merging(st, &p) {
            out.held.push((p, Held::InTool));
        } else if open.is_some_and(|c| c.user_choices() > 0 || c.saving) {
            out.held.push((p, Held::Unsaved));
        } else {
            out.paths.push(p);
        }
    }
    out
}

/// Why the action can't run now, or `None`: nothing is conflicted, no conflicted file has a name
/// git can be given (the first row's hazard says why), or every usable file is held back.
pub fn blocked(st: &AppState, t: Lang) -> Option<String> {
    let rows = unmerged(st.snap.as_ref()?);
    if rows.is_empty() {
        return Some(t.tr("conflict.trivial.blocked.none").into());
    }
    if conflicted_paths(st.snap.as_ref()?).is_empty() {
        return Some(
            rows.iter().find_map(|f| f.hazard.as_ref()).map_or_else(|| t.tr("conflict.trivial.blocked.none").into(), |h| h.reason(t)),
        );
    }
    request(st).paths.is_empty().then(|| t.tr("conflict.trivial.blocked.held").into())
}

/// Why `held` was held back.
pub fn held_words(t: Lang, held: &Held) -> String {
    match held {
        Held::InTool => t.tr("conflict.trivial.why.in_tool").into(),
        Held::Unsaved => t.tr("conflict.trivial.why.unsaved").into(),
    }
}

/// Why a file was skipped.
pub fn skip_words(t: Lang, skip: &TrivialSkip) -> String {
    match skip {
        TrivialSkip::Binary => t.tr("conflict.trivial.why.binary").into(),
        TrivialSkip::NotText => t.tr("conflict.trivial.why.not_text").into(),
        TrivialSkip::Converted(attr) => t.trf("conflict.trivial.why.converted", &[("attr", attr)]),
        TrivialSkip::SavedPicks => t.tr("conflict.trivial.why.saved").into(),
        TrivialSkip::Edited => t.tr("conflict.trivial.why.edited").into(),
        TrivialSkip::Unreadable => t.tr("conflict.trivial.why.unreadable").into(),
        TrivialSkip::Failed(e) => typed::read(t, e),
    }
}

/// The log line naming the skipped (or held back) files, each with its reason: at most ten,
/// then "and N more".
pub fn skipped_text(t: Lang, files: &[(String, String)]) -> String {
    let mut named: Vec<String> =
        files.iter().take(NAMED).map(|(path, why)| t.trf("conflict.trivial.file", &[("path", path), ("why", why)])).collect();
    if files.len() > NAMED {
        named.push(t.trn("conflict.trivial.more", files.len() - NAMED, &[]));
    }
    t.trn("conflict.trivial.skipped", files.len(), &[("files", &named.join(", "))])
}

/// How many of the status's conflicted rows are not in `resolved`: the files still to look at,
/// whether left, skipped, held back or without a name git can be given.
pub fn remaining(snap: &Snapshot, resolved: &[String]) -> usize {
    unmerged(snap).iter().filter(|f| !resolved.contains(&f.path)).count()
}

/// The toast's words: what was resolved (or that nothing was), the files still needing a look,
/// and a pointer to the log when files were skipped.
pub fn summary(t: Lang, resolved: usize, remaining: usize, skipped: bool) -> String {
    let mut parts =
        vec![if resolved > 0 { t.trn("conflict.trivial.resolved", resolved, &[]) } else { t.tr("conflict.trivial.none").into() }];
    if remaining > 0 {
        parts.push(t.trn("conflict.trivial.remaining", remaining, &[]));
    }
    if skipped {
        parts.push(t.tr("conflict.trivial.see_log").into());
    }
    parts.join(" ")
}

/// The conflicted file to open after the open one, `after`, was resolved along with `resolved`:
/// the first path after it that isn't resolved, wrapping to the first.
pub fn next_after(snap: &Snapshot, resolved: &[String], after: &str) -> Option<String> {
    let paths: Vec<String> = conflicted_paths(snap).into_iter().filter(|p| !resolved.contains(p)).collect();
    paths.iter().find(|p| p.as_str() > after).or(paths.first()).filter(|p| p.as_str() != after).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::op_fixture::{rows, snapshot};
    use glowfly_core::repo::{FileEntry, Hazard};

    fn snap(name: &str, paths: &[&str]) -> Snapshot {
        let mut s = snapshot(name);
        s.unstaged = rows(&paths.iter().map(|p| (*p, '!')).collect::<Vec<_>>());
        s
    }

    #[test]
    fn the_summary_counts_what_happened() {
        let en = Lang::En;
        assert_eq!(
            summary(en, 2, 3, true),
            "Resolved 2 files with their trivial picks. 3 files still need a look. The activity log names the files that were skipped."
        );
        assert_eq!(summary(en, 1, 0, false), "Resolved 1 file with its trivial picks.");
        assert_eq!(summary(en, 0, 2, false), "No file has only trivial conflicts. 2 files still need a look.");
        assert_ne!(summary(Lang::De, 2, 3, true), summary(en, 2, 3, true));
    }

    #[test]
    fn the_log_line_names_ten_then_the_rest() {
        let few = [("logo.bin".to_owned(), "binary".to_owned()), ("data.csv".to_owned(), "git converts it, filter=lfs".to_owned())];
        assert_eq!(
            skipped_text(Lang::En, &few),
            "Take the trivial picks skipped 2 files: logo.bin (binary), data.csv (git converts it, filter=lfs)"
        );
        let many: Vec<(String, String)> = (1..=12).map(|i| (format!("f{i}"), "binary".to_owned())).collect();
        let text = skipped_text(Lang::En, &many);
        assert!(text.starts_with("Take the trivial picks skipped 12 files: f1 (binary), f2 (binary)"), "{text}");
        assert!(text.contains("f10 (binary), and 2 more") && !text.contains("f11"), "{text}");
        assert!(skipped_text(Lang::En, &few[..1]).starts_with("Take the trivial picks skipped 1 file: "));
    }

    #[test]
    fn remaining_counts_a_row_once_and_skips_the_resolved() {
        let mut s = snap("trivial-remaining", &["a", "b"]);
        let odd = FileEntry { hazard: Some(Hazard::NotUtf8(vec![0xff])), ..FileEntry::new("odd", '!') };
        s.unstaged.extend([odd.clone(), odd]);
        assert_eq!(remaining(&s, &[]), 3);
        assert_eq!(remaining(&s, &["a".to_owned()]), 2);
    }

    #[test]
    fn the_next_file_wraps_and_skips_the_resolved() {
        let s = snap("trivial-next", &["a", "b", "c", "d"]);
        let done = |v: &[&str]| v.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
        assert_eq!(next_after(&s, &done(&["b", "c"]), "b"), Some("d".into()));
        assert_eq!(next_after(&s, &done(&["c", "d"]), "d"), Some("a".into()));
        assert_eq!(next_after(&s, &done(&["a", "b", "c", "d"]), "b"), None);
    }
}
