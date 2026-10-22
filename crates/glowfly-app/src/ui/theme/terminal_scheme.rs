//! The colours the terminal draws in (Preferences > Terminal > Colours): the app theme's own,
//! another built-in theme's, or an imported scheme laid over the app theme's.

use super::builtin;
use super::color::Palette;
use super::families::Ansi;
use eframe::egui::Color32;
use glowfly_core::settings::terminal_schemes::{SchemeCatalog, SchemeFile};
use glowfly_core::settings::{BuiltinTheme, TerminalColours};

/// The terminal colours `choice` picks, with `palette` (the app theme in use) for "Match app
/// theme", a missing scheme or built-in, and the keys a scheme leaves out.
pub fn resolve(choice: &TerminalColours, palette: &Palette, schemes: &SchemeCatalog) -> Ansi {
    match choice {
        TerminalColours::MatchTheme => palette.ansi,
        TerminalColours::Theme(id) => BuiltinTheme::from_id(id).map_or(palette.ansi, |t| builtin(t).ansi),
        TerminalColours::Scheme(id) => schemes.get(id).map_or(palette.ansi, |f| over(palette, f)),
    }
}

/// `file`'s colours over `palette`'s terminal colours.
fn over(palette: &Palette, file: &SchemeFile) -> Ansi {
    let mut p = *palette;
    for (key, [r, g, b]) in &file.colors {
        if let Some(slot) = p.get_mut(key) {
            *slot = Color32::from_rgb(*r, *g, *b);
        }
    }
    p.ansi
}

/// The override for `color::set_terminal`: None when the terminal matches the app theme.
pub fn override_for(choice: &TerminalColours, palette: &Palette, schemes: &SchemeCatalog) -> Option<Ansi> {
    let t = resolve(choice, palette, schemes);
    (t != palette.ansi).then_some(t)
}
