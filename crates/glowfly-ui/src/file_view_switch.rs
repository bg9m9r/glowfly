//! The Diff | File | Blame switch over one file as of a commit, shared by the file history's
//! header (Diff or File raised) and the blame header over a file history (Blame raised), so
//! moving among the three reads as one control.

use super::file_menu;
use super::widgets::segmented::{Segment, segmented_opts};
use crate::state::history::HistoryPane;
use crate::ui::theme::sizes::ControlSize;
use eframe::egui::Ui;

/// Which of the three views of the file is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FileShow {
    Diff,
    File,
    Blame,
}

impl From<HistoryPane> for FileShow {
    fn from(pane: HistoryPane) -> Self {
        match pane {
            HistoryPane::Diff => Self::Diff,
            HistoryPane::File => Self::File,
        }
    }
}

pub(crate) const DIFF: &str = "Diff";
pub(crate) const FILE: &str = "File";
const DIFF_TIP: &str = "This commit's changes to the file";
const FILE_TIP: &str = "The whole file as of this commit";
const BLAME_TIP: &str = "Blame the file as of the selected commit";
/// Why every segment is disabled in a history with no commit selected.
pub(crate) const NO_COMMIT: &str = "Select a commit first";
/// Why Blame is disabled for the commit that deletes the file.
pub(crate) const DELETES: &str = "This commit deletes the file";

/// Draws Diff | File | Blame with `current` raised; `all_off` disables every segment and
/// `blame_off` only Blame, each explained on hover. Returns the segment clicked, if it isn't
/// `current`.
pub(crate) fn show(ui: &mut Ui, current: FileShow, all_off: Option<&str>, blame_off: Option<&str>) -> Option<FileShow> {
    let options = [
        Segment::new(FileShow::Diff, DIFF, DIFF_TIP).why(all_off),
        Segment::new(FileShow::File, FILE, FILE_TIP).why(all_off),
        Segment::new(FileShow::Blame, file_menu::BLAME, BLAME_TIP).why(all_off.or(blame_off)),
    ];
    segmented_opts(ui, ControlSize::Regular, current, &options)
}
