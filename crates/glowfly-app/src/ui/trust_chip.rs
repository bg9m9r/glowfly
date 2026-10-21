//! The toolbar's "Untrusted" chip: shown while the open repository's git is held; its tooltip
//! says what is paused and why, and a click brings the trust prompt back (update/trust.rs).
//! While Glowfly waits on your decision (Undecided, or Changed: trusted before but it gained
//! settings or hooks since) it is the coral needs-you pill with the warning mark; after you
//! chose not to trust the repository it is a neutral chip.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, TrustMsg};
use crate::state::AppState;
use crate::state::trust::{self, Standing};
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Sense, Stroke, StrokeKind, pos2, vec2};

/// The chip's height; the toolbar row is 48px.
const HEIGHT: f32 = 26.0;

/// Draws the chip (in a right-to-left layout) while the active repository is Undecided,
/// NotTrusted or Changed; nothing otherwise.
pub fn show(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let standing = match st.trust_standing() {
        Some(s @ (Standing::Undecided | Standing::NotTrusted | Standing::Changed)) => s,
        _ => return,
    };
    let waiting = matches!(standing, Standing::Undecided | Standing::Changed);
    let ink = if waiting { super::needs_you::ink() } else { color::text() };
    let galley = ui.painter().layout_no_wrap("Untrusted".into(), type_scale::font(type_scale::SMALL), ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 34.0, HEIGHT), Sense::click());
    let p = ui.painter();
    if waiting {
        super::needs_you::paint_pill(p, rect, resp.hovered());
    } else {
        let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
        p.rect_filled(rect, radius, if resp.hovered() { color::hover() } else { color::raised() });
        p.rect_stroke(rect, radius, Stroke::new(1.0_f32, color::border()), StrokeKind::Inside);
        widgets::paint_icon(p, pos2(rect.left() + 14.0, rect.center().y), Icon::Shield, color::weak());
    }
    let text_pos = pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0);
    p.galley(text_pos, galley, ink);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Untrusted"));
    let resp = resp.on_hover_text(trust::paused_note(standing)).on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        msgs.push(Msg::Trust(TrustMsg::Ask { real: snap.real_workdir.clone() }));
    }
}
