//! The repository lists of the toolbar's repository menu and File > Open recent: each
//! repository group (its swatch in the header's icon column), the ungrouped Favorites and Recent
//! under group headers (a header shows only while it has rows), one `repo_menu_row` per
//! repository with shared columns, a
//! filter field above them once there are more than `FILTER_FROM` (focused when the menu
//! opens; Enter opens the first match, Down moves to it), and the closing group, Remove
//! missing entries… and Clear recent…, both of which ask first.

use super::common::{menu_danger, menu_item};
use super::repo_menu_row::{self, Act, Columns, Row, Status, Words};
use super::repo_swatch;
use super::theme::color;
use super::widgets::menu;
use super::widgets::menu_row::PaintIcon;
use super::widgets::text_field::TextField;
use crate::msg::{Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_colours::listed_colour;
use crate::state::repo_groups::{any_colour, members, ungrouped};
use crate::state::repo_lists::{
    clear_recent_msg, favorites, is_missing, is_open, matches_filter, missing_counts, recent_rows, remove_missing_msg,
};
use eframe::egui::{self, Id, Key, RichText};
use glowfly_core::settings::RepoColour;
use std::path::{Path, PathBuf};

/// The menu's width while it lists repositories: room for a name, its parent and a status.
pub const WIDTH: f32 = 380.0;
/// The filter field shows once more repositories than this are listed.
pub const FILTER_FROM: usize = 8;

/// The Favorites and Recent groups, with the filter field above them when `filter` and the
/// lists are long. Returns whether anything was drawn.
pub fn lists(ui: &mut egui::Ui, st: &AppState, filter: bool, msgs: &mut Vec<Msg>) -> bool {
    let t = st.settings.lang();
    let favs = favorites(&st.settings);
    let recent: Vec<PathBuf> = recent_rows(st).into_iter().cloned().collect();
    if favs.is_empty() && recent.is_empty() {
        return false;
    }
    let s = &st.settings;
    ui.set_min_width(WIDTH);
    let query = if filter && favs.len() + recent.len() > FILTER_FROM { Some(filter_field(ui, t.tr("repos.menu.filter"))) } else { None };
    let q = query.as_ref().map_or("", |(q, _)| q.as_str());
    let make = |p: &PathBuf, removable| row(st, p, removable);
    let rows_of =
        |list: &[PathBuf], removable| -> Vec<Row> { list.iter().filter(|p| matches_filter(p, q)).map(|p| make(p, removable)).collect() };
    // (header, its swatch, rows): the groups, then the ungrouped favorites, then Recent.
    let mut sections: Vec<(&str, Option<RepoColour>, Vec<Row>)> =
        s.repo_groups.iter().enumerate().map(|(g, group)| (group.name.as_str(), group.hue(), rows_of(&members(s, g), false))).collect();
    sections.push((t.tr("repos.favorites.header"), None, rows_of(&ungrouped(s), false)));
    sections.push((t.tr("repos.recent.header"), None, rows_of(&recent, true)));
    let words = words(st);
    let all: Vec<&Row> = sections.iter().flat_map(|(_, _, rows)| rows).collect();
    let cols = Columns::measure(ui, &all, &words, any_colour(s));
    if let Some((_, field)) = &query {
        if let Some(first) = all.first() {
            if super::widgets::text_field::entered(ui, field) {
                msgs.push(Msg::Repo(RepoMsg::Open(first.path.clone())));
                ui.close_menu();
            }
            if field.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)) {
                ui.memory_mut(|m| m.request_focus(repo_menu_row::row_id(&first.full)));
            }
        } else {
            menu::inset(ui, |ui| ui.label(RichText::new(t.tr("repos.menu.no_match")).color(color::weak())));
        }
    }
    for (title, hue, rows) in &sections {
        if rows.is_empty() {
            continue;
        }
        menu::group_header_with(ui, title, hue.map(|c| (repo_swatch::paint_slot as PaintIcon, color::repo_hue(c))));
        for r in rows {
            let done = repo_menu_row::show(ui, r, cols, &words);
            act(ui, r, done, msgs);
        }
    }
    true
}

