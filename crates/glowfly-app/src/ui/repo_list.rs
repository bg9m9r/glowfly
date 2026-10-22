//! Favorite and recent repository rows, shared by the welcome screen, the toolbar's repository
//! menu and File > Open recent: star, the repository's colour swatch (a slot kept on every row
//! once any repository has a colour, so the paths line up), path, "open" and "not found" tags,
//! the favorites' keys, and on the welcome screen the row menu's Color submenu, the header links (Open all, Remove missing, Clear) and dragging
//! favorites into another order (`repo_list_drag.rs`).

use super::common::{menu_item, menu_item_if};
use super::repo_swatch;
use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::FAVORITE_BINDINGS;
use crate::msg::{Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_colours::listed_colour;
use crate::state::repo_lists::{
    clear_recent_msg, favorite_keys_note, favorite_label, favorites, is_favorite, is_missing, is_open, missing_counts, recent_rows,
    remove_missing_msg, unopened_favorites,
};
use crate::ui::theme::type_scale;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align, Color32, Layout, Response, RichText, Sense, Stroke};
use glowfly_core::i18n::Lang;
use std::path::Path;

/// The width of the welcome screen's repository column, when the window is wide enough.
const COLUMN_WIDTH: f32 = 560.0;

/// Where a row is drawn.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Welcome,
    Menu,
}

/// Which list a row belongs to: a favorite (its index and how many there are) or a recent
/// entry. Only Recent rows can be removed; only favorites move.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Favorite { index: usize, count: usize },
    Recent,
}

/// The welcome screen's Favorites and Recent sections, in a centred column below its buttons.
/// Nothing is drawn while both lists are empty.
pub fn welcome_sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let favs = favorites(&st.settings);
    let recent = recent_rows(st);
    if favs.is_empty() && recent.is_empty() {
        return;
    }
    ui.add_space(20.0);
    let width = ui.available_width().min(COLUMN_WIDTH);
    let top = ui.cursor().top();
    let full = ui.max_rect();
    let rect =
        egui::Rect::from_min_max(egui::pos2(full.center().x - width / 2.0, top), egui::pos2(full.center().x + width / 2.0, full.bottom()));
    let (missing_recent, missing_favs) = missing_counts(st);
    // "Remove missing" goes on the Recent header, or on Favorites when only favorites are gone.
    let remove_on_favorites = missing_recent == 0 && missing_favs > 0;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
        egui::ScrollArea::vertical().max_height(ui.available_height()).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                header(ui, t.tr("repos.favorites.header"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !unopened_favorites(st).is_empty() && small_link(ui, t.tr("repos.open_all.link")) {
                        msgs.push(Msg::Recent(RecentMsg::OpenAll(favs.clone())));
                    }
                    if remove_on_favorites {
                        remove_missing_link(ui, st, msgs);
                    }
                });
            });
            if favs.is_empty() {
                ui.label(RichText::new(t.tr("repos.favorites.empty")).color(color::weak()));
            }
            let draggable = favs.len() >= 2;
            if draggable {
                let note = match favorite_keys_note(t, &st.keymap) {
                    Some(keys) => t.trf("repos.drag_note_keys", &[("keys", &keys)]),
                    None => t.tr("repos.drag_note").to_owned(),
                };
                ui.label(RichText::new(note).size(type_scale::SMALL).color(color::weak()));
            }
            let mut rows = Vec::new();
            for (index, p) in favs.iter().enumerate() {
                let section = Section::Favorite { index, count: favs.len() };
                rows.extend(row(ui, st, p, Place::Welcome, section, draggable, msgs));
            }
            super::repo_list_drag::track(ui, &rows, msgs);
            if !recent.is_empty() {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    header(ui, t.tr("repos.recent.header"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_link(ui, t.tr("repos.clear.link")) {
                            msgs.push(clear_recent_msg(st));
                        }
                        if missing_recent > 0 {
                            remove_missing_link(ui, st, msgs);
                        }
                    });
                });
                for p in recent {
                    row(ui, st, p, Place::Welcome, Section::Recent, false, msgs);
                }
            }
        });
    });
}

