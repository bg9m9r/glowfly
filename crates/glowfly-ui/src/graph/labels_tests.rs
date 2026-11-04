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
    // Not monotone in the first chip's width by design: just short of room for "+N" the first
    // chip takes the whole room, a pixel wider it shrinks to the kept width beside "+N".
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
fn hidden_labels_offer_the_full_list_with_or_without_the_plus_chip() {
    let no_folds = |_| false;
    // "+N" dropped beside a whole first chip: hovering it still lists every label.
    let dropped = fit(&[50.0, 40.0], KEEP, 80.0, plus, 4.0);
    assert_eq!(dropped, Fit { shown: 1, first_w: 50.0, first_cut: false, plus: false });
    assert_eq!(hover_tip(dropped, 2, true, Some(0), no_folds), Some(Tip::All));
    // With the "+N" chip, the same.
    let with_plus = fit(&[50.0, 40.0], KEEP, 90.0, plus, 4.0);
    assert!(with_plus.plus, "{with_plus:?}");
    assert_eq!(hover_tip(with_plus, 2, true, None, no_folds), Some(Tip::All));
    // Every chip shown: a whole chip asks for nothing, one folding remotes in for itself.
    let all = fit(&[50.0, 40.0], KEEP, 200.0, plus, 4.0);
    assert_eq!(hover_tip(all, 2, true, Some(0), no_folds), None);
    assert_eq!(hover_tip(all, 2, true, Some(1), |i| i == 1), Some(Tip::One(1)));
    // A lone cut chip with nothing hidden names itself; off the chips, nothing.
    let cut = fit(&[200.0], KEEP, 100.0, plus, 4.0);
    assert_eq!(hover_tip(cut, 1, true, Some(0), no_folds), Some(Tip::One(0)));
    assert_eq!(hover_tip(dropped, 2, false, None, no_folds), None);
}

#[test]
fn the_kept_text_is_six_characters_of_the_name() {
    let of = |style, text: &str| kept_text(&ChipSpec { style, text: text.to_owned(), ..spec(false, false) });
    assert_eq!(of(ChipStyle::Branch, "feature/login"), "featur…");
    assert_eq!(of(ChipStyle::CurrentBranch, "✔ feature/login"), "✔ featur…", "the check mark isn't one of the six");
    assert_eq!(of(ChipStyle::Branch, "main"), "main", "a short name whole");
    assert_eq!(of(ChipStyle::Tag, "v1.2.3"), "v1.2.3", "exactly six, whole");
    assert_eq!(of(ChipStyle::Remote, "origin/main"), "origin…");
    assert_eq!(of(ChipStyle::Branch, "feature"), "feature", "seven graphemes, as wide as six and an ellipsis, whole");
    // Graphemes, not chars: an accent built from two chars, a family emoji from five.
    let accented = "e\u{301}".repeat(9);
    assert_eq!(of(ChipStyle::Branch, &accented), format!("{}…", "e\u{301}".repeat(6)));
    let family = "👨\u{200d}👩\u{200d}👧";
    assert_eq!(of(ChipStyle::Tag, &format!("{family}{}", "x".repeat(8))), format!("{family}xxxxx…"));
    assert_eq!(of(ChipStyle::Branch, "機能ブランチの名前です"), "機能ブランチ…");
}

/// A context with the bundled fonts installed (the semibold face among them).
fn ctx() -> eframe::egui::Context {
    let ctx = eframe::egui::Context::default();
    ctx.set_fonts(crate::ui::theme::fonts::definitions().clone());
    for _ in 0..2 {
        let _ = ctx.run(eframe::egui::RawInput::default(), |_| {});
    }
    ctx
}

