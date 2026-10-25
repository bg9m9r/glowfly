//! Tests of the snapshot checks themselves: the pixel tolerance, the skip variable, cropping and
//! the text comparison's files.

use super::compare::{check, line_diff};
use super::{TOLERATED_PIXELS, crop_image, skip_requested, within_tolerance};
use eframe::egui::{Rect, pos2};

#[test]
fn tolerance() {
    assert!(within_tolerance(0));
    assert!(within_tolerance(14), "the CI renderer's usual difference");
    assert!(within_tolerance(TOLERATED_PIXELS));
    assert!(!within_tolerance(TOLERATED_PIXELS + 1));
}

#[test]
fn skip_values() {
    for v in ["1", "true", "yes"] {
        assert!(skip_requested(Some(v)), "{v}");
    }
    for v in [None, Some(""), Some("0"), Some("false"), Some(" ")] {
        assert!(!skip_requested(v), "{v:?}");
    }
}

#[test]
fn crops_in_pixels_and_stays_inside() {
    let img = image::RgbaImage::new(200, 100);
    let cut = crop_image(img.clone(), Some(Rect::from_min_max(pos2(10.0, 5.0), pos2(30.0, 20.0))), 2.0);
    assert_eq!(cut.dimensions(), (40, 30));
    let edge = crop_image(img.clone(), Some(Rect::from_min_max(pos2(-10.0, 90.0), pos2(300.0, 300.0))), 1.0);
    assert_eq!(edge.dimensions(), (200, 10));
    assert_eq!(crop_image(img, None, 1.0).dimensions(), (200, 100));
}

#[test]
fn line_diff_marks_removed_and_added_lines() {
    assert_eq!(line_diff("a\nb\nc\n", "a\nx\nc\nd\n"), "- b\n+ x\n+ d\n");
    assert_eq!(line_diff("same\n", "same\n"), "");
}

#[test]
fn text_check_writes_new_and_diff_files() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-text-snap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let err = check(&dir, "x", "one\n", false).unwrap_err();
    assert!(err.contains("Missing text snapshot"), "{err}");
    check(&dir, "x", "one\n", true).unwrap();
    // The approved file may come back from git with CRLF endings.
    std::fs::write(dir.join("x.txt"), "one\r\n").unwrap();
    check(&dir, "x", "one\n", false).unwrap();
    assert!(dir.join("x.new.txt").exists(), "the new text is always written");
    assert!(!dir.join("x.diff.txt").exists());
    let err = check(&dir, "x", "two\n", false).unwrap_err();
    assert!(err.contains("- one") && err.contains("+ two"), "{err}");
    assert_eq!(std::fs::read_to_string(dir.join("x.diff.txt")).unwrap(), "- one\n+ two\n");
    check(&dir, "x", "one\n", false).unwrap();
    assert!(!dir.join("x.diff.txt").exists(), "a match clears the old diff");
    let _ = std::fs::remove_dir_all(&dir);
}
