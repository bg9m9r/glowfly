//! The blame view: every line of one file as it is in a commit, each with the commit that last
//! changed it. Consecutive lines from one commit form a run: the gutter names its author, date
//! and commit once (on the run's first line in view) and its summary on the next line, runs
//! alternate their shading, and a strip at the gutter's edge is stronger for newer commits.
//! Clicking a run's gutter selects its commit on the graph; right-clicking a line or a gutter
//! opens the line's menu (`menu.rs`). The header (`header.rs`) steps back, toggles whitespace,
//! moves and wrapping, finds in the file and goes to a line. The body (`body.rs`) wraps long
//! lines or, unwrapped, scrolls them sideways under a fixed gutter. While a blame at HEAD runs
//! again after HEAD moved, the old one stays on screen.

mod body;
mod gutter;
mod header;
pub(in crate::ui) mod menu;
pub(in crate::ui) mod rows;

use super::UiCache;
use super::pane_status::{self, Failure};
use crate::msg::{BlameMsg, Msg};
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameView};
use eframe::egui::Ui;
use menu::LineTarget;

pub(in crate::ui) const LOADING: &str = "Loading the blame…";
pub(in crate::ui) const FAILED_TITLE: &str = "Couldn't blame this file";
pub(in crate::ui) const HELD_TITLE: &str = "Blame is paused until you trust this repository";

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = st.blame.as_mut() else { return };
    let scroll_to = view.scroll_to.take();
    ui.add_space(6.0);
    let over_history = st.history.is_some() && st.blame.as_ref().is_some_and(|v| v.over_history);
    header::show(ui, &mut st.blame, &st.settings, st.snap.as_ref(), over_history, msgs);
    ui.separator();
    let Some(view) = &st.blame else { return };
    match &view.data {
        // A re-run after HEAD moved keeps the old blame on screen.
        Some(data) if !data.blame.lines.is_empty() => {
            let out = body::body(ui, st, cache, view, data, scroll_to, msgs);
            if let Some(view) = st.blame.as_mut() {
                view.top_line = out.top;
                if out.clicked.is_some() {
                    view.cursor = out.clicked;
                }
            }
        }
        _ if view.loading => pane_status::loading(ui, LOADING),
        _ => match &view.error {
            Some(e) => {
                let title = if e == crate::state::blame::HELD { HELD_TITLE } else { FAILED_TITLE };
                let f = Failure { title, error: e, retry: Some(Msg::Blame(BlameMsg::Retry)) };
                pane_status::failed(ui, st.settings.lang(), f, msgs);
            }
            None => super::history_file::empty_file(ui),
        },
    }
}

/// The menu target of line `i`.
fn target<'a>(st: &AppState, view: &BlameView, data: &'a BlameData, i: usize) -> Option<LineTarget<'a>> {
    let blame = &data.blame;
    let commit = blame.commit_of_line(i)?;
    let before = !commit.oid.is_zero() && blame.origin_of_line(i).is_some_and(|o| o.previous.is_some());
    Some(LineTarget {
        line: i,
        text: blame.lines.get(i)?,
        commit,
        before,
        skipped: view.skipped.contains(&commit.oid),
        on_graph: st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&commit.oid)),
    })
}
