//! The toolbar's "Untrusted" chip: shown while the open repository's git is held; its tooltip
//! says what is paused and why, and a click brings the trust prompt back (update/trust.rs).

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, TrustMsg};
use crate::state::AppState;
use crate::state::trust::{self, Standing};
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Sense, Stroke, StrokeKind, pos2, vec2};

/// The chip's height; the toolbar row is 48px.
const HEIGHT: f32 = 26.0;

/// Draws the chip (in a right-to-left layout) while the active repository is Undecided or
/// NotTrusted; nothing otherwise.
pub fn show(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let standing = match st.trust_standing() {
        Some(s @ (Standing::Undecided | Standing::NotTrusted)) => s,
        _ => return,
    };
    let galley = ui.painter().layout_no_wrap("Untrusted".into(), type_scale::font(type_scale::SMALL), color::text());
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 34.0, HEIGHT), Sense::click());
    let amber = color::amber();
    let p = ui.painter();
    let fill = amber.gamma_multiply(if resp.hovered() { 0.28 } else { 0.16 });
    p.rect_filled(rect, CornerRadius::same((HEIGHT / 2.0) as u8), fill);
    p.rect_stroke(rect, CornerRadius::same((HEIGHT / 2.0) as u8), Stroke::new(1.0_f32, amber.gamma_multiply(0.6)), StrokeKind::Inside);
    widgets::paint_icon(p, pos2(rect.left() + 14.0, rect.center().y), Icon::Shield, amber);
    let text_pos = pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0);
    p.galley(text_pos, galley, color::text());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Untrusted"));
    let resp = resp.on_hover_text(trust::paused_note(standing)).on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        msgs.push(Msg::Trust(TrustMsg::Ask { real: snap.real_workdir.clone() }));
    }
}
