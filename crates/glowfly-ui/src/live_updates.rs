//! The status bar's live-updates state, next to the version while a repository is watched
//! (`WatchState::live_updates`): a mark, then its label in `weak`. A `success` dot, "Live"; a
//! weak ring while the watcher's probe is out, "Checking…"; a weak dot for a read-only
//! repository the probe couldn't test, "Unconfirmed"; a warn dot when polled, "Slow updates";
//! and a warn ring with a slash when the watcher failed, "Not watching". Only the two warn
//! states take a click (and the keyboard focus, with an inset ring), which refreshes now.
//! Painted shapes, not glyphs; the tooltip says what each state means.

use super::theme::color;
use super::theme::tokens::{self, radius};
use crate::keymap::BindingId;
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::watch::{LiveUpdates, REMOTE_POLL};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, Sense, Stroke, pos2, vec2};

const HEIGHT: f32 = 20.0;
/// The dot's slot: a 6 px dot centred in 14 px.
const SLOT: f32 = 14.0;
const DOT: f32 = 6.0;
/// Between the dot's slot and the label (6 px from the dot's edge).
const GAP: f32 = 2.0;
/// The slash across the failed state's ring: its half extent and stroke.
const SLASH: f32 = 3.5;
const SLASH_W: f32 = 1.25;

/// A state's mark.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Dot,
    Ring,
    /// A ring crossed from top left to bottom right.
    Slashed,
}

/// How a state looks: its mark and colour, its label, its tooltip, and whether a click
/// refreshes.
struct Look {
    mark: Mark,
    tone: Color32,
    label: &'static str,
    tip: String,
    clickable: bool,
}

fn look(st: &AppState, live: LiveUpdates) -> Look {
    let t = st.settings.lang();
    let quiet = |mark, tone, label, tip: &str| Look { mark, tone, label, tip: tip.to_owned(), clickable: false };
    match live {
        LiveUpdates::Checking => quiet(Mark::Ring, color::weak(), t.tr("status.live.checking.label"), t.tr("status.live.checking.tip")),
        LiveUpdates::Live => quiet(Mark::Dot, color::added(), t.tr("status.live.live.label"), t.tr("status.live.live.tip")),
        LiveUpdates::Unknown => quiet(Mark::Dot, color::weak(), t.tr("status.live.unknown.label"), t.tr("status.live.unknown.tip")),
        LiveUpdates::Silent => Look {
            mark: Mark::Dot,
            tone: color::warn(),
            label: t.tr("status.live.silent.label"),
            tip: t.trf("status.live.silent.tip", &[("secs", &REMOTE_POLL.as_secs())]),
            clickable: true,
        },
        LiveUpdates::Failed => {
            let tip = match st.keymap.hint(BindingId::Refresh) {
                Some(k) => t.trf("status.live.failed.tip_key", &[("key", &k)]),
                None => t.tr("status.live.failed.tip_no_key").to_owned(),
            };
            Look { mark: Mark::Slashed, tone: color::warn(), label: t.tr("status.live.failed.label"), tip, clickable: true }
        }
    }
}

/// Draws the mark and label in the status bar's right-to-left row.
pub(super) fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(live) = st.watch.live_updates() else { return };
    let look = look(st, live);
    let font = type_scale::font(type_scale::SMALL);
    let text = ui.painter().layout_no_wrap(look.label.to_owned(), font, color::weak());
    let clickable = look.clickable;
    let sense = if clickable { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(SLOT + GAP + text.size().x, HEIGHT), sense);
    let p = ui.painter();
    let centre = pos2(rect.left() + SLOT / 2.0, rect.center().y);
    let stroke = Stroke::new(1.0_f32, look.tone);
    let _ = match look.mark {
        Mark::Dot => p.circle_filled(centre, DOT / 2.0, look.tone),
        Mark::Ring | Mark::Slashed => p.circle_stroke(centre, DOT / 2.0 - 0.5, stroke),
    };
    if look.mark == Mark::Slashed {
        let d = vec2(SLASH, SLASH);
        p.line_segment([centre - d, centre + d], Stroke::new(SLASH_W, look.tone));
    }
    let at = pos2(rect.left() + SLOT + GAP, rect.center().y - text.size().y / 2.0);
    p.galley(at, text, color::weak());
    // Packed in the bar's row, like a quiet button: the inset ring.
    if clickable && resp.has_focus() {
        tokens::focus_ring_inset(p, rect, radius::SM);
    }
    let name = look.label.to_owned();
    let kind = if clickable { egui::WidgetType::Button } else { egui::WidgetType::Label };
    resp.widget_info(|| egui::WidgetInfo::labeled(kind, true, name.clone()));
    let resp = resp.on_hover_text(&look.tip);
    if clickable {
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            msgs.push(Msg::Repo(RepoMsg::Refresh));
        }
    }
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_live_updates.rs"]
mod snapshot_tests;
