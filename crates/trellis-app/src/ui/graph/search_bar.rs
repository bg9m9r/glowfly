//! The graph's search bar: the query field (with a painted clear button while it has text), the
//! Highlight | Filter mode, the author picker, the match count in a fixed-width slot (so Close
//! doesn't move as it changes) and Close. It edits only the query and author filter text;
//! everything else is a `SearchMsg` for update/search.rs.

use crate::keymap::BindingId;
use crate::msg::{Msg, SearchMsg};
use crate::state::history_index::HistoryCount;
use crate::state::{AppState, Selection};
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use crate::ui::widgets::segmented::segmented;
use crate::ui::widgets::{self, Icon, truncate};
use eframe::egui::{self, Align, CursorIcon, Key, Layout, PopupCloseBehavior, Rect, RichText, Sense, pos2, vec2};
use trellis_core::matching::contains_lower;

/// How many authors the picker lists before asking for a narrower filter.
const MAX_AUTHORS: usize = 200;

/// Room after the query field: the Highlight | Filter mode (~150), the author picker (150 plus
/// its arrow and padding), the match count's slot (`COUNT_W`), Close (~55) and the spacing.
const CONTROLS_W: f32 = 565.0;

/// The match count's slot: kept whether or not there is a count, so the controls after it stay
/// put while it changes ("No matches", "3 of 12", "12 so far, searching…").
const COUNT_W: f32 = 150.0;

/// The field's right inner margin, where the clear button sits so text never runs under it.
const CLEAR_ROOM: f32 = 26.0;

/// The empty query field's hint (cut with "…" in a narrow field), also its hover text when the
/// field is empty and the hint is cut.
const HINT: &str = "Search messages, authors and SHAs; narrow with author: or path:";

pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(super::layout::EDGE_INSET);
        // The field gives up width before the controls after it run out of room.
        let width = (ui.available_width() - CONTROLS_W).clamp(160.0, 420.0);
        let empty = st.view.search.is_empty();
        // The text's width: the field keeps the same outer width with the clear button's room.
        let inner = width - (CLEAR_ROOM - 4.0);
        let (hint, hint_cut) = widgets::truncate::hint(ui, HINT, inner);
        // egui's default margin (4, 2), with room for the clear button at the right.
        let margin = egui::Margin { left: 4, right: CLEAR_ROOM as i8, top: 2, bottom: 2 };
        let edit = egui::TextEdit::singleline(&mut st.view.search).hint_text(hint).desired_width(inner).margin(margin);
        let r = controls::field(ui, edit);
        let r = if empty && hint_cut { r.on_hover_text(HINT) } else { r };
        if !st.view.search.is_empty() {
            clear_button(ui, r.rect, st.settings.lang().tr("search.clear.button"), msgs);
        }

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

        // The SearchFilter key flips between the two modes, so both tips name it.
        let highlight = st.keymap.tip("Mark the matching commits and dim the rest", BindingId::SearchFilter);
        let filter = st.keymap.tip("Show only matching commits", BindingId::SearchFilter);
        if segmented(ui, st.search.filter, &[(false, "Highlight", &highlight), (true, "Filter", &filter)]).is_some() {
            msgs.push(Msg::Search(SearchMsg::ToggleFilter));
        }

        author_picker(ui, st, msgs);

        let selected = match &st.selection {
            Selection::Commit(o) => st.snap.as_ref().and_then(|s| s.row_of.get(o).copied()),
            _ => None,
        };
        let history = st.history_count();
        let count = st.search.count_text(selected, history);
        ui.allocate_ui_with_layout(vec2(COUNT_W, ui.spacing().interact_size.y), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(COUNT_W);
            let Some(text) = count else { return };
            let (r, cut) = truncate::cut_label(ui, RichText::new(&text).color(color::weak()), COUNT_W);
            let loaded = st.snap.as_ref().filter(|s| s.truncated).map(|s| s.commits.len());
            let why = if st.search.searching_files() {
                Some("Looking through the files each loaded commit changed".to_owned())
            } else {
                match history {
                    HistoryCount::Indexing => loaded.map(|len| {
                        format!("Indexing the whole history in the background. Until it is done, the search covers the newest {len} loaded commits.")
                    }),
                    HistoryCount::Whole { positions, commits, complete } => Some(whole_text(positions.len(), commits, complete, st.search.matches.len())),
                    HistoryCount::Loaded | HistoryCount::Searching => {
                        loaded.map(|len| format!("Searches the newest {len} loaded commits; Load more searches further back."))
                    }
                }
            };
            match (cut, why) {
                (true, Some(why)) => r.on_hover_text(format!("{text}\n{why}")),
                (true, None) => r.on_hover_text(text),
                (false, Some(why)) => r.on_hover_text(why),
                (false, None) => r,
            };
        });

        if ui.add(buttons::secondary(st.settings.lang().tr("common.close.button")).small()).clicked() {
            msgs.push(Msg::Search(SearchMsg::Close));
        }
    });
}

/// The count's hover text once the whole-history index answered: `total` matches among its
/// `commits` commits (the whole history when `complete`), `loaded` of them in the graph.
fn whole_text(total: usize, commits: usize, complete: bool, loaded: usize) -> String {
    let matches = if total == 1 { "1 match".to_owned() } else { format!("{total} matches") };
    let scope = if complete { format!("in the whole history ({commits} commits)") } else { format!("in the newest {commits} commits") };
    format!("{matches} {scope}, {loaded} of them loaded. Enter loads older matches as it reaches them.")
}

/// The painted × inside the right end of the query field (`field` is its rect): empties the
/// query and the author filter and gives the field the keyboard again.
fn clear_button(ui: &mut egui::Ui, field: Rect, name: &str, msgs: &mut Vec<Msg>) {
    let x = Rect::from_center_size(pos2(field.right() - CLEAR_ROOM / 2.0, field.center().y), vec2(18.0, 18.0));
    let c = ui.interact(x, ui.id().with("search-clear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(name);
    let col = if c.hovered() { color::text() } else { color::faint() };
    widgets::paint_icon(ui.painter(), x.center(), Icon::Close, col);
    access::name_button(&c, true, name);
    if c.clicked() {
        msgs.push(Msg::Search(SearchMsg::Clear));
    }
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
        .width(150.0)
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
