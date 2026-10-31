//! A split menu row's trailing button (`menu_row::Item::trailing_button`): a small square at the
//! row's right end, centred where the menu's chevrons are (`menu_columns::BUTTON_W`), holding a
//! painted icon. It is its own target on top of the row: hovered it lights the row's pill and a
//! stronger pill of its own, it takes the focus separately (with its own focus ring), and it has
//! its own tooltip and accessible name. A blocked button (its row's reason, or one of its own)
//! is greyed, never clicks, and gives its reason on hover, on focus and to screen readers.

use super::super::access;
use super::super::theme::color;
use super::super::theme::tokens::radius;
use super::menu_columns::BUTTON_W;
use super::menu_reason;
use super::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{Rect, Response, Sense, Ui, pos2, vec2};

/// A split row's trailing button: its icon, its tooltip and name, and a reason of its own.
pub(super) struct Trailing<'a> {
    pub icon: Icon,
    pub tip: String,
    pub blocked: Option<&'a str>,
}

/// Which part of a split row was clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The row: its action.
    Row,
    /// The trailing button.
    Button,
}

/// A drawn row and, on a split row, its trailing button.
pub struct Split {
    pub row: Response,
    pub button: Option<Response>,
}

impl Split {
    /// The part clicked this pass, if any (each clicks only while it is enabled).
    pub fn clicked(&self) -> Option<Part> {
        if self.button.as_ref().is_some_and(Response::clicked) {
            Some(Part::Button)
        } else if self.row.clicked() {
            Some(Part::Row)
        } else {
            None
        }
    }
}

/// The icon's size in the button.
const ICON_SIZE: f32 = super::icon::SIZE_SM;

/// Registers the button of `row`, centred `centre_in` from its right edge: clickable while
/// `enabled`, else only hovered and focused.
pub(super) fn interact(ui: &Ui, row: &Response, centre_in: f32, enabled: bool) -> Response {
    let c = pos2(row.rect.right() - centre_in, row.rect.center().y);
    let rect = Rect::from_center_size(c, vec2(BUTTON_W, BUTTON_W));
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    ui.interact(rect, row.id.with("trailing"), sense)
}

/// Paints the button over its row and names it: its pill while hovered or pressed, the icon
/// (`text` while lit, `weak` at rest, `faint` blocked), the focus ring, then its tooltip. Blocked
/// by its own reason, else by its row's (`row_blocked`).
pub(super) fn finish(ui: &Ui, b: Response, t: &Trailing<'_>, row_blocked: Option<&str>) -> Response {
    let why = t.blocked.or(row_blocked);
    let enabled = why.is_none();
    if ui.is_rect_visible(b.rect) {
        let p = ui.painter();
        let lit = b.hovered() || b.has_focus();
        if enabled && b.is_pointer_button_down_on() {
            p.rect_filled(b.rect, radius::SM, color::pressed_layer());
        } else if enabled && b.hovered() {
            p.rect_filled(b.rect, radius::SM, color::hover_layer());
        }
        let ink = match (enabled, lit) {
            (false, _) => color::faint(),
            (true, true) => color::text(),
            (true, false) => color::weak(),
        };
        paint_icon_sized(p, b.rect.center(), t.icon, ink, ICON_SIZE);
        menu_reason::paint_focus(ui, &b);
    }
    access::name_button(&b, enabled, &t.tip);
    match why {
        Some(why) => {
            access::describe(&b, why);
            tooltip::attach_text(b, &format!("{}\n\n{why}", t.tip))
        }
        None => tooltip::attach_text(b, &t.tip),
    }
}

#[cfg(test)]
mod tests {
    use super::super::menu::styled;
    use super::super::menu_columns::{self as cols, Columns};
    use super::super::menu_row::item;
    use super::*;
    use egui_kittest::kittest::Queryable;

    /// Draws a menu with one split row and a submenu-free plain row, keeping what was clicked.
    fn harness(blocked: Option<&'static str>, own: Option<&'static str>) -> egui_kittest::Harness<'static, (Vec<Part>, Columns)> {
        let mut h = crate::ui::snapshot_check::builder().build_ui_state(
            move |ui, (clicks, used): &mut (Vec<Part>, Columns)| {
                styled(ui, |ui| {
                    let s = item("Cherry-pick")
                        .blocked(blocked)
                        .trailing_button(Icon::Eye, "Preview cherry-pick")
                        .trailing_blocked(own)
                        .show_split(ui);
                    clicks.extend(s.clicked());
                    item("Revert").show(ui);
                    *used = cols::used(ui);
                })
            },
            (Vec::new(), Columns::default()),
        );
        h.run();
        h
    }

    #[test]
    fn the_row_and_its_button_click_apart() {
        let mut h = harness(None, None);
        assert!(h.state().1.button, "the menu keeps the end slot");
        h.get_by_label("Preview cherry-pick").click();
        h.run();
        h.get_by_label("Cherry-pick").click();
        h.run();
        assert_eq!(h.state().0, vec![Part::Button, Part::Row]);
    }

    #[test]
    fn a_blocked_row_blocks_its_button_unless_it_has_its_own_state() {
        let mut h = harness(Some("Already on main"), None);
        let b = h.get_by_label("Preview cherry-pick");
        assert!(b.is_disabled());
        assert_eq!(b.description().as_deref(), Some("Already on main"));
        b.click();
        h.run();
        assert!(h.state().0.is_empty());
        let mut h = harness(None, Some("Merges can't be previewed"));
        assert!(h.get_by_label("Preview cherry-pick").is_disabled());
        assert!(!h.get_by_label("Cherry-pick").is_disabled());
        h.get_by_label("Cherry-pick").click();
        h.run();
        assert_eq!(h.state().0, vec![Part::Row]);
    }
}
