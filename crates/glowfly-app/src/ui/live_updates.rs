//! The status bar's live-updates dot, next to the version while a repository is watched
//! (`WatchState::live_updates`): a faint ring while the watcher's probe is out, an accent dot
//! while changes arrive live, a weak ring for a read-only repository the probe couldn't test,
//! and a warn dot ("Slow updates", polled) or warn ring ("Not live", the watcher failed) with a
//! label. Only the two warn states take a click, which refreshes now. A painted shape, not a
//! glyph; the tooltip says what each state means.

use super::theme::color;
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
/// Between the label and the dot.
const GAP: f32 = 4.0;

/// How a state looks: filled or a ring, its colour, its label (warn states only), its tooltip.
struct Look {
    filled: bool,
    tone: Color32,
    label: Option<&'static str>,
    tip: String,
}

fn look(st: &AppState, live: LiveUpdates) -> Look {
    let t = st.settings.lang();
    let (filled, tone, label, tip) = match live {
        LiveUpdates::Checking => (false, color::faint(), None, t.tr("status.live.checking.tip").to_owned()),
        LiveUpdates::Live => (true, color::accent(), None, t.tr("status.live.live.tip").to_owned()),
        LiveUpdates::Unknown => (false, color::weak(), None, t.tr("status.live.unknown.tip").to_owned()),
        LiveUpdates::Silent => (
            true,
            color::warn(),
            Some(t.tr("status.live.silent.label")),
            t.trf("status.live.silent.tip", &[("secs", &REMOTE_POLL.as_secs())]),
        ),
        LiveUpdates::Failed => {
            let tip = match st.keymap.hint(BindingId::Refresh) {
                Some(k) => t.trf("status.live.failed.tip_key", &[("key", &k)]),
                None => t.tr("status.live.failed.tip_no_key").to_owned(),
            };
            (false, color::warn(), Some(t.tr("status.live.failed.label")), tip)
        }
    };
    Look { filled, tone, label, tip }
}

/// Draws the dot (and label) in the status bar's right-to-left row.
pub(super) fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(live) = st.watch.live_updates() else { return };
    let look = look(st, live);
    let font = type_scale::font(type_scale::SMALL);
    let text = look.label.map(|l| ui.painter().layout_no_wrap(l.to_owned(), font, color::weak()));
    let text_w = text.as_ref().map_or(0.0, |g| g.size().x + GAP);
    let clickable = look.label.is_some();
    let sense = if clickable { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + SLOT, HEIGHT), sense);
    let p = ui.painter();
    let centre = pos2(rect.right() - SLOT / 2.0, rect.center().y);
    if look.filled {
        p.circle_filled(centre, DOT / 2.0, look.tone);
    } else {
        p.circle_stroke(centre, DOT / 2.0 - 0.5, Stroke::new(1.0_f32, look.tone));
    }
    if let Some(g) = text {
        let at = pos2(rect.left(), rect.center().y - g.size().y / 2.0);
        p.galley(at, g, color::weak());
    }
    let name = look.label.map_or_else(|| look.tip.clone(), str::to_owned);
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
