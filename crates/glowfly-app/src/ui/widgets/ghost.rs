//! Ghost controls for quiet headers (the diff header, the terminal panel's): a ghost button (an
//! icon, a label or both; transparent until hovered), its width, the thin divider between
//! groups, the rich tooltip for icon-only controls, and the × that closes a view.

use super::button_look::{self, Press};
use super::{Icon, icon, paint_icon_sized, tooltip};
use crate::keymap::{BindingId, Keymap};
use crate::state::tip::Tip;
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::tokens::{self, radius};
use crate::ui::theme::type_scale;
use eframe::egui::{Align2, Color32, CursorIcon, FontId, Response, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;

/// Every control's height and corners.
pub const HEIGHT: f32 = 28.0;
pub const RADIUS: u8 = radius::MD;
/// Room either side of a labelled control's content, and between its icon and label.
const PAD_X: f32 = 8.0;
const ICON_GAP: f32 = 6.0;
/// Icons are drawn this size.
pub const ICON: f32 = icon::SIZE_MD;
/// Space between the controls of one group.
pub const SPACE: f32 = 4.0;
/// The divider's line height and the room either side of it.
const DIVIDER_H: f32 = 20.0;
const DIVIDER_PAD: f32 = 8.0;
/// A divider with its margins.
pub const DIVIDER_W: f32 = 1.0 + 2.0 * DIVIDER_PAD;

pub fn font() -> FontId {
    type_scale::font(type_scale::BODY)
}

/// The width of `text` in `font`.
pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER).size().x
}

/// The width a ghost control takes: a 28 px square for an icon alone, else its content with
/// 8 px either side. `extra` is room for something painted after the label (a chevron).
pub fn ghost_width(ui: &Ui, icon: bool, label: Option<&str>, extra: f32) -> f32 {
    match label {
        None => HEIGHT,
        Some(text) => {
            let icon_w = if icon { ICON + ICON_GAP } else { 0.0 };
            2.0 * PAD_X + icon_w + text_w(ui, text, font()) + extra
        }
    }
}

/// A ghost control: the quiet button look, transparent until hovered (the hover layer, the
/// pressed layer while held, the inset accent ring when focused), `faint` when disabled, named `name` for screen readers. A
/// disabled control still senses hover and focus, so it can explain itself.
pub fn ghost(ui: &mut Ui, icon: Option<Icon>, label: Option<&str>, name: &str, enabled: bool) -> Response {
    ghost_with(ui, icon, label, name, enabled, 0.0)
}

/// `ghost` with `extra` px after the label (the caller paints there).
pub fn ghost_with(ui: &mut Ui, icon: Option<Icon>, label: Option<&str>, name: &str, enabled: bool, extra: f32) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let w = ghost_width(ui, icon.is_some(), label, extra);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, HEIGHT), sense);
    let p = ui.painter();
    if enabled {
        button_look::paint_quiet(p, rect, RADIUS, Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false));
    }
    if resp.has_focus() {
        tokens::focus_ring_inset(p, rect, RADIUS);
    }
    let fg = if !enabled {
        color::faint()
    } else if resp.hovered() {
        color::strong()
    } else {
        color::text()
    };
    match label {
        None => {
            if let Some(icon) = icon {
                paint_icon_sized(p, rect.center(), icon, fg, ICON);
            }
        }
        Some(text) => {
            let mut x = rect.left() + PAD_X;
            if let Some(icon) = icon {
                paint_icon_sized(p, pos2(x + ICON / 2.0, rect.center().y), icon, fg, ICON);
                x += ICON + ICON_GAP;
            }
            p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font(), fg);
        }
    }
    access::name_button(&resp, enabled, name);
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

/// The 1 x 20 px line between two groups, with 8 px either side.
pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(DIVIDER_W, HEIGHT), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    let y = rect.center().y;
    ui.painter().line_segment([pos2(x, y - DIVIDER_H / 2.0), pos2(x, y + DIVIDER_H / 2.0)], Stroke::new(1.0_f32, color::border()));
}

/// `resp` with the shared rich tooltip `tip`.
pub fn icon_tip(resp: Response, keys: &Keymap, t: Lang, tip: Tip) -> Response {
    tooltip::attach(resp, keys, t, || tip)
}

/// The × that closes a view (the diff, blame, a history, an issue or pull request): `name` for
/// screen readers, Close and the Esc cap in the tooltip.
pub fn close_view(ui: &mut Ui, keys: &Keymap, t: Lang, name: &str) -> Response {
    icon_tip(ghost(ui, Some(Icon::Close), None, name, true), keys, t, Tip::new(t.tr("common.close.button")).keys(BindingId::CloseView))
}
