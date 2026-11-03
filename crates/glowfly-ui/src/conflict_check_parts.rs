//! The parts the conflict check's popover and report dialog share (issue #523's design): the
//! names block (the branch and what it was compared with, each name cut in its middle by the
//! rule every name follows, `truncate::middle`, with the whole name on hover), the file list (24 px rows of a file mark and a path cut on its left, no
//! box), and the popover's header row and footer divider. Every part sets its own heights, on
//! the 8 px grid; the caller spaces them.

use super::theme::color;
use super::theme::type_scale;
use super::widgets::icon::{Icon, paint_icon_sized};
use super::widgets::truncate::{cut_left_in, middle};
use crate::state::conflict_check::{self as cc, ConflictReport};
use eframe::egui::{self, Align2, FontId, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::conflict_check::TargetKind;
use glowfly_core::i18n::Lang;

/// A names, file or header row's height.
pub(super) const ROW_H: f32 = 24.0;
/// The file list heading's row height.
const HEADING_H: f32 = 20.0;
/// The file mark's size, and the gap after it.
const FILE_ICON: f32 = 14.0;
const ICON_GAP: f32 = 8.0;
/// The gap after the widest names label.
const LABEL_GAP: f32 = 12.0;

/// One line of `text` in `font`, painted in `rect` (part of a `ROW_H` row) and cut by `cut` to
/// its width; it is named by the whole text, which shows on hover when it was cut.
fn cut_row(ui: &mut Ui, rect: Rect, text: &str, font: FontId, colour: egui::Color32, cut: CutFn) -> Response {
    cut_row_named(ui, rect, text, text, font, colour, cut)
}

/// `cut_row` named `name` for screen readers (the text with its role).
fn cut_row_named(ui: &mut Ui, rect: Rect, text: &str, name: &str, font: FontId, colour: egui::Color32, cut: CutFn) -> Response {
    let shown = cut(ui.painter(), text, &font, rect.width().max(0.0));
    ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, &shown, font, colour);
    let resp = ui.interact(rect, ui.id().with(("cut-row", text, rect.top() as i32)), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, name));
    if shown != text { resp.on_hover_text(text) } else { resp }
}

type CutFn = fn(&egui::Painter, &str, &FontId, f32) -> String;

/// The label for what the branch was compared with.
fn target_label(t: Lang, kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Upstream => t.tr("conflict_check.names.upstream"),
        TargetKind::DefaultBranch => t.tr("conflict_check.names.default"),
        TargetKind::Chosen => t.tr("conflict_check.names.chosen"),
    }
}

/// The names block: "Branch" and the checked-out branch, then the target's label and name, on
/// two rows; then, for a remote target, that its last fetched state counts (4 px below).
pub(super) fn names(ui: &mut Ui, t: Lang, report: &ConflictReport, remote: bool) {
    let small = type_scale::font(type_scale::SMALL);
    let body = type_scale::font(type_scale::BODY);
    let labels = [t.tr("conflict_check.names.branch"), target_label(t, report.request.target.kind)];
    let label_w =
        labels.iter().map(|l| ui.painter().layout_no_wrap((*l).to_owned(), small.clone(), color::weak()).size().x).fold(0.0, f32::max)
            + LABEL_GAP;
    let rows = [(labels[0], report.request.branch.as_str()), (labels[1], report.request.target.name.as_str())];
    for (label, name) in rows {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        // The label's bottom on the name's, so their baselines line up.
        let bottom = rect.center().y + ui.fonts(|f| f.row_height(&body)) / 2.0;
        ui.painter().text(pos2(rect.left(), bottom), Align2::LEFT_BOTTOM, label, small.clone(), color::weak());
        let value = Rect::from_min_max(pos2(rect.left() + label_w, rect.top()), rect.max);
        let read = t.trf("conflict_check.names.access", &[("role", &label), ("name", &name)]);
        cut_row_named(ui, value, name, &read, body.clone(), color::text(), middle);
    }
    if remote {
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new(t.tr("conflict_check.fetched_note")).font(small).color(color::weak())).wrap());
    }
}

/// The file list: its "N files changed on both sides:" heading, 4 px, then a row per path (a
/// file mark and the path in monospace, cut on its left so the file name stays). In the popover
/// (`short`) the first `POPOVER_FILES` and an "and K more" row lined up with the paths; in the
/// report dialog and the operation preview every path, scrolling in the dialog's body.
pub(super) fn file_list(ui: &mut Ui, t: Lang, paths: &[String], short: bool) {
    let small = type_scale::font(type_scale::SMALL);
    let heading = t.trn("conflict_check.popover.both_sides", paths.len(), &[]);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEADING_H), Sense::hover());
    ui.painter().text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, &heading, small.clone(), color::weak());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &heading));
    ui.add_space(4.0);
    let (shown, more) = cc::file_list(t, paths, short.then_some(cc::POPOVER_FILES));
    let indent = FILE_ICON + ICON_GAP;
    let mono = type_scale::mono(type_scale::BODY);
    for p in &shown {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        let mark = pos2(rect.left() + FILE_ICON / 2.0, rect.center().y);
        paint_icon_sized(ui.painter(), mark, Icon::File, color::faint(), FILE_ICON);
        let path = Rect::from_min_max(pos2(rect.left() + indent, rect.top()), rect.max);
        cut_row(ui, path, p, mono.clone(), color::text(), cut_left_in);
    }
    if let Some(more) = more {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        ui.painter().text(pos2(rect.left() + indent, rect.center().y), Align2::LEFT_CENTER, &more, small, color::weak());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &more));
    }
}

/// A 1 px line in the edge colour across the popover, through its padding (`pad`) on both
/// sides.
pub(super) fn divider(ui: &mut Ui, pad: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    let line = Rect::from_min_max(pos2(rect.left() - pad, rect.top()), pos2(rect.right() + pad, rect.bottom()));
    ui.painter().rect_filled(line, 0.0, color::border());
}
