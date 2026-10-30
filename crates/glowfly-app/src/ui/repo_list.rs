//! The welcome screen's repository sections: each repository group (`repo_list_groups.rs` draws
//! its header), then the ungrouped Favorites, then Recent. A row has its star, the repository's
//! colour swatch (a slot kept on every row once any repository or group has a colour, so the
//! paths line up), path, "open" and "not found" tags, the favorites' keys and the row menu with
//! Move to group and Color submenus; the headers have their links (New group, Open all, Remove
//! missing, Clear), and favorites drag into another order or group (`repo_list_drag.rs`). The
//! menus' rows are `repo_menu_list.rs`.

use super::common::{menu_item, menu_item_if};
use super::repo_list_drag::DropSection;
use super::repo_list_groups;
use super::repo_swatch;
use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::FAVORITE_BINDINGS;
use crate::msg::{GroupMsg, Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_colours::listed_colour;
use crate::state::repo_groups::{any_colour, group_hue, members, ungrouped};
use crate::state::repo_lists::{
    clear_recent_msg, favorite_keys_note, favorite_label, favorites, is_favorite, is_missing, is_open, missing_counts, recent_rows,
    remove_missing_msg,
};
use crate::ui::theme::type_scale;
use crate::ui::widgets::menu::{self, separator};
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align, Color32, Layout, Response, RichText, Sense, Stroke};
use glowfly_core::i18n::Lang;
use std::path::{Path, PathBuf};

/// The width of the welcome screen's repository column, when the window is wide enough.
const COLUMN_WIDTH: f32 = 560.0;

/// Which list a row belongs to: a favorite (its group, None for the ungrouped Favorites, its
/// index there, how many that section has, and its place among all favorites, which picks its
/// key) or a recent entry. Only Recent rows can be removed; only favorites move.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Favorite { group: Option<usize>, index: usize, count: usize, key: usize },
    Recent,
}

/// The welcome screen's repository sections, in a centred column below its buttons. Nothing is
/// drawn while there is no favorite, group or recent entry.
pub fn welcome_sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let groups = &st.settings.repo_groups;
    let all = favorites(&st.settings);
    let loose = ungrouped(&st.settings);
    let recent = recent_rows(st);
    if all.is_empty() && recent.is_empty() && groups.is_empty() {
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
    let draggable = !all.is_empty() && (all.len() >= 2 || !groups.is_empty());
    // New group goes on the first section's header once there is a favorite to put in one.
    let offer_new = !all.is_empty() || !groups.is_empty();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
        egui::ScrollArea::vertical().max_height(ui.available_height()).auto_shrink([false, true]).show(ui, |ui| {
            let mut sections = Vec::new();
            let mut key = 0;
            for g in 0..groups.len() {
                if g > 0 {
                    ui.add_space(12.0);
                }
                repo_list_groups::header(ui, st, g, g == 0 && offer_new, msgs);
                if g == 0 && draggable {
                    drag_note(ui, st);
                }
                let rows = members(&st.settings, g);
                sections.push(section(ui, st, Some(g), &rows, &mut key, draggable, msgs));
            }
            if !groups.is_empty() {
                ui.add_space(12.0);
            }
            ui.horizontal(|ui| {
                header(ui, t.tr("repos.favorites.header"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if loose.iter().any(|p| !is_open(st, p) && !is_missing(st, p)) && small_link(ui, t.tr("repos.open_all.link")) {
                        msgs.push(Msg::Recent(RecentMsg::OpenAll(loose.clone())));
                    }
                    if remove_on_favorites {
                        remove_missing_link(ui, st, msgs);
                    }
                    if groups.is_empty() && offer_new {
                        repo_list_groups::new_group_link(ui, t, msgs);
                    }
                });
            });
            if all.is_empty() {
                ui.label(RichText::new(t.tr("repos.favorites.empty")).color(color::weak()));
            } else {
                if groups.is_empty() && draggable {
                    drag_note(ui, st);
                }
                sections.push(section(ui, st, None, &loose, &mut key, draggable, msgs));
            }
            super::repo_list_drag::track(ui, &sections, msgs);
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
                    row(ui, st, p, Section::Recent, false, msgs);
                }
            }
        });
    });
}

/// The rows of one favorites section (`group`; None: the ungrouped ones), or its placeholder
/// line while it has none. `key` counts the favorites drawn so far.
fn section(
    ui: &mut egui::Ui,
    st: &AppState,
    group: Option<usize>,
    paths: &[PathBuf],
    key: &mut usize,
    draggable: bool,
    msgs: &mut Vec<Msg>,
) -> DropSection {
    let t = st.settings.lang();
    let mut rows = Vec::new();
    for (index, p) in paths.iter().enumerate() {
        let section = Section::Favorite { group, index, count: paths.len(), key: *key };
        *key += 1;
        if let Some((grip, rect)) = row(ui, st, p, section, draggable, msgs) {
            rows.push((p.clone(), grip, rect));
        }
    }
    let empty = paths.is_empty().then(|| {
        let text = if group.is_some() { t.tr("repo_group.empty") } else { t.tr("repos.favorites.all_grouped") };
        repo_list_groups::placeholder(ui, text)
    });
    DropSection { group, rows, empty }
}

