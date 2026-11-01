//! The header's View button and its menu: the layout (Inline or Split) and what is shown (Hunks
//! or Whole file) as radio groups under their headers, Ignore whitespace and Wrap long lines as
//! check items, and Expand all / Collapse all for the whole file's folds. The menu stays open
//! while its settings change and closes on a click outside it or Esc.

use super::controls::{self, ghost_with};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_header::ViewGroup;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::widgets;
use crate::ui::widgets::menu::{self, popup_below};
use eframe::egui::{PopupCloseBehavior, Ui, pos2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{DiffLayout, DiffMode};

/// The popup's width.
const MENU_W: f32 = 240.0;
/// Room for the painted chevron after the label.
const CHEVRON_W: f32 = 14.0;

/// The button's width with its full label and with "View".
pub(super) fn widths(ui: &Ui, t: Lang, v: Option<&ViewGroup>) -> (f32, f32) {
    let Some(v) = v else { return (0.0, 0.0) };
    let w = |label: &str| controls::ghost_width(ui, ControlSize::Regular, false, Some(label), CHEVRON_W);
    (w(&v.label(t)), w(t.tr("diff.header.view.button")))
}

/// Draws the button (its label shortened to "View" with `short`) and, while open, its popup.
pub(super) fn show(ui: &mut Ui, t: Lang, v: &ViewGroup, short: bool, msgs: &mut Vec<Msg>) {
    let label = if short { t.tr("diff.header.view.button").to_owned() } else { v.label(t) };
    let id = ui.make_persistent_id("diff-view-menu");
    let open = ui.memory(|m| m.is_popup_open(id));
    let resp = ghost_with(ui, ControlSize::Regular, None, Some(&label), &label, true, CHEVRON_W);
    let c = pos2(resp.rect.right() - 8.0 - 4.0, resp.rect.center().y);
    widgets::chevron(ui.painter(), c, true, color::weak());
    let resp = if open { resp } else { resp.on_hover_text(t.tr("diff.header.view.menu.tip")) };
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    popup_below(ui, id, &resp, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(MENU_W);
        menu(ui, t, v, msgs);
    });
}

fn menu(ui: &mut Ui, t: Lang, v: &ViewGroup, msgs: &mut Vec<Msg>) {
    if v.rows {
        menu::group_header(ui, t.tr("diff.header.view.layout"));
        let layouts = [
            (DiffLayout::Inline, t.tr("diff.header.view.inline"), t.tr("diff.header.view.inline.tip")),
            (DiffLayout::Split, t.tr("diff.header.view.split"), t.tr("diff.header.view.split.tip")),
        ];
        for (l, label, tip) in layouts {
            if menu::item(label).option(v.layout == l).show(ui).on_hover_text(tip).clicked() && v.layout != l {
                msgs.push(Msg::Diff(DiffMsg::SetDiffLayout(l)));
            }
        }
        menu::group_header(ui, t.tr("diff.header.view.show"));
        let modes = [
            (DiffMode::Hunks, t.tr("diff.header.view.hunks"), t.tr("diff.header.view.hunks.tip")),
            (DiffMode::WholeFile, t.tr("diff.header.view.whole_file"), t.tr("diff.header.view.whole_file.tip")),
        ];
        for (m, label, tip) in modes {
            if menu::item(label).option(v.mode == m).show(ui).on_hover_text(tip).clicked() && v.mode != m {
                msgs.push(Msg::Diff(DiffMsg::SetDiffMode(m)));
            }
        }
        menu::separator(ui);
    }
    let ws = v.ignore_ws;
    let tip = if ws { t.tr("diff.header.view.ws_on.tip") } else { t.tr("diff.header.view.ws.tip") };
    if menu::item(t.tr("diff.header.view.ignore_ws")).check(ws).show(ui).on_hover_text(tip).clicked() {
        msgs.push(Msg::Diff(DiffMsg::SetIgnoreWhitespace(!ws)));
    }
    if let Some(on) = v.wrap {
        let tip = if on { t.tr("diff.header.view.wrap_on.tip") } else { t.tr("diff.header.view.wrap_off.tip") };
        if menu::item(t.tr("diff.header.view.wrap")).check(on).show(ui).on_hover_text(tip).clicked() {
            msgs.push(Msg::Diff(DiffMsg::SetWordWrap(!on)));
        }
    }
    if let Some(c) = v.folds.filter(|_| v.mode == DiffMode::WholeFile) {
        menu::separator(ui);
        let none_closed = (c.closed == 0).then(|| t.tr("diff.header.view.expand_all_off.tip"));
        let r = menu::item(t.tr("diff.header.view.expand_all")).blocked(none_closed).show(ui);
        if r.on_hover_text(t.tr("diff.header.view.expand_all.tip")).clicked() {
            msgs.push(Msg::Diff(DiffMsg::ExpandAllUnchanged));
        }
        let none_open = (c.open == 0).then(|| t.tr("diff.header.view.collapse_all_off.tip"));
        let r = menu::item(t.tr("diff.header.view.collapse_all")).blocked(none_open).show(ui);
        if r.on_hover_text(t.tr("diff.header.view.collapse_all.tip")).clicked() {
            msgs.push(Msg::Diff(DiffMsg::CollapseAllUnchanged));
        }
    }
}
