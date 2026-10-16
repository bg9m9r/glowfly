//! The build-status marker on pull request rows, branch rows and graph ref labels, and the pull
//! request menu's build page item. Each state has its own shape as well as its own colour, so it
//! reads without colour vision; all painted, since the bundled fonts lack the glyphs.

use super::common::menu_item_if;
use super::theme::color;
use crate::msg::{ForgeMsg, Msg};
use crate::state::build_status::{MAX_SEEN, tip_queries};
use crate::state::{AppState, ForgeState, Status};
use eframe::egui::{self, Painter, Pos2, Rect, Response, Stroke, Ui, vec2};
use git2::Oid;
use std::time::Instant;
use trellis_core::forge::{CheckState, PullRequest};

/// Width a row keeps free for the marker.
pub const MARKER_SLOT: f32 = 14.0;

/// Paints the check-state marker centred on `c`: a ticked disc (passed), a crossed disc
/// (failed) or a ring with a dot (pending).
pub fn paint_marker(p: &Painter, c: Pos2, state: CheckState) {
    let mark = Stroke::new(1.4_f32, color::panel());
    match state {
        CheckState::Passed => {
            p.circle_filled(c, 5.0, color::green());
            p.line(vec![c + vec2(-2.4, 0.2), c + vec2(-0.6, 2.0), c + vec2(2.6, -1.8)], mark);
        }
        CheckState::Failed => {
            p.circle_filled(c, 5.0, color::red());
            let h = 1.9;
            p.line_segment([c + vec2(-h, -h), c + vec2(h, h)], mark);
            p.line_segment([c + vec2(-h, h), c + vec2(h, -h)], mark);
        }
        CheckState::Pending => {
            p.circle_stroke(c, 4.2, Stroke::new(1.6_f32, color::amber()));
            p.circle_filled(c, 1.4, color::amber());
        }
    }
}

/// The marker on a panel-coloured halo, for contrast on a coloured fill (graph ref chips).
pub fn paint_marker_on(p: &Painter, c: Pos2, state: CheckState) {
    p.circle_filled(c, 6.2, color::panel());
    paint_marker(p, c, state);
}

/// The marker column branch rows keep: only once some tip has a status, so repositories without
/// CI look as before.
pub fn tip_slot(fs: Option<&ForgeState>) -> f32 {
    if fs.is_some_and(|f| f.tips.any()) { MARKER_SLOT } else { 0.0 }
}

/// For a drawn branch row: notes its tip as on screen, paints its marker in `marker` when its
/// status is known, and returns the tooltip line.
pub fn tip_row(ui: &Ui, row: &Response, marker: Rect, fs: Option<&ForgeState>, oid: Option<Oid>, seen: &mut Vec<Oid>) -> Option<String> {
    let (fs, oid) = (fs?, oid?);
    if seen.len() < MAX_SEEN && !seen.contains(&oid) {
        seen.push(oid);
    }
    let c = fs.tips.checks(oid)?;
    if marker.width() > 0.0 {
        paint_marker(&ui.painter_at(row.rect), marker.center(), c.state);
    }
    Some(c.summary())
}

/// "Open failed build" / "Open build page" in a pull request's menu, when it has checks.
pub fn pr_menu_item(ui: &mut Ui, pr: &PullRequest, msgs: &mut Vec<Msg>) {
    let Some(c) = &pr.checks else { return };
    let label = if c.state == CheckState::Failed { "Open failed build" } else { "Open build page" };
    let blocked = c.url.is_empty().then_some("No link to this build was given by the service");
    menu_item_if(msgs, ui, label, blocked, Msg::OpenUrl(c.url.clone()));
}

/// After a frame is drawn: reports a changed set of tips on screen, and wakes the window when a
/// shown tip is due to be asked about again (so pending markers update without input).
pub(super) fn after_draw(st: &mut AppState, ctx: &egui::Context, now: Instant, msgs: &mut Vec<Msg>) {
    let seen = std::mem::take(&mut st.view.tips_seen);
    let (Some(fs), Some(snap)) = (st.forge.as_ref(), st.snap.as_ref()) else { return };
    // Only tips the host can be asked about (its remote's branches and what tracks them).
    let seen: Vec<Oid> = tip_queries(snap, &fs.remote, &seen).into_iter().map(|(o, _)| o).collect();
    if fs.host_status == Status::Ok
        && let Some(d) = fs.tips.next_wake(now)
    {
        ctx.request_repaint_after(d);
    }
    if seen != fs.tips.visible {
        msgs.push(Msg::Forge(ForgeMsg::TipsShown(seen)));
    }
}
