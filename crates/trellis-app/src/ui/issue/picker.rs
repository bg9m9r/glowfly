//! A multi-picker for the Create issue dialog's labels and assignees: the chosen items as chips
//! with a painted remove cross, a search field, and up to `MAX_ROWS` matching items to add
//! (shown while searching). Where the service takes new labels, the typed text can be added
//! as one. It edits the form's choice and query in place, like the dialog's other fields.

use super::super::theme::color;
use super::super::widgets::{self, Icon};
use crate::state::issue_form::too_many;
use crate::ui::theme::controls;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};
use trellis_core::matching::contains_lower;
use trellis_core::text::printable;

/// The most matches listed under the search field.
const MAX_ROWS: usize = 6;

/// What a picker offers.
pub(super) struct Picker<'a> {
    pub id_salt: &'a str,
    pub hint: &'a str,
    /// (id, label) of every item the service offers.
    pub options: &'a [(String, String)],
    /// The most items that can be chosen.
    pub max: usize,
    /// Typed text can be added as a new item.
    pub free: bool,
    /// What a typed item is added as: "label", "tag".
    pub add_word: &'a str,
}

/// Draws the picker over `chosen` (ids) and `query`.
pub(super) fn multi(ui: &mut Ui, p: &Picker, query: &mut String, chosen: &mut Vec<String>) {
    let label_of = |id: &str| p.options.iter().find(|(i, _)| i == id).map_or_else(|| id.to_owned(), |(_, l)| l.clone());
    if !chosen.is_empty() {
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (i, id) in chosen.iter().enumerate() {
                if chip(ui, &label_of(id)) {
                    remove = Some(i);
                }
            }
        });
        if let Some(i) = remove {
            chosen.remove(i);
        }
    }
    let field = controls::field(ui, egui::TextEdit::singleline(query).hint_text(p.hint).desired_width(f32::INFINITY));
    let list_id = ui.make_persistent_id((p.id_salt, "matches"));
    let last: Option<Rect> = ui.data(|d| d.get_temp(list_id));
    // Still shown while the pointer is over it, so a click that takes the focus off the field lands.
    let over = last.is_some_and(|r| ui.rect_contains_pointer(r));
    if query.trim().is_empty() && !field.has_focus() && !over {
        ui.data_mut(|d| d.remove::<Rect>(list_id));
        return;
    }
    let needle = query.trim().to_lowercase();
    let full = chosen.len() >= p.max;
    let matches: Vec<&(String, String)> = p
        .options
        .iter()
        .filter(|(id, label)| !chosen.contains(id) && (contains_lower(label, &needle) || contains_lower(id, &needle)))
        .take(MAX_ROWS)
        .collect();
    let mut picked = None;
    let list = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (id, label) in &matches {
            let r = ui.add_enabled(!full, egui::Button::new(RichText::new(printable(label)).color(color::text())).frame(false));
            let r = if full { r.on_disabled_hover_text(too_many(p.max)) } else { r };
            if r.clicked() {
                picked = Some(id.clone());
            }
        }
        let typed = query.trim();
        let exists = p.options.iter().any(|(_, l)| l.eq_ignore_ascii_case(typed)) || chosen.iter().any(|c| c.eq_ignore_ascii_case(typed));
        if p.free && !typed.is_empty() && !exists {
            let r =
                ui.add_enabled(!full, egui::Button::new(format!("Add {} \u{201c}{}\u{201d}", p.add_word, printable(typed))).frame(false));
            if r.clicked() {
                picked = Some(typed.to_owned());
            }
        }
        if matches.is_empty() && !(p.free && !typed.is_empty()) {
            ui.label(RichText::new("No matches").size(type_scale::CAPTION).color(color::faint()));
        }
    });
    ui.data_mut(|d| d.insert_temp(list_id, list.response.rect));
    if let Some(id) = picked {
        chosen.push(id);
        query.clear();
    }
}

/// A chosen item's chip; returns true when its cross was clicked.
fn chip(ui: &mut Ui, text: &str) -> bool {
    let text = printable(text);
    let galley = ui.painter().layout_no_wrap(text, type_scale::font(type_scale::CAPTION), color::text());
    let size = vec2(galley.size().x + 30.0, 20.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(radius::SM), color::raised());
    p.galley(pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0), galley, color::text());
    let cross = Rect::from_center_size(pos2(rect.right() - 11.0, rect.center().y), vec2(18.0, 18.0));
    let r = ui.interact(cross, ui.id().with(("remove", rect.left().to_bits(), rect.top().to_bits())), Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(cross, CornerRadius::same(radius::SM), color::hover());
    }
    // Painted: the bundled fonts have no cross glyph to rely on.
    widgets::paint_icon(ui.painter(), cross.center(), Icon::Close, color::weak());
    r.on_hover_text("Remove").clicked()
}
