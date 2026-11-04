//! The hex view of a small binary file (#471): a 300-byte file grown to 316 bytes with two bytes
//! changed at 0x12, inline (16 bytes a row, a fold between the changes) and split (8 a side),
//! in Graphite, Porcelain and the colour-safe palette; an added file; a file over the cap,
//! which keeps the size-only page and says why; and the German caption, fold and reason for
//! clipping.

use super::super::empty_state::BINARY_TITLE;
use super::super::snapshot_tests::{git, harness, harness_palette, remove_fixture, with_repo};
use super::snapshot_tests_language_check::shown_texts;
use super::theme;
use crate::snapshot_check::CheckSnapshot;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, DiffTarget, HEX_MAX_BYTES, LoadOptions};
use glowfly_core::settings::{ColourPalette, DiffLayout, ThemeChoice};
use glowfly_model::state::AppState;
use glowfly_model::state::diff_preview::DiffPreview;
use std::path::PathBuf;
use std::sync::Arc;

const D: &str = "2026-01-11T10:00:00+00:00";

/// 300 bytes: a counting pattern with a readable name at 0x40.
fn old_bytes() -> Vec<u8> {
    let mut b: Vec<u8> = (0..300u32).map(|i| (i * 7 % 256) as u8).collect();
    let name = b"Glowfly sample.v1";
    b[0x40..0x40 + name.len()].copy_from_slice(name);
    b
}

/// The old bytes with 0x12 and 0x13 changed and 16 bytes appended.
fn new_bytes() -> Vec<u8> {
    let mut b = old_bytes();
    b[0x12] = 0xff;
    b[0x13] = b'Z';
    b.extend_from_slice(b"\x00\x01tail: grown!\x7f\x80");
    b
}

/// `data.bin` committed with `old` (unless `None`), the working file `new`, its diff open with
/// the sides loaded.
fn with_hex(name: &str, theme: ThemeChoice, old: Option<&[u8]>, new: &[u8]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    if let Some(old) = old {
        std::fs::write(dir.join("data.bin"), old).unwrap();
        git(&dir, &["add", "data.bin"], D);
        git(&dir, &["commit", "-q", "-m", "Add data.bin"], D);
    }
    std::fs::write(dir.join("data.bin"), new).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("data.bin".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    assert!(diff.binary);
    st.session.diff.view = Some((target.clone(), diff.into()));
    let sides = repo::binary_sides(&dir, &target).unwrap();
    st.session.diff_preview = Some(DiffPreview { target, generation: 0, sides: Some(Ok(Arc::new(sides))) });
    (st, dir)
}

fn changed(name: &str, theme: ThemeChoice, layout: DiffLayout) -> (AppState, PathBuf) {
    let (mut st, dir) = with_hex(name, theme, Some(&old_bytes()), &new_bytes());
    st.settings.diff_layout = layout;
    (st, dir)
}

fn render(st: AppState, dark: bool, snapshot: &str, shows: &[&str]) {
    let mut h = harness(st, dark);
    h.run();
    for text in shows {
        assert!(shown_texts(&h).iter().any(|t| t.contains(text)), "{snapshot} shows {text}");
    }
    h.check_snapshot(snapshot);
}

#[test]
fn diff_hex_inline() {
    let (st, dir) = changed("diff-hex-inline", ThemeChoice::Dark, DiffLayout::Inline);
    let caption = "300 bytes before, 316 bytes after \u{b7} 18 bytes differ";
    render(st, true, "diff_hex_inline_dark", &[caption, "12 unchanged rows", "ff", "il: grown!"]);
    remove_fixture(&dir);
    let (st, dir) = changed("diff-hex-inline-light", ThemeChoice::Light, DiffLayout::Inline);
    render(st, false, "diff_hex_inline_light", &[caption]);
    remove_fixture(&dir);
}

#[test]
fn diff_hex_split() {
    let (st, dir) = changed("diff-hex-split", ThemeChoice::Dark, DiffLayout::Split);
    render(st, true, "diff_hex_split_dark", &["unchanged rows"]);
    remove_fixture(&dir);
    let (st, dir) = changed("diff-hex-split-light", ThemeChoice::Light, DiffLayout::Split);
    render(st, false, "diff_hex_split_light", &["unchanged rows"]);
    remove_fixture(&dir);
}

#[test]
fn diff_hex_inline_colour_safe_dark() {
    let (mut st, dir) = changed("diff-hex-colour-safe", ThemeChoice::Dark, DiffLayout::Inline);
    st.settings.colour_palette = ColourPalette::ColourBlind;
    let mut h = harness_palette(st, theme::applied(theme::GRAPHITE, ColourPalette::ColourBlind));
    h.run();
    h.check_snapshot("diff_hex_inline_colour_safe_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_hex_added_dark() {
    let (st, dir) = with_hex("diff-hex-added", ThemeChoice::Dark, None, &old_bytes()[..40]);
    render(st, true, "diff_hex_added_dark", &["Added, 40 bytes", "0020"]);
    remove_fixture(&dir);
}

/// A 70 KiB file on its new side, over the cap: neither side shows its bytes.
fn too_large(name: &str) -> (AppState, PathBuf) {
    let mut big = vec![0u8; 70 << 10];
    assert!(big.len() as u64 > HEX_MAX_BYTES);
    big[100] = 1;
    with_hex(name, ThemeChoice::Dark, Some(&[0u8; 64]), &big)
}

#[test]
fn diff_hex_too_large_dark() {
    let (st, dir) = too_large("diff-hex-too-large");
    render(st, true, "diff_hex_too_large_dark", &[BINARY_TITLE, "Too large for the byte view (over 64 KB)."]);
    remove_fixture(&dir);
}

#[test]
fn diff_hex_german() {
    let de = Lang::De;
    let (mut st, dir) = changed("diff-hex-de", ThemeChoice::Dark, DiffLayout::Inline);
    st.settings.language = "de".into();
    let fold = de.trn("diff.hex.fold", 12, &[("count", &12)]);
    render(st, true, "diff_hex_inline_de", &[&fold, "unterscheiden sich"]);
    remove_fixture(&dir);
    let (mut st, dir) = too_large("diff-hex-too-large-de");
    st.settings.language = "de".into();
    render(st, true, "diff_hex_too_large_de", &[de.tr("diff.hex.too_large")]);
    remove_fixture(&dir);
}
