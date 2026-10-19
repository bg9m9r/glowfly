//! The diff header as plain data, built once per frame by ui/diff/header/ and tested without
//! egui: the file's folder, name, kind and change counts, the context it is shown in, where
//! Previous and Next go and the counter between them, the View menu's settings, the file
//! actions with their disabled reasons, and the one primary action.

use super::diff_nav::{self, Counter, StepKind};
use super::files::NOT_IN_WORKDIR;
use super::{AppState, Selection};
use git2::Oid;
use trellis_core::preview_image::is_image_path;
use trellis_core::repo::{DiffTarget, FileDiff, Snapshot};
use trellis_core::settings::{DiffLayout, DiffMode};
use trellis_core::whole_file;

/// The highlight's name for files it found no language for.
const PLAIN_TEXT: &str = "Plain text";

/// Everything the header draws.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderModel {
    pub target: DiffTarget,
    /// The whole path, for the name's hover when it is cut.
    pub path: String,
    /// The folder with its trailing '/' ("src/ui/"), or "" at the top level.
    pub folder: String,
    pub name: String,
    pub kind: FileKind,
    /// The status letter (untracked shows as A), `None` when the file's list isn't loaded.
    pub status: Option<char>,
    pub change: ChangeText,
    pub context: Context,
    pub prev: StepKind,
    pub next: StepKind,
    /// Whether the arrows show: there are rows, or a neighbouring file to step to.
    pub show_nav: bool,
    pub counter: Option<Counter>,
    /// The View menu; `None` for binary and LFS-summary diffs.
    pub view: Option<ViewGroup>,
    pub actions: Actions,
    pub primary: Option<Primary>,
}

/// Which icon the file gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Image,
    Binary,
    Code,
    Text,
}

/// What the status chip says after the letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeText {
    /// A text diff's added and removed lines, as shown.
    Lines {
        added: usize,
        removed: usize,
    },
    /// A binary file's sizes; both `None` until the sides arrive (or when they failed).
    Sizes {
        old: Option<u64>,
        new: Option<u64>,
    },
    None,
}

/// Where the diff comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Context {
    /// A commit's change: its short id, and its summary when known.
    Commit {
        short: String,
        summary: Option<String>,
    },
    Unstaged,
    Staged,
    /// "a1b2c3d..e4f5a6b".
    Between(String),
    /// "a1b2c3d..working tree".
    ToWorkdir(String),
}

/// How many folds of the whole file on screen are open and closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoldCounts {
    pub open: usize,
    pub closed: usize,
}

/// The View menu's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewGroup {
    pub layout: DiffLayout,
    pub mode: DiffMode,
    /// The diff has rows to lay out (else the menu holds only Ignore whitespace).
    pub rows: bool,
    pub ignore_ws: bool,
    /// The wrap setting, with rows only.
    pub wrap: Option<bool>,
    /// In the whole-file view with folds on screen.
    pub folds: Option<FoldCounts>,
}

impl ViewGroup {
    /// The button's label: "Inline · Hunks", or "View" without rows.
    pub fn label(&self) -> String {
        if !self.rows {
            return VIEW.to_owned();
        }
        let layout = match self.layout {
            DiffLayout::Inline => "Inline",
            DiffLayout::Split => "Split",
        };
        let mode = match self.mode {
            DiffMode::Hunks => "Hunks",
            DiffMode::WholeFile => "Whole file",
        };
        format!("{layout} \u{b7} {mode}")
    }
}

/// The View button's short label.
pub const VIEW: &str = "View";

/// Restore a commit's version of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreAction {
    pub oid: Oid,
    pub path: String,
    pub status: char,
}

/// Open and Show in folder for the working file `path`, off with `missing` when it is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenAction {
    pub path: String,
    pub missing: Option<&'static str>,
}

/// The file actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actions {
    pub restore: Option<RestoreAction>,
    pub open: Option<OpenAction>,
    /// The external diff tool, always usable, with its setup note while no tool is set
    /// (`Some(None)` once one is: running it then opens the tool, else Preferences).
    pub diff_tool: Option<Option<&'static str>>,
}

impl Actions {
    /// Whether any labelled action (Restore, Open, Show in folder) shows.
    pub fn labelled(&self) -> bool {
        self.restore.is_some() || self.open.is_some()
    }
}

/// The header's one filled button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Primary {
    Stage(String),
    Unstage(String),
}

/// The header of the diff on screen.
pub fn model(st: &AppState) -> Option<HeaderModel> {
    let (target, diff) = st.diff.as_ref()?;
    let path = target.path().to_owned();
    let (folder, name) = split_path(&path);
    let lfs_summary = super::lfs_notes::summary(st).is_some();
    let has_rows = !diff.binary && !diff.hunks.is_empty() && !lfs_summary;
    let [prev, next] = [diff_nav::step(st, -1).kind(), diff_nav::step(st, 1).kind()];
    let change = if diff.binary {
        let sides = super::diff_preview::shown(st);
        ChangeText::Sizes {
            old: sides.and_then(|s| s.old.as_ref()).map(|s| s.size),
            new: sides.and_then(|s| s.new.as_ref()).map(|s| s.size),
        }
    } else if has_rows {
        let (added, removed) = line_counts(diff);
        ChangeText::Lines { added, removed }
    } else {
        ChangeText::None
    };
    Some(HeaderModel {
        folder,
        name,
        kind: kind(&path, diff),
        status: diff_nav::file_status(st, target).map(|s| if s == '?' { 'A' } else { s }),
        change,
        context: context(st, target),
        prev,
        next,
        show_nav: has_rows || prev != StepKind::End || next != StepKind::End,
        counter: diff_nav::counter(st),
        view: (!diff.binary && !lfs_summary).then(|| view_group(st, diff, has_rows)),
        actions: actions(st, target),
        primary: match target {
            DiffTarget::Unstaged(p) => Some(Primary::Stage(p.clone())),
            DiffTarget::Staged(p) => Some(Primary::Unstage(p.clone())),
            _ => None,
        },
        path,
        target: target.clone(),
    })
}

