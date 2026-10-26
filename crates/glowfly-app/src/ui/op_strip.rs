//! The strip under the toolbar while the active repository has an operation in progress or
//! conflicts left (`state::op_strip`): the operation, how its conflicts stand, and Show work in
//! progress. A click anywhere on it opens the WIP panel; Continue and Abort stay in the panel.
//! It is a warning while conflicts are left, shows a spinner while a job runs, and is
//! information with a check otherwise.

use super::theme::color;
use super::widgets::Icon;
use super::widgets::banner::{self, Banner, BannerKind};
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::op_strip::strip_text;
use crate::ui::widgets::buttons;
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref().filter(|_| !st.dashboard.open) else { return };
    let t = st.settings.lang();
    let Some(text) = strip_text(t, snap, st.session.conflict_files.resolved_rows(snap).len()) else { return };
    let mut b = if text.conflicts {
        Banner::new(BannerKind::Warning)
    } else if st.job.is_some() {
        Banner::new(BannerKind::Progress)
    } else {
        Banner::new(BannerKind::Info).icon(Icon::Check, color::added())
    };
    b = b.title(text.title.as_str()).clickable(true);
    if let Some((detail, all)) = &text.detail {
        b = b.detail(detail.as_str(), if *all { color::added() } else { color::warn() });
    }
    let r = banner::top(ctx, "op_strip", b, |ui| {
        if ui.add(buttons::secondary(t.tr("op_strip.show.button"))).clicked() {
            msgs.push(Msg::Repo(RepoMsg::ShowInProgress));
        }
    });
    if r.clicked() {
        msgs.push(Msg::Repo(RepoMsg::ShowInProgress));
    }
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_op_strip.rs"]
mod snapshot_tests;
