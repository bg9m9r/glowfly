//! The SVG preview (#472): a changed icon.svg (a semi-transparent blue circle, then the same
//! circle with an orange rounded square and "Hi" added) as its text diff with the Text / Preview
//! switch, drawn before and after in Porcelain and Graphite, added, with an unreadable After,
//! with a linked file left out, stored binary (no switch), too large, loading, and the German
//! texts and a narrow header for clipping.

use super::super::snapshot_tests::{git, harness, harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_language_check::shown_texts;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::vec2;
use glowfly_core::i18n::Lang;
use glowfly_core::preview_svg::SVG_MAX_BYTES;
use glowfly_core::repo::{self, BinarySide, BinarySides, DiffTarget, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::state::AppState;
use glowfly_model::state::diff_preview::DiffPreview;
use std::path::PathBuf;
use std::sync::Arc;

const D: &str = "2026-01-11T10:00:00+00:00";

const BEFORE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <circle cx="28" cy="30" r="20" fill="#3b82f6" fill-opacity="0.7"/>
</svg>
"##;

const AFTER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <circle cx="28" cy="30" r="20" fill="#3b82f6" fill-opacity="0.7"/>
  <rect x="34" y="34" width="24" height="24" rx="6" fill="#f97316" fill-opacity="0.85"/>
  <text x="8" y="60" font-size="12" fill="#111827">Hi</text>
</svg>
"##;

const BROKEN: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\n  <circle cx=\"28\"\n";

/// The After with a second picture named by path, which is left out.
fn linked() -> String {
    AFTER.replace("  <text", "  <image href=\"badge.png\" x=\"40\" y=\"4\" width=\"20\" height=\"20\"/>\n  <text")
}

/// `icon.svg` committed as `old` (unless `None`), the working file `new`, its diff open with the
/// Text / Preview switch on `preview` and, on Preview, the sides loaded.
fn with_svg(name: &str, theme: ThemeChoice, old: Option<&str>, new: &str, preview: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    if let Some(old) = old {
        std::fs::write(dir.join("icon.svg"), old).unwrap();
        git(&dir, &["add", "icon.svg"], D);
        git(&dir, &["commit", "-q", "-m", "Add icon.svg"], D);
    }
    std::fs::write(dir.join("icon.svg"), new).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("icon.svg".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.session.diff.view = Some((target.clone(), diff.into()));
    st.settings.diff_svg_preview = preview;
    if preview {
        let sides = repo::binary_sides(&dir, &target).unwrap();
        st.session.diff_preview = Some(DiffPreview { target, generation: 0, sides: Some(Ok(Arc::new(sides))) });
    }
    (st, dir)
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
fn diff_svg_text_light() {
    let (st, dir) = with_svg("diff-svg-text", ThemeChoice::Light, Some(BEFORE), AFTER, false);
    render(st, false, "diff_svg_text_light", &["Text", "Preview", "View", "fill=\"#f97316\""]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_preview() {
    let (st, dir) = with_svg("diff-svg-preview", ThemeChoice::Light, Some(BEFORE), AFTER, true);
    let s = glowfly_model::state::diff_preview::shown(&st).unwrap();
    assert!(s.old.iter().chain(&s.new).all(|s| s.image.is_some()), "both sides drawn");
    render(st, false, "diff_svg_preview_light", &["Before", "After", "64 \u{d7} 64 px"]);
    remove_fixture(&dir);
    let (st, dir) = with_svg("diff-svg-preview-dark", ThemeChoice::Dark, Some(BEFORE), AFTER, true);
    render(st, true, "diff_svg_preview_dark", &[]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_added_light() {
    let (st, dir) = with_svg("diff-svg-added", ThemeChoice::Light, None, AFTER, true);
    render(st, false, "diff_svg_added_light", &["Added"]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_failed_light() {
    let (st, dir) = with_svg("diff-svg-failed", ThemeChoice::Light, Some(BEFORE), BROKEN, true);
    render(st, false, "diff_svg_failed_light", &["Can't preview this SVG", "64 \u{d7} 64 px"]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_linked_light() {
    let (st, dir) = with_svg("diff-svg-linked", ThemeChoice::Light, Some(BEFORE), &linked(), true);
    render(st, false, "diff_svg_linked_light", &["Linked files not shown"]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_binary_light() {
    // Stored binary: the preview straight away, no switch and no View menu.
    let (mut st, dir) = with_repo(ThemeChoice::Light, "diff-svg-binary");
    std::fs::write(dir.join(".gitattributes"), "*.svg binary\n").unwrap();
    std::fs::write(dir.join("icon.svg"), BEFORE).unwrap();
    git(&dir, &["add", "."], D);
    git(&dir, &["commit", "-q", "-m", "Add icon.svg"], D);
    std::fs::write(dir.join("icon.svg"), AFTER).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("icon.svg".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    assert!(diff.binary);
    st.session.diff.view = Some((target.clone(), diff.into()));
    let sides = repo::binary_sides(&dir, &target).unwrap();
    st.session.diff_preview = Some(DiffPreview { target, generation: 0, sides: Some(Ok(Arc::new(sides))) });
    let mut h = harness(st, false);
    h.run();
    let texts = shown_texts(&h);
    assert!(texts.iter().any(|t| t.contains("64 \u{d7} 64 px")), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Text" || t == "Preview"), "no switch: {texts:?}");
    h.check_snapshot("diff_svg_binary_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_too_large() {
    let (mut st, dir) = with_svg("diff-svg-too-large", ThemeChoice::Dark, Some(BEFORE), AFTER, true);
    let big = SVG_MAX_BYTES + (1 << 20);
    let p = st.session.diff_preview.as_mut().unwrap();
    let old = p.sides.as_ref().unwrap().as_ref().unwrap().old.clone();
    let new = Some(BinarySide { size: big, image: None, linked: false, bytes: None });
    p.sides = Some(Ok(Arc::new(BinarySides { old, new })));
    render(st, true, "diff_svg_too_large", &["Too large to preview", "5.0 MB \u{b7} over 4.0 MB"]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_loading() {
    let (mut st, dir) = with_svg("diff-svg-loading", ThemeChoice::Dark, Some(BEFORE), AFTER, true);
    st.session.diff_preview.as_mut().unwrap().sides = None;
    render(st, true, "diff_svg_loading", &["Before", "After"]);
    remove_fixture(&dir);
}

#[test]
fn diff_svg_german() {
    let de = Lang::De;
    let (mut st, dir) = with_svg("diff-svg-de", ThemeChoice::Dark, Some(BEFORE), &linked(), true);
    st.settings.language = "de".into();
    render(st, true, "diff_svg_preview_de", &[de.tr("diff.header.svg.preview"), de.tr("diff.svg.linked")]);
    remove_fixture(&dir);
    let (mut st, dir) = with_svg("diff-svg-narrow-de", ThemeChoice::Dark, Some(BEFORE), AFTER, false);
    st.settings.language = "de".into();
    let mut h = harness_sized(st, true, vec2(NARROW, 600.0));
    h.run();
    assert_whole(&h, de.tr("diff.header.svg.preview"), de.tr("diff.header.more"));
    h.check_snapshot("diff_svg_header_narrow_de");
    drop(h);
    remove_fixture(&dir);
}

/// A window whose diff header is short of room: the folder and chips give way, the switch doesn't.
const NARROW: f32 = 1240.0;

/// The switch's `preview` segment ends before ⋯ (named `more`) begins: it isn't clipped.
fn assert_whole<S>(h: &egui_kittest::Harness<'_, S>, preview: &str, more: &str) {
    use egui_kittest::kittest::Queryable;
    let p = h.get_by_label(preview).bounding_box().expect("laid out");
    let m = h.get_by_label(more).bounding_box().expect("laid out");
    assert!(p.x1 <= m.x0, "the switch is whole: {p:?} before {m:?}");
}

#[test]
fn diff_svg_header_narrow_light() {
    let (st, dir) = with_svg("diff-svg-narrow", ThemeChoice::Light, Some(BEFORE), AFTER, false);
    let mut h = harness_sized(st, false, vec2(NARROW, 600.0));
    h.run();
    assert_whole(&h, "Preview", Lang::En.tr("diff.header.more"));
    h.check_snapshot("diff_svg_header_narrow_light");
    drop(h);
    remove_fixture(&dir);
}
