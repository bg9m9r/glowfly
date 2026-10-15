//! File actions as buttons in headers (the file history's selected commit, the commit diff
//! header): restore one commit's version of a file, open the working-tree file in its default
//! program, show it in the file manager.

use super::file_menu::{self, NOT_IN_WORKDIR, OPEN_DEFAULT, SHOW_IN_FOLDER};
use crate::msg::FilesMsg;
use crate::msg::Msg;
use eframe::egui;
use git2::Oid;

/// One commit's version of a file, as the header buttons act on it.
pub(super) struct CommitFile<'a> {
    pub oid: Oid,
    /// The file's path in that commit: what Restore writes.
    pub path: &'a str,
    /// Its change in that commit; 'D' restores from before the commit.
    pub status: char,
    /// The working-tree path Open and Show act on (the history's current path; `path` in the
    /// diff header).
    pub open_path: &'a str,
    /// `open_path` is in the working tree.
    pub in_workdir: bool,
}

/// How much the labels say: in full where there is room, short in the crowded diff header.
#[derive(Clone, Copy)]
pub(super) enum Labels {
    Full,
    Compact,
}

pub(super) const RESTORE_SHORT: &str = "Restore…";
pub(super) const OPEN_SHORT: &str = "Open";

/// Restore, Open and Show, in that reading order whichever way `ui` lays out.
pub(super) fn show(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, f: &CommitFile, labels: Labels) {
    let full = matches!(labels, Labels::Full);
    let deleted = f.status == 'D';
    let restore_tip = if deleted {
        format!("Bring back {} as it was before this commit deleted it", f.path)
    } else {
        format!("Replace {} in your working tree with this commit's version", f.path)
    };
    let missing = (!f.in_workdir).then_some(NOT_IN_WORKDIR);
    let open_path = f.open_path.to_owned();
    let mut buttons = [
        (
            if full { file_menu::restore_label(deleted) } else { RESTORE_SHORT },
            restore_tip,
            None,
            file_menu::restore_msg(f.oid, f.path, f.status),
        ),
        (
            if full { OPEN_DEFAULT } else { OPEN_SHORT },
            OPEN_DEFAULT.to_owned(),
            missing,
            Msg::Files(FilesMsg::OpenWorkingFile(open_path.clone())),
        ),
        (SHOW_IN_FOLDER, SHOW_IN_FOLDER.to_owned(), missing, Msg::Files(FilesMsg::ShowWorkingFile(open_path))),
    ];
    if ui.layout().prefer_right_to_left() {
        buttons.reverse();
    }
    for (label, tip, reason, msg) in buttons {
        let mut r = ui.add_enabled(reason.is_none(), egui::Button::new(label)).on_hover_text(tip);
        if let Some(why) = reason {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() {
            msgs.push(msg);
            // Leave the arrow keys to the list they were moving through.
            r.surrender_focus();
        }
    }
}
