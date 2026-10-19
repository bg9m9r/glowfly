//! The status bar's update chip: an available, downloading, downloaded or failed update, with a
//! painted arrow (or a spinner while downloading). A click shows the notes, installs, shows the
//! file or tries again. The chip fits its text and, when the status bar runs out of room, cuts
//! it with "…" (whole in the tooltip).

use super::theme::color;
use super::widgets::truncate::{cut_job, paint_cut};
use super::widgets::{self, Icon};
use crate::msg::{Msg, UpdateMsg};
use crate::state::AppState;
use crate::state::updates::{ChipKind, UpdateStatus};
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Sense, Stroke, StrokeKind, pos2, vec2};

const HEIGHT: f32 = 20.0;
/// The chip's width beside its text: the icon before it (26 px) and the space after (8 px).
const PAD: f32 = 34.0;

pub fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some((text, kind)) = st.updates.chip(t) else { return };
    let accent = match kind {
        ChipKind::Failed => color::removed(),
        ChipKind::Available | ChipKind::Downloading | ChipKind::Downloaded => color::info(),
    };
    let font = type_scale::font(type_scale::SMALL);
    let text_w = ui.painter().layout_no_wrap(text.clone(), font.clone(), color::text()).size().x;
    let ellipsis_w = ui.fonts(|f| f.glyph_width(&font, '…'));
    let sense = if kind == ChipKind::Downloading { Sense::hover() } else { Sense::click() };
    let w = (text_w + PAD).min(ui.available_width()).max(PAD + ellipsis_w);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, HEIGHT), sense);
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
    let cut = paint_cut(ui.painter(), pos2(rect.left() + 26.0, rect.center().y), cut_job(text.clone(), font, color::text()), w - PAD);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    if kind == ChipKind::Downloading {
        if cut.cut {
            resp.on_hover_text(&text);
        }
        return;
    }
    let tip = match kind {
        ChipKind::Available => t.tr("updates.chip_tip.available"),
        ChipKind::Downloaded if st.updates.installer().is_some() => t.tr("updates.chip_tip.install"),
        ChipKind::Downloaded => t.tr("updates.chip_tip.show_file"),
        ChipKind::Failed | ChipKind::Downloading => t.tr("updates.chip_tip.retry"),
    };
    let tip = if cut.cut { format!("{text}\n{tip}") } else { tip.to_owned() };
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
