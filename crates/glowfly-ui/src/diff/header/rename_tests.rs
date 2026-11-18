//! The diff header of a renamed or copied file: the old path first, then an arrow, then the new
//! folder and name; the old path gives way before the folder, keeps its whole accessible name
//! when cut, and the whole "old to new" text is on hover.

use super::tests::{CLOSE_NAME, MORE, commit, harness, themed, unstaged};
use super::*;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::DiffTarget;
use glowfly_model::state::diff_header::{Context, Primary};

const OLD: &str = "src/core/old_app.txt";

/// A staged rename of src/core/old_app.txt to src/ui/app.txt.
fn renamed() -> HeaderModel {
    let path = "src/ui/app.txt";
    HeaderModel {
        target: DiffTarget::Staged(path.into()),
        renamed_from: Some(OLD.into()),
        status: Some('R'),
        context: Context::Staged,
        primary: Some(Primary::Unstage(path.into())),
        ..unstaged()
    }
}

/// The box of the node named `label`, when it shows.
fn at(h: &Harness<'_, Vec<Msg>>, label: &str) -> Option<egui::Rect> {
    let b = h.query_by_label(label)?.bounding_box()?;
    Some(egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32)))
}

fn mid(r: egui::Rect) -> f32 {
    r.center().y
}

#[test]
fn a_rename_names_both_paths_old_first() {
    let h = harness(renamed(), 1400.0);
    let [old, to, folder, name] = [OLD, "to", "src/ui/", "app.txt"].map(|l| at(&h, l).unwrap_or_else(|| panic!("{l} is shown")));
    assert!(old.right() <= to.left() && to.right() <= folder.left() && folder.right() <= name.left(), "{old:?} {to:?} {folder:?} {name:?}");
    let close = at(&h, CLOSE_NAME).unwrap();
    for (label, r) in [(OLD, old), ("to", to), ("src/ui/", folder), ("app.txt", name)] {
        assert!((mid(r) - mid(close)).abs() <= 2.0, "{label} sits on the row: {r:?}, Close {close:?}");
    }
    assert!((to.left() - old.right() - 6.0).abs() < 0.5 && (folder.left() - to.right() - 6.0).abs() < 0.5, "6 px each side of the arrow");
    assert!(at(&h, "R").is_none() && h.query_by_label_contains("R").is_some(), "the status chip still shows the letter");
}

#[test]
fn a_header_without_an_old_path_has_no_arrow() {
    for m in [unstaged(), commit()] {
        let h = harness(m, 1400.0);
        assert!(h.query_by_label("to").is_none() && h.query_by_label(OLD).is_none());
    }
}

#[test]
fn the_old_path_gives_way_before_the_folder() {
    let full = at(&harness(renamed(), 1400.0), OLD).expect("wide").width();
    let name_w = at(&harness(renamed(), 1400.0), "app.txt").unwrap().width();
    let (mut last, mut gone, mut cut_seen) = (full, false, false);
    for width in (260..=1400).rev().step_by(10) {
        let h = harness(renamed(), width as f32);
        let old = at(&h, OLD);
        let arrow = at(&h, "to");
        assert_eq!(old.is_some(), arrow.is_some(), "{width}: the arrow goes with the old path");
        match old {
            Some(old) => {
                assert!(!gone, "{width}: the old path doesn't come back");
                assert!(old.width() <= last + 0.5, "{width}: it only shrinks: {} after {last}", old.width());
                cut_seen |= old.width() + 0.5 < full;
                last = old.width();
                assert!(at(&h, "src/ui/").is_some(), "{width}: the folder is whole while the old path shows");
                assert!((at(&h, "app.txt").unwrap().width() - name_w).abs() < 0.5, "{width}: the name keeps its width");
            }
            None => gone = true,
        }
    }
    assert!(cut_seen, "the old path was cut before it was dropped");
    assert!(gone, "and dropped when narrow");
}

#[test]
fn a_cut_old_path_keeps_its_whole_accessible_name() {
    let full = at(&harness(renamed(), 1400.0), OLD).unwrap().width();
    let cut = (500..1400).rev().step_by(5).find_map(|w| {
        let h = harness(renamed(), w as f32);
        at(&h, OLD).filter(|b| b.width() + 0.5 < full).map(|_| h)
    });
    let h = cut.expect("some width cuts the old path but keeps it");
    assert!(h.query_by_label(OLD).is_some(), "named by the whole path though cut");
    assert!(h.query_by_label("to").is_some());
}

#[test]
fn hovering_shows_the_whole_text() {
    let mut h = harness(renamed(), 1400.0);
    h.get_by_label(OLD).hover();
    h.run();
    assert!(h.query_by_label("src/core/old_app.txt to src/ui/app.txt").is_some(), "the old path's hover");
    let mut h = harness(renamed(), 1400.0);
    h.get_by_label("app.txt").hover();
    h.run();
    assert!(h.query_by_label("src/core/old_app.txt to src/ui/app.txt").is_some(), "the name's hover, for any rename");
}

#[test]
fn a_copy_says_where_it_was_copied_from() {
    let copy = HeaderModel { status: Some('A'), renamed_from: Some("src/core/app.txt".into()), ..renamed() };
    let mut h = harness(copy, 1400.0);
    assert!(h.query_by_label("src/core/app.txt").is_some());
    h.get_by_label("app.txt").hover();
    h.run();
    assert!(h.query_by_label("src/ui/app.txt, copied from src/core/app.txt").is_some());
}

#[test]
fn one_row_at_every_width() {
    for width in [1400.0, 900.0, 700.0, 520.0, 400.0, 360.0] {
        let h = harness(renamed(), width);
        let close = at(&h, CLOSE_NAME).unwrap();
        let more = at(&h, MORE).expect("⋯ never gives way");
        assert!(more.left() >= 0.0 && more.right() <= close.left(), "{width}: {more:?}");
        for label in [OLD, "to", "src/ui/", "app.txt"] {
            if let Some(r) = at(&h, label) {
                assert!((mid(r) - mid(close)).abs() <= 2.0, "{width}: {label} at {}, Close at {}", mid(r), mid(close));
                assert!(r.right() <= more.left() + 0.5, "{width}: {label} stays before ⋯: {r:?}");
            }
        }
    }
}

#[test]
fn file_header_renamed_snapshots() {
    use crate::snapshot_check::CheckSnapshot;
    let mut h = themed(renamed(), 900.0, 48.0);
    assert!(h.query_by_label(OLD).is_some());
    h.check_snapshot("file_header_renamed");
    let mut h = themed(renamed(), 520.0, 48.0);
    assert!(h.query_by_label(MORE).is_some());
    h.check_snapshot("file_header_renamed_narrow");
}
