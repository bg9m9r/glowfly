//! The active color palette. Call sites read colors through functions (`color::text()`) so the
//! theme can switch at runtime (Preferences > UI Customization > Theme). The palette is a
//! per-thread copy the shell sets with [`set_palette`] at the start of each frame.

use super::families::{Ansi, Attention, Branch, LANE_COUNT, REPO_HUES, Repository, Status};
use eframe::egui::Color32;
use glowfly_core::highlight::TokenKind;
use glowfly_core::settings::{BranchPalette, RepoColour};
use std::cell::RefCell;

pub use super::surfaces::{field_fill, guide_fill, hover_layer, inset, tool_backing, tool_hover, tool_press, tool_pressed};

/// The colors of one theme, in families: neutrals, brand, status, attention, diff, syntax,
/// repository, branch and the terminal's basic colours. Every built-in meets the contrast floors and hue bands that the theme
/// tests check; see docs/COLOUR.md for what each colour is for and the rules.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    /// Dark background: picks egui's dark style and dims graph lanes.
    pub dark: bool,
    // Neutrals.
    /// The base surface (content behind panels).
    pub bg: Color32,
    pub panel: Color32,
    /// Chrome: the toolbar, status bar and other frames around the content.
    pub toolbar: Color32,
    /// The fill of controls, chips, cards, tooltips and toasts: a step off the panel.
    pub raised: Color32,
    pub input: Color32,
    /// Hairlines between regions.
    pub border: Color32,
    /// The 1 px border of text fields, checkboxes and radio buttons: at least 3:1 on `input`,
    /// `panel`, `bg` and `toolbar`.
    pub input_border: Color32,
    /// The 1 px frame of modal dialogs: at least 3:1 on `panel`.
    pub dialog_border: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub text: Color32,
    /// Text on selected rows and filled accents.
    pub strong: Color32,
    pub weak: Color32,
    pub faint: Color32,
    // Brand.
    pub accent: Color32,
    pub primary: Color32,
    /// Text and icons on a `primary` fill.
    pub on_primary: Color32,
    pub primary_off: Color32,
    pub primary_border: Color32,
    pub danger: Color32,
    pub danger_off: Color32,
    /// Tag chips (sand in Dark and Light).
    pub tag_bg: Color32,
    pub tag_fg: Color32,
    pub status: Status,
    pub attention: Attention,
    // Diff.
    pub diff_add: Color32,
    pub diff_del: Color32,
    /// Selected lines (line-level staging): stronger than `diff_add` / `diff_del`.
    pub diff_add_sel: Color32,
    pub diff_del_sel: Color32,
    /// Changed words inside paired -/+ lines: a step stronger than `diff_*_sel`.
    pub diff_add_word: Color32,
    pub diff_del_word: Color32,
    pub hunk_bg: Color32,
    pub hunk_fg: Color32,
    // Syntax colors for diff text.
    pub syn_comment: Color32,
    pub syn_string: Color32,
    pub syn_number: Color32,
    pub syn_constant: Color32,
    pub syn_keyword: Color32,
    pub syn_type: Color32,
    pub syn_function: Color32,
    pub syn_attr: Color32,
    /// The repository hues (tab swatches, the top line, the repository list, worktree group
    /// strips). Not theme-file keys yet: always the base palette's.
    pub repository: Repository,
    pub branch: Branch,
    /// The branch family the colour-blind safe preset puts in `branch` (`branch_palette::applied`).
    pub colour_blind: Branch,
    /// The theme's own terminal colours (the `terminal_*` keys). The terminal draws in
    /// [`term`], which Preferences > Terminal > Colours can point elsewhere.
    pub ansi: Ansi,
}

/// The palette a thread starts with, before the shell sets one: the standard dark (Dusk), and in
/// tests Garden dark, the palette view snapshots draw in (`snapshot_tests::harness_sized`).
#[cfg(not(test))]
const START: Palette = super::builtin::DARK;
#[cfg(test)]
const START: Palette = super::builtin::GARDEN_DARK;

thread_local! {
    static ACTIVE: RefCell<Palette> = const { RefCell::new(START) };
    /// The terminal colours chosen in Preferences in place of the palette's own, if any.
    static TERMINAL: RefCell<Option<Ansi>> = const { RefCell::new(None) };
}

/// Draws the terminal in `t` rather than the palette's own colours (`None`: the palette's), on
/// this thread (the shell, once per frame).
pub fn set_terminal(t: Option<Ansi>) {
    TERMINAL.with_borrow_mut(|a| {
        if *a != t {
            *a = t;
        }
    });
}

/// The colours the terminal draws in: the ones Preferences chose, else the palette's own.
pub fn term() -> Ansi {
    TERMINAL.with_borrow(|t| *t).unwrap_or_else(|| ACTIVE.with_borrow(|p| p.ansi))
}

/// Makes `p` this thread's palette (the shell, once per frame).
pub fn set_palette(p: &Palette) {
    ACTIVE.with_borrow_mut(|a| {
        if a != p {
            *a = *p;
        }
    });
}

/// A copy of this thread's palette.
pub fn current() -> Palette {
    ACTIVE.with_borrow(|p| *p)
}

/// `f` of this thread's palette, without copying it (`surfaces.rs`'s accessors).
pub(super) fn with<R>(f: impl FnOnce(&Palette) -> R) -> R {
    ACTIVE.with_borrow(f)
}

/// Whether the active palette has a dark background.
pub fn is_dark() -> bool {
    ACTIVE.with_borrow(|p| p.dark)
}

/// Selects the Garden dark or light palette, the one view snapshots draw in (tests that draw
/// without the shell).
#[cfg(test)]
pub fn set_dark(dark: bool) {
    set_palette(if dark { &super::builtin::GARDEN_DARK } else { &super::builtin::GARDEN_LIGHT });
}

