//! One start-page repository row (`repo_row` draws it, with the start page's grip, pill, key cap
//! and ⋯): what a click (open or switch to its tab), the star and × send, and the row's menu, on
//! a right-click or from ⋯: Open, the favorite toggle, a favorite's Move up, Move down and Move
//! to group, a Recent entry's Remove from recent, Copy path, then Color.

use super::common::{menu_item, menu_item_if};
use super::repo_menu_list;
use super::repo_row::{self, Act, Columns, Row, Shown, Words};
use crate::keymap::FAVORITE_BINDINGS;
use crate::msg::{GroupMsg, Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::state::repo_groups::group_hue;
use crate::state::repo_lists::{favorite_label, is_favorite};
use crate::ui::widgets::menu::separator;
use eframe::egui::{self, Id};
use std::path::{Path, PathBuf};

/// Which list a row belongs to: a favorite (its group, None for the ungrouped ones, its index
/// there and how many that section has) or a Recent entry. Only Recent rows can be removed;
/// only favorites move.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Section {
    Favorite { group: Option<usize>, index: usize, count: usize },
    Recent,
}

/// The rows of the favorites `paths` in `group` (None: the ungrouped ones); `key` counts the
/// favorites drawn before them, which picks each one's key.
pub(super) fn favorite_rows(
    st: &AppState,
    paths: &[PathBuf],
    group: Option<usize>,
    key: &mut usize,
    draggable: bool,
) -> Vec<(Row, Section)> {
    let count = paths.len();
    let mut out = Vec::with_capacity(count);
    for (index, p) in paths.iter().enumerate() {
        let mut row = make(st, p, false);
        row.key = FAVORITE_BINDINGS.get(*key).and_then(|&id| st.keymap.hint(id)).map(str::to_owned);
        row.draggable = draggable;
        *key += 1;
        out.push((row, Section::Favorite { group, index, count }));
    }
    out
}

/// The rows of the Recent list.
pub(super) fn recent_rows(st: &AppState, paths: &[PathBuf]) -> Vec<(Row, Section)> {
    paths.iter().map(|p| (make(st, p, true), Section::Recent)).collect()
}

/// The menus' row for `path`, under the start page's own id.
fn make(st: &AppState, path: &Path, removable: bool) -> Row {
    let mut row = repo_menu_list::row(st, path, removable);
    row.id = Id::new(("welcome-repo-row", &row.full));
    row
}

/// Draws `row` of `section` and sends what was done on it.
pub(super) fn show(
    ui: &mut egui::Ui,
    st: &AppState,
    (row, section): &(Row, Section),
    cols: Columns,
    words: &Words<'_>,
    msgs: &mut Vec<Msg>,
) -> Shown {
    let path = row.path.as_path();
    let mut menu_msgs = Vec::new();
    let mut items = |ui: &mut egui::Ui| {
        row_menu(ui, st, path, *section, &mut menu_msgs);
        separator(ui);
        let t = st.settings.lang();
        let own = (st.settings.repo_colour_for(path), group_hue(st, path).is_some());
        super::repo_colour_menu::items(ui, t, Some(path), own, &mut menu_msgs);
    };
    let shown = repo_row::show(ui, row, cols, words, Some(&mut items));
    msgs.append(&mut menu_msgs);
    match shown.act {
        Some(Act::Open) => msgs.push(Msg::Repo(RepoMsg::Open(path.to_path_buf()))),
        Some(Act::ToggleFavorite) => msgs.push(Msg::Recent(RecentMsg::SetFavorite { path: path.to_path_buf(), favorite: !row.favorite })),
        Some(Act::Remove) => msgs.push(Msg::Recent(RecentMsg::Remove(path.to_path_buf()))),
        None => {}
    }
    shown
}

/// The row's menu before Color. A favorite moves up and down within its section, and to
/// another group.
fn row_menu(ui: &mut egui::Ui, st: &AppState, path: &Path, section: Section, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let fav = is_favorite(st, path);
    menu_item(msgs, ui, t.tr("repos.row_menu.open"), Msg::Repo(RepoMsg::Open(path.to_path_buf())));
    menu_item(msgs, ui, favorite_label(t, fav), Msg::Recent(RecentMsg::SetFavorite { path: path.to_path_buf(), favorite: !fav }));
    match section {
        Section::Recent => menu_item(msgs, ui, t.tr("repos.remove_recent"), Msg::Recent(RecentMsg::Remove(path.to_path_buf()))),
        Section::Favorite { group, index, count } => {
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
