//! Palettes from theme files: the base theme with the file's colors on top.

use super::color::Palette;
use super::{DARK, builtin};
use crate::themes::ThemeState;
use eframe::egui::Color32;
use trellis_core::settings::{ColorValue, ThemeChoice, ThemeFile};

/// The palette of a loaded theme file: its base, with every color it sets.
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
    p
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