/// The closing group, after a separator: Remove missing entries (n)… while the last check
/// found any, and Clear recent… in the danger style while the Recent list has rows. Nothing
/// when neither applies.
pub fn tidy(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let (recent, favs) = missing_counts(st);
    let ask = remove_missing_msg(st);
    if ask.is_some() || !st.recent.is_empty() {
        menu::separator(ui);
    }
    if let Some(ask) = ask {
        menu_item(msgs, ui, t.trn("toolbar.repo_menu.remove_missing", recent + favs, &[]), ask);
    }
    if !st.recent.is_empty() {
        menu_danger(msgs, ui, t.tr("repos.menu.clear_recent"), None, clear_recent_msg(st));
    }
}

/// File > Open recent: the lists, then the closing group.
pub fn recent_submenu(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    lists(ui, st, false, msgs);
    tidy(ui, st, msgs);
}

/// The filter field, focused when the menu opens, and the text in it (kept while the menu
/// stays open, emptied when it opens again).
fn filter_field(ui: &mut egui::Ui, hint: &str) -> (String, egui::Response) {
    let id = Id::new("repo-menu-filter");
    let pass = ui.ctx().cumulative_pass_nr();
    let (last, mut text) = ui.data(|d| d.get_temp::<(u64, String)>(id)).unwrap_or_default();
    let opened = last + 1 < pass;
    if opened {
        text.clear();
    }
    let r = menu::inset(ui, |ui| ui.add(TextField::new(&mut text).id(id.with("field")).hint(hint)));
    super::access::name_field(&r, hint);
    super::menu_keys::register(ui, &r, None, false);
    if opened {
        r.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id, (pass, text.clone())));
    (text, r)
}

/// Runs what was done on `r`: opening closes the menu; the star and × leave it open, so several
/// rows can be tidied at once.
fn act(ui: &mut egui::Ui, r: &Row, done: Option<Act>, msgs: &mut Vec<Msg>) {
    match done {
        Some(Act::Open) => {
            msgs.push(Msg::Repo(RepoMsg::Open(r.path.clone())));
            ui.close_menu();
        }
        Some(Act::ToggleFavorite) => msgs.push(Msg::Recent(RecentMsg::SetFavorite { path: r.path.clone(), favorite: !r.favorite })),
        Some(Act::Remove) => msgs.push(Msg::Recent(RecentMsg::Remove(r.path.clone()))),
        None => {}
    }
}

/// The row for `path`, a favorite unless `removable` (a Recent row).
fn row(st: &AppState, path: &Path, removable: bool) -> Row {
    let (name, parent) = glowfly_core::native_path::name_and_parent(path);
    let current = st.snap.as_ref().is_some_and(|s| st.same_folder(&s.workdir, path));
    let status = if is_missing(st, path) {
        Some(Status::NotFound)
    } else if current {
        Some(Status::Current)
    } else if is_open(st, path) {
        Some(Status::InTab)
    } else {
        None
    };
    Row {
        path: path.to_path_buf(),
        name,
        parent,
        full: glowfly_core::native_path::shown(path),
        status,
        favorite: !removable,
        removable,
        swatch: listed_colour(st, path).map(color::repo_hue),
    }
}

fn words(st: &AppState) -> Words<'static> {
    let t = st.settings.lang();
    Words {
        current: t.tr("repos.menu.status.current"),
        in_tab: t.tr("repos.menu.status.in_tab"),
        not_found: t.tr("repos.menu.status.not_found"),
        not_found_tip: t.tr("repos.not_found.tip"),
        favorite_add: t.tr("repos.favorite.add"),
        favorite_remove: t.tr("repos.favorite.remove"),
        remove: t.tr("repos.remove_recent"),
    }
}
