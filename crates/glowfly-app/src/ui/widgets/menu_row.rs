//! A menu row laid out on its menu's columns (`menu_columns.rs`): an optional mark in the state
//! column (a painted ✓ for a check item or the current choice, a dot for a radio), an optional
//! dot or swatch in the icon column, the label, the info mark of a blocked row in the reason
//! slot, and the shortcut (or a setup item's "Set up…") right-aligned in the shortcut column.
//! `menu_reason::menu_button` and every menu item helper draw through it.
//!
//! A split row (`trailing_button`, then `show_split`) adds a small icon button at its right
//! end, in the end slot where the chevrons are (`menu_split.rs`): the row runs its action, the
//! button a variant of it (a preview), with its own tooltip, focus and accessible name.
//!
//! A label whose interpolated names were cut (`menu_name.rs`) carries its full text
//! (`item_named`), which becomes its accessible name and its tooltip.
//!
//! Every row (and its trailing button) registers with the menu keyboard (`ui/menu_keys`), the
//! row by its label for typeahead.
//!
//! The row is one target as wide as the menu: enabled rows take egui's interaction colours (the
//! hover pill, the danger style's), blocked ones (`ItemState::Blocked`) are greyed, take the
//! focus but never click, and give their reason on hover, on focus and to screen readers. A
//! marked row is a checkbox or a radio button to screen readers, with its state.

use super::super::access;
use super::super::theme::{color, type_scale};
use super::menu_columns::{self as cols, Columns, STATE_W};
use super::menu_reason::{self, ItemState};
use super::menu_split::{self, Split, Trailing};
use super::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{Color32, Painter, Rect, Response, Sense, TextStyle, TextWrapMode, Ui, WidgetInfo, WidgetText, WidgetType, pos2, vec2};
use std::sync::Arc;

/// The mark in a row's state column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A check item: ✓ while on.
    Check(bool),
    /// One of a group's options: a dot while chosen.
    Radio(bool),
    /// The current choice of a list (a colour, a branch, a filter, a tab): ✓ while it is.
    Current(bool),
}

/// Paints a row's dot or swatch in `c`, centred in the icon column's `slot`.
pub type PaintIcon = fn(&Painter, Rect, Color32);

/// The radio dot's radius (a 6 px dot).
const DOT_R: f32 = 3.0;
/// The ✓'s size.
const CHECK_SIZE: f32 = super::icon::SIZE_SM;
const _: () = assert!(CHECK_SIZE <= STATE_W, "the check fits its column");

/// A menu row under construction: `item(label)`, then the parts it has, then `show`.
#[must_use = "a menu item draws only through `show`"]
pub struct Item<'a> {
    label: WidgetText,
    shortcut: WidgetText,
    state: ItemState<'a>,
    mark: Option<Mark>,
    icon: Option<(PaintIcon, Option<Color32>)>,
    full: Option<String>,
    trailing: Option<Trailing<'a>>,
}

/// A menu row labelled `label`: ready, with no mark, icon or shortcut.
pub fn item<'a>(label: impl Into<WidgetText>) -> Item<'a> {
    Item {
        label: label.into(),
        shortcut: WidgetText::default(),
        state: ItemState::Ready,
        mark: None,
        icon: None,
        full: None,
        trailing: None,
    }
}

/// A menu row labelled with `n`, its names cut to fit (`menu_name::named`): the full label is
/// its tooltip and accessible name when anything was cut.
pub fn item_named<'a>(n: super::menu_name::Named) -> Item<'a> {
    let full = (n.full != n.shown).then_some(n.full);
    Item { full, ..item(n.shown) }
}

