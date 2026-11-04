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
use crate::theme::type_scale;
use crate::widgets::chip::{self, ChipSize, Look};
use eframe::egui::{self, RichText, Sense, pos2, vec2};
use glowfly_core::app_update::InstallMethod;
use glowfly_model::msg::{Msg, PrefsMsg, UpdateMsg};
use glowfly_model::state::updates::{ChipKind, UpdateStatus, download_failed_text, stale_text};
use glowfly_model::state::{AppState, PrefPage};

/// The chip's size: a Badge, its icon in the glyph slot.
const CHIP: ChipSize = ChipSize::Badge;

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
    let m = CHIP.metrics();
    let font = type_scale::font(m.text);
    let text_w = ui.painter().layout_no_wrap(text.clone(), font.clone(), color::text()).size().x;
    let ellipsis_w = ui.fonts(|f| f.glyph_width(&font, '…'));
    let working = matches!(kind, ChipKind::Downloading | ChipKind::Installing);
    let sense = if working { Sense::hover() } else { Sense::click() };
    // Beside the text: the padding either side, the icon and its gap.
    let lead = m.glyph + m.gap;
    let w = chip::width(CHIP, lead + text_w).min(ui.available_width()).max(chip::width(CHIP, lead + ellipsis_w));
    // The click takes the bar's whole height; the badge is painted at its own.
    let (hit, resp) = ui.allocate_exact_size(vec2(w, ui.available_height().max(m.h)), sense);
    let rect = chip::rect_at(CHIP, hit.left(), hit.center().y, w);
    let p = ui.painter();
    let (fill, edge) = if failed {
        (if resp.hovered() { color::hover() } else { color::raised() }, color::border())
    } else {
        (color::wash(accent, if resp.hovered() { 0.26 } else { 0.14 }), accent)
    };
    chip::frame(p, rect, CHIP, Look::new(fill, color::text()).edged(edge));
    let icon_at = pos2(rect.left() + m.pad_x + m.glyph / 2.0, rect.center().y);
    if working {
        let spin = egui::Rect::from_center_size(icon_at, vec2(m.glyph, m.glyph));
        ui.put(spin, egui::Spinner::new().size(m.glyph).color(accent));
    } else {
        // A down arrow onto a line: the glyph fonts lack, painted in the badge's glyph slot.
        widgets::paint_icon_sized(ui.painter(), icon_at, Icon::Pull, accent, m.glyph);
    }
    let room = w - 2.0 * m.pad_x - lead;
    let cut =
        paint_cut(ui.painter(), pos2(rect.left() + m.pad_x + lead, rect.center().y), cut_job(text.clone(), font, color::text()), room);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    if working {
        if cut.cut {
            resp.on_hover_text(&text);
        }
        return;
    }
    let tip = match (kind, st.updates.settled()) {
        (ChipKind::Available, UpdateStatus::Stale { file, problem, .. }) => {
            format!(
                "{}
{}",
                stale_text(t, file, *problem),
                t.tr("updates.chip_tip.retry")
            )
        }
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
