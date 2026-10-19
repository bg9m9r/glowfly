//! The graph's search bar: the query field, the Filter toggle, the author picker, the match
//! count and Close. It edits only the query and author filter text; everything else is a
//! `SearchMsg` for update/search.rs.

use crate::keymap::BindingId;
use crate::msg::{Msg, SearchMsg};
use crate::state::{AppState, Selection};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::widgets;
use eframe::egui::{self, Key, PopupCloseBehavior, RichText};
use trellis_core::matching::contains_lower;

/// How many authors the picker lists before asking for a narrower filter.
const MAX_AUTHORS: usize = 200;

/// Room for the Filter toggle, author picker, match count ("12 so far, searching…") and Close
/// after the query field.
const CONTROLS_W: f32 = 430.0;

/// The empty query field's hint (cut with "…" in a narrow field), also its hover text when the
/// field is empty and the hint is cut.
const HINT: &str = "Search messages, authors and SHAs; narrow with author: or path:";

pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        // The field gives up width before the controls after it run out of room.
        let width = (ui.available_width() - CONTROLS_W).clamp(160.0, 420.0);
        let empty = st.view.search.is_empty();
        let (hint, hint_cut) = widgets::truncate::hint(ui, HINT, width);
        let r = controls::field(ui, egui::TextEdit::singleline(&mut st.view.search).hint_text(hint).desired_width(width));
        let r = if empty && hint_cut { r.on_hover_text(HINT) } else { r };

        // A credential prompt, the command palette or the author picker keeps the keyboard.
        let covered = st.askpass.showing() || st.palette.is_some();
        let picking = ui.memory(|m| m.any_popup_open());
        if !r.has_focus() && st.view.search.is_empty() && !covered && !picking {
            r.request_focus();
        }
        // Search commits from the command palette focuses the field even with text in it.
        if !covered && std::mem::take(&mut st.view.focus_search) {
            r.request_focus();
        }
        if r.changed() {
            msgs.push(Msg::Search(SearchMsg::QueryChanged));
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            msgs.push(Msg::Search(SearchMsg::Next));
            r.request_focus();
        }

        let tip = st.keymap.tip("Show only matching commits", BindingId::SearchFilter);
        if ui.selectable_label(st.search.filter, "Filter").on_hover_text(tip).clicked() {
            msgs.push(Msg::Search(SearchMsg::ToggleFilter));
        }

        author_picker(ui, st, msgs);

        let selected = match &st.selection {
            Selection::Commit(o) => st.snap.as_ref().and_then(|s| s.row_of.get(o).copied()),
            _ => None,
        };
        if let Some(text) = st.search.count_text(selected) {
            let r = ui.label(RichText::new(text).color(color::weak()));
            if st.search.searching_files() {
                r.on_hover_text("Looking through the files each loaded commit changed");
            } else if let Some(snap) = st.snap.as_ref().filter(|s| s.truncated) {
                let len = snap.commits.len();
                r.on_hover_text(format!("Searches the newest {len} loaded commits; Load more searches further back."));
            }
        }

        if ui.small_button("Close").clicked() {
            msgs.push(Msg::Search(SearchMsg::Close));
        }
    });
}

/// The author picker: "Any author", a filter field, then the history's authors.
fn author_picker(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let chosen = st.search.query.authors();
    let text = match chosen {
        [] => "Any author".to_owned(),
        [one] => one.clone(),
        many => format!("{} authors", many.len()),
    };
    let filter = &mut st.view.author_filter;
    let search = &st.search;
    let open = egui::ComboBox::from_id_salt("search-author")
        .width(160.0)
        .selected_text(text)
        // Clicks in the filter field keep the list open.
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            let chosen = search.query.authors();
            if ui.selectable_label(chosen.is_empty(), "Any author").clicked() {
                msgs.push(Msg::Search(SearchMsg::PickAuthor(None)));
                ui.memory_mut(|m| m.close_popup());
            }
            controls::field(ui, egui::TextEdit::singleline(filter).hint_text("Filter authors").desired_width(220.0));
            let Some(list) = &search.authors else {
                ui.label(RichText::new("Reading authors…").color(color::weak()));
                return;
            };
            let f = filter.trim().to_lowercase();
            let mut matching = list.iter().filter(|a| contains_lower(&a.name, &f) || contains_lower(&a.email, &f));
            let mut shown = 0;
            for a in matching.by_ref().take(MAX_AUTHORS) {
                shown += 1;
                let picked = chosen.iter().any(|c| *c == a.email.to_lowercase());
                let noun = if a.commits == 1 { "commit" } else { "commits" };
                let r = ui.selectable_label(picked, format!("{} <{}>", a.name, a.email)).on_hover_text(format!("{} {noun}", a.commits));
                if r.clicked() {
                    msgs.push(Msg::Search(SearchMsg::PickAuthor(Some(a.email.clone()))));
                    ui.memory_mut(|m| m.close_popup());
                }
            }
            let more = matching.count();
            if shown == 0 {
                ui.label(RichText::new("No matching authors").color(color::weak()));
            } else if more > 0 {
                ui.label(RichText::new(format!("{more} more, type to narrow")).color(color::weak()));
            }
        });
    if open.inner.is_some() && st.search.authors.is_none() {
        msgs.push(Msg::Search(SearchMsg::WantAuthors));
    }
}
