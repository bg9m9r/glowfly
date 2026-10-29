//! Debug builds: a warning when a frame draws a character that no loaded font has, so it shows
//! as a box. The glyph check covers the app's own text; this catches what only arrives at run
//! time (server messages, theme names, hook output, file names). Run after each frame's
//! drawing: it reads the frame's text shapes, skips ASCII and every character it has already
//! asked about in that font family, and warns once per character and family.

use eframe::egui::{self, FontFamily, FontId, LayerId, Shape};
use std::collections::HashSet;

/// The characters already asked about in each family, kept in the context's memory.
#[derive(Clone, Default)]
struct Asked(HashSet<(char, FontFamily)>);

/// Warns about the characters this frame draws without a glyph and returns them (for tests).
/// Only the first time each one is drawn in a family.
pub fn check(ctx: &egui::Context) -> Vec<char> {
    if !cfg!(debug_assertions) {
        return Vec::new();
    }
    let id = egui::Id::new("missing_glyphs");
    // Taken out and put back, not cloned: the set grows with every character drawn.
    let mut asked = ctx.data_mut(|d| d.remove_temp::<Asked>(id)).unwrap_or_default();
    let mut layers: Vec<LayerId> = ctx.memory(|m| m.layer_ids().collect());
    if !layers.contains(&LayerId::background()) {
        layers.push(LayerId::background());
    }
    let mut new: Vec<(char, FontId)> = Vec::new();
    ctx.graphics(|g| {
        for layer in &layers {
            for clipped in g.get(*layer).into_iter().flat_map(|list| list.all_entries()) {
                collect(&clipped.shape, &asked, &mut new);
            }
        }
    });
    let mut missing = Vec::new();
    for (c, font) in new {
        if !asked.0.insert((c, font.family.clone())) {
            continue;
        }
        if !ctx.fonts(|f| f.has_glyph(&font, c)) {
            tracing::warn!("U+{:04X} {c:?} has no glyph in the {} fonts, so it draws as a box", c as u32, font.family);
            missing.push(c);
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, asked));
    missing
}

/// The non-ASCII characters of the text in `shape` not yet asked about, with their fonts.
fn collect(shape: &Shape, asked: &Asked, out: &mut Vec<(char, FontId)>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, asked, out)),
        Shape::Text(text) => {
            let job = &text.galley.job;
            for section in &job.sections {
                let font = &section.format.font_id;
                for c in job.text.get(section.byte_range.clone()).unwrap_or("").chars() {
                    if !c.is_ascii()
                        && !asked.0.contains(&(c, font.family.clone()))
                        && !out.iter().any(|(o, f)| *o == c && f.family == font.family)
                    {
                        out.push((c, font.clone()));
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs one frame drawing `text` in `ctx` (with the app's fonts), returning `check`'s answer.
    fn frame(ctx: &egui::Context, text: &str) -> Vec<char> {
        let mut out = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(text);
                ui.monospace(text);
            });
            out = check(ctx);
        });
        out
    }

    #[test]
    fn warns_once_per_character_without_a_glyph() {
        let ctx = egui::Context::default();
        super::super::fonts::install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        // Thai ก and Devanagari क are in no loaded font; é, … and 日 are.
        let text = "caf\u{e9}\u{2026} \u{65E5} \u{0E01}\u{0915}\u{0E01}";
        assert_eq!(frame(&ctx, text), ['\u{0E01}', '\u{0915}', '\u{0E01}', '\u{0915}'], "once in each family");
        assert!(frame(&ctx, text).is_empty(), "not again on the next frame");
        assert_eq!(frame(&ctx, "\u{0E02}"), ['\u{0E02}', '\u{0E02}'], "a new one still is");
    }
}
