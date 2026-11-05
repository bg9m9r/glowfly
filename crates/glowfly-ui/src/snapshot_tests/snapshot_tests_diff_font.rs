//! The font preview (#472): fonts/brand.ttf changed from Noto Sans Regular to SemiBold (the
//! bundled fonts, so the fixture is the same on every OS) drawn before and after in Porcelain
//! and Graphite, added, with a truncated After, too large, loading, as a two-face collection,
//! and the German texts.

use super::super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::snapshot_tests_language_check::shown_texts;
use crate::snapshot_check::CheckSnapshot;
use glowfly_core::bundled_fonts::{NOTO_SANS, NOTO_SANS_SEMIBOLD};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, BinarySide, BinarySides, DiffTarget, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::state::AppState;
use glowfly_model::state::diff_preview::DiffPreview;
use std::path::PathBuf;
use std::sync::Arc;

const D: &str = "2026-01-11T10:00:00+00:00";
const PATH: &str = "fonts/brand.ttf";

/// `fonts/brand.ttf` committed as `old` (unless `None`), the working file `new`, its diff open
/// and the sides loaded.
fn with_font(name: &str, theme: ThemeChoice, old: Option<&[u8]>, new: &[u8]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    if let Some(old) = old {
        std::fs::write(dir.join(PATH), old).unwrap();
        git(&dir, &["add", PATH], D);
        git(&dir, &["commit", "-q", "-m", "Add brand.ttf"], D);
    }
    std::fs::write(dir.join(PATH), new).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged(PATH.into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    assert!(diff.binary, "a font is binary to git");
    st.session.diff.view = Some((target.clone(), diff.into()));
    let sides = repo::binary_sides(&dir, &target).unwrap();
    st.session.diff_preview = Some(DiffPreview { target, generation: 0, sides: Some(Ok(Arc::new(sides))) });
    (st, dir)
}

/// The loaded sides, changed by `change`.
fn change_sides(st: &mut AppState, change: impl FnOnce(&mut BinarySides)) {
    let p = st.session.diff_preview.as_mut().unwrap();
    let mut sides = BinarySides::clone(p.sides.as_ref().unwrap().as_ref().unwrap());
    change(&mut sides);
    p.sides = Some(Ok(Arc::new(sides)));
}

fn render(st: AppState, dark: bool, snapshot: &str, shows: &[&str]) {
    let mut h = harness(st, dark);
    h.run();
    let texts = shown_texts(&h);
    for text in shows {
        assert!(texts.iter().any(|t| t.contains(text)), "{snapshot} shows {text}: {texts:?}");
    }
    h.check_snapshot(snapshot);
}

#[test]
fn diff_font_changed() {
    let (st, dir) = with_font("diff-font-changed", ThemeChoice::Light, Some(NOTO_SANS), NOTO_SANS_SEMIBOLD);
    let s = glowfly_model::state::diff_preview::shown(&st).unwrap();
    assert!(s.old.iter().chain(&s.new).all(|s| s.image.is_some() && s.font.is_some()), "both sides drawn");
    render(st, false, "diff_font_changed_light", &["Before", "After", "Noto Sans Regular", "Noto Sans SemiBold", "TrueType"]);
    remove_fixture(&dir);
    let (st, dir) = with_font("diff-font-changed-dark", ThemeChoice::Dark, Some(NOTO_SANS), NOTO_SANS_SEMIBOLD);
    render(st, true, "diff_font_changed_dark", &[]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_added_light() {
    let (st, dir) = with_font("diff-font-added", ThemeChoice::Light, None, NOTO_SANS);
    render(st, false, "diff_font_added_light", &["Added", "Noto Sans Regular"]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_unreadable_light() {
    let (st, dir) = with_font("diff-font-unreadable", ThemeChoice::Light, Some(NOTO_SANS), &NOTO_SANS[..1024]);
    render(st, false, "diff_font_unreadable_light", &["Can't preview this font", "1.0 KB"]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_too_large_dark() {
    let (mut st, dir) = with_font("diff-font-too-large", ThemeChoice::Dark, Some(NOTO_SANS), NOTO_SANS_SEMIBOLD);
    change_sides(&mut st, |s| s.new = Some(BinarySide { size: 17 << 20, image: None, linked: false, font: None, bytes: None }));
    render(st, true, "diff_font_too_large_dark", &["Too large to preview", "17.0 MB \u{b7} over 16.0 MB"]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_loading_dark() {
    let (mut st, dir) = with_font("diff-font-loading", ThemeChoice::Dark, Some(NOTO_SANS), NOTO_SANS_SEMIBOLD);
    st.session.diff_preview.as_mut().unwrap().sides = None;
    render(st, true, "diff_font_loading_dark", &["Before", "After"]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_collection_light() {
    let (mut st, dir) = with_font("diff-font-collection", ThemeChoice::Light, Some(NOTO_SANS), NOTO_SANS_SEMIBOLD);
    change_sides(&mut st, |s| {
        if let Some(face) = s.new.as_mut().and_then(|s| s.font.as_mut()) {
            face.faces = 2;
        }
    });
    render(st, false, "diff_font_collection_light", &["2 faces"]);
    remove_fixture(&dir);
}

#[test]
fn diff_font_german() {
    let de = Lang::De;
    let (mut st, dir) = with_font("diff-font-de", ThemeChoice::Dark, Some(NOTO_SANS), &NOTO_SANS[..1024]);
    st.settings.language = "de".into();
    render(st, true, "diff_font_german", &[de.tr("diff.font.cant_preview"), "Noto Sans Regular"]);
    remove_fixture(&dir);
}
