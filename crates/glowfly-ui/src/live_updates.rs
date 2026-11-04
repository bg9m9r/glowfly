//! The status bar's live-updates state, next to the version while a repository is watched
//! (`WatchState::live_updates`): a mark leading its label in a Badge (`widgets::chip`). A
//! `success` dot, "Live"; a weak ring while the watcher's probe is out, "Checking…"; a weak dot
//! for a read-only repository the probe couldn't test, "Unconfirmed" (these three bare, the
//! label in `weak`); a warn dot when polled, "Slow updates"; and a warn ring with a slash when
//! the watcher failed, "Not watching" (these two on a faint wash of warn). Only the two warn
//! states take a click (and the keyboard focus, with an inset ring), which refreshes now.
//! Painted shapes, not glyphs; the tooltip says what each state means.

use super::theme::color;
use super::widgets::chip::{self, Chip, ChipSize, Lead};
use eframe::egui::{self, Color32, Stroke, vec2};
use glowfly_model::keymap::BindingId;
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::watch::{LiveUpdates, REMOTE_POLL};

const DOT: f32 = 6.0;
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

/// Paints the state's mark centred at `c`.
fn paint_mark(p: &egui::Painter, c: egui::Pos2, look: &Look) {
    let _ = match look.mark {
        Mark::Dot => p.circle_filled(c, DOT / 2.0, look.tone),
        Mark::Ring | Mark::Slashed => p.circle_stroke(c, DOT / 2.0 - 0.5, Stroke::new(1.0_f32, look.tone)),
    };
    if look.mark == Mark::Slashed {
        let d = vec2(SLASH, SLASH);
        p.line_segment([c - d, c + d], Stroke::new(SLASH_W, look.tone));
    }
}

/// Draws the state's Badge in the status bar's right-to-left row: a warn state on a faint wash
/// of warn, refreshing when clicked; a quiet one bare, its label in `weak`.
pub(super) fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(live) = st.watch.live_updates() else { return };
    let look = look(st, live);
    let mark = |p: &egui::Painter, c: egui::Pos2, _slot: f32| paint_mark(p, c, &look);
    let badge = Chip::new(ChipSize::Badge, look.label).lead(Lead::Paint(&mark));
    if look.clickable {
        let badge = badge.look(chip::Look::new(color::wash(look.tone, 0.18), color::text()));
        let resp = badge.button(ui).on_hover_text(&look.tip);
        if resp.clicked() {
            msgs.push(Msg::Repo(RepoMsg::Refresh));
        }
        return;
    }
    badge.look(chip::Look::new(Color32::TRANSPARENT, color::weak())).show(ui).on_hover_text(&look.tip);
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_live_updates.rs"]
mod snapshot_tests;
