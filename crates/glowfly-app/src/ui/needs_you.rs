//! The coral "needs you" mark: the one solid-filled status, for moments Glowfly waits on you
//! (a conflicted file, a merge stopped on conflicts, a repository whose trust is undecided, a
//! sign-in). Every coral surface is painted here and carries the warning triangle in the
//! coral's ink, so the colour never stands alone. This is the only module that reads
//! `attention.needs_you`.

use super::theme::color::{self, Palette};
use super::theme::contrast;
use super::widgets::severity::{self, Severity};
use eframe::egui::{Color32, CornerRadius, Painter, Rect, pos2, vec2};

/// How far a hovered pill's fill moves toward white (dark palettes) or black (light ones).
const HOVER_SHADE: f32 = 0.10;
/// The warning mark's size on a pill, and its centre's distance from the pill's left edge.
const PILL_MARK: f32 = 13.0;
const PILL_MARK_X: f32 = 14.0;

/// The coral fill.
pub(crate) fn fill() -> Color32 {
    fill_of(&color::current())
}

/// The coral fill of `p` (the Mica contrast check).
pub(crate) fn fill_of(p: &Palette) -> Color32 {
    p.attention.needs_you
}

/// The ink on the coral fill.
pub(crate) fn ink() -> Color32 {
    color::current().attention.on_needs_you
}

/// A coral rounded square filling `rect` with the warning mark in the ink: a conflicted file's
/// status, the conflicted banner's icon.
pub(crate) fn badge(p: &Painter, rect: Rect) {
    let radius = (rect.height() / 4.0).round().clamp(1.0, 4.0) as u8;
    p.rect_filled(rect, CornerRadius::same(radius), fill());
    let c = rect.center() + vec2(0.0, -rect.height() * 0.03);
    severity::paint(p, c, rect.height() * 0.72, Severity::Warning, ink());
}

/// A coral pill filling `rect` (shaded while `hovered`) with the warning mark near its left
/// edge; the caller paints its text in [`ink`] after the mark.
pub(crate) fn paint_pill(p: &Painter, rect: Rect, hovered: bool) {
    let base = fill();
    let pole = if color::is_dark() { Color32::WHITE } else { Color32::BLACK };
    let fill = if hovered { contrast::mix(base, pole, HOVER_SHADE) } else { base };
    p.rect_filled(rect, CornerRadius::same((rect.height() / 2.0).round() as u8), fill);
    severity::paint(p, pos2(rect.left() + PILL_MARK_X, rect.center().y - 0.5), PILL_MARK, Severity::Warning, ink());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn the_marks_read_the_active_palette() {
        color::set_palette(&LIGHT);
        assert_eq!((fill(), ink()), (LIGHT.attention.needs_you, LIGHT.attention.on_needs_you));
        color::set_palette(&DARK);
        assert_eq!((fill(), ink()), (DARK.attention.needs_you, DARK.attention.on_needs_you));
    }

    #[test]
    fn only_this_module_reads_the_coral() {
        // Every coral fill must come with the warning mark, so nothing else may reach for it.
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![ui];
        let mut found = Vec::new();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    let src = std::fs::read_to_string(&path).unwrap();
                    let test_file = name.contains("tests") || name == "builtin.rs" || name == "builtin_extra.rs";
                    if name != "needs_you.rs" && !test_file && src.contains("attention.needs_you") {
                        found.push(name);
                    }
                }
            }
        }
        assert!(found.is_empty(), "read attention.needs_you outside needs_you.rs: {found:?}");
    }
}
