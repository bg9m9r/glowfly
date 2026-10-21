//! What Glowfly tells about Git LFS outside the preferences page: the main window's banner when
//! a repository stores files in LFS but git-lfs is missing, the badge on file rows that hold
//! pointers, and the summary a diff shows instead of pointer text (ui/lfs_banner.rs,
//! ui/lfs_badge.rs, ui/diff/lfs_note.rs; the state is `lfs::LfsSession`).

use super::AppState;
use super::lfs::{LfsDiffNote, LfsInstall};
use glowfly_core::lfs::pointer::{LfsChange, LfsSide, Pointer, size_text};
use glowfly_core::paths::same_spelling;
use std::path::Path;

pub const BANNER_TEXT: &str = "This repository stores files in Git LFS, but git-lfs isn't installed. Changes to those files may be \
                               committed as regular files instead of LFS pointers.";
pub const BANNER_SETTINGS: &str = "Git LFS settings…";
pub const BANNER_GET: &str = "Get Git LFS";
pub const BANNER_DISMISS: &str = "Dismiss";
pub const BADGE: &str = "LFS";
pub const BADGE_TIP: &str = "Stored in Git LFS: Git keeps a small pointer file and git-lfs keeps the content.";
pub const SUMMARY_SHOW_POINTER: &str = "Show pointer text";
pub const POINTER_BAR: &str = "Showing the Git LFS pointer text.";
pub const SHOW_SUMMARY: &str = "Show summary";

const STORED: &str = "Stored in Git LFS";

/// The active workdir when its banner shows: git-lfs is known to be missing, the root
/// .gitattributes stores patterns in LFS, and the banner wasn't dismissed for it this session.
pub fn banner_workdir(st: &AppState) -> Option<&Path> {
    let snap = st.snap.as_ref()?;
    let shows = st.lfs.install == LfsInstall::Missing
        && !snap.lfs_patterns.is_empty()
        && !st.lfs.banner_dismissed.iter().any(|d| same_spelling(d, &snap.workdir));
    shows.then_some(snap.workdir.as_path())
}

/// The open diff's note when it is for the diff on screen.
fn note(st: &AppState) -> Option<&LfsDiffNote> {
    let (target, _) = st.diff.as_ref()?;
    st.lfs.diff.as_ref().filter(|n| n.target == *target)
}

/// The note when the open diff shows its LFS summary instead of rows.
pub fn summary(st: &AppState) -> Option<&LfsDiffNote> {
    note(st).filter(|n| !n.show_pointer)
}

/// Whether the open diff is an LFS one showing its pointer text ("Show pointer text").
pub fn pointer_shown(st: &AppState) -> bool {
    note(st).is_some_and(|n| n.show_pointer)
}

/// The summary's text: a title, lines (each with the oid to show on hover, if any) and a warning.
#[derive(Clone, Debug, PartialEq)]
pub struct LfsSummary {
    pub title: String,
    pub lines: Vec<(String, Option<String>)>,
    pub warning: Option<String>,
}

fn oid(p: &Pointer) -> String {
    format!("sha256:{}", p.oid)
}