/// Declares an accessor per color and `Palette`'s by-name access, from one list of
/// `key => field path` pairs (in theme-file key order).
macro_rules! palette_fields {
    ($($name:ident => $($field:ident).+ $([$idx:literal])?),* $(,)?) => {
        // Every color gets one; a few are only read through a `Palette` (ref chips, controls).
        $(#[allow(dead_code)] pub fn $name() -> Color32 { ACTIVE.with_borrow(|p| p.$($field).+ $([$idx])?) })*

        impl Palette {
            /// The color names, in theme-file order.
            pub const KEYS: &'static [&'static str] = &[$(stringify!($name)),*];

            pub fn get(&self, key: &str) -> Option<Color32> {
                match key {
                    $(stringify!($name) => Some(self.$($field).+ $([$idx])?),)*
                    _ => None,
                }
            }

            pub fn get_mut(&mut self, key: &str) -> Option<&mut Color32> {
                match key {
                    $(stringify!($name) => Some(&mut self.$($field).+ $([$idx])?),)*
                    _ => None,
                }
            }

            /// Every color as (name, [r, g, b]), in theme-file order.
            pub fn hex_entries(&self) -> Vec<(&'static str, [u8; 3])> {
                Self::KEYS.iter().filter_map(|k| self.get(k).map(|c| (*k, [c.r(), c.g(), c.b()]))).collect()
            }
        }
    };
}

palette_fields!(
    bg => bg,
    panel => panel,
    toolbar => toolbar,
    raised => raised,
    input => input,
    border => border,
    input_border => input_border,
    dialog_border => dialog_border,
    hover => hover,
    selected => selected,
    accent => accent,
    text => text,
    strong => strong,
    weak => weak,
    faint => faint,
    added => status.added,
    removed => status.removed,
    modified => status.modified,
    info => status.info,
    progress => status.progress,
    diff_add => diff_add,
    diff_del => diff_del,
    diff_add_sel => diff_add_sel,
    diff_del_sel => diff_del_sel,
    diff_add_word => diff_add_word,
    diff_del_word => diff_del_word,
    hunk_bg => hunk_bg,
    hunk_fg => hunk_fg,
    warn => attention.warn,
    banner => attention.banner,
    primary => primary,
    on_primary => on_primary,
    primary_off => primary_off,
    primary_border => primary_border,
    danger => danger,
    danger_off => danger_off,
    tag_bg => tag_bg,
    tag_fg => tag_fg,
    syn_comment => syn_comment,
    syn_string => syn_string,
    syn_number => syn_number,
    syn_constant => syn_constant,
    syn_keyword => syn_keyword,
    syn_type => syn_type,
    syn_function => syn_function,
    syn_attr => syn_attr,
    terminal_background => ansi.background,
    terminal_foreground => ansi.foreground,
    terminal_cursor => ansi.cursor,
    terminal_selection => ansi.selection,
    terminal_black => ansi.colors[0],
    terminal_red => ansi.colors[1],
    terminal_green => ansi.colors[2],
    terminal_yellow => ansi.colors[3],
    terminal_blue => ansi.colors[4],
    terminal_magenta => ansi.colors[5],
    terminal_cyan => ansi.colors[6],
    terminal_white => ansi.colors[7],
    terminal_bright_black => ansi.colors[8],
    terminal_bright_red => ansi.colors[9],
    terminal_bright_green => ansi.colors[10],
    terminal_bright_yellow => ansi.colors[11],
    terminal_bright_blue => ansi.colors[12],
    terminal_bright_magenta => ansi.colors[13],
    terminal_bright_cyan => ansi.colors[14],
    terminal_bright_white => ansi.colors[15],
);

/// The active palette's branch preset (how many hues branches and people pick from).
pub fn branch_preset() -> BranchPalette {
    ACTIVE.with_borrow(|p| p.branch.preset)
}

/// Graph lane `i`'s colour in the active palette (they repeat after `LANE_COUNT`).
pub fn lane(i: usize) -> Color32 {
    ACTIVE.with_borrow(|p| p.branch.lanes[i % LANE_COUNT])
}

/// The terminal's basic colour `i` (0-15, repeating after) in the colours it draws in ([`term`]).
pub fn ansi(i: u8) -> Color32 {
    term().colors[usize::from(i % 16)]
}

/// The order worktree tab groups take the repository hues in: each hue is far from the next
/// (Rose, Teal, Iris, Ochre, Sky, Plum, Fern, Slate), so neighbouring groups differ strongly.
const GROUP_ORDER: [usize; REPO_HUES] = [0, 3, 6, 1, 4, 7, 2, 5];

/// The strip colour of the `n`th worktree tab group in the active palette.
pub fn repo_group(n: usize) -> Color32 {
    ACTIVE.with_borrow(|p| p.repository.hues[GROUP_ORDER[n % REPO_HUES]])
}

/// The colour given to a repository, in the active palette.
pub fn repo_hue(c: RepoColour) -> Color32 {
    ACTIVE.with_borrow(|p| p.repository.hues[c.index()])
}

/// The color for a syntax token kind in the active palette. `Plain` is `text`, though
/// callers draw plain runs in the row's own text color.
pub fn token(k: TokenKind) -> Color32 {
    match k {
        TokenKind::Plain => text(),
        TokenKind::Comment => syn_comment(),
        TokenKind::String => syn_string(),
        TokenKind::Number => syn_number(),
        TokenKind::Constant => syn_constant(),
        TokenKind::Keyword => syn_keyword(),
        TokenKind::Type => syn_type(),
        TokenKind::Function => syn_function(),
        TokenKind::Attribute => syn_attr(),
    }
}
