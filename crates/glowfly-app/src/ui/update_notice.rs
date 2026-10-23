//! The status bar's update chip: an available, downloading, downloaded, installing or failed
//! update, with a painted arrow (or a spinner while downloading or installing). A click shows
//! the notes, installs (Install and restart), shows the file or tries again. The chip fits its text and, when the status bar runs out of room, cuts
//! it with "…" (whole in the tooltip). It is an accent tint while an update is on its way and a
//! neutral chip when it failed: the status bar's chrome keeps to the accent. A failed chip says
//! why on hover, and a Details link to its right (drawn first, so a narrow bar never cuts it)
//! opens Preferences > General, whose Updates row shows the error and Try again.

use super::theme::color;
use super::widgets::truncate::{cut_job, paint_cut};
use super::widgets::{self, Icon};
use crate::msg::{Msg, PrefsMsg, UpdateMsg};
use crate::state::updates::{ChipKind, UpdateStatus, download_failed_text};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use glowfly_core::app_update::InstallMethod;

const HEIGHT: f32 = 20.0;
/// The chip's width beside its text: the icon before it (26 px) and the space after (8 px).
const PAD: f32 = 34.0;

pub fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some((text, kind)) = st.updates.chip(t) else { return };
    let failed = kind == ChipKind::Failed;
    // The status bar lays out right to left, so this lands right of the chip.
    if failed {
        let details =
            ui.link(RichText::new(t.tr("updates.details.link")).size(type_scale::SMALL)).on_hover_text(t.tr("updates.details.tip"));
        if details.clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
        }
        ui.add_space(4.0);
    }
    let accent = if failed { color::weak() } else { color::accent() };
    let font = type_scale::font(type_scale::SMALL);
    let text_w = ui.painter().layout_no_wrap(text.clone(), font.clone(), color::text()).size().x;
    let ellipsis_w = ui.fonts(|f| f.glyph_width(&font, '…'));
    let working = matches!(kind, ChipKind::Downloading | ChipKind::Installing);
    let sense = if working { Sense::hover() } else { Sense::click() };
    let w = (text_w + PAD).min(ui.available_width()).max(PAD + ellipsis_w);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, HEIGHT), sense);
    let p = ui.painter();
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    let (fill, stroke) = if failed {
        (if resp.hovered() { color::hover() } else { color::raised() }, color::border())
    } else {
        (color::wash(accent, if resp.hovered() { 0.26 } else { 0.14 }), accent)
    };
    p.rect_filled(rect, radius, fill);
    p.rect_stroke(rect, radius, Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
    let icon_at = pos2(rect.left() + 14.0, rect.center().y);
    if working {
        let spin = egui::Rect::from_center_size(icon_at, vec2(12.0, 12.0));
        ui.put(spin, egui::Spinner::new().size(12.0).color(accent));
    } else {
        // A down arrow onto a line: the glyph fonts lack, painted (16 px, inside the chip).
        widgets::paint_icon(ui.painter(), icon_at, Icon::Pull, accent);
    }
    let cut = paint_cut(ui.painter(), pos2(rect.left() + 26.0, rect.center().y), cut_job(text.clone(), font, color::text()), w - PAD);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    if working {
        if cut.cut {
            resp.on_hover_text(&text);
        }
        return;
    }
    let tip = match (kind, st.updates.settled()) {
        (ChipKind::Available, _) => t.tr("updates.chip_tip.available").to_owned(),
        (ChipKind::Downloaded, _) => match st.updates.downloaded().map(|d| d.1) {
            Some(m) if m.restarts() => t.tr("updates.chip_tip.install_restart").to_owned(),
            Some(InstallMethod::OpenAndQuit) => t.tr("updates.chip_tip.open_quit").to_owned(),
            _ => t.tr("updates.chip_tip.show_file").to_owned(),
        },
        (ChipKind::Failed, UpdateStatus::DownloadFailed { error, .. }) => {
            format!("{}\n{}", download_failed_text(t, error), t.tr("updates.chip_tip.retry"))
        }
        (ChipKind::Failed | ChipKind::Downloading | ChipKind::Installing, _) => t.tr("updates.chip_tip.retry").to_owned(),
    };
    let tip = if cut.cut { format!("{text}\n{tip}") } else { tip };
    if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        let m = match st.updates.settled() {
            UpdateStatus::Available(_) => UpdateMsg::ShowNotes,
            UpdateStatus::Downloaded { method, .. } if method.acts() => UpdateMsg::InstallAndRestart,
            UpdateStatus::Downloaded { .. } => UpdateMsg::ShowFile,
            _ => UpdateMsg::Download,
        };
        msgs.push(Msg::Updates(m));
    }
}
