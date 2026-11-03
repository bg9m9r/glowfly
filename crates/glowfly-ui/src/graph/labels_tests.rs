//! The chip row's fitting, chip widths (with their marks and the halo's room) and hit testing.

use super::*;

/// "+n" chips 30 px wide.
fn plus(_: usize) -> f32 {
    30.0
}

/// The first chip cut to its first six characters: 60 px.
const KEEP: f32 = 60.0;

#[test]
fn every_chip_fits() {
    let f = fit(&[50.0, 40.0, 30.0], KEEP, 200.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 3, first_w: 50.0, first_cut: false, plus: false });
}

#[test]
fn a_chip_that_doesnt_fit_hides_the_rest() {
    // 50 + 4 + 40 = 94; the third (60) would reach 158 and leave no room for "+2".
    let f = fit(&[50.0, 40.0, 60.0, 20.0], KEEP, 150.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 2, first_w: 50.0, first_cut: false, plus: true });
    // The last chip needs no "+N" after it.
    assert_eq!(fit(&[50.0, 40.0], KEEP, 94.0, plus, 4.0).shown, 2);
    assert_eq!(fit(&[50.0, 40.0], KEEP, 93.0, plus, 4.0), Fit { shown: 1, first_w: 50.0, first_cut: false, plus: true });
}

#[test]
fn a_long_first_chip_is_cut_and_keeps_the_plus() {
    let f = fit(&[300.0, 40.0], KEEP, 150.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 1, first_w: 116.0, first_cut: true, plus: true });
}

#[test]
fn a_room_narrower_than_the_plus_chip_shows_the_first_chip_only() {
    let f = fit(&[100.0, 40.0, 40.0], KEEP, 25.0, plus, 4.0);
    assert_eq!(f, Fit { shown: 1, first_w: 25.0, first_cut: true, plus: false });
}

#[test]
fn one_chip_reserves_nothing() {
    assert_eq!(fit(&[140.0], KEEP, 150.0, plus, 4.0), Fit { shown: 1, first_w: 140.0, first_cut: false, plus: false });
    assert_eq!(fit(&[200.0], KEEP, 150.0, plus, 4.0), Fit { shown: 1, first_w: 150.0, first_cut: true, plus: false });
    assert_eq!(fit(&[], KEEP, 150.0, plus, 4.0).shown, 0);
}

#[test]
fn the_plus_chip_drops_before_the_first_chip_goes_below_six_characters() {
    // 94 px: 60 for the kept first chip, 4 + 30 for "+1".
    assert_eq!(fit(&[100.0, 40.0], KEEP, 94.0, plus, 4.0), Fit { shown: 1, first_w: KEEP, first_cut: true, plus: true });
    // One pixel less and the "+N" chip goes; the first chip takes its room.
    assert_eq!(fit(&[100.0, 40.0], KEEP, 93.0, plus, 4.0), Fit { shown: 1, first_w: 93.0, first_cut: true, plus: false });
    assert_eq!(fit(&[100.0, 40.0, 40.0], KEEP, 70.0, plus, 4.0), Fit { shown: 1, first_w: 70.0, first_cut: true, plus: false });
}

#[test]
fn beside_a_plus_chip_the_first_chip_never_goes_below_six_characters() {
    for room in 0..=200 {
        let f = fit(&[150.0, 40.0, 40.0], KEEP, room as f32, plus, 4.0);
        assert_eq!(f.shown, 1, "room {room}: the cut first chip stops the row");
        if f.plus {
            assert!(f.first_w >= KEEP, "room {room}: {f:?}");
        } else {
            assert!(f.first_w < KEEP + 34.0, "room {room}: the plus chip dropped only when it had no room: {f:?}");
        }
    }
}

#[test]
fn a_first_chip_shorter_than_six_characters_keeps_the_plus_while_it_fits_whole() {
    assert_eq!(fit(&[40.0, 40.0], KEEP, 74.0, plus, 4.0), Fit { shown: 1, first_w: 40.0, first_cut: false, plus: true });
    assert_eq!(fit(&[40.0, 40.0], KEEP, 73.0, plus, 4.0), Fit { shown: 1, first_w: 40.0, first_cut: false, plus: false });
}

#[test]
fn the_kept_text_is_six_characters_of_the_name() {
    let of = |style, text: &str| kept_text(&ChipSpec { style, text: text.to_owned(), ..spec(true, false, false) });
    assert_eq!(of(ChipStyle::Branch, "feature/login"), "featur…");
    assert_eq!(of(ChipStyle::CurrentBranch, "✔ feature/login"), "✔ featur…", "the check mark isn't one of the six");
    assert_eq!(of(ChipStyle::Branch, "main"), "main", "a short name whole");
    assert_eq!(of(ChipStyle::Tag, "v1.2.3"), "v1.2.3", "exactly six, whole");
    assert_eq!(of(ChipStyle::Remote, "origin/main"), "origin…");
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
    let f = fit(&[chip_w(&spec(true, true, false), 90.0)], KEEP, 100.0, plus, 4.0);
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
fn a_halo_takes_no_room_but_keeps_the_next_chip_clear() {
    let with = |style| chip_w(&ChipSpec { style, ..spec(true, false, false) }, 50.0);
    for style in ChipStyle::ALL {
        assert_eq!(with(style), with(ChipStyle::Branch), "{style:?}: the halo paints into the inset, padding or gap");
    }
    let leads = halo_leads(&[ChipStyle::CurrentBranch, ChipStyle::Remote, ChipStyle::Tag]);
    assert_eq!(leads, vec![0.0, HALO_ROOM, 0.0], "only the chip after the halo moves, by the halo's reach");
    assert_eq!(halo_leads(&[ChipStyle::Branch, ChipStyle::Head]), vec![0.0, 0.0]);
}

#[test]
fn a_narrow_tables_clip_holds_the_halo() {
    let span = (300.0, 380.0);
    let clip = inline_clip(span);
    // A halo chip filling the span, its ring reaching past both ends.
    let chip = chip::rect_at(CHIP, span.0, 20.0, span.1 - span.0).expand(HALO_ROOM);
    assert!(clip.0 <= chip.left() && chip.right() <= clip.1, "{clip:?} holds {chip:?}");
    assert_eq!(halo_reach(ChipStyle::CurrentBranch), HALO_ROOM);
    assert_eq!(halo_reach(ChipStyle::Remote), 0.0, "the line to the node starts at a plain chip's edge");
}