impl<'a> Item<'a> {
    /// The key shown in the shortcut column (nothing when empty).
    pub fn shortcut(mut self, shortcut: impl Into<WidgetText>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    /// Ready, destructive, blocked with its reason, or opening its setting first.
    pub fn state(mut self, state: ItemState<'a>) -> Self {
        self.state = state;
        self
    }

    /// Blocked by `why` when there is a reason.
    pub fn blocked(self, why: Option<&'a str>) -> Self {
        self.state(ItemState::blocked_by(why))
    }

    /// A check item, ✓ while `on`.
    pub fn check(self, on: bool) -> Self {
        self.mark(Mark::Check(on))
    }

    /// One of a group's options (a radio item), dotted while `on`.
    pub fn option(self, on: bool) -> Self {
        self.mark(Mark::Radio(on))
    }

    /// One of a list's choices, ✓ while it is the current one.
    pub fn current(self, on: bool) -> Self {
        self.mark(Mark::Current(on))
    }

    pub fn mark(mut self, mark: Mark) -> Self {
        self.mark = Some(mark);
        self
    }

    /// A dot or swatch `paint` draws in `c`; None keeps the icon column empty on this row.
    pub fn icon(mut self, paint: PaintIcon, c: Option<Color32>) -> Self {
        self.icon = Some((paint, c));
        self
    }

    /// A split row: `icon` as a button at the row's right end, named and tipped `tip`. Blocked
    /// as the row is (`trailing_blocked` gives it a reason of its own). Draw it with `show_split`.
    pub fn trailing_button(mut self, icon: Icon, tip: impl Into<String>) -> Self {
        self.trailing = Some(Trailing { icon, tip: tip.into(), blocked: None });
        self
    }

    /// The trailing button blocked by `why` when there is a reason, whatever the row's state.
    pub fn trailing_blocked(mut self, why: Option<&'a str>) -> Self {
        if let Some(t) = &mut self.trailing {
            t.blocked = why;
        }
        self
    }

    /// Draws the row; clicked only when it is enabled.
    pub fn show(self, ui: &mut Ui) -> Response {
        self.show_split(ui).row
    }

    /// Draws the row and its trailing button: which part was clicked is `Split::clicked`.
    pub fn show_split(self, ui: &mut Ui) -> Split {
        match self.state {
            ItemState::Danger => {
                ui.scope(|ui| {
                    super::menu::danger_style(ui);
                    self.draw(ui)
                })
                .inner
            }
            _ => self.draw(ui),
        }
    }

    fn draw(self, ui: &mut Ui) -> Split {
        let Item { label, shortcut, state, mark, icon, full, trailing } = self;
        let blocked = match state {
            ItemState::Blocked(why) => Some(why),
            _ => None,
        };
        let key_text = match state {
            ItemState::Setup { suffix, .. } if shortcut.is_empty() => WidgetText::from(suffix),
            _ => shortcut,
        };
        let key = (!key_text.is_empty()).then(|| key_galley(ui, key_text));
        let need = Columns {
            state: mark.is_some(),
            icon: icon.is_some(),
            info: blocked.is_some(),
            key_w: key.as_ref().map_or(0.0, |k| k.size().x),
            chevron: false,
            button: trailing.is_some(),
        };
        let c = cols::note(ui, need);
        let galley = label.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
        let pad = ui.spacing().button_padding.x;
        let h = ui.spacing().interact_size.y.max(galley.size().y);
        let sense = if blocked.is_some() { Sense::hover() | Sense::FOCUSABLE } else { Sense::click() };
        let (rect, resp) = ui.allocate_at_least(vec2(c.row_w(pad, galley.size().x), h), sense);
        let name = full.clone().unwrap_or_else(|| galley.text().to_owned());
        let enabled = blocked.is_none();
        // Registered after the row, so it is on top and takes the pointer over its square.
        let button = trailing.as_ref().map(|t| menu_split::interact(ui, &resp, c.end_centre(pad), t.blocked.or(blocked).is_none()));
        let button_lit = button.as_ref().is_some_and(Response::hovered);
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let ink = if enabled {
                // The row stays lit while its button is hovered: the button is part of it.
                let vis = if button_lit && !resp.hovered() { &ui.visuals().widgets.hovered } else { ui.style().interact(&resp) };
                if ui.visuals().button_frame {
                    p.rect_filled(rect.expand(vis.expansion), vis.corner_radius, vis.weak_bg_fill);
                }
                vis.text_color()
            } else {
                color::faint()
            };
            let cy = rect.center().y;
            if let Some(m) = mark {
                paint_mark(p, pos2(rect.left() + c.state_x(pad), cy), m, ink);
            }
            if let Some((paint, Some(tone))) = icon {
                let slot = Rect::from_min_size(pos2(rect.left() + c.icon_left(pad), rect.top()), vec2(cols::ICON_W, rect.height()));
                paint(p, slot, tone);
            }
            p.galley(pos2(rect.left() + c.label_x(pad), cy - galley.size().y / 2.0), galley, ink);
            if let Some(key) = key {
                let tone = if enabled { color::weak() } else { color::faint() };
                let at = pos2(rect.right() - c.key_right(pad) - key.size().x, cy - key.size().y / 2.0);
                p.galley_with_override_text_color(at, key, tone);
            }
            if !enabled {
                let lit = resp.hovered() || resp.has_focus();
                menu_reason::paint_info(p, rect, c.info_right(pad) + (cols::INFO_SLOT - menu_reason::INFO_W) / 2.0, lit);
                menu_reason::paint_focus(ui, &resp);
            }
        }
        let button = match (trailing, button) {
            (Some(t), Some(b)) => Some(menu_split::finish(ui, b, &t, blocked)),
            _ => None,
        };
        // The menu keyboard's items: the row by its label, and its button beside it.
        crate::ui::menu_keys::register(ui, &resp, Some(&name), false);
        if let Some(b) = &button {
            crate::ui::menu_keys::register(ui, b, None, false);
        }
        match mark {
            Some(Mark::Check(on)) => resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, &name)),
            Some(Mark::Radio(on) | Mark::Current(on)) => {
                resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, on, &name));
            }
            None => access::name_button(&resp, enabled, &name),
        };
        let row = match (state, full) {
            (ItemState::Blocked(why), Some(full)) => {
                access::describe(&resp, why);
                tooltip::attach_text(resp, &format!("{full}\n\n{why}"))
            }
            (ItemState::Blocked(why), None) => menu_reason::explain(resp, why),
            (ItemState::Setup { note, .. }, Some(full)) => tooltip::attach_text(resp, &format!("{full}\n\n{note}")),
            (ItemState::Setup { note, .. }, None) => tooltip::attach_text(resp, note),
            (ItemState::Ready | ItemState::Danger, Some(full)) => tooltip::attach_text(resp, &full),
            (ItemState::Ready | ItemState::Danger, None) => resp,
        };
        Split { row, button }
    }
}

