//! The bundled fonts: Noto Sans for interface text (Regular, and SemiBold as the `semibold`
//! family) and JetBrains Mono for monospace text, each ahead of egui's own fonts, which stay as
//! fallbacks (emoji, symbols). Behind them all come the fonts for Chinese, Japanese and Korean:
//! the system's own, which the shell hands over at startup (`use_cjk`, from
//! `platform::system_fonts`); none until one is handed over. Tests hand over a small subset of
//! Noto Sans CJK instead (`use_test_cjk`, from the snapshot builder, the journeys' harness and
//! the glyph check), so snapshots and the glyph check don't depend on the machine's fonts; the
//! subset is compiled into tests only. This is the one font setup: the app installs it before
//! its first pass, `theme::apply_style` installs it for every themed context, and the glyph
//! check asks these fonts. The files and their licences are in `assets/fonts/`.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::{Arc, OnceLock};

const NOTO_SANS: &[u8] = include_bytes!("../../../../../assets/fonts/NotoSans-Regular.ttf");
const NOTO_SANS_SEMIBOLD: &[u8] = include_bytes!("../../../../../assets/fonts/NotoSans-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../../../../assets/fonts/JetBrainsMono-Regular.ttf");

const REGULAR: &str = "NotoSans-Regular";
const SEMIBOLD_FONT: &str = "NotoSans-SemiBold";
const MONO: &str = "JetBrainsMono-Regular";

/// The tests' CJK font (`use_test_cjk`): the characters of the CJK snapshot and the ja_JP, zh_CN
/// and ko_KR dates, cut from Noto Sans CJK SC (`assets/fonts/test/README.md`). Not in the app.
#[cfg(test)]
const TEST_CJK: &[u8] = include_bytes!("../../../../../assets/fonts/test/NotoSansCJKsc-TestSubset.otf");

/// The CJK fallbacks the shell handed over (`use_cjk`).
static CJK: OnceLock<Vec<(String, FontData)>> = OnceLock::new();

/// Puts `fonts` (name, font) behind the bundled ones for Chinese, Japanese and Korean, in place
/// of the bundled subset: the shell, once, with the system's, before anything asks for the
/// fonts (`definitions` reads them once).
pub fn use_cjk(fonts: Vec<(String, FontData)>) {
    if CJK.set(fonts).is_err() {
        tracing::warn!("the CJK fonts were already chosen");
    }
}

/// Tests: hands over the bundled CJK subset as the CJK fallback, unless it already is. Every
/// test entry point that draws calls it before it installs the fonts.
#[cfg(test)]
pub(crate) fn use_test_cjk() {
    let _ = CJK.set(vec![("NotoSansCJK-TestSubset".to_owned(), FontData::from_static(TEST_CJK))]);
}

/// The name of the semibold family (`FontFamily::Name`).
pub const SEMIBOLD: &str = "semibold";

/// egui's default fonts with the bundled ones in front: Proportional starts with Noto Sans,
/// Monospace with JetBrains Mono (then egui's Hack, then Noto Sans for what neither has), and
/// the semibold family with Noto Sans SemiBold, then the whole Proportional list. egui's lists
/// are taken from its defaults, so an egui upgrade keeps its fallbacks. The CJK fallbacks end
/// both lists: after egui's symbol fonts, so a symbol keeps the glyph it had before.
/// Until CJK fonts are handed over, the definitions have none, and are kept apart from the ones
/// built after, so an early call doesn't fix the fonts without them.
pub fn definitions() -> &'static FontDefinitions {
    static WITHOUT_CJK: OnceLock<FontDefinitions> = OnceLock::new();
    static DEFS: OnceLock<FontDefinitions> = OnceLock::new();
    match CJK.get() {
        Some(cjk) => DEFS.get_or_init(|| build(cjk)),
        None => WITHOUT_CJK.get_or_init(|| build(&[])),
    }
}

/// The definitions with `cjk` (name, font) as the CJK fallbacks.
fn build(cjk: &[(String, FontData)]) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    for (name, data) in [(REGULAR, NOTO_SANS), (SEMIBOLD_FONT, NOTO_SANS_SEMIBOLD), (MONO, JETBRAINS_MONO)] {
        defs.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(data)));
    }
    let egui_list = |defs: &FontDefinitions, f: &FontFamily| defs.families.get(f).cloned().unwrap_or_default();
    let mut proportional = vec![REGULAR.to_owned()];
    proportional.extend(egui_list(&defs, &FontFamily::Proportional));
    let mut monospace = vec![MONO.to_owned()];
    let mut egui_mono = egui_list(&defs, &FontFamily::Monospace).into_iter();
    // egui's monospace font first, then Noto Sans ahead of egui's proportional fallbacks.
    monospace.extend(egui_mono.next());
    monospace.push(REGULAR.to_owned());
    monospace.extend(egui_mono);
    for (name, data) in cjk.iter().cloned() {
        defs.font_data.insert(name.clone(), Arc::new(data));
        proportional.push(name.clone());
        monospace.push(name);
    }
    let mut semibold = vec![SEMIBOLD_FONT.to_owned()];
    semibold.extend(proportional.iter().cloned());
    defs.families.insert(FontFamily::Proportional, proportional);
    defs.families.insert(FontFamily::Monospace, monospace);
    defs.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    defs
}

