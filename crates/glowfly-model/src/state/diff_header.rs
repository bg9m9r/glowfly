//! The diff header as plain data, built once per frame by ui/diff/header/ and tested without
//! egui: the file's folder, name, kind and change counts, the context it is shown in, where
//! Previous and Next go and the counter between them, the View menu's settings, the file
//! actions with their disabled reasons, and the one primary action. Its slow parts (change rows,
//! counts, file order) come from a `HeaderCache` that keeps them between frames.

pub mod cache;

use super::diff_nav::{self, Counter, StepKind};
use super::files::not_in_workdir;
use super::{AppState, Selection};
use cache::HeaderCache;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::preview_image::is_image_path;
use glowfly_core::repo::{DiffTarget, FileDiff, Snapshot, is_empty_tree};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file;

/// The highlight's name for files it found no language for: the syntax set's own id, compared
/// with and never shown, so it stays as the syntax set spells it.
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
    /// "working tree..a1b2c3d".
    FromWorkdir(String),
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
    pub fn label(&self, t: Lang) -> String {
        if !self.rows {
            return t.tr("diff.header.view.button").to_owned();
        }
        let layout = match self.layout {
            DiffLayout::Inline => t.tr("diff.header.view.inline"),
            DiffLayout::Split => t.tr("diff.header.view.split"),
        };
        let mode = match self.mode {
            DiffMode::Hunks => t.tr("diff.header.view.hunks"),
            DiffMode::WholeFile => t.tr("diff.header.view.whole_file"),
        };
        t.trf("diff.header.view.label", &[("layout", &layout), ("mode", &mode)])
    }
}

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

/// The header of the diff on screen, its slow parts built fresh; the diff view keeps them in a
/// `HeaderCache` between frames.
pub fn model(st: &AppState) -> Option<HeaderModel> {
    HeaderCache::default().model(st)
}

