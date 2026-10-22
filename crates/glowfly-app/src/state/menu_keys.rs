//! The menus from the keyboard: the five top-level menus in order, each one's mnemonic letter
//! (Alt+letter opens it) derived from its translated title, and what the views keep between
//! frames while a menu is used from the keyboard (`MenuKeys`). `ui/menu_bar` draws the menus
//! and takes the presses. No egui types here.

use crate::keymap::{BindingId, KeyName, Keymap, Mods};
use crate::state::custom_commands::CommandKey;

/// A top-level menu, in the order the bar shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopMenu {
    File,
    Edit,
    View,
    Window,
    Help,
}

impl TopMenu {
    pub const ALL: [TopMenu; 5] = [TopMenu::File, TopMenu::Edit, TopMenu::View, TopMenu::Window, TopMenu::Help];

    /// The catalog key of its title.
    pub fn title_key(self) -> &'static str {
        match self {
            TopMenu::File => "menu.file.title",
            TopMenu::Edit => "menu.edit.title",
            TopMenu::View => "menu.view.title",
            TopMenu::Window => "menu.window.title",
            TopMenu::Help => "menu.help.title",
        }
    }

    /// The menu `d` places further along the bar, wrapping at either end.
    pub fn step(self, d: isize) -> TopMenu {
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0) as isize;
        Self::ALL[(i + d).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

/// A menu's mnemonic: the `index`th char of its title, `letter` in lowercase ASCII.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mnemonic {
    pub index: usize,
    pub letter: char,
}

/// Each title's mnemonic, in order: its first char when that is a free ASCII letter, else its
/// next one that is, skipping `reserved` letters (lowercase) and those earlier menus took,
/// ignoring case. A title without a free ASCII letter gets none.
pub fn mnemonics(titles: [&str; 5], reserved: &[char]) -> [Option<Mnemonic>; 5] {
    let mut taken: Vec<char> = reserved.iter().map(char::to_ascii_lowercase).collect();
    titles.map(|title| {
        let m = title.chars().enumerate().find_map(|(index, c)| {
            let letter = c.to_ascii_lowercase();
            (c.is_ascii_alphabetic() && !taken.contains(&letter)).then_some(Mnemonic { index, letter })
        });
        if let Some(m) = m {
            taken.push(m.letter);
        }
        m
    })
}

/// The letters a binding or custom command uses as a plain Alt+letter key (lowercase): a menu
/// can't have those as its mnemonic, since the key does that action.
pub fn reserved(keys: &Keymap, custom: &[CommandKey]) -> Vec<char> {
    let alt = Mods { alt: true, ..Default::default() };
    let bound = BindingId::ALL.iter().flat_map(|&id| keys.chords(id).iter().copied());
    let mut out: Vec<char> = bound
        .chain(custom.iter().map(|(chord, _, _)| *chord))
        .filter(|c| c.mods == alt)
        .filter_map(|c| match c.key {
            KeyName::Letter(l) => Some(l.to_ascii_lowercase()),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The menu whose mnemonic is `letter` (lowercase).
pub fn menu_for(letter: char, marks: &[Option<Mnemonic>; 5]) -> Option<TopMenu> {
    marks.iter().position(|m| m.is_some_and(|m| m.letter == letter)).map(|i| TopMenu::ALL[i])
}

/// The menus' keyboard state across frames (`ViewState::menu_keys`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuKeys {
    /// A mnemonic pressed this frame: set by the shortcut input, taken by the view that draws
    /// the menus.
    pub request: Option<TopMenu>,
    /// The open menu came from the keyboard: the letters stay underlined.
    pub keyboard: bool,
    /// Focus the first item of the menu drawn next.
    pub focus_first: bool,
    /// The title bar's menu shows this menu alone (under a row of the five titles).
    pub only: Option<TopMenu>,
    /// Focus this row of the title bar's five-row list next.
    pub back_to: Option<TopMenu>,
}

impl MenuKeys {
    /// Menu `m` was opened with its mnemonic.
    pub fn opened_by_key(&mut self, m: TopMenu) {
        self.keyboard = true;
        self.focus_first = true;
        self.only = Some(m);
    }

    /// Left or Right (`d` = -1 or 1) in the title bar's single menu: the neighbouring menu.
    pub fn stepped(&mut self, d: isize) {
        self.only = self.only.map(|m| m.step(d));
        self.focus_first = true;
    }

    /// No menu is open any more.
    pub fn closed(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
#[path = "menu_keys_tests.rs"]
mod tests;
