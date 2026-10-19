//! The active color palette. Call sites read colors through functions (`color::text()`) so the
//! theme can switch at runtime (Preferences > UI Customization > Theme). The palette is a
//! per-thread copy the shell sets with [`set_palette`] at the start of each frame.

use eframe::egui::Color32;
use std::cell::RefCell;
use trellis_core::highlight::TokenKind;

/// The colors of one theme.
///
/// Contrast rules, checked for every built-in by a test (WCAG contrast ratio; "standard" and
/// "high contrast" give the two minimums where they differ):
/// - `text` on `bg`, `panel`, `toolbar`, `input`, `hover`, `selected`, `hunk_bg` and every
///   `diff_*` background: 4.5:1 standard, 7:1 high contrast. `strong` on `selected`: the same.
/// - `weak` on `bg`, `panel`, `toolbar`, `input`, `hover` and `selected`: 4.5:1.
/// - `faint` on `bg`, `panel` and `input`: 4.5:1, and at least 1.0 below `weak`'s ratio on
///   `panel` (which is below `text`'s), so the three steps stay apart.
/// - `input_border` on `bg`, `panel`, `input` and `toolbar`, and `dialog_border` on `panel`:
///   3:1 standard, 4.5:1 high contrast.
/// - `accent` on `bg`, `panel` and `input`: 3:1 (focus rings, and the `input`-coloured mark on
///   an accent-filled checkbox).
/// - `tag_fg` on `tag_bg`: 4.5:1 standard, 7:1 high contrast.
/// - `green`, `red` and `amber` on `bg`, `panel`, `hover`, `selected` and the graph's lane bands:
///   3:1 standard, 4.5:1 high contrast (line counts and status letters).
/// - Every `syn_*` on `panel`, `diff_add`, `diff_del`, `diff_add_word` and `diff_del_word`:
///   2.4:1 standard, 4.5:1 high contrast.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    /// Dark background: picks egui's dark style, dims graph lanes, sets terminal ANSI brightening.
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub toolbar: Color32,
    pub input: Color32,
    pub border: Color32,
    /// The 1 px border of text fields, checkboxes and radio buttons: at least 3:1 on `input`,
    /// `panel`, `bg` and `toolbar`.
    pub input_border: Color32,
    /// The 1 px frame of modal dialogs: at least 3:1 on `panel`.
    pub dialog_border: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub accent: Color32,
    pub text: Color32,
    /// Text on selected rows and filled accents.
    pub strong: Color32,
    pub weak: Color32,
    pub faint: Color32,
    pub green: Color32,
    pub red: Color32,
    pub amber: Color32,
    pub blue: Color32,
    pub purple: Color32,
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
    pub banner: Color32,
    pub primary: Color32,
    pub primary_off: Color32,
    pub primary_border: Color32,
    pub danger: Color32,
    pub danger_off: Color32,
    pub tag_bg: Color32,
    pub tag_fg: Color32,
    /// Syntax colors for diff text.
    pub syn_comment: Color32,
    pub syn_string: Color32,
    pub syn_number: Color32,
    pub syn_constant: Color32,
    pub syn_keyword: Color32,
    pub syn_type: Color32,
    pub syn_function: Color32,
    pub syn_attr: Color32,
}

thread_local! {
    static ACTIVE: RefCell<Palette> = const { RefCell::new(super::builtin::DARK) };
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

/// Whether the active palette has a dark background.
pub fn is_dark() -> bool {
    ACTIVE.with_borrow(|p| p.dark)
}

/// Selects the built-in Dark or Light palette (tests that draw without the shell).
#[cfg(test)]
pub fn set_dark(dark: bool) {
    set_palette(if dark { &super::builtin::DARK } else { &super::builtin::LIGHT });
}

/// Declares an accessor per color and `Palette`'s by-name access, from one list of the fields.
macro_rules! palette_fields {
    ($($name:ident),* $(,)?) => {
        // Every color gets one; a few are only read through a `Palette` (ref chips, controls).
        $(#[allow(dead_code)] pub fn $name() -> Color32 { ACTIVE.with_borrow(|p| p.$name) })*

        impl Palette {
            /// The color names, in field order (the names theme files use).
            pub const KEYS: &'static [&'static str] = &[$(stringify!($name)),*];

            pub fn get(&self, key: &str) -> Option<Color32> {
                match key {
                    $(stringify!($name) => Some(self.$name),)*
                    _ => None,
                }
            }

            pub fn get_mut(&mut self, key: &str) -> Option<&mut Color32> {
                match key {
                    $(stringify!($name) => Some(&mut self.$name),)*
                    _ => None,
                }
            }

            /// Every color as (name, [r, g, b]), in field order.
            pub fn hex_entries(&self) -> Vec<(&'static str, [u8; 3])> {
                Self::KEYS.iter().filter_map(|k| self.get(k).map(|c| (*k, [c.r(), c.g(), c.b()]))).collect()
            }
        }
    };
}

palette_fields!(
    bg,
    panel,
    toolbar,
    input,
    border,
    input_border,
    dialog_border,
    hover,
    selected,
    accent,
    text,
    strong,
    weak,
    faint,
    green,
    red,
    amber,
    blue,
    purple,
    diff_add,
    diff_del,
    diff_add_sel,
    diff_del_sel,
    diff_add_word,
    diff_del_word,
    hunk_bg,
    hunk_fg,
    banner,
    primary,
    primary_off,
    primary_border,
    danger,
    danger_off,
    tag_bg,
    tag_fg,
    syn_comment,
    syn_string,
    syn_number,
    syn_constant,
    syn_keyword,
    syn_type,
    syn_function,
    syn_attr,
);

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