/// Installs the bundled fonts in `ctx`, from its next pass on (or its first, before any). egui
/// skips the update when they are already in.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions().clone());
}

/// The semibold font at `size`, or the regular one while `ui`'s context doesn't have the
/// bundled fonts yet: egui panics on a family it doesn't know, and fonts installed during a
/// pass arrive only on the next one, so the first frame of a test harness (and any context that
/// never installed them) would otherwise panic.
pub fn semibold(ui: &egui::Ui, size: f32) -> FontId {
    let family = FontFamily::Name(SEMIBOLD.into());
    if ui.fonts(|f| f.families().contains(&family)) { FontId::new(size, family) } else { FontId::proportional(size) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CJK_TEST: &str = "NotoSansCJK-TestSubset";

    #[test]
    fn definitions_put_the_bundled_fonts_first_and_keep_eguis_as_fallback() {
        use_test_cjk();
        let ours = definitions();
        let egui_defaults = FontDefinitions::default();
        let list = |d: &FontDefinitions, f: FontFamily| d.families[&f].clone();
        let (mut prop, mut mono) = (list(ours, FontFamily::Proportional), list(ours, FontFamily::Monospace));
        let semi = list(ours, FontFamily::Name(SEMIBOLD.into()));
        assert_eq!(semi[1..], prop[..], "semibold falls back to the whole Proportional list");
        // The CJK fallback ends both lists; the rest is checked without it.
        for l in [&mut prop, &mut mono] {
            assert_eq!(l.pop().as_deref(), Some(CJK_TEST), "the CJK font comes last: {l:?}");
        }
        assert_eq!(prop[0], REGULAR);
        assert_eq!(mono[0], MONO);
        assert_eq!(semi[0], SEMIBOLD_FONT);
        assert_eq!(prop[1..], list(&egui_defaults, FontFamily::Proportional)[..], "then egui's whole Proportional list");
        for (family, ours) in [(FontFamily::Proportional, &prop), (FontFamily::Monospace, &mono)] {
            let theirs = list(&egui_defaults, family.clone());
            assert!(ours.ends_with(&theirs[1..]), "{family:?} ends with egui's fallbacks: {ours:?}");
            let kept: Vec<&String> = ours.iter().filter(|n| theirs.contains(n)).collect();
            assert_eq!(kept, theirs.iter().collect::<Vec<_>>(), "{family:?} keeps egui's list in order");
        }
        for name in [REGULAR, SEMIBOLD_FONT, MONO, CJK_TEST] {
            assert!(ours.font_data.contains_key(name), "{name} is installed");
        }
    }

    #[test]
    fn bundled_fonts_are_truetype() {
        for (name, data) in [(REGULAR, NOTO_SANS), (SEMIBOLD_FONT, NOTO_SANS_SEMIBOLD), (MONO, JETBRAINS_MONO)] {
            assert_eq!(data[..4], [0, 1, 0, 0], "{name} starts with the TrueType magic");
            assert!(data.len() > 100_000, "{name} is only {} bytes", data.len());
        }
    }

    #[test]
    fn licence_files_match_the_notices() {
        let files = [
            ("Noto Sans", include_str!("../../../../../assets/fonts/NotoSans-OFL.txt")),
            ("JetBrains Mono", include_str!("../../../../../assets/fonts/JetBrainsMono-OFL.txt")),
            ("Noto Sans CJK", include_str!("../../../../../assets/fonts/test/NotoSansCJK-OFL.txt")),
        ];
        for (name, file) in files {
            let notice = glowfly_core::notices::licenses().iter().find(|n| n.name == name).unwrap_or_else(|| panic!("no {name} notice"));
            assert_eq!(file.replace("\r\n", "\n").trim(), notice.text.trim(), "{name}'s licence in THIRD-PARTY-NOTICES.md");
        }
    }

    #[test]
    fn semibold_falls_back_until_the_fonts_are_in() {
        let ctx = egui::Context::default();
        let family = |ctx: &egui::Context| {
            let mut out = None;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let font = semibold(ui, 13.0);
                    let size = ui.fonts(|f| f.layout_no_wrap("Ag".into(), font.clone(), egui::Color32::WHITE).size());
                    assert!(size.x > 0.0 && size.y > 0.0);
                    out = Some(font.family);
                });
            });
            out.expect("the panel ran")
        };
        assert_eq!(family(&ctx), FontFamily::Proportional);
        install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        assert_eq!(family(&ctx), FontFamily::Name(SEMIBOLD.into()));
    }

    #[test]
    fn every_family_draws_cjk_and_hangul_from_the_fallback() {
        use_test_cjk();
        let ctx = egui::Context::default();
        install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let fonts = [FontId::proportional(13.0), FontId::monospace(13.0), FontId::new(13.0, FontFamily::Name(SEMIBOLD.into()))];
        for font in fonts {
            // 日 (Han), あ (kana), 한 (Hangul).
            for c in ['\u{65E5}', '\u{3042}', '\u{D55C}'] {
                assert!(ctx.fonts(|f| f.has_glyph(&font, c)), "U+{:04X} in {font:?}", c as u32);
            }
        }
    }
}
