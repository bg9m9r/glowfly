//! tabs.txt: the tabs open when Glowfly last closed, their labels and the active one. A save
//! appends the tabs a late read found that aren't open (see the parent module); they never
//! become the active tab, and come back at the next start.

use super::{Begun, Carried, SaveRefused, begin, finish, note_skipped, read_state_file, strip_bom};
use glowfly_core::persist::{self, KEPT_BACKUPS};
use glowfly_model::msg::SaveNotes;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn tabs_file() -> Option<PathBuf> {
    glowfly_core::settings::config_dir().map(|d| d.join("tabs.txt"))
}

/// The open tabs of a saved session: their folders, their labels (one per folder) and the
/// index of the active one.
pub(super) type SavedTabs = (Vec<PathBuf>, Vec<Option<String>>, usize);

/// The tabs open when Glowfly last closed, their labels and the index of the active one.
/// Nothing is looked at here (this runs before the window opens, and a network share may not
/// answer): a tab whose folder is gone fails to load when selected, like any other. Called once,
/// at start: only the newest `KEPT_BACKUPS` copies of the file are kept.
pub fn load_tabs() -> SavedTabs {
    let Some(f) = tabs_file() else { return SavedTabs::default() };
    let tabs = load_tabs_with(&f, &glowfly_core::settings::READ_WAITS, |p| std::fs::read(p));
    persist::prune_backups(&f, KEPT_BACKUPS);
    tabs
}

/// `load_tabs` from `file`, reading with `read` (see `read_state_file`).
pub(super) fn load_tabs_with(file: &Path, waits: &[Duration], read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> SavedTabs {
    let Some(bytes) = read_state_file(file, waits, read) else { return SavedTabs::default() };
    let (tabs, skipped) = parse_tabs_bytes(&bytes);
    note_skipped(file, skipped, bytes);
    tabs
}

/// tabs.txt: the active index on the first line, then one repository per line
/// (`paths::encode`, which escapes every control character), followed by a tab character and
/// the tab's label when it has one. Files from earlier versions have no labels.
pub fn parse_tabs(text: &str) -> SavedTabs {
    parse_tabs_bytes(text.as_bytes()).0
}

/// `parse_tabs` of the file's bytes (a leading BOM dropped), and how many lines it skipped. A
/// line whose path isn't valid UTF-8 is skipped (it can't name its folder); a label is decoded
/// lossily, since a damaged character there names nothing.
pub(super) fn parse_tabs_bytes(bytes: &[u8]) -> (SavedTabs, usize) {
    let mut lines = strip_bom(bytes).split(|b| *b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let active: usize = lines.next().and_then(|l| String::from_utf8_lossy(l).trim().parse().ok()).unwrap_or(0);
    let mut skipped = 0;
    let (paths, labels): (Vec<PathBuf>, Vec<Option<String>>) = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (path, label) = match l.iter().position(|b| *b == b'\t') {
                Some(at) => (&l[..at], &l[at + 1..]),
                None => (l, &[][..]),
            };
            let Ok(path) = std::str::from_utf8(path) else {
                skipped += 1;
                return None;
            };
            let label: String = String::from_utf8_lossy(label).chars().filter(|c| !c.is_control()).collect();
            let label = label.trim();
            Some((glowfly_core::paths::decode(path), (!label.is_empty()).then(|| label.to_owned())))
        })
        .unzip();
    let active = active.min(paths.len().saturating_sub(1));
    ((paths, labels, active), skipped)
}

pub(super) fn format_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> String {
    let mut text = active.to_string();
    for (k, p) in paths.iter().enumerate() {
        text.push('\n');
        text.push_str(&glowfly_core::paths::encode(p));
        if let Some(label) = labels.get(k).and_then(Option::as_deref) {
            let label: String = label.chars().filter(|c| !c.is_control()).collect();
            text.push('\t');
            text.push_str(label.trim());
        }
    }
    text
}

pub fn save_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> Result<SaveNotes, SaveRefused> {
    let Some(f) = tabs_file() else { return Ok(SaveNotes::default()) };
    save_tabs_to(&f, paths, labels, active)
}

/// Writes the open tabs to `file`.
pub fn save_tabs_to(file: &Path, paths: &[PathBuf], labels: &[Option<String>], active: usize) -> Result<SaveNotes, SaveRefused> {
    save_tabs_with(file, paths, labels, active, |p| std::fs::read(p))
}

/// Writes the open tabs to `file`, then the tabs a late read found that aren't open, with their
/// labels. `read` is the one read of a file whose start read failed.
pub(super) fn save_tabs_with(
    file: &Path,
    paths: &[PathBuf],
    labels: &[Option<String>],
    active: usize,
    read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<SaveNotes, SaveRefused> {
    let Begun { mut notes, late, carried } = begin(file, false, read, parse_tabs_bytes)?;
    let (mut carry_paths, mut carry_labels) = match carried {
        Some(Carried::Tabs(p, l)) => (p, l),
        _ => (Vec::new(), Vec::new()),
    };
    if let Some((disk_paths, disk_labels, _)) = late {
        notes.read_late = Some(disk_paths.clone());
        (carry_paths, carry_labels) = (disk_paths, disk_labels);
    }
    // A tab this save holds is the session's own from now on.
    carry_labels.resize(carry_paths.len(), None);
    let (carry_paths, carry_labels): (Vec<PathBuf>, Vec<Option<String>>) =
        carry_paths.into_iter().zip(carry_labels).filter(|(p, _)| !paths.iter().any(|s| glowfly_core::paths::same_spelling(s, p))).unzip();
    let mut all_paths = paths.to_vec();
    let mut all_labels = labels.to_vec();
    all_labels.resize(paths.len(), None);
    all_paths.extend(carry_paths.iter().cloned());
    all_labels.extend(carry_labels.iter().cloned());
    let active = if paths.is_empty() { 0 } else { active };
    let carried = (!carry_paths.is_empty()).then_some(Carried::Tabs(carry_paths, carry_labels));
    finish(file, &format_tabs(&all_paths, &all_labels, active), carried).map_err(SaveRefused::Io)?;
    Ok(notes)
}