#[test]
fn a_cut_chip_keeps_both_ends_of_its_name() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), eframe::egui::LayerId::background(), Rect::EVERYTHING);
    let named = |style, text: &str| ChipSpec { style, text: text.to_owned(), ..spec(false, false) };
    let (a, b) = ("worktree-agent-a1b2c3d4e5f6", "worktree-agent-a1b2c3d4e9z7");
    let regular = FontId::proportional(FONT);
    let room = 0.8 * text_w(&p, a, &regular);
    let (cut_a, cut_b) = (chip_text(&p, &named(ChipStyle::Branch, a), room), chip_text(&p, &named(ChipStyle::Branch, b), room));
    assert!(cut_a.starts_with("worktree") && cut_a.ends_with("d4e5f6") && cut_a.contains('…'), "{cut_a}");
    assert!(text_w(&p, &cut_a, &regular) <= room, "{cut_a} fits {room}");
    assert_ne!(cut_a, cut_b, "names that share a long start stay apart");
    // The checked-out branch's check mark stays whole, before its name cut in semibold.
    let current = named(ChipStyle::CurrentBranch, &format!("{CHECK}{a}"));
    let semi = current.font(&p);
    let head = chip_text(&p, &current, room + text_w(&p, CHECK, &semi));
    let (mark, cut) = head.split_at(CHECK.len());
    assert_eq!(mark, CHECK);
    assert!(cut.starts_with("work") && cut.ends_with('6') && cut.contains('…'), "{head}");
    assert!(text_w(&p, cut, &semi) <= room, "{cut} fits {room} in semibold");
    assert_eq!(chip_text(&p, &named(ChipStyle::Branch, "main"), room), "main", "a name that fits is whole");
}

fn spec(remote: bool, build: bool) -> ChipSpec {
    let build = build.then_some(CheckState::Passed);
    ChipSpec { text: String::new(), style: ChipStyle::Branch, lead_tag: false, remote, build }
}

/// The checked-out branch's chip is set, and so measured, in semibold; the others in regular.
#[test]
fn the_current_branch_is_measured_in_semibold() {
    let ctx = ctx();
    let p = Painter::new(ctx.clone(), eframe::egui::LayerId::background(), Rect::EVERYTHING);
    let named = |style, text: &str| ChipSpec { style, text: text.to_owned(), ..spec(false, false) };
    let current = named(ChipStyle::CurrentBranch, "✔ feature/login");
    let semi = crate::ui::theme::fonts::semibold_in(&ctx, FONT);
    assert_eq!(current.font(&p), semi);
    assert_ne!(semi.family, eframe::egui::FontFamily::Proportional, "the bundled semibold face is installed");
    assert_eq!(spec_w(&p, &current), chip_w(&current, text_w(&p, &current.text, &semi)));
    assert!(text_w(&p, &current.text, &semi) > text_w(&p, &current.text, &FontId::proportional(FONT)), "semibold is wider");
    for style in [ChipStyle::Branch, ChipStyle::Remote, ChipStyle::Tag, ChipStyle::Head] {
        assert_eq!(named(style, "main").font(&p), FontId::proportional(FONT), "{style:?}");
    }
}

/// A local branch has no mark of its own; one in sync with its copies on remotes has the cloud.
#[test]
fn a_local_chip_has_no_mark_and_an_in_sync_one_the_cloud() {
    let label = |name: &str, kind| glowfly_core::repo::RefLabel { name: name.into(), full: format!("refs/x/{name}"), kind, is_head: false };
    let (main, up) = (label("main", RefKind::Local), label("origin/main", RefKind::Remote));
    let alone = ChipSpec::of(&Chip { label: &main, remotes: Vec::new() }, None);
    assert_eq!((alone.trailing(), alone.remote), (0, false), "no local mark");
    let in_sync = ChipSpec::of(&Chip { label: &main, remotes: vec![&up] }, None);
    assert_eq!((in_sync.trailing(), in_sync.remote), (1, true), "the cloud");
    let remote = ChipSpec::of(&Chip { label: &up, remotes: Vec::new() }, None);
    assert!(remote.remote && remote.style == ChipStyle::Remote);
}

#[test]
fn chip_width_counts_its_marks() {
    // A 50 px name and the Chip size's padding either side.
    const BASE: f32 = 50.0 + 2.0 * TEXT_PAD;
    assert_eq!(chip_w(&spec(false, false), 50.0), BASE, "a local branch: a name alone");
    assert_eq!(chip_w(&spec(true, false), 50.0), BASE + MARK_W, "in sync");
    assert_eq!(chip_w(&spec(true, true), 50.0), BASE + 2.0 * MARK_W, "in sync, with a build state");
    let tag = ChipSpec { lead_tag: true, ..spec(false, false) };
    assert_eq!(chip_w(&tag, 50.0), BASE + TAG_LEAD);
    // In a room narrower than the chip the name is cut and the marks keep their room.
    let f = fit(&[chip_w(&spec(true, true), 90.0)], KEEP, 100.0, plus, 4.0);
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
    let with = |style| chip_w(&ChipSpec { style, ..spec(false, false) }, 50.0);
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
