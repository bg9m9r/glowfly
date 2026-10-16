//! Favorite and recent repository rows, shared by the welcome screen, the toolbar's repository
//! menu and File > Open recent.

use super::common::menu_item;
use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_lists::{clear_recent_confirm, favorite_label, favorites, is_favorite, is_missing, recent_rows};
use eframe::egui::{self, Align, Color32, Layout, Response, RichText, Sense, Stroke};
use std::path::Path;

/// The width of the welcome screen's repository column, when the window is wide enough.
const COLUMN_WIDTH: f32 = 560.0;
const NOT_FOUND_TIP: &str = "This folder can't be found. It may have been moved or deleted, or be on a drive that isn't connected.";
const EMPTY_FAVORITES: &str = "Star a repository to keep it here.";

/// Where a row is drawn.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Welcome,
    Menu,
}

/// Which list a row belongs to; only Recent rows can be removed.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Favorite,
    Recent,
}

/// The welcome screen's Favorites and Recent sections, in a centred column below its buttons.
/// Nothing is drawn while both lists are empty.
pub fn welcome_sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
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
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
        egui::ScrollArea::vertical().max_height(ui.available_height()).auto_shrink([false, true]).show(ui, |ui| {
            header(ui, "Favorites");
            if favs.is_empty() {
                ui.label(RichText::new(EMPTY_FAVORITES).color(color::weak()));
            }
            for p in &favs {
                row(ui, st, p, Place::Welcome, Section::Favorite, msgs);
            }
            if !recent.is_empty() {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    header(ui, "Recent");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Link::new(RichText::new("Clear").small())).clicked() {
                            msgs.push(clear_confirm(st));
                        }
                    });
                });
                for p in recent {
                    row(ui, st, p, Place::Welcome, Section::Recent, msgs);
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

/// Whether there is a favorite or a recent repository to list.
pub fn has_entries(st: &AppState) -> bool {
    !favorites(&st.settings).is_empty() || !recent_rows(st).is_empty()
}

fn sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>, lead_separator: bool) {
    let favs = favorites(&st.settings);
    if !favs.is_empty() {
        if lead_separator {
            ui.separator();
        }
        header(ui, "Favorites");
        for p in &favs {
            row(ui, st, p, Place::Menu, Section::Favorite, msgs);
        }
    }
    let recent = recent_rows(st);
    if !recent.is_empty() {
        if lead_separator || !favs.is_empty() {
            ui.separator();
        }
        header(ui, "Recent");
        for p in recent {
            row(ui, st, p, Place::Menu, Section::Recent, msgs);
        }
        menu_item(msgs, ui, "Clear recent", clear_confirm(st));
    }
}

fn header(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).color(color::weak()));
}

/// The question asked before the Recent list is emptied.
fn clear_confirm(st: &AppState) -> Msg {
    let (title, body) = clear_recent_confirm(st.recent.len());
    Msg::confirm(&title, body, "Clear", Msg::Recent(RecentMsg::ClearRecent))
}

/// One repository: star toggle, the path (opens it), a "not found" note and, for Recent rows, ×.
fn row(ui: &mut egui::Ui, st: &AppState, path: &Path, place: Place, section: Section, msgs: &mut Vec<Msg>) {
    let fav = is_favorite(st, path);
    let missing = is_missing(st, path);
    let text = trellis_core::native_path::shown(path);
    let open = || Msg::Repo(RepoMsg::Open(path.to_path_buf()));
    let favorite = || Msg::Recent(RecentMsg::SetFavorite { path: path.to_path_buf(), favorite: !fav });
    let remove = || Msg::Recent(RecentMsg::Remove(path.to_path_buf()));
    ui.horizontal(|ui| {
        let (icon, col) = if fav { (Icon::StarFilled, color::amber()) } else { (Icon::Star, color::weak()) };
        if widgets::small_icon(ui, icon, favorite_label(fav), col).clicked() {
            msgs.push(favorite());
        }
        if place == Place::Menu {
            let col = if missing { color::faint() } else { color::text() };
            let mut r = ui.add(egui::Button::new(RichText::new(&text).color(col)));
            if missing {
                r = r.on_hover_text(NOT_FOUND_TIP);
            }
            if r.clicked() {
                msgs.push(open());
                ui.close_menu();
            }
            if section == Section::Recent && widgets::small_icon(ui, Icon::Close, "Remove from recent", color::weak()).clicked() {
                msgs.push(remove());
            }
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if section == Section::Recent && widgets::small_icon(ui, Icon::Close, "Remove from recent", color::weak()).clicked() {
                msgs.push(remove());
            }
            if missing {
                ui.label(RichText::new("not found").small().color(color::weak())).on_hover_text(NOT_FOUND_TIP);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let r = link(ui, &text, if missing { color::faint() } else { color::accent() });
                if r.clicked() {
                    msgs.push(open());
                }
                r.context_menu(|ui| {
                    menu_item(msgs, ui, "Open", open());
                    menu_item(msgs, ui, favorite_label(fav), favorite());
                    if section == Section::Recent {
                        menu_item(msgs, ui, "Remove from recent", remove());
                    }
                    menu_item(msgs, ui, "Copy path", Msg::Copy(text.clone()));
                });
            });
        });
    });
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
