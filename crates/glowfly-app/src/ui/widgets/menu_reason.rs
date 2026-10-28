//! Menu items that say why they are disabled, or that they open a setting first:
//!
//! - a blocked item has its label greyed (`color::faint()`) and a painted info mark in the
//!   menu's reason slot (`menu_columns.rs`: one x on every row, left of the shortcuts). The whole reason shows in the shared tooltip
//!   on hover, or under the item while it has the keyboard focus, and screen readers get it as
//!   the item's description. The item takes the focus but never clicks;
//! - a setup item stays enabled with a weak "Set up…" (in the language) after its label, and its
//!   note in the same tooltip.
//!
//! - a destructive item (`Danger`) has its label in the danger colour and a danger-tinted hover
//!   pill (`menu::danger_style`); menus put it in their last group, and blocked it greys as any.
//!
//! The reason is never text in the menu, so a long one doesn't widen it, and the item's
//! accessible name stays its label.

use super::super::access;
use super::super::theme::color;
use super::super::theme::tokens::{self, radius};
use super::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{Painter, Rect, Response, Ui, WidgetText, pos2};
use glowfly_core::i18n::Lang;

/// The info mark's width.
pub const INFO_W: f32 = super::icon::SIZE_SM;

/// A menu item's state: usable, usable and destructive, disabled with the whole reason, or
/// usable while what it needs isn't set up yet (carrying the hover note and the suffix; it opens
/// the setting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemState<'a> {
    Ready,
    Danger,
    Blocked(&'a str),
    Setup { note: &'a str, suffix: &'a str },
}

impl<'a> ItemState<'a> {
    /// Usable, opening the setting `note` describes, with the "Set up…" suffix in the language.
    pub fn setup(t: Lang, note: &'a str) -> Self {
        Self::Setup { note, suffix: t.tr("common.set_up.suffix") }
    }

    /// Blocked by `why` when there is a reason, else ready.
    pub fn blocked_by(why: Option<&'a str>) -> Self {
        why.map_or(Self::Ready, Self::Blocked)
    }

    /// A destructive item, blocked by `why` when there is a reason.
    pub fn danger_unless(why: Option<&'a str>) -> Self {
        why.map_or(Self::Danger, Self::Blocked)
    }
}

/// A menu item: `label` (never wrapped), `shortcut` in the menu's shortcut column when not
/// empty, laid out on the menu's columns (`menu_row::item` for one with a mark or an icon). A
/// danger item is in the danger colour with a danger-tinted hover pill. A blocked item is greyed
/// with an info mark and its reason in a tooltip; a setup item stays enabled with a weak
/// "Set up…" in the shortcut column.
pub fn menu_button(ui: &mut Ui, label: impl Into<WidgetText>, shortcut: impl Into<WidgetText>, state: ItemState<'_>) -> Response {
    super::menu_row::item(label).shortcut(shortcut).state(state).show(ui)
}

/// Paints the info mark at `row`'s right, `right_inset` in from its edge, centred on it:
/// `color::text()` while `lit` (the row hovered or focused), else `color::weak()`.
pub fn paint_info(p: &Painter, row: Rect, right_inset: f32, lit: bool) {
    let c = pos2(row.right() - right_inset - INFO_W / 2.0, row.center().y);
    paint_icon_sized(p, c, Icon::Info, if lit { color::text() } else { color::weak() }, INFO_W);
}

/// The accent ring inside `resp`'s rect while it has the keyboard focus, with a menu row's
/// corners.
pub fn paint_focus(ui: &Ui, resp: &Response) {
    if resp.has_focus() {
        tokens::focus_ring_inset(ui.painter(), resp.rect, radius::SM);
    }
}

/// Gives a blocked row `why` as its description and in the tooltip on hover or focus.
pub fn explain(resp: Response, why: &str) -> Response {
    access::describe(&resp, why);
    tooltip::attach_text(resp, why)
}

#[cfg(test)]
#[path = "menu_reason_tests.rs"]
mod tests;