/// What the summary says about `c` (sizes in 1024-based units, "was" for the old side).
pub fn summary_text(c: &LfsChange, install: &LfsInstall) -> LfsSummary {
    let plain = |title: &str, lines: Vec<(String, Option<String>)>| LfsSummary { title: title.into(), lines, warning: None };
    match (&c.old, &c.new) {
        (LfsSide::Absent, LfsSide::Pointer(p)) => plain(STORED, vec![(format!("Size: {}", size_text(p.size)), Some(oid(p)))]),
        (LfsSide::Pointer(p), LfsSide::Absent) => plain(STORED, vec![(format!("Size: {} (deleted)", size_text(p.size)), Some(oid(p)))]),
        (LfsSide::Pointer(a), LfsSide::Pointer(b)) if a.oid == b.oid => {
            plain(STORED, vec![("The content didn't change; only the pointer text did.".into(), Some(oid(b)))])
        }
        (LfsSide::Pointer(a), LfsSide::Pointer(b)) => plain(
            STORED,
            vec![(format!("Size: {}, was {}", size_text(b.size), size_text(a.size)), Some(format!("{}\nwas {}", oid(b), oid(a))))],
        ),
        (LfsSide::Regular { size }, LfsSide::Pointer(p)) => plain(
            "Moved into Git LFS",
            vec![(format!("Size: {}, was a regular file of {}", size_text(p.size), size_text(*size)), Some(oid(p)))],
        ),
        (LfsSide::Pointer(p), LfsSide::Regular { size }) => {
            // The working copy git-lfs smudged: what the pointer stands for, not a change of storage.
            if c.new_in_workdir && c.filtered && matches!(install, LfsInstall::Installed(_)) {
                let line = format!("Working copy: {}, committed version: {}", size_text(*size), size_text(p.size));
                plain(STORED, vec![(line, Some(oid(p)))])
            } else {
                LfsSummary {
                    title: format!("Was stored in Git LFS ({})", size_text(p.size)),
                    lines: Vec::new(),
                    warning: Some(format!(
                        "This version is a regular file ({}), not an LFS pointer: committing it stores the whole file in Git.",
                        size_text(*size)
                    )),
                }
            }
        }
        // Neither side is a pointer: no note is made for such a diff.
        _ => plain(STORED, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ptr(size: u64, digit: char) -> LfsSide {
        LfsSide::Pointer(Pointer { oid: digit.to_string().repeat(64), size })
    }

    fn change(old: LfsSide, new: LfsSide) -> LfsChange {
        LfsChange { old, new, new_in_workdir: false, filtered: false }
    }

    const MB: u64 = 1024 * 1024;
    const OLD: u64 = 13_002_342; // 12.4 MB
    const NEW: u64 = 13 * MB;

    fn lines(s: &LfsSummary) -> Vec<&str> {
        s.lines.iter().map(|(l, _)| l.as_str()).collect()
    }

    #[test]
    fn added_deleted_and_changed_pointers() {
        let installed = LfsInstall::Installed("3.7.1".into());
        let added = summary_text(&change(LfsSide::Absent, ptr(OLD, 'a')), &installed);
        assert_eq!((added.title.as_str(), lines(&added), added.warning.as_deref()), (STORED, vec!["Size: 12.4 MB"], None));
        assert_eq!(added.lines[0].1.as_deref(), Some(format!("sha256:{}", "a".repeat(64)).as_str()), "the oid on hover");
        let deleted = summary_text(&change(ptr(OLD, 'a'), LfsSide::Absent), &installed);
        assert_eq!((deleted.title.as_str(), lines(&deleted)), (STORED, vec!["Size: 12.4 MB (deleted)"]));
        let changed = summary_text(&change(ptr(OLD, 'a'), ptr(NEW, 'b')), &installed);
        assert_eq!(
            (changed.title.as_str(), lines(&changed), changed.warning.as_deref()),
            (STORED, vec!["Size: 13.0 MB, was 12.4 MB"], None)
        );
        let same = summary_text(&change(ptr(OLD, 'a'), ptr(OLD, 'a')), &installed);
        assert_eq!(lines(&same), ["The content didn't change; only the pointer text did."]);
    }

    #[test]
    fn a_regular_file_moved_into_lfs() {
        let s = summary_text(&change(LfsSide::Regular { size: 3 * 1024 }, ptr(3 * 1024, 'a')), &LfsInstall::Missing);
        assert_eq!(
            (s.title.as_str(), lines(&s), s.warning.as_deref()),
            ("Moved into Git LFS", vec!["Size: 3.0 KB, was a regular file of 3.0 KB"], None)
        );
    }

    #[test]
    fn a_smudged_working_copy_is_no_warning_but_a_regular_file_is() {
        let size = 13_736_345; // 13.1 MB
        let smudged = LfsChange { new_in_workdir: true, filtered: true, ..change(ptr(OLD, 'a'), LfsSide::Regular { size }) };
        let s = summary_text(&smudged, &LfsInstall::Installed("3.7.1".into()));
        assert_eq!(
            (s.title.as_str(), lines(&s), s.warning.as_deref()),
            (STORED, vec!["Working copy: 13.1 MB, committed version: 12.4 MB"], None)
        );
        let warning = "This version is a regular file (13.1 MB), not an LFS pointer: committing it stores the whole file in Git.";
        let missing = summary_text(&smudged, &LfsInstall::Missing);
        assert_eq!((missing.title.as_str(), missing.warning.as_deref()), ("Was stored in Git LFS (12.4 MB)", Some(warning)));
        let staged = summary_text(&change(ptr(OLD, 'a'), LfsSide::Regular { size }), &LfsInstall::Installed("3.7.1".into()));
        assert_eq!(
            (staged.title.as_str(), staged.warning.as_deref()),
            ("Was stored in Git LFS (12.4 MB)", Some(warning)),
            "a staged regular file"
        );
        let unfiltered = LfsChange { filtered: false, ..smudged };
        assert!(summary_text(&unfiltered, &LfsInstall::Installed("3.7.1".into())).warning.is_some(), "no filter=lfs");
    }
}
