//! The diff header in the whole window (a working-tree diff with every group labelled, a commit's
//! diff with its file actions and context chip) and the binary body's image preview: a changed
//! PNG before and after in both themes, an added image beside its "Added" placeholder, and an
//! image that can't be decoded. The header's narrow, View menu and binary layouts are drawn
//! alone in `diff/header/tests.rs`.

use super::snapshot_tests::{git, harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_revert::with_commit_diff;
use crate::state::AppState;
use crate::state::diff_preview::DiffPreview;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DATE: &str = "2026-01-11T10:00:00+00:00";
/// Wide enough for every header group with its labels.
const WIDE: egui::Vec2 = egui::vec2(1800.0, 820.0);

fn render(name: &str, st: AppState, dark: bool) -> egui_kittest::Harness<'static, (AppState, Vec<crate::msg::Msg>)> {
    let mut h = harness_sized(st, dark, WIDE);
    h.run();
    h.check_snapshot(name);
    h
}

/// Shows `target`'s diff of the fixture, after re-reading its status.
fn open(st: &mut AppState, dir: &Path, target: DiffTarget) {
    st.snap = Some(repo::load(dir, &LoadOptions::all(500)).unwrap());
    let diff = repo::load_diff(dir, &target).unwrap();
    st.session.diff.view = Some((target, diff.into()));
}

#[test]
fn file_header_wide_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "file-header-wide");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let lines: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    std::fs::write(dir.join("src/app.txt"), &lines).unwrap();
    git(&dir, &["add", "src/app.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Add src/app.txt"], DATE);
    std::fs::write(dir.join("src/app.txt"), lines.replace("line 3\n", "line three\nand more\n").replace("line 25\n", "")).unwrap();
    open(&mut st, &dir, DiffTarget::Unstaged("src/app.txt".into()));
    let h = render("file_header_wide_dark", st, true);
    for label in ["src/", "app.txt", "Unstaged", "Open", super::file_menu::SHOW_IN_FOLDER, "Stage file", "Close diff"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("M +2 \u{2212}2").is_some(), "the status chip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_header_commit_light() {
    let (st, dir) = with_commit_diff("file-header-commit", ThemeChoice::Light, DiffLayout::Inline, false);
    let short = st.session.detail.as_ref().unwrap().oid.to_string()[..7].to_owned();
    // The window is `diff_commit_header_buttons`'s; here, what the header holds.
    let mut h = harness_sized(st, false, WIDE);
    h.run();
    for label in ["Restore\u{2026}", "Open", super::file_menu::SHOW_IN_FOLDER, short.as_str()] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Stage file").is_none(), "no primary on a commit");
    drop(h);
    remove_fixture(&dir);
}

/// A 48 x 32 pattern of red and blue squares, or with `transparent` a 64 x 40 one whose right
/// half fades out.
fn png(transparent: bool) -> Vec<u8> {
    let (w, h) = if transparent { (64, 40) } else { (48, 32) };
    let img = RgbaImage::from_fn(w, h, |x, y| {
        let base = if (x / 8 + y / 8) % 2 == 0 { [220, 60, 60] } else { [60, 90, 220] };
        let alpha = if transparent && x >= w / 2 { (255 * (w - x) / (w / 2)).min(255) as u8 } else { 255 };
        Rgba([base[0], base[1], base[2], alpha])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

/// The fixture with `logo.png` holding `before` committed (left out when `None`) and `after` in
/// the working tree, its diff open with its sides loaded.
fn with_image(name: &str, theme: ThemeChoice, before: Option<&[u8]>, after: &[u8]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    if let Some(bytes) = before {
        std::fs::write(dir.join("logo.png"), bytes).unwrap();
        git(&dir, &["add", "logo.png"], DATE);
        git(&dir, &["commit", "-q", "-m", "Add logo.png"], DATE);
    }
    std::fs::write(dir.join("logo.png"), after).unwrap();
    let target = DiffTarget::Unstaged("logo.png".into());
    open(&mut st, &dir, target.clone());
    let sides = repo::binary_sides(&dir, &target).unwrap();
    st.session.diff_preview = Some(DiffPreview { target, generation: st.session.diff.shown_gen, sides: Some(Ok(Arc::new(sides))) });
    (st, dir)
}

#[test]
fn diff_image_before_after() {
    for (theme, dark, name) in
        [(ThemeChoice::Dark, true, "diff_image_before_after_dark"), (ThemeChoice::Light, false, "diff_image_before_after_light")]
    {
        let (st, dir) = with_image(&format!("diff-image-{dark}"), theme, Some(&png(false)), &png(true));
        let h = render(name, st, dark);
        assert!(h.query_by_label("Before").is_some() && h.query_by_label("After").is_some());
        assert!(h.query_by_label_contains("48 \u{d7} 32 px").is_some() && h.query_by_label_contains("64 \u{d7} 40 px").is_some());
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn diff_image_added() {
    let (st, dir) = with_image("diff-image-added", ThemeChoice::Dark, None, &png(true));
    let h = render("diff_image_added", st, true);
    assert!(h.query_by_label("Added").is_some(), "the missing side's placeholder");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_image_undecodable() {
    let (st, dir) =
        with_image("diff-image-corrupt", ThemeChoice::Dark, Some(b"\x89PNG\r\n\x1a\n\x00\x00one"), b"\x89PNG\r\n\x1a\n\x00\x00and two");
    let h = render("diff_image_undecodable", st, true);
    assert!(h.query_all_by_label("Can't preview this image").count() == 2);
    drop(h);
    remove_fixture(&dir);
}
