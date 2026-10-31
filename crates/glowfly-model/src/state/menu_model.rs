//! The five menus (File, Edit, View, Window, Help) as plain data, built from the state on demand
//! (`menu_model_items.rs` builds each): every item's label, the binding whose key it shows, why
//! it is disabled, its check or radio mark, the action it runs, and the separators, headings and
//! submenus between them. The drawn menus (`ui/menu_bar/draw.rs`) and the macOS system menu bar
//! (`menu_native.rs`) both come from it, so they can't disagree. `Msg` can't be cloned, so a
//! model owns its messages and is built again whenever one is needed.

use super::AppState;
use super::menu_keys::TopMenu;
use crate::keymap::{BindingId, Chord};
use crate::msg::{Msg, SearchMsg};

/// One top-level menu.
pub struct Menu {
    /// Its title in the language.
    pub title: &'static str,
    pub entries: Vec<Entry>,
}

/// A row of a menu.
pub enum Entry {
    Item(Item),
    Separator,
    /// A weak title over a group of items.
    Heading(String),
    Submenu(Submenu),
    /// An item `take_item` already took: it keeps the later items' indices.
    Taken,
}

/// A submenu, disabled with `blocked` as its reason.
pub struct Submenu {
    pub kind: SubmenuKind,
    pub label: String,
    pub blocked: Option<String>,
    pub entries: Vec<Entry>,
}

/// Which submenu: the drawn Open recent keeps its own rows (stars, swatches, ×), so the entries
/// of `Recent` serve only the system menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuKind {
    Recent,
    Theme,
}

/// A clickable item.
pub struct Item {
    pub label: String,
    /// The binding whose key it shows (the tests check `chord` against it).
    #[cfg_attr(not(test), allow(dead_code))]
    pub key: Option<BindingId>,
    /// The key shown: `key`'s first chord, or a tab's digit.
    pub chord: Option<Chord>,
    /// Disabled, with this reason.
    pub blocked: Option<String>,
    pub mark: Mark,
    pub role: Role,
    pub action: MenuAction,
}

/// How an item shows its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    None,
    /// A check item, ✓ while on (the panels, full screen, Ignore whitespace …).
    Check(bool),
    /// One of a group's options (Diff layout, the themes), a dot while chosen.
    Radio(bool),
    /// The current one of a list (the Window menu's tabs), ✓ while it is.
    Selected(bool),
}

impl Mark {
    /// Whether the mark is checked, selected or on.
    pub fn on(self) -> bool {
        match self {
            Mark::None => false,
            Mark::Check(on) | Mark::Radio(on) | Mark::Selected(on) => on,
        }
    }
}

/// Where the macOS system menu bar puts an item: About, Preferences and Quit go to the app menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Plain,
    Prefs,
    Quit,
    About,
}

/// What clicking an item does.
pub enum MenuAction {
    /// Sends the message.
    Send(Box<Msg>),
    /// Shows or hides the activity log (view state, like the toolbar's Activity button).
    ToggleActivityLog,
    /// What the filter key does: shows the find bar and flips the filter.
    ShowFilter,
}

impl MenuAction {
    pub fn send(msg: Msg) -> Self {
        MenuAction::Send(Box::new(msg))
    }

    pub fn run(self, st: &mut AppState, msgs: &mut Vec<Msg>) {
        match self {
            MenuAction::Send(m) => msgs.push(*m),
            MenuAction::ToggleActivityLog => st.view.show_log = !st.view.show_log,
            MenuAction::ShowFilter => {
                st.view.show_search = true;
                msgs.push(Msg::Search(SearchMsg::ToggleFilter));
            }
        }
    }
}

/// Menu `m`'s entries.
pub fn entries(st: &AppState, m: TopMenu) -> Vec<Entry> {
    use super::menu_model_items as b;
    match m {
        TopMenu::File => b::file(st),
        TopMenu::Edit => b::edit(st),
        TopMenu::View => b::view(st),
        TopMenu::Window => b::window(st),
        TopMenu::Help => b::help(st),
    }
}

/// The five menus, in the bar's order.
pub fn build(st: &AppState) -> [Menu; 5] {
    let t = st.settings.lang();
    TopMenu::ALL.map(|m| Menu { title: m.title(t), entries: entries(st, m) })
}

/// Every item, submenus' included, in the order `take_item` numbers them (on a model nothing
/// was taken from): the order of the system menu bar's tags.
#[cfg(test)]
pub fn items(menus: &[Menu; 5]) -> Vec<&Item> {
    fn walk<'a>(entries: &'a [Entry], out: &mut Vec<&'a Item>) {
        for e in entries {
            match e {
                Entry::Item(it) => out.push(it),
                Entry::Submenu(s) => walk(&s.entries, out),
                Entry::Separator | Entry::Heading(_) | Entry::Taken => {}
            }
        }
    }
    let mut out = Vec::new();
    for m in menus {
        walk(&m.entries, &mut out);
    }
    out
}

/// Takes out the item `index` in `items`' order; None when there is none or it was taken.
pub fn take_item(menus: &mut [Menu; 5], index: usize) -> Option<Item> {
    fn walk(entries: &mut [Entry], left: &mut usize) -> Option<Option<Item>> {
        for e in entries {
            match e {
                Entry::Item(_) | Entry::Taken => {
                    if *left == 0 {
                        return Some(match std::mem::replace(e, Entry::Taken) {
                            Entry::Item(it) => Some(it),
                            _ => None,
                        });
                    }
                    *left -= 1;
                }
                Entry::Submenu(s) => {
                    if let Some(found) = walk(&mut s.entries, left) {
                        return Some(found);
                    }
                }
                Entry::Separator | Entry::Heading(_) => {}
            }
        }
        None
    }
    let mut left = index;
    menus.iter_mut().find_map(|m| walk(&mut m.entries, &mut left)).flatten()
}

#[cfg(test)]
#[path = "menu_model_tests.rs"]
pub(crate) mod tests;
