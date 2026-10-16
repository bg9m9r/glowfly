//! The status bar's update chip: an available, downloading, downloaded or failed update, with a
//! painted arrow (or a spinner while downloading). A click shows the notes, installs, shows the
//! file or tries again.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, UpdateMsg};
use crate::state::AppState;
use crate::state::updates::{ChipKind, UpdateStatus};
use eframe::egui::{self, CornerRadius, FontId, Sense, Stroke, StrokeKind, pos2, vec2};

const HEIGHT: f32 = 20.0;

pub fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some((text, kind)) = st.updates.chip() else { return };
    let accent = match kind {
        ChipKind::Failed => color::red(),
        ChipKind::Available | ChipKind::Downloading | ChipKind::Downloaded => color::blue(),
    };
    let galley = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(12.0), color::text());
    let sense = if kind == ChipKind::Downloading { Sense::hover() } else { Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 34.0, HEIGHT), sense);
    let p = ui.painter();
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    p.rect_filled(rect, radius, accent.gamma_multiply(if resp.hovered() { 0.26 } else { 0.14 }));
    p.rect_stroke(rect, radius, Stroke::new(1.0_f32, accent.gamma_multiply(0.6)), StrokeKind::Inside);
    let icon_at = pos2(rect.left() + 14.0, rect.center().y);
    if kind == ChipKind::Downloading {
        let spin = egui::Rect::from_center_size(icon_at, vec2(12.0, 12.0));
        ui.put(spin, egui::Spinner::new().size(12.0).color(accent));
    } else {
        // A down arrow onto a line: the glyph fonts lack, painted (16 px, inside the chip).
        widgets::paint_icon(ui.painter(), icon_at, Icon::Pull, accent);
    }
    ui.painter().galley(pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, color::text());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    if kind == ChipKind::Downloading {
        return;
    }
    let tip = match kind {
        ChipKind::Available => "Show what's new",
        ChipKind::Downloaded if st.updates.installer().is_some() => "Install the update",
        ChipKind::Downloaded => "Show the downloaded file",
        ChipKind::Failed | ChipKind::Downloading => "Download again",
    };
    if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        let m = match st.updates.settled() {
            UpdateStatus::Available(_) => UpdateMsg::ShowNotes,
            UpdateStatus::Downloaded { installer: true, .. } => UpdateMsg::Install,
            UpdateStatus::Downloaded { .. } => UpdateMsg::ShowFile,
            _ => UpdateMsg::Download,
        };
        msgs.push(Msg::Updates(m));
    }
}