/// The weak line under the first header: how to drag, and the favorites' keys.
fn drag_note(ui: &mut egui::Ui, st: &AppState) {
    let t = st.settings.lang();
    let grouped = !st.settings.repo_groups.is_empty();
    let note = match (favorite_keys_note(t, &st.keymap), grouped) {
        (Some(keys), false) => t.trf("repos.drag_note_keys", &[("keys", &keys)]),
        (Some(keys), true) => t.trf("repos.drag_note_groups_keys", &[("keys", &keys)]),
        (None, false) => t.tr("repos.drag_note").to_owned(),
        (None, true) => t.tr("repos.drag_note_groups").to_owned(),
    };
    ui.label(RichText::new(note).size(type_scale::SMALL).color(color::weak()));
}

/// A section header's title text: weak, as every header shows it.
pub(super) fn header_text(title: &str) -> RichText {
    RichText::new(title).color(color::weak())
}

fn header(ui: &mut egui::Ui, title: &str) {
    menu::inset(ui, |ui| ui.label(header_text(title)));
}

pub(super) fn small_link(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Link::new(RichText::new(text).size(type_scale::SMALL))).clicked()
}

fn remove_missing_link(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if small_link(ui, st.settings.lang().tr("repos.remove_missing.link")) {
        msgs.extend(remove_missing_msg(st));
    }
}

/// A weak "open" tag; clicking it switches to the tab showing the repository (`open`).
fn open_tag(ui: &mut egui::Ui, t: Lang, open: Msg, msgs: &mut Vec<Msg>) {
    let text = RichText::new(t.tr("repos.open_tag.label")).size(type_scale::SMALL).color(color::weak());
    let tag = egui::Label::new(text).selectable(false).sense(Sense::click());
    let clicked = ui.add(tag).on_hover_text(t.tr("repos.open_tag.tip")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
    if clicked {
        msgs.push(open);
    }
}

/// One repository: a grip (welcome favorites, when `draggable`), star toggle, the path (opens
/// it), "not found" and "open" tags, a favorite's key and, for Recent rows, ×. Returns the
/// grip's response and the row's rect when it has a grip.
fn row(
    ui: &mut egui::Ui,
    st: &AppState,
    path: &Path,
    section: Section,
    draggable: bool,
    msgs: &mut Vec<Msg>,
) -> Option<(Response, egui::Rect)> {
    let t = st.settings.lang();
    let fav = is_favorite(st, path);
    let missing = is_missing(st, path);
    let in_tab = is_open(st, path);
    let text = glowfly_core::native_path::shown(path);
    let room = any_colour(&st.settings);
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if section == Section::Recent && widgets::small_icon(ui, Icon::Close, t.tr("repos.remove_recent"), color::weak()).clicked() {
                msgs.push(remove());
            }
            if let Section::Favorite { key, .. } = section
                && let Some(key) = FAVORITE_BINDINGS.get(key).and_then(|&id| st.keymap.hint(id))
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
                    row_menu(ui, st, path, section, [open(), favorite(), remove()], msgs);
                    separator(ui);
                    let own = (st.settings.repo_colour_for(path), group_hue(st, path).is_some());
                    super::repo_colour_menu::items(ui, t, Some(path), own, msgs);
                });
            });
        });
    });
    grip.map(|g| (g, r.response.rect))
}

/// The welcome row's context menu; `acts` are its Open, favorite toggle and Remove messages. A
/// favorite moves up and down within its section, and to another group.
fn row_menu(ui: &mut egui::Ui, st: &AppState, path: &Path, section: Section, acts: [Msg; 3], msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let [open, favorite, remove] = acts;
    menu_item(msgs, ui, t.tr("repos.row_menu.open"), open);
    menu_item(msgs, ui, favorite_label(t, is_favorite(st, path)), favorite);
    match section {
        Section::Recent => menu_item(msgs, ui, t.tr("repos.remove_recent"), remove),
        Section::Favorite { group, index, count, .. } => {
            let (up, down) = match group {
                Some(_) => (t.tr("repos.row_menu.move_up.blocked_group"), t.tr("repos.row_menu.move_down.blocked_group")),
                None => (t.tr("repos.row_menu.move_up.blocked"), t.tr("repos.row_menu.move_down.blocked")),
            };
            let first = (index == 0).then_some(up);
            let last = (index + 1 >= count).then_some(down);
            let moves =
                [(t.tr("repos.row_menu.move_up"), first, index.saturating_sub(1)), (t.tr("repos.row_menu.move_down"), last, index + 1)];
            for (label, blocked, index) in moves {
                menu_item_if(msgs, ui, label, blocked, Msg::Groups(GroupMsg::MoveRepo { path: path.to_path_buf(), group, index }));
            }
            super::repo_group_menu::items(ui, t, &st.settings.repo_groups, path, group, msgs);
        }
    }
    menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(glowfly_core::native_path::shown(path)));
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