/// The repository menu's Favorites (when there are any) and Recent sections and "Clear recent".
pub fn menu_sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    sections(ui, st, msgs, true);
}

/// File > Open recent: the same sections, without the separator above the first.
pub fn recent_submenu(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    sections(ui, st, msgs, false);
}

fn sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>, lead_separator: bool) {
    let t = st.settings.lang();
    let favs = favorites(&st.settings);
    if !favs.is_empty() {
        if lead_separator {
            ui.separator();
        }
        header(ui, t.tr("repos.favorites.header"));
        for (index, p) in favs.iter().enumerate() {
            row(ui, st, p, Place::Menu, Section::Favorite { index, count: favs.len() }, false, msgs);
        }
    }
    let recent = recent_rows(st);
    if !recent.is_empty() {
        if lead_separator || !favs.is_empty() {
            ui.separator();
        }
        header(ui, t.tr("repos.recent.header"));
        for p in recent {
            row(ui, st, p, Place::Menu, Section::Recent, false, msgs);
        }
        menu_item(msgs, ui, t.tr("repos.menu.clear_recent"), clear_recent_msg(st));
    }
}

fn header(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).color(color::weak()));
}

fn small_link(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Link::new(RichText::new(text).size(type_scale::SMALL))).clicked()
}

fn remove_missing_link(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if small_link(ui, st.settings.lang().tr("repos.remove_missing.link")) {
        msgs.extend(remove_missing_msg(st));
    }
}

/// A weak "open" tag; clicking it switches to the tab showing the repository (`open`).
fn open_tag(ui: &mut egui::Ui, t: Lang, open: Msg, msgs: &mut Vec<Msg>) -> bool {
    let text = RichText::new(t.tr("repos.open_tag.label")).size(type_scale::SMALL).color(color::weak());
    let tag = egui::Label::new(text).selectable(false).sense(Sense::click());
    let clicked = ui.add(tag).on_hover_text(t.tr("repos.open_tag.tip")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
    if clicked {
        msgs.push(open);
    }
    clicked
}

/// One repository: a grip (welcome favorites, when `draggable`), star toggle, the path (opens
/// it), "not found" and "open" tags, a favorite's key and, for Recent rows, ×. Returns the
/// grip's response and the row's rect when it has a grip.
fn row(
    ui: &mut egui::Ui,
    st: &AppState,
    path: &Path,
    place: Place,
    section: Section,
    draggable: bool,
    msgs: &mut Vec<Msg>,
) -> Option<(Response, egui::Rect)> {
    let t = st.settings.lang();
    let fav = is_favorite(st, path);
    let missing = is_missing(st, path);
    let in_tab = is_open(st, path);
    let text = glowfly_core::native_path::shown(path);
    let room = !st.settings.repo_colours.is_empty();
    let swatch = listed_colour(st, path);
    let open = || Msg::Repo(RepoMsg::Open(path.to_path_buf()));
    let favorite = || Msg::Recent(RecentMsg::SetFavorite { path: path.to_path_buf(), favorite: !fav });
    let remove = || Msg::Recent(RecentMsg::Remove(path.to_path_buf()));
    let mut grip = None;
    let r = ui.horizontal(|ui| {
        if draggable {
            grip = Some(super::repo_list_drag::grip(ui, t.tr("repos.drag.grip")));
        }
        let (icon, col) = if fav { (Icon::StarFilled, color::warn()) } else { (Icon::Star, color::weak()) };
        if widgets::small_icon(ui, icon, favorite_label(t, fav), col).clicked() {
            msgs.push(favorite());
        }
        if place == Place::Menu {
            let col = if missing { color::faint() } else { color::text() };
            swatch_slot(ui, room, swatch);
            let mut r = ui.add(egui::Button::new(RichText::new(&text).color(col)));
            if missing {
                r = r.on_hover_text(t.tr("repos.not_found.tip"));
            }
            if r.clicked() {
                msgs.push(open());
                ui.close_menu();
            }
            if in_tab && open_tag(ui, t, open(), msgs) {
                ui.close_menu();
            }
            if section == Section::Recent && widgets::small_icon(ui, Icon::Close, t.tr("repos.remove_recent"), color::weak()).clicked() {
                msgs.push(remove());
            }
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if section == Section::Recent && widgets::small_icon(ui, Icon::Close, t.tr("repos.remove_recent"), color::weak()).clicked() {
                msgs.push(remove());
            }
            if let Section::Favorite { index, .. } = section
                && let Some(key) = FAVORITE_BINDINGS.get(index).and_then(|&id| st.keymap.hint(id))
            {
                ui.label(RichText::new(key).size(type_scale::SMALL).color(color::weak()));
            }
            if missing {
                ui.label(RichText::new(t.tr("repos.not_found.label")).size(type_scale::SMALL).color(color::weak()))
                    .on_hover_text(t.tr("repos.not_found.tip"));
            }
            if in_tab {
                open_tag(ui, t, open(), msgs);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                swatch_slot(ui, room, swatch);
                let r = link(ui, &text, if missing { color::faint() } else { color::accent() });
                if r.clicked() {
                    msgs.push(open());
                }
                r.row_menu(|ui| {
                    row_menu(ui, t, &text, section, fav, [open(), favorite(), remove()], msgs);
                    ui.separator();
                    super::repo_colour_menu::items(ui, t, Some(path), swatch, msgs);
                });
            });
        });
    });
    grip.map(|g| (g, r.response.rect))
}

