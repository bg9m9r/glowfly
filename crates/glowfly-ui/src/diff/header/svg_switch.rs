//! The header's Text / Preview switch for an SVG that git diffs as text: Text shows the XML's
//! changed lines, Preview the drawing before and after (`diff::preview`). The choice is kept in
//! the settings for every SVG (`DiffMsg::SetSvgPreview`).

use crate::theme::sizes::ControlSize;
use crate::widgets::segmented::{Segment, segmented_opts, track_width};
use eframe::egui::Ui;
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{DiffMsg, Msg};

/// The switch's width, or 0 without one.
pub(super) fn width(ui: &Ui, t: Lang, svg: Option<bool>) -> f32 {
    if svg.is_none() {
        return 0.0;
    }
    track_width(ui, ControlSize::Regular, &[t.tr("diff.header.svg.text"), t.tr("diff.header.svg.preview")])
}

/// Draws the switch, Preview raised when `on`.
pub(super) fn show(ui: &mut Ui, t: Lang, on: bool, msgs: &mut Vec<Msg>) {
    let options = [
        Segment::new(false, t.tr("diff.header.svg.text"), t.tr("diff.header.svg.text.tip")),
        Segment::new(true, t.tr("diff.header.svg.preview"), t.tr("diff.header.svg.preview.tip")),
    ];
    if let Some(v) = segmented_opts(ui, ControlSize::Regular, on, &options) {
        msgs.push(Msg::Diff(DiffMsg::SetSvgPreview(v)));
    }
}