/// A shortcut or suffix in the small size; its colour is set when painted.
fn key_galley(ui: &Ui, text: WidgetText) -> Arc<eframe::egui::Galley> {
    let text = match text {
        WidgetText::RichText(r) => WidgetText::RichText(r.size(type_scale::SMALL)),
        WidgetText::LayoutJob(_) | WidgetText::Galley(_) => text,
    };
    let font = eframe::egui::FontSelection::FontId(type_scale::font(type_scale::SMALL));
    let job = text.into_layout_job(ui.style(), font, eframe::egui::Align::Center);
    ui.fonts(|f| f.layout_job(job))
}

/// Paints `m` centred at `c` in `ink`: ✓ for a check or the current choice, a dot for a radio,
/// nothing while off.
fn paint_mark(p: &Painter, c: eframe::egui::Pos2, m: Mark, ink: Color32) {
    match m {
        Mark::Check(true) | Mark::Current(true) => paint_icon_sized(p, c, Icon::Check, ink, CHECK_SIZE),
        Mark::Radio(true) => {
            p.circle_filled(c, DOT_R, ink);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::super::menu::{group_header, separator, styled};
    use super::*;
    use eframe::egui::accesskit::{Role, Toggled};
    use egui_kittest::kittest::Queryable;

    /// Draws `rows` as a menu's contents every pass, keeping the columns of the last.
    fn menu(rows: impl Fn(&mut Ui) + 'static) -> (egui_kittest::Harness<'static, Columns>, Columns) {
        let mut h = crate::ui::snapshot_check::builder().build_ui_state(
            move |ui, used: &mut Columns| {
                styled(ui, |ui| {
                    rows(ui);
                    *used = cols::used(ui);
                })
            },
            Columns::default(),
        );
        h.run();
        let used = *h.state();
        (h, used)
    }

    #[test]
    fn one_marked_row_moves_every_label() {
        let (h, used) = menu(|ui| {
            item("Plain").shortcut("Ctrl+P").show(ui);
            item("Ticked").check(true).show(ui);
            item("Blocked").blocked(Some("Why")).show(ui);
        });
        assert!(used.state && used.info && !used.icon && !used.chevron, "{used:?}");
        assert!(used.key_w > 0.0, "the shortcut column is as wide as Ctrl+P");
        let ticked = h.get_by_label("Ticked");
        assert_eq!((ticked.role(), ticked.toggled()), (Role::CheckBox, Some(Toggled::True)));
        assert_eq!(h.get_by_label("Plain").role(), Role::Button);
    }

    #[test]
    fn choices_are_radio_buttons_with_their_state() {
        let (h, used) = menu(|ui| {
            item("Rose").current(false).icon(|_, _, _| {}, Some(Color32::RED)).show(ui);
            item("Inline").option(true).show(ui);
        });
        assert!(used.state && used.icon);
        assert_eq!(h.get_by_label("Rose").toggled(), Some(Toggled::False));
        assert_eq!((h.get_by_label("Inline").role(), h.get_by_label("Inline").toggled()), (Role::RadioButton, Some(Toggled::True)));
    }

    #[test]
    fn a_header_replaces_its_separator_and_is_a_heading() {
        let (h, _) = menu(|ui| {
            let a = item("One").show(ui);
            let header = group_header(ui, "Group");
            separator(ui);
            let b = item("Two").show(ui);
            assert_eq!(header.rect.top() - a.rect.bottom(), 8.0, "8 px above the header");
            assert_eq!(b.rect.top() - header.rect.bottom(), 2.0, "no separator after it");
        });
        assert_eq!(h.get_by_label("Group").role(), Role::Heading);
    }
}
