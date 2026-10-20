//! The header's View button and its popup: the layout (Inline | Split) and what is shown (Hunks |
//! Whole file), Ignore whitespace and Wrap long lines, and Expand all / Collapse all for the
//! whole file's folds. The popup stays open while its settings change and closes on a click
//! outside it or Esc.

use super::controls::{self, ghost_with};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_header::ViewGroup;
use crate::ui::theme::type_scale;
use crate::ui::theme::{color, controls as theme_controls};
use crate::ui::widgets::{self, buttons, segmented::segmented};
use eframe::egui::{self, PopupCloseBehavior, RichText, Ui, pos2};
use trellis_core::i18n::Lang;
use trellis_core::settings::{DiffLayout, DiffMode};

/// The popup's width.
const MENU_W: f32 = 240.0;
/// Room for the painted chevron after the label.
const CHEVRON_W: f32 = 14.0;

/// The button's width with its full label and with "View".
pub(super) fn widths(ui: &Ui, t: Lang, v: Option<&ViewGroup>) -> (f32, f32) {
    let Some(v) = v else { return (0.0, 0.0) };
    let w = |label: &str| controls::ghost_width(ui, false, Some(label), CHEVRON_W);
    (w(&v.label(t)), w(t.tr("diff.header.view.button")))
}

/// Draws the button (its label shortened to "View" with `short`) and, while open, its popup.
pub(super) fn show(ui: &mut Ui, t: Lang, v: &ViewGroup, short: bool, msgs: &mut Vec<Msg>) {
    let label = if short { t.tr("diff.header.view.button").to_owned() } else { v.label(t) };
    let id = ui.make_persistent_id("diff-view-menu");
    let open = ui.memory(|m| m.is_popup_open(id));
    let resp = ghost_with(ui, None, Some(&label), &label, true, CHEVRON_W);
    let c = pos2(resp.rect.right() - 8.0 - 4.0, resp.rect.center().y);
    widgets::chevron(ui.painter(), c, true, color::weak());
    let resp = if open { resp } else { resp.on_hover_text(t.tr("diff.header.view.menu.tip")) };
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    egui::popup::popup_below_widget(ui, id, &resp, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(MENU_W);
        ui.set_max_width(MENU_W);
        ui.spacing_mut().item_spacing.y = 6.0;
        menu(ui, t, v, msgs);
    });
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
}

fn menu(ui: &mut Ui, t: Lang, v: &ViewGroup, msgs: &mut Vec<Msg>) {
    if v.rows {
        caption(ui, t.tr("diff.header.view.layout"));
        let layouts = [
            (DiffLayout::Inline, t.tr("diff.header.view.inline"), t.tr("diff.header.view.inline.tip")),
            (DiffLayout::Split, t.tr("diff.header.view.split"), t.tr("diff.header.view.split.tip")),
        ];
        if let Some(l) = segmented(ui, v.layout, &layouts) {
            msgs.push(Msg::Diff(DiffMsg::SetDiffLayout(l)));
        }
        caption(ui, t.tr("diff.header.view.show"));
        let modes = [
            (DiffMode::Hunks, t.tr("diff.header.view.hunks"), t.tr("diff.header.view.hunks.tip")),
            (DiffMode::WholeFile, t.tr("diff.header.view.whole_file"), t.tr("diff.header.view.whole_file.tip")),
        ];
        if let Some(m) = segmented(ui, v.mode, &modes) {
            msgs.push(Msg::Diff(DiffMsg::SetDiffMode(m)));
        }
        ui.separator();
    }
    let mut ws = v.ignore_ws;
    let tip = if ws { t.tr("diff.header.view.ws_on.tip") } else { t.tr("diff.header.view.ws.tip") };
    if theme_controls::checkbox(ui, &mut ws, t.tr("diff.header.view.ignore_ws")).on_hover_text(tip).changed() {
        msgs.push(Msg::Diff(DiffMsg::SetIgnoreWhitespace(ws)));
    }
    if let Some(on) = v.wrap {
        let mut wrap = on;
        let tip = if on { t.tr("diff.header.view.wrap_on.tip") } else { t.tr("diff.header.view.wrap_off.tip") };
        if theme_controls::checkbox(ui, &mut wrap, t.tr("diff.header.view.wrap")).on_hover_text(tip).changed() {
            msgs.push(Msg::Diff(DiffMsg::SetWordWrap(wrap)));
        }
    }
    if let Some(c) = v.folds.filter(|_| v.mode == DiffMode::WholeFile) {
        ui.separator();
        ui.horizontal(|ui| {
            let expand = buttons::secondary(t.tr("diff.header.view.expand_all"))
                .small()
                .enabled(c.closed > 0)
                .tip(t.tr("diff.header.view.expand_all.tip"))
                .why(Some(t.tr("diff.header.view.expand_all_off.tip")));
            if ui.add(expand).clicked() {
                msgs.push(Msg::Diff(DiffMsg::ExpandAllUnchanged));
            }
            let collapse = buttons::secondary(t.tr("diff.header.view.collapse_all"))
                .small()
                .enabled(c.open > 0)
                .tip(t.tr("diff.header.view.collapse_all.tip"))
                .why(Some(t.tr("diff.header.view.collapse_all_off.tip")));
            if ui.add(collapse).clicked() {
                msgs.push(Msg::Diff(DiffMsg::CollapseAllUnchanged));
            }
        });
    }
}
