//! The tip row (state/hints.rs): a quiet accent wash inset 8 px in its pane, with the info mark,
//! one wrapping sentence, the tip's Small secondary button and a ghost × whose menu offers
//! "Don't show this again" and "Turn off tips". The mark, the sentence's first line, the button
//! and the × share one centre line; under 420 px the button drops below the text. The row
//! itself isn't interactive; it is a group named by its sentence.

use super::buttons;
use super::ghost;
use super::menu;
use super::{Icon, paint_icon_sized};
use crate::common::menu_item;
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::{HOVER, radius};
use crate::theme::{color, surfaces, type_scale};
use crate::widgets::tooltip;
use eframe::egui::{self, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::Msg;
use glowfly_model::onboarding::OnboardingMsg;
use glowfly_model::state::hints::HintView;

/// The inset from the pane's sides, and above and below the row.
const INSET_X: f32 = 8.0;
const INSET_Y: f32 = 6.0;
/// Inside the row: left and right of the content, and above and below it.
const PAD_LEFT: f32 = 10.0;
const PAD_RIGHT: f32 = 6.0;
const PAD_Y: f32 = 6.0;
/// The info mark and the gap after it.
const MARK: f32 = 16.0;
const MARK_GAP: f32 = 8.0;
/// Between the sentence and the button, the button and the ×, and the text and the button
/// below it in the narrow layout.
const TEXT_GAP: f32 = 12.0;
const BUTTON_GAP: f32 = 4.0;
const STACK_GAP: f32 = 6.0;
/// Narrower than this, the button goes under the text.
const NARROW: f32 = 420.0;
/// The controls' size: their height is the band the first text line is centred in.
const SIZE: ControlSize = ControlSize::Small;

/// Draws the tip `hint` across `ui`'s width and pushes its messages.
pub fn show(ui: &mut Ui, t: Lang, hint: &HintView, msgs: &mut Vec<Msg>) {
    let band = SIZE.metrics().h;
    let avail = ui.available_width();
    let row_w = (avail - 2.0 * INSET_X).max(0.0);
    let narrow = row_w < NARROW;
    let button = buttons::secondary(hint.button).size(SIZE);
    let button_w = button.desired_width(ui);
    let inner_w = (row_w - PAD_LEFT - PAD_RIGHT).max(0.0);
    let text_x = MARK + MARK_GAP;
    let right = if narrow { band } else { button_w + BUTTON_GAP + band };
    let wrap = (inner_w - text_x - TEXT_GAP - right).max(40.0);
    let galley = ui.painter().layout(hint.text.clone(), type_scale::font(type_scale::BODY), color::text(), wrap);
    let line_h = galley.rows.first().map_or(galley.size().y, |r| r.height());
    // The first line sits centred in the controls' band, on a whole point (`sizes::text_pos`);
    // the row keeps the height the exact centre gives it.
    let exact_top = ((band - line_h) / 2.0).max(0.0);
    let text_top = exact_top.floor();
    let text_block = band.max(exact_top + galley.size().y);
    let content_h = if narrow { text_block + STACK_GAP + band } else { text_block };
    let row_h = content_h + 2.0 * PAD_Y;

    let (outer, _) = ui.allocate_exact_size(vec2(avail, row_h + 2.0 * INSET_Y), Sense::hover());
    let row = Rect::from_min_size(outer.min + vec2(INSET_X, INSET_Y), vec2(row_w, row_h));
    ui.painter().rect_filled(row, radius::MD, surfaces::wash(color::accent(), HOVER));
    let top = row.top() + PAD_Y;
    let left = row.left() + PAD_LEFT;
    let mid = top + band / 2.0;
    paint_icon_sized(ui.painter(), pos2(left + MARK / 2.0, mid), Icon::Info, color::accent(), MARK);
    let text_pos = pos2(left + text_x, top + text_top);
    ui.painter().galley(text_pos, galley, color::text());

    // The controls in a child of the row, so they never move the pane's cursor.
    let mut row_ui = ui.new_child(UiBuilder::new().max_rect(row).id_salt(("hint_row", hint.id.id())));
    let group = row_ui.id();
    row_ui.ctx().accesskit_node_builder(group, |n| {
        n.set_role(egui::accesskit::Role::Group);
        n.set_label(hint.text.as_str());
    });
    let close = Rect::from_min_size(pos2(row.right() - PAD_RIGHT - band, top), vec2(band, band));
    let button_rect = if narrow {
        Rect::from_min_size(pos2(text_pos.x, top + text_block + STACK_GAP), vec2(button_w, band))
    } else {
        Rect::from_min_size(pos2(close.left() - BUTTON_GAP - button_w, top), vec2(button_w, band))
    };
    let tip = t.tr("hints.options.tip");
    let (acted, x) = ui.ctx().with_accessibility_parent(group, || {
        let acted = row_ui.put(button_rect, button).clicked();
        (acted, ghost::ghost_at(&row_ui, group.with("options"), close, row, SIZE, Icon::Close, None, tip))
    });
    if acted {
        msgs.push(Msg::Onboarding(OnboardingMsg::HintAct(hint.id)));
    }
    let x = tooltip::attach_text(x, tip);
    menu::below(&row_ui, &x, |ui| {
        menu_item(msgs, ui, t.tr("hints.menu.dismiss"), Msg::Onboarding(OnboardingMsg::DismissHint(hint.id)));
        menu_item(msgs, ui, t.tr("hints.menu.off"), Msg::Onboarding(OnboardingMsg::HintsOff));
    });
}
