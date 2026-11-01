//! Ghost controls for quiet headers (the diff header, the terminal panel's): a ghost button (an
//! icon, a label or both; transparent until hovered), its width, the thin divider between
//! groups, the rich tooltip for icon-only controls, and the × that closes a view; and the ghost
//! button placed over a row (the graph's Compact New branch… on the checked-out commit). Each
//! takes a `ControlSize` for its height, padding, text, icon, gap and corners.

use super::button_look::{self, Press};
use super::{Icon, paint_icon_sized, tooltip};
use crate::keymap::{BindingId, Keymap};
use crate::state::tip::Tip;
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens;
use crate::ui::theme::type_scale;
use eframe::egui::{Align2, Color32, CursorIcon, FontId, Id, Painter, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;

/// Icons in a Regular ghost control (the diff header's identity marks match them).
pub const ICON: f32 = ControlSize::Regular.metrics().icon;
/// Space between the controls of one group.
pub const SPACE: f32 = 4.0;
/// The divider's line height and the room either side of it.
const DIVIDER_H: f32 = 20.0;
const DIVIDER_PAD: f32 = 8.0;
/// A divider with its margins.
pub const DIVIDER_W: f32 = 1.0 + 2.0 * DIVIDER_PAD;

/// The text of a Regular ghost control.
pub fn font() -> FontId {
    font_of(ControlSize::Regular)
}

fn font_of(size: ControlSize) -> FontId {
    type_scale::font(size.metrics().text)
}

/// The width of `text` in `font`.
pub fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER).size().x
}

/// The width a ghost control of `size` takes: a square of its height for an icon alone, else
/// its content with the size's quiet padding either side. `extra` is room for something painted
/// after the label (a chevron).
pub fn ghost_width(ui: &Ui, size: ControlSize, icon: bool, label: Option<&str>, extra: f32) -> f32 {
    let m = size.metrics();
    match label {
        None => m.h,
        Some(text) => {
            let icon_w = if icon { m.icon + m.icon_gap } else { 0.0 };
            2.0 * m.quiet_pad_x + icon_w + text_w(ui, text, font_of(size)) + extra
        }
    }
}

/// A ghost control of `size`: the quiet button look, transparent until hovered (the hover
/// layer, the pressed layer while held, the inset accent ring when focused), `faint` when
/// disabled, named `name` for screen readers. A disabled control still senses hover and focus,
/// so it can explain itself.
pub fn ghost(ui: &mut Ui, size: ControlSize, icon: Option<Icon>, label: Option<&str>, name: &str, enabled: bool) -> Response {
    ghost_with(ui, size, icon, label, name, enabled, 0.0)
}

/// `ghost` with `extra` px after the label (the caller paints there).
pub fn ghost_with(
    ui: &mut Ui,
    size: ControlSize,
    icon: Option<Icon>,
    label: Option<&str>,
    name: &str,
    enabled: bool,
    extra: f32,
) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let w = ghost_width(ui, size, icon.is_some(), label, extra);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, size.metrics().h), sense);
    let look = Look { enabled, press: Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false), focused: resp.has_focus() };
    paint(ui.painter(), rect, size, icon, label, look);
    access::name_button(&resp, enabled, name);
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

/// A ghost button of `size` (an icon and a label, or the icon alone) at `rect`, painted clipped
/// to `clip`, over whatever was laid out there (a graph row): created after that, it takes the
/// clicks there. `name` is for screen readers.
#[allow(clippy::too_many_arguments)]
pub fn ghost_at(ui: &Ui, id: Id, rect: Rect, clip: Rect, size: ControlSize, icon: Icon, label: Option<&str>, name: &str) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let look = Look { enabled: true, press: Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false), focused: resp.has_focus() };
    paint(&ui.painter().with_clip_rect(clip), rect, size, Some(icon), label, look);
    access::name_button(&resp, true, name);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// What state a ghost control is drawn in.
struct Look {
    enabled: bool,
    press: Press,
    focused: bool,
}

/// Paints a ghost control of `size` in `rect`: the quiet layer for its press state (none when
/// disabled), the inset focus ring, then its icon and label, centred when the icon is alone.
fn paint(p: &Painter, rect: Rect, size: ControlSize, icon: Option<Icon>, label: Option<&str>, look: Look) {
    let m = size.metrics();
    if look.enabled {
        button_look::paint_quiet(p, rect, m.radius, look.press);
    }
    if look.focused {
        tokens::focus_ring_inset(p, rect, m.radius);
    }
    let fg = if !look.enabled {
        color::faint()
    } else if look.press != Press::Rest {
        color::strong()
    } else {
        color::text()
    };
    match label {
        None => {
            if let Some(icon) = icon {
                paint_icon_sized(p, rect.center(), icon, fg, m.icon);
            }
        }
        Some(text) => {
            let mut x = rect.left() + m.quiet_pad_x;
            if let Some(icon) = icon {
                paint_icon_sized(p, pos2(x + m.icon / 2.0, rect.center().y), icon, fg, m.icon);
                x += m.icon + m.icon_gap;
            }
            p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font_of(size), fg);
        }
    }
}

/// The 1 x 20 px line between two groups, with 8 px either side.
pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(DIVIDER_W, ControlSize::Regular.metrics().h), Sense::hover());
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
    icon_tip(
        ghost(ui, ControlSize::Regular, Some(Icon::Close), None, name, true),
        keys,
        t,
        Tip::new(t.tr("common.close.button")).keys(BindingId::CloseView),
    )
}
