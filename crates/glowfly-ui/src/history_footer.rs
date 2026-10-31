//! The end of the file history's list (`history.rs`): how far the walk searched, Search further,
//! and the loading, failed and empty states, as a line under the entries or, with none yet, as
//! the list pane's centred block.

use super::common::short;
use super::empty_state::{EmptyIcon, empty_state};
use super::pane_status::{self, Failure};
use super::theme::color;
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::history::{FileHistoryView, HistoryKind};
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};

pub(super) const LOADING_HISTORY: &str = "Loading the history…";
pub(super) const FAILED_FILE: &str = "Couldn't load the history of this file";
const FAILED_FOLDER: &str = "Couldn't load the history of this folder";
const NO_COMMITS_FILE: &str = "No commits change this file";
const NO_COMMITS_FOLDER: &str = "No commits change this folder";
const NO_COMMITS_HINT: &str = "Commits on other branches aren't searched.";

fn failure<'a>(view: &FileHistoryView, error: &'a str) -> Failure<'a> {
    let title = match view.kind {
        HistoryKind::File => FAILED_FILE,
        HistoryKind::Folder => FAILED_FOLDER,
    };
    Failure { title, error, retry: Some(Msg::History(HistoryMsg::RetryFileHistory)) }
}

/// The list pane without entries: loading the first batch, its failure, or no commits at all.
pub(super) fn empty_list(ui: &mut Ui, st: &AppState, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    if view.loading {
        pane_status::loading(ui, LOADING_HISTORY);
    } else if let Some(e) = &view.error {
        pane_status::failed(ui, st.settings.lang(), failure(view, e), msgs);
    } else if view.complete() {
        let none = match view.kind {
            HistoryKind::File => NO_COMMITS_FILE,
            HistoryKind::Folder => NO_COMMITS_FOLDER,
        };
        empty_state(ui, EmptyIcon::Commits, none, NO_COMMITS_HINT, &[]);
    } else {
        // Stopped at the commit limit before finding one.
        ui.add_space(8.0);
        footer(ui, st, view, msgs);
    }
}

/// Under the entries: Search further is loading or failed, the first commit was reached, or
/// how far the walk searched with Search further.
pub(super) fn footer(ui: &mut Ui, st: &AppState, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 10, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        if view.loading {
            pane_status::loading_line(ui, t.tr("common.loading"));
        } else if let Some(e) = &view.error {
            pane_status::failed_line(ui, t, failure(view, e), msgs);
        } else if view.complete() {
            ui.label(RichText::new("Reached the first commit").color(color::faint()));
        } else {
            // Stopped at the commit limit.
            if let Some((oid, time)) = view.searched_until {
                let date = st.settings.format_date_word(time, st.view.utc_offset);
                ui.label(RichText::new(format!("Searched back to {date} ({})", short(oid))).color(color::weak()));
            }
            if ui.add(buttons::secondary("Search further")).clicked() {
                msgs.push(Msg::History(HistoryMsg::MoreFileHistory));
            }
        }
    });
    ui.add_space(8.0);
}