/// The header of the diff on screen, from `parts` (refreshed for `st`).
pub(super) fn build(st: &AppState, parts: &HeaderCache) -> Option<HeaderModel> {
    let (target, diff) = st.session.diff.view.as_ref()?;
    let path = target.path().to_owned();
    let (folder, name) = split_path(&path);
    let lfs_summary = super::lfs_notes::summary(st).is_some();
    let has_rows = !diff.binary && !diff.hunks.is_empty() && !lfs_summary;
    let pos = diff_nav::position(st);
    let order = &parts.order;
    let step = |dir: isize| diff_nav::step_from(&parts.rows, pos, dir, || order.neighbour(dir).cloned()).kind();
    let [prev, next] = [step(-1), step(1)];
    let change = if diff.binary {
        let sides = super::diff_preview::shown(st);
        ChangeText::Sizes {
            old: sides.and_then(|s| s.old.as_ref()).map(|s| s.size),
            new: sides.and_then(|s| s.new.as_ref()).map(|s| s.size),
        }
    } else if has_rows {
        let (added, removed) = parts.counts;
        ChangeText::Lines { added, removed }
    } else {
        ChangeText::None
    };
    Some(HeaderModel {
        folder,
        name,
        kind: kind(&path, diff),
        status: order.status.map(|s| if s == '?' { 'A' } else { s }),
        change,
        context: context(st, target, st.settings.lang()),
        prev,
        next,
        show_nav: has_rows || prev != StepKind::End || next != StepKind::End,
        counter: diff_nav::counter_from(&parts.rows, pos, || order.place),
        view: (!diff.binary && !lfs_summary).then(|| view_group(st, diff, has_rows)),
        actions: actions(st, target, order.status),
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
pub fn split_path(p: &str) -> (String, String) {
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

fn context(st: &AppState, target: &DiffTarget, t: Lang) -> Context {
    match target {
        DiffTarget::Commit(oid, _) => Context::Commit { short: short(*oid), summary: summary(st, *oid) },
        DiffTarget::Unstaged(_) => Context::Unstaged,
        DiffTarget::Staged(_) => Context::Staged,
        DiffTarget::Between(a, b, _) => {
            // A line of commits from the first one is compared with the empty tree.
            let end = |o: Oid| if is_empty_tree(o) { t.tr("diff.header.empty_tree").to_owned() } else { short(o) };
            Context::Between(format!("{}..{}", end(*a), end(*b)))
        }
        DiffTarget::ToWorkdir(o, _) => Context::ToWorkdir(t.trf("diff.header.to_workdir", &[("sha", &short(*o))])),
        DiffTarget::FromWorkdir(o, _) => Context::FromWorkdir(t.trf("diff.header.from_workdir", &[("sha", &short(*o))])),
    }
}

/// The commit's summary line: from the open detail, else the loaded history.
fn summary(st: &AppState, oid: Oid) -> Option<String> {
    if let Some(d) = st.session.detail.as_ref().filter(|d| d.oid == oid) {
        return d.message.lines().next().map(str::to_owned);
    }
    let snap = st.snap.as_ref()?;
    snap.row_of.get(&oid).and_then(|&i| snap.commits.get(i)).map(|c| c.summary.to_string())
}

fn view_group(st: &AppState, diff: &FileDiff, rows: bool) -> ViewGroup {
    let s = &st.settings;
    let folds = whole_file::shown(diff, s.diff_mode)
        .filter(|_| rows && s.diff_mode == DiffMode::WholeFile)
        .map(|w| {
            let all = whole_file::folds(w);
            let open = all.iter().filter(|f| st.session.tab_view.diff_expanded.contains(f.key)).count();
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

fn actions(st: &AppState, target: &DiffTarget, status: Option<char>) -> Actions {
    let t = st.settings.lang();
    let diff_tool = Some(super::tool_compare::ToolSetup::of(t, &st.settings.external_tools).diff);
    let none = Actions { restore: None, open: None, diff_tool };
    match target {
        // Restore, Open and Show for a commit's file, when the open detail lists it.
        DiffTarget::Commit(oid, path) => {
            let Some(d) = st.session.detail.as_ref().filter(|d| d.oid == *oid) else { return none };
            let Some(f) = d.files.iter().find(|f| f.path == *path) else { return none };
            Actions {
                restore: Some(RestoreAction { oid: *oid, path: f.path.clone(), status: f.status }),
                open: Some(OpenAction { path: f.path.clone(), missing: (!d.in_workdir(&f.path)).then(|| not_in_workdir(t)) }),
                diff_tool,
            }
        }
        DiffTarget::Unstaged(p) | DiffTarget::Staged(p) | DiffTarget::ToWorkdir(_, p) | DiffTarget::FromWorkdir(_, p) => Actions {
            restore: None,
            open: Some(OpenAction { path: p.clone(), missing: (!working_file_on_disk(st, target, status)).then(|| not_in_workdir(t)) }),
            diff_tool,
        },
        DiffTarget::Between(..) => none,
    }
}

/// Whether the working file of a working-tree diff is there: not for an unstaged deletion, nor
/// for a staged one unless the path is listed again as untracked; for a comparison with the
/// working tree, as the comparison found it (from the working tree to a commit, its 'A' rows are
/// the files the working tree lacks; `status` is the file's letter in that list).
fn working_file_on_disk(st: &AppState, target: &DiffTarget, status: Option<char>) -> bool {
    let snap: Option<&Snapshot> = st.snap.as_ref();
    let unstaged = |p: &str| snap.and_then(|s| s.unstaged.iter().find(|f| f.path == p)).map(|f| f.status);
    match target {
        DiffTarget::Unstaged(p) => unstaged(p) != Some('D'),
        DiffTarget::Staged(p) => {
            let staged = snap.and_then(|s| s.staged.iter().find(|f| f.path == *p)).map(|f| f.status);
            staged != Some('D') || unstaged(p) == Some('?')
        }
        DiffTarget::ToWorkdir(o, p) => match &st.session.selection {
            Selection::VsWorkdir(w) if w.oid == *o && !w.swapped => match &w.changes {
                Some(Ok(c)) => c.in_workdir(p),
                _ => status != Some('D'),
            },
            _ => status != Some('D'),
        },
        DiffTarget::FromWorkdir(o, p) => match &st.session.selection {
            Selection::VsWorkdir(w) if w.oid == *o && w.swapped => match &w.changes {
                Some(Ok(c)) => c.in_workdir(p),
                _ => status != Some('A'),
            },
            _ => status != Some('A'),
        },
        DiffTarget::Commit(..) | DiffTarget::Between(..) => true,
    }
}
