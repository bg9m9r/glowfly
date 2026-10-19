//! The header's View button and its popup: the layout (Inline | Split) and what is shown (Hunks |
//! Whole file), Ignore whitespace and Wrap long lines, and Expand all / Collapse all for the
//! whole file's folds. The popup stays open while its settings change and closes on a click
//! outside it or Esc.

use super::controls::{self, ghost_with};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_header::{VIEW, ViewGroup};
use crate::ui::theme::{color, controls as theme_controls};
use crate::ui::widgets::{self, buttons, segmented::segmented};
use eframe::egui::{self, FontId, PopupCloseBehavior, RichText, Ui, pos2};
use trellis_core::settings::{DiffLayout, DiffMode};

pub(super) const HUNKS_TIP: &str = "Only the changed lines, with a few lines of context around each change";
pub(super) const WHOLE_FILE_TIP: &str =
    "Every line of the file, with changes marked in place. Single-line staging is available in the Hunks view.";
const EXPAND_ALL_TIP: &str = "Show every unchanged line of the file";
const EXPAND_ALL_OFF_TIP: &str = "Every unchanged run is already shown";
const COLLAPSE_ALL_TIP: &str = "Fold every long unchanged run back into one row";
const COLLAPSE_ALL_OFF_TIP: &str = "No opened runs to fold";
pub(super) const COLLAPSE_ALL: &str = "Collapse all";
pub(super) const EXPAND_ALL: &str = "Expand all";
pub(super) const IGNORE_WS: &str = "Ignore whitespace";
pub(super) const WRAP: &str = "Wrap long lines";
const WRAP_OFF_TIP: &str = "Continue long lines on the next line so they fit the width of the viewer";
const WRAP_ON_TIP: &str = "Long lines continue on the next line. Turn off to scroll sideways instead.";
const WS_WHAT: &str = "Hide changes that only add or remove trailing whitespace or change the amount of existing whitespace (such as \
                       re-indenting)";

/// The popup's width.
const MENU_W: f32 = 240.0;
/// Room for the painted chevron after the label.
const CHEVRON_W: f32 = 14.0;

/// The button's width with its full label and with "View".
pub(super) fn widths(ui: &Ui, v: Option<&ViewGroup>) -> (f32, f32) {
    let Some(v) = v else { return (0.0, 0.0) };
    let w = |label: &str| controls::ghost_width(ui, false, Some(label), CHEVRON_W);
    (w(&v.label()), w(VIEW))
}

/// Draws the button (its label shortened to "View" with `short`) and, while open, its popup.
pub(super) fn show(ui: &mut Ui, v: &ViewGroup, short: bool, msgs: &mut Vec<Msg>) {
    let label = if short { VIEW.to_owned() } else { v.label() };
    let id = ui.make_persistent_id("diff-view-menu");
    let open = ui.memory(|m| m.is_popup_open(id));
    let resp = ghost_with(ui, None, Some(&label), &label, true, CHEVRON_W);
    let c = pos2(resp.rect.right() - 8.0 - 4.0, resp.rect.center().y);
    widgets::chevron(ui.painter(), c, true, color::weak());
    let resp = if open { resp } else { resp.on_hover_text("Layout, what to show, whitespace and wrapping") };
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    egui::popup::popup_below_widget(ui, id, &resp, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(MENU_W);
        ui.set_max_width(MENU_W);
        ui.spacing_mut().item_spacing.y = 6.0;
        menu(ui, v, msgs);
    });
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(FontId::proportional(12.0)).color(color::weak()));
}

fn menu(ui: &mut Ui, v: &ViewGroup, msgs: &mut Vec<Msg>) {
    if v.rows {
        caption(ui, "Layout");
        let layouts = [(DiffLayout::Inline, "Inline", "Changes in one column"), (DiffLayout::Split, "Split", "Old and new side by side")];
        if let Some(l) = segmented(ui, v.layout, &layouts) {
            msgs.push(Msg::Diff(DiffMsg::SetDiffLayout(l)));
        }
        caption(ui, "Show");
        let modes = [(DiffMode::Hunks, "Hunks", HUNKS_TIP), (DiffMode::WholeFile, "Whole file", WHOLE_FILE_TIP)];
        if let Some(m) = segmented(ui, v.mode, &modes) {
            msgs.push(Msg::Diff(DiffMsg::SetDiffMode(m)));
        }
        ui.separator();
    }
    let mut ws = v.ignore_ws;
    let tip = if ws {
        format!("{WS_WHAT}. Hunk and line actions then apply only the changes shown and leave the hidden whitespace changes as they are.")
    } else {
        WS_WHAT.to_owned()
    };
    if theme_controls::checkbox(ui, &mut ws, IGNORE_WS).on_hover_text(tip).changed() {
        msgs.push(Msg::Diff(DiffMsg::SetIgnoreWhitespace(ws)));
    }
    if let Some(on) = v.wrap {
        let mut wrap = on;
        let tip = if on { WRAP_ON_TIP } else { WRAP_OFF_TIP };
        if theme_controls::checkbox(ui, &mut wrap, WRAP).on_hover_text(tip).changed() {
            msgs.push(Msg::Diff(DiffMsg::SetWordWrap(wrap)));
        }
    }
    if let Some(c) = v.folds.filter(|_| v.mode == DiffMode::WholeFile) {
        ui.separator();
        ui.horizontal(|ui| {
            let expand = buttons::secondary(EXPAND_ALL).small().enabled(c.closed > 0).tip(EXPAND_ALL_TIP).why(Some(EXPAND_ALL_OFF_TIP));
            if ui.add(expand).clicked() {
                msgs.push(Msg::Diff(DiffMsg::ExpandAllUnchanged));
            }
            let collapse =
                buttons::secondary(COLLAPSE_ALL).small().enabled(c.open > 0).tip(COLLAPSE_ALL_TIP).why(Some(COLLAPSE_ALL_OFF_TIP));
            if ui.add(collapse).clicked() {
                msgs.push(Msg::Diff(DiffMsg::CollapseAllUnchanged));
            }
        });
    }
}