/// `p` split at its last '/' into the folder (with the '/') and the name, bytes as given.
fn split_path(p: &str) -> (String, String) {
    match p.rfind('/') {
        Some(i) => (p[..=i].to_owned(), p[i + 1..].to_owned()),
        None => (String::new(), p.to_owned()),
    }
}

fn kind(path: &str, d: &FileDiff) -> FileKind {
    if is_image_path(path) {
        FileKind::Image
    } else if d.binary {
        FileKind::Binary
    } else if d.highlight.as_ref().is_some_and(|h| h.syntax != PLAIN_TEXT) {
        FileKind::Code
    } else {
        FileKind::Text
    }
}

/// The '+' and '-' lines of `d`'s hunks.
pub fn line_counts(d: &FileDiff) -> (usize, usize) {
    let lines = || d.hunks.iter().flat_map(|h| &h.lines);
    (lines().filter(|l| l.origin == '+').count(), lines().filter(|l| l.origin == '-').count())
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

fn context(st: &AppState, target: &DiffTarget) -> Context {
    match target {
        DiffTarget::Commit(oid, _) => Context::Commit { short: short(*oid), summary: summary(st, *oid) },
        DiffTarget::Unstaged(_) => Context::Unstaged,
        DiffTarget::Staged(_) => Context::Staged,
        DiffTarget::Between(a, b, _) => Context::Between(format!("{}..{}", short(*a), short(*b))),
        DiffTarget::ToWorkdir(o, _) => Context::ToWorkdir(format!("{}..working tree", short(*o))),
    }
}

/// The commit's summary line: from the open detail, else the loaded history.
fn summary(st: &AppState, oid: Oid) -> Option<String> {
    if let Some(d) = st.detail.as_ref().filter(|d| d.oid == oid) {
        return d.message.lines().next().map(str::to_owned);
    }
    let snap = st.snap.as_ref()?;
    snap.row_of.get(&oid).and_then(|&i| snap.commits.get(i)).map(|c| c.summary.clone())
}

fn view_group(st: &AppState, diff: &FileDiff, rows: bool) -> ViewGroup {
    let s = &st.settings;
    let folds = whole_file::shown(diff, s.diff_mode)
        .filter(|_| rows && s.diff_mode == DiffMode::WholeFile)
        .map(|w| {
            let all = whole_file::folds(w);
            let open = all.iter().filter(|f| st.view.diff_expanded.contains(f.key)).count();
            FoldCounts { open, closed: all.len() - open }
        })
        .filter(|c| c.open + c.closed > 0);
    ViewGroup {
        layout: s.diff_layout,
        mode: s.diff_mode,
        rows,
        ignore_ws: s.diff_ignore_whitespace,
        wrap: rows.then_some(s.diff_word_wrap),
        folds,
    }
}

fn actions(st: &AppState, target: &DiffTarget) -> Actions {
    let diff_tool = Some(super::tool_compare::ToolSetup::of(&st.settings.external_tools).diff);
    let none = Actions { restore: None, open: None, diff_tool };
    match target {
        // Restore, Open and Show for a commit's file, when the open detail lists it.
        DiffTarget::Commit(oid, path) => {
            let Some(d) = st.detail.as_ref().filter(|d| d.oid == *oid) else { return none };
            let Some(f) = d.files.iter().find(|f| f.path == *path) else { return none };
            Actions {
                restore: Some(RestoreAction { oid: *oid, path: f.path.clone(), status: f.status }),
                open: Some(OpenAction { path: f.path.clone(), missing: (!d.in_workdir(&f.path)).then_some(NOT_IN_WORKDIR) }),
                diff_tool,
            }
        }
        DiffTarget::Unstaged(p) | DiffTarget::Staged(p) | DiffTarget::ToWorkdir(_, p) => Actions {
            restore: None,
            open: Some(OpenAction { path: p.clone(), missing: (!working_file_on_disk(st, target)).then_some(NOT_IN_WORKDIR) }),
            diff_tool,
        },
        DiffTarget::Between(..) => none,
    }
}

/// Whether the working file of a working-tree diff is there: not for an unstaged deletion, nor
/// for a staged one unless the path is listed again as untracked; for a comparison with the
/// working tree, as the comparison found it.
fn working_file_on_disk(st: &AppState, target: &DiffTarget) -> bool {
    let snap: Option<&Snapshot> = st.snap.as_ref();
    let unstaged = |p: &str| snap.and_then(|s| s.unstaged.iter().find(|f| f.path == p)).map(|f| f.status);
    match target {
        DiffTarget::Unstaged(p) => unstaged(p) != Some('D'),
        DiffTarget::Staged(p) => {
            let staged = snap.and_then(|s| s.staged.iter().find(|f| f.path == *p)).map(|f| f.status);
            staged != Some('D') || unstaged(p) == Some('?')
        }
        DiffTarget::ToWorkdir(o, p) => match &st.selection {
            Selection::VsWorkdir(w) if w.oid == *o => match &w.changes {
                Some(Ok(c)) => c.in_workdir(p),
                _ => diff_nav::file_status(st, target) != Some('D'),
            },
            _ => diff_nav::file_status(st, target) != Some('D'),
        },
        DiffTarget::Commit(..) | DiffTarget::Between(..) => true,
    }
}

#[cfg(test)]
#[path = "diff_header_tests.rs"]
mod tests;
