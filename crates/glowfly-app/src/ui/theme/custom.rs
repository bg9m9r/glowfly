//! Palettes from theme files: the base theme with the file's colors on top.

use super::builtin::garden_twin;
use super::color::Palette;
use super::contrast::ratio;
use super::{DARK, builtin};
use crate::themes::ThemeState;
use eframe::egui::Color32;
use glowfly_core::settings::{ColorValue, ThemeChoice, ThemeFile};

/// The contrast the primary button's text needs.
const PRIMARY_INK_MIN: f64 = 4.5;

/// The palette of a loaded theme file: its base, with every color it sets. A file that sets its
/// own `primary` but no `on_primary` keeps the base's ink only while it reads on that primary;
/// otherwise the button text is white or black, whichever reads better. A file on Dusk or Dawn
/// whose accent isn't the base's (one written from the green Dark or Light before Dusk and Dawn,
/// say) takes the Garden branch families of its kind: Dusk and Dawn's make room for their gold,
/// and only the Garden ones keep out of the way of a green accent.
pub fn build(file: &ThemeFile) -> Palette {
    let base = *builtin(file.base);
    let mut p = base;
    for (key, value) in &file.colors {
        let color = match value {
            ColorValue::Rgb([r, g, b]) => Some(Color32::from_rgb(*r, *g, *b)),
            ColorValue::Base(k) => base.get(k),
        };
        if let (Some(slot), Some(c)) = (p.get_mut(key), color) {
            *slot = c;
        }
    }
    if let Some(g) = garden_twin(file.base).filter(|_| p.accent != base.accent) {
        (p.branch, p.colour_blind) = (g.branch, g.colour_blind);
    }
    if file.colors.contains_key("primary") && !file.colors.contains_key("on_primary") {
        p.on_primary = primary_ink(base.on_primary, p.primary);
    }
    p
}

/// `ink` if it reads on `primary`, else white or black, whichever reads better.
fn primary_ink(ink: Color32, primary: Color32) -> Color32 {
    if ratio(ink, primary) >= PRIMARY_INK_MIN {
        return ink;
    }
    if ratio(Color32::WHITE, primary) >= ratio(Color32::BLACK, primary) { Color32::WHITE } else { Color32::BLACK }
}

/// The palette `choice` shows: a built-in, a loaded theme file, or Dark for a file that isn't
/// in the folder.
pub fn resolve(choice: &ThemeChoice, themes: &ThemeState, system_dark: bool) -> Palette {
    match choice {
        ThemeChoice::Custom(id) => themes.catalog.get(id).map_or(DARK, build),
        other => other.builtin(system_dark).map_or(DARK, |t| *builtin(t)),
    }
}

/// Every color of `p`, for a new theme file made from it.
pub fn starter(p: &Palette) -> Vec<(&'static str, [u8; 3])> {
    p.hex_entries()
}
