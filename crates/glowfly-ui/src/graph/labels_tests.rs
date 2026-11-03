//! The chip row's fitting, chip widths (with their marks and the halo's room) and hit testing.

use super::*;

/// "+n" chips 30 px wide.
fn plus(_: usize) -> f32 {
    30.0
}

#[test]
fn every_chip_fits() {
    let f = fit(&[50.0, 40.0, 30.0], 200.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 3, first_w: 50.0, first_cut: false, plus: false });
}

#[test]
fn a_chip_that_doesnt_fit_hides_the_rest() {
    // 50 + 4 + 40 = 94; the third (60) would reach 158 and leave no room for "+2".
    let f = fit(&[50.0, 40.0, 60.0, 20.0], 150.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 2, first_w: 50.0, first_cut: false, plus: true });
    // The last chip needs no "+N" after it.
    assert_eq!(fit(&[50.0, 40.0], 94.0, plus, 4.0).shown, 2);
    assert_eq!(fit(&[50.0, 40.0], 93.0, plus, 4.0), Fit { shown: 1, first_w: 50.0, first_cut: false, plus: true });
}

#[test]
fn a_long_first_chip_is_cut_and_keeps_the_plus() {
    let f = fit(&[300.0, 40.0], 150.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 1, first_w: 116.0, first_cut: true, plus: true });
}

#[test]
fn a_room_narrower_than_the_plus_chip_shows_the_first_chip_only() {
    let f = fit(&[100.0, 40.0, 40.0], 25.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 1, first_w: 25.0, first_cut: true, plus: false });
}

#[test]
fn one_chip_reserves_nothing() {
    assert_eq!(fit(&[140.0], 150.0, plus, 4.0), Fit { shown: 1, first_w: 140.0, first_cut: false, plus: false });
    assert_eq!(fit(&[200.0], 150.0, plus, 4.0), Fit { shown: 1, first_w: 150.0, first_cut: true, plus: false });
    assert_eq!(fit(&[], 150.0, plus, 4.0).shown, 0);
}

fn spec(local: bool, remote: bool, build: bool) -> ChipSpec {
    let build = build.then_some(CheckState::Passed);
    ChipSpec { text: String::new(), style: ChipStyle::Branch, lead_tag: false, local, remote, build }
}

#[test]
fn chip_width_counts_its_marks() {
    // A 50 px name and the Chip size's padding either side.
    const BASE: f32 = 50.0 + 2.0 * TEXT_PAD;
    assert_eq!(chip_w(&spec(false, false, false), 50.0), BASE, "a name alone");
    assert_eq!(chip_w(&spec(true, false, false), 50.0), BASE + MARK_W, "a local branch");
    assert_eq!(chip_w(&spec(true, true, true), 50.0), BASE + 3.0 * MARK_W, "in sync, with a build state");
    let tag = ChipSpec { lead_tag: true, ..spec(false, false, false) };
    assert_eq!(chip_w(&tag, 50.0), BASE + TAG_LEAD);
    // In a room narrower than the chip the name is cut and the marks keep their room.
    let f = fit(&[chip_w(&spec(true, true, false), 90.0)], 100.0, plus, 4.0);
    assert!(f.first_cut && f.first_w == 100.0, "{f:?}");
}

#[test]
fn chip_at_finds_the_chip_under_the_pointer() {
    let rect = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, 9.0), pos2(x1, 31.0));
    let cell = RefsCell { chip_rects: vec![rect(10.0, 60.0), rect(64.0, 120.0)], ..RefsCell::default() };
    assert_eq!(cell.chip_at(pos2(30.0, 20.0)), Some(0));
    assert_eq!(cell.chip_at(pos2(90.0, 20.0)), Some(1));
    assert_eq!(cell.chip_at(pos2(62.0, 20.0)), None, "the gap between chips");
    assert_eq!(cell.chip_at(pos2(30.0, 35.0)), None, "below the chip, still on the row");
    assert_eq!(cell.chip_at(pos2(130.0, 20.0)), None, "past the last chip");
}

#[test]
fn the_checked_out_branch_and_head_keep_their_halos_room() {
    let with = |style| chip_w(&ChipSpec { style, ..spec(true, false, false) }, 50.0);
    let plain = with(ChipStyle::Branch);
    assert_eq!(with(ChipStyle::CurrentBranch), plain + 2.0 * HALO_ROOM, "3 px either side");
    assert_eq!(with(ChipStyle::Head), plain + 2.0 * HALO_ROOM);
    for style in [ChipStyle::Remote, ChipStyle::Tag] {
        assert_eq!(with(style), plain, "{style:?} has no halo");
    }
}
