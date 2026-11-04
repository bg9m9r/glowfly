//! The largest texture side egui is told it may use. egui makes its font atlas as wide as that
//! side (16384 with glow on many cards) and doubles the atlas's height as glyphs are added, so a
//! wide atlas grows in steps of megabytes, each also copied for the upload. Capped, the atlas
//! is narrower and its doublings come in smaller steps: never larger, often half the size
//! (tests below). The other textures stay under the cap: image
//! previews at most `preview_image::FILE_VIEW_SHOWN_MAX` a side, avatars and the app mark far less.

use eframe::egui::RawInput;

/// The cap: room for the largest image preview, and a font atlas a quarter as wide as 16384.
pub(super) const MAX_TEXTURE_SIDE: usize = 4096;

/// Caps the frame's `max_texture_side` (`GlowflyApp`'s `raw_input_hook`). A side the backend
/// didn't report stays unreported: egui then uses its own small default.
pub(super) fn cap(raw_input: &mut RawInput) {
    if let Some(side) = &mut raw_input.max_texture_side {
        *side = (*side).min(MAX_TEXTURE_SIDE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use glowfly_core::preview_image::FILE_VIEW_SHOWN_MAX;

    /// The font atlas's size after each step of a growing load of text drawn with the app's
    /// fonts (ASCII and Latin-1 at one more size per step), when the backend reports 16384 and
    /// the shell caps it as the app does when `capped`.
    fn atlas_growth(capped: bool) -> Vec<[usize; 2]> {
        let ctx = egui::Context::default();
        glowfly_ui::theme::fonts::use_test_cjk();
        glowfly_ui::theme::fonts::install(&ctx);
        let input = || {
            let mut raw = egui::RawInput { max_texture_side: Some(16384), ..Default::default() };
            if capped {
                cap(&mut raw);
            }
            raw
        };
        // The fonts are installed by the first pass.
        let _ = ctx.run(input(), |_| {});
        let text: String = (' '..='~').chain('\u{a0}'..='\u{ff}').collect();
        let mut sizes = Vec::new();
        for step in 0..16 {
            let _ = ctx.run(input(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for size in (0..=step).map(|s| 12.0 + s as f32) {
                        ui.label(egui::RichText::new(&text).size(size));
                        ui.label(egui::RichText::new(&text).size(size).monospace());
                    }
                });
            });
            sizes.push(ctx.fonts(|f| f.font_image_size()));
        }
        sizes
    }

    /// epaint keeps the atlas as f32 coverage: 4 bytes a pixel.
    fn bytes([w, h]: [usize; 2]) -> usize {
        w * h * 4
    }

    #[test]
    fn a_reported_side_is_capped_and_a_missing_one_left_alone() {
        for (reported, after) in [(Some(16384), Some(4096)), (Some(4096), Some(4096)), (Some(2048), Some(2048)), (None, None)] {
            let mut raw = RawInput { max_texture_side: reported, ..Default::default() };
            cap(&mut raw);
            assert_eq!(raw.max_texture_side, after, "{reported:?}");
        }
    }

    #[test]
    fn the_largest_image_preview_fits_under_the_cap() {
        assert!(FILE_VIEW_SHOWN_MAX as usize <= MAX_TEXTURE_SIDE);
    }

    /// Both atlases double, but the narrow one doubles from a quarter of the width, so its
    /// steps land between the wide one's: it is never larger, and half as large at most loads.
    #[test]
    fn the_capped_font_atlas_is_never_larger_and_often_half() {
        let wide = atlas_growth(false);
        let capped = atlas_growth(true);
        for (step, (w, c)) in wide.iter().zip(&capped).enumerate() {
            eprintln!("step {step:2}: uncapped {w:?} = {} B, capped {c:?} = {} B", bytes(*w), bytes(*c));
            assert!(bytes(*c) <= bytes(*w), "step {step}: capped {c:?} vs uncapped {w:?}");
        }
        assert_eq!(wide[0][0], 16384, "the uncapped atlas is as wide as the reported side");
        assert_eq!(capped[0][0], MAX_TEXTURE_SIDE);
        assert!(bytes(capped[0]) * 2 <= bytes(wide[0]), "a light load: {:?} capped vs {:?}", capped[0], wide[0]);
        let halved = wide.iter().zip(&capped).filter(|&(w, c)| bytes(*c) * 2 <= bytes(*w)).count();
        assert!(halved * 2 > wide.len(), "half as large at only {halved} of {} loads", wide.len());
    }
}