/// The welcome row's context menu; `acts` are its Open, favorite toggle and Remove messages.
fn row_menu(ui: &mut egui::Ui, t: Lang, text: &str, section: Section, fav: bool, acts: [Msg; 3], msgs: &mut Vec<Msg>) {
    let [open, favorite, remove] = acts;
    menu_item(msgs, ui, t.tr("repos.row_menu.open"), open);
    menu_item(msgs, ui, favorite_label(t, fav), favorite);
    match section {
        Section::Recent => menu_item(msgs, ui, t.tr("repos.remove_recent"), remove),
        Section::Favorite { index, count } => {
            let first = (index == 0).then(|| t.tr("repos.row_menu.move_up.blocked"));
            let last = (index + 1 >= count).then(|| t.tr("repos.row_menu.move_down.blocked"));
            let moves =
                [(t.tr("repos.row_menu.move_up"), first, index.saturating_sub(1)), (t.tr("repos.row_menu.move_down"), last, index + 1)];
            for (label, blocked, to) in moves {
                menu_item_if(msgs, ui, label, blocked, Msg::Recent(RecentMsg::MoveFavorite { from: index, to }));
            }
        }
    }
    menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(text.to_owned()));
}

/// The swatch's slot before a path while any repository has a colour (`room`), with the
/// swatch in it when this one has a colour.
fn swatch_slot(ui: &mut egui::Ui, room: bool, swatch: Option<glowfly_core::settings::RepoColour>) {
    if !room {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(egui::vec2(repo_swatch::ROOM, repo_swatch::SIZE), Sense::hover());
    if let Some(c) = swatch {
        repo_swatch::paint(ui.painter(), rect.left_center(), repo_swatch::SIZE, color::repo_hue(c));
    }
}

/// A link that is cut short with "…" when it doesn't fit (the full text shows on hover).
fn link(ui: &mut egui::Ui, text: &str, col: Color32) -> Response {
    let r = ui.add(egui::Label::new(RichText::new(text).color(col)).truncate().selectable(false).sense(Sense::click()));
    if r.hovered() {
        let y = r.rect.bottom() - 1.0;
        ui.painter().hline(r.rect.x_range(), y, Stroke::new(1.0_f32, col));
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}
