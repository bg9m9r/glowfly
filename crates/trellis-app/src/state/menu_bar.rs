//! What the menu bar's items say and whether they are enabled (ui/menu_bar/ draws them): the
//! Window menu's key hints, the zoom limits, the Edit menu's copy items and About's version text.

use super::{AppState, Selection};
use crate::keymap::{BindingId, Chord, KeyName, Keymap};
use crate::state::git_env::GitState;
use trellis_core::settings::{DEFAULT_ZOOM_PERCENT, ZOOM_STEPS};

/// Help > Trellis website.
pub const WEBSITE: &str = env!("CARGO_PKG_REPOSITORY");
/// Help > Report an issue.
pub const ISSUES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues");
/// The version in About and the copied version info.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The licence line in About.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// Why Edit's copy items are disabled without a commit.
pub const NO_COMMIT: &str = "Select a commit in the graph first";
/// Why Copy branch name is disabled.
pub const NO_BRANCH: &str = "No branch is checked out";

/// The Window menu's key for tab `i` of `len`: Go to tab's key with the tab's digit for the first
/// eight, the Go to the last tab key for a last tab beyond them, else none.
pub fn tab_hint(keys: &Keymap, i: usize, len: usize) -> Option<String> {
    if i < 8 {
        let first = keys.chords(BindingId::GoToTab).first()?;
        return Some(Chord { key: KeyName::Digit(i as u8 + 1), ..*first }.text());
    }
    if i + 1 == len { keys.hint(BindingId::LastTab).map(str::to_owned) } else { None }
}

/// Why Zoom in, Zoom out and Reset zoom are disabled at zoom `level`, in that order.
pub fn zoom_blocked(level: u16) -> [Option<String>; 3] {
    let (min, max) = (ZOOM_STEPS[0], ZOOM_STEPS[ZOOM_STEPS.len() - 1]);
    [
        (level >= max).then(|| format!("Already at the largest zoom ({max}%)")),
        (level <= min).then(|| format!("Already at the smallest zoom ({min}%)")),
        (level == DEFAULT_ZOOM_PERCENT).then(|| format!("Already at {DEFAULT_ZOOM_PERCENT}%")),
    ]
}

/// What Copy commit SHA copies.
#[derive(Debug, PartialEq)]
pub enum ShaCopy {
    /// One commit's full SHA.
    One(String),
    /// A multi-selection of this many commits (`ui::menus::copy_shas` joins them).
    Many(usize),
}

impl ShaCopy {
    pub fn label(&self) -> String {
        match self {
            ShaCopy::One(_) => "Copy commit SHA".to_owned(),
            ShaCopy::Many(n) => format!("Copy {n} commit SHAs"),
        }
    }
}

/// Copy commit SHA for the selection, or why it is disabled.
pub fn copy_sha(st: &AppState) -> Result<ShaCopy, &'static str> {
    match &st.selection {
        Selection::Commit(oid) => Ok(ShaCopy::One(oid.to_string())),
        Selection::Commits(m) => Ok(ShaCopy::Many(m.oids.len())),
        Selection::None | Selection::Wip => Err(NO_COMMIT),
    }
}

/// The selected commit's summary for Copy commit message, or why it is disabled.
pub fn copy_message(st: &AppState) -> Result<String, &'static str> {
    let (Selection::Commit(oid), Some(snap)) = (&st.selection, &st.snap) else { return Err(NO_COMMIT) };
    snap.row_of.get(oid).map(|&row| snap.commits[row].summary.clone()).ok_or(NO_COMMIT)
}

/// The checked-out branch for Copy branch name, or why it is disabled.
pub fn copy_branch(st: &AppState) -> Result<String, &'static str> {
    st.snap.as_ref().and_then(|s| s.head_branch.clone()).ok_or(NO_BRANCH)
}

/// About's git line, "Git 2.47.1", when the startup check found a version.
pub fn git_line(git: &GitState) -> Option<String> {
    let v = git.info.as_ref()?.version?;
    Some(format!("Git {}", trellis_core::git_version::show(v)))
}

/// About's Copy version info: "Trellis 0.1.0" and the git line with where git is, for bug
/// reports.
pub fn version_info(git: &GitState) -> String {
    let mut out = format!("Trellis {VERSION}");
    if let Some(line) = git_line(git) {
        out.push('\n');
        out.push_str(&line);
        if let Some(path) = git.info.as_ref().and_then(|i| i.path.as_ref()) {
            out.push_str(&format!(" ({})", trellis_core::native_path::shown(path)));
        }
    }
    out
}

/// License text for a dialog that wraps to its width: the lines of each paragraph joined with
/// spaces, so the file's hard line breaks don't leave ragged half-lines. A line that starts
/// indented or as a list item keeps its own line, and blank lines still separate paragraphs.
pub fn reflow(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_blank = true;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if blank {
            if !prev_blank {
                out.push_str("\n\n");
            }
        } else if prev_blank {
            out.push_str(line.trim_end());
        } else if starts_own_line(line) {
            out.push('\n');
            out.push_str(line.trim_end());
        } else {
            out.push(' ');
            out.push_str(line.trim());
        }
        prev_blank = blank;
    }
    out.trim_end().to_owned()
}

/// An indented line or a list item ("-", "*", "1.", "a)", "(b)").
fn starts_own_line(line: &str) -> bool {
    if line.starts_with(char::is_whitespace) {
        return true;
    }
    let first = line.split_whitespace().next().unwrap_or_default();
    if matches!(first, "-" | "*" | "•") {
        return true;
    }
    let mark = first.trim_start_matches('(');
    let body = mark.trim_end_matches(['.', ')']);
    mark.len() > body.len() && !body.is_empty() && body.len() <= 3 && body.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Tab `i`'s name as the tab strip shows it (the active tab's from its loaded repository).
pub fn tab_name(st: &AppState, i: usize) -> &str {
    match (&st.snap, i == st.active_tab) {
        (Some(snap), true) => snap.name.as_str(),
        _ => st.tabs[i].name.as_str(),
    }
}

#[cfg(test)]
#[path = "menu_bar_tests.rs"]
mod tests;
