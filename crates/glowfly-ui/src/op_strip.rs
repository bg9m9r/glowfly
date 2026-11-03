//! The strip under the toolbar while the active repository has an operation in progress or
//! conflicts left (`state::op_strip`): the operation, how its conflicts stand, and Show work in
//! progress. A click anywhere on it opens the WIP panel; Continue and Abort stay in the panel.
//! It is a warning while conflicts are left, shows a spinner while a job runs, and is
//! information with a check otherwise. While the conflict view is open it reads as a sentence
//! ("Merging theme-tweaks into main · 1 conflicted file left") with the operation's quiet Abort
//! and its Commit merge or Continue (`state::conflict_strip`) instead, and doesn't click.

use super::theme::color;
use super::widgets::Icon;
use super::widgets::banner::{self, Banner, BannerKind};
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::conflict_strip::{ConflictStrip, conflict_strip};
use crate::state::op_actions::action_text;
use crate::state::op_confirm::banner_msg;
use crate::state::op_strip::strip_text;
use crate::ui::widgets::buttons;
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref().filter(|_| !st.dashboard.open) else { return };
    if let Some(strip) = conflict_strip(st) {
        return conflict_form(ctx, st, strip, msgs);
    }
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

/// The strip while the conflict view is open: right to left, Commit merge or Continue (disabled
/// with why while files are conflicted), then the quiet Abort.
fn conflict_form(ctx: &egui::Context, st: &AppState, strip: ConflictStrip, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let left = strip.conflicted > 0;
    let b = if left { Banner::new(BannerKind::Warning) } else { Banner::new(BannerKind::Info).icon(Icon::Check, color::added()) };
    let tone = if left { color::warn() } else { color::added() };
    let b = if strip.title.is_empty() {
        b.detail(strip.detail.as_str(), tone)
    } else {
        b.title(strip.title.as_str()).detail(format!("\u{b7} {}", strip.detail), tone)
    };
    banner::top(ctx, "op_strip", b, |ui| {
        if let Some(f) = &strip.finish
            && ui.add(buttons::primary(f.label.as_str()).enabled(f.why.is_none()).why(f.why.as_deref())).clicked()
        {
            msgs.push(f.msg());
        }
        if let Some((label, args)) = strip.abort
            && ui.add(buttons::quiet(action_text(t, label))).clicked()
        {
            msgs.push(banner_msg(st, label, args));
        }
    });
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_op_strip.rs"]
mod snapshot_tests;
