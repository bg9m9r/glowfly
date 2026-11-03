//! The WIP list headers' fit order: labels give way to icon-only squares, then the title is cut
//! before its count, and last a button moves into the more-actions menu.

use super::*;
use crate::ui::widgets::truncate::counted_galley;

/// A room `avail` wide for a title `title_w` wide (cut to 72), buttons 8 apart, a 32 px
/// more-actions button and 24 px icon-only squares.
fn room_of(avail: f32, title_w: f32) -> Room {
    Room { avail, title_w, title_min: 72.0, gap: 8.0, more_w: 32.0, icon_w: 24.0 }
}

const TITLE: f32 = 140.0;
/// Unstage all's label.
const LABEL: f32 = 90.0;

#[test]
fn a_lone_action_keeps_its_label_beside_the_whole_title() {
    let wide = LABEL + 8.0 + TITLE;
    assert_eq!(fit(room_of(wide, TITLE), &[LABEL], 1), Fit { shown: 1, icons: false });
    assert_eq!(fit(room_of(1000.0, TITLE), &[LABEL], 1), Fit { shown: 1, icons: false });
}

#[test]
fn the_label_goes_before_the_title_is_cut_and_the_menu_comes_last() {
    let labelled = LABEL + 8.0 + TITLE;
    // One px short of the label: an icon-only square beside the whole title.
    assert_eq!(fit(room_of(labelled - 1.0, TITLE), &[LABEL], 1), Fit { shown: 1, icons: true });
    // The title is cut, down to its least, while the square stays.
    let least = 24.0 + 8.0 + 72.0;
    assert_eq!(fit(room_of(least + 20.0, TITLE), &[LABEL], 1), Fit { shown: 1, icons: true });
    assert_eq!(fit(room_of(least, TITLE), &[LABEL], 1), Fit { shown: 1, icons: true });
    // Narrower still, the action moves into the menu.
    assert_eq!(fit(room_of(least - 1.0, TITLE), &[LABEL], 1).shown, 0);
    assert_eq!(fit(room_of(0.0, TITLE), &[LABEL], 1).shown, 0);
}

#[test]
fn unstaged_files_shows_stage_all_beside_its_menu() {
    // Stage all, Discard all, New file…: only Stage all shows; the menu is always there.
    let three = [70.0, 80.0, 60.0];
    let labelled = 70.0 + 8.0 + 32.0 + TITLE;
    assert_eq!(fit(room_of(labelled, TITLE), &three, 1), Fit { shown: 1, icons: false });
    assert_eq!(fit(room_of(labelled - 1.0, TITLE), &three, 1), Fit { shown: 1, icons: true });
    assert_eq!(fit(room_of(24.0 + 8.0 + 32.0 + 72.0 - 1.0, TITLE), &three, 1), Fit { shown: 0, icons: true });
    assert_eq!(fit(room_of(1000.0, TITLE), &[], 1), Fit { shown: 0, icons: false }, "no buttons, no menu");
}

#[test]
fn a_short_title_is_never_cut_below_itself() {
    // A title narrower than its least: the square fits beside it whole.
    assert_eq!(fit(room_of(24.0 + 8.0 + 50.0, 50.0), &[LABEL], 1), Fit { shown: 1, icons: true });
}

/// Runs `f` with the app's theme in a kittest frame.
fn with_ui(f: impl FnOnce(&mut Ui)) {
    let mut f = Some(f);
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        crate::ui::theme::apply_theme(ui.ctx());
        if let Some(f) = f.take() {
            f(ui);
        }
    });
    h.run();
}

/// The fit order with the real widths of Staged Files' header and Unstage all, in English, at
/// widths around each step.
#[test]
fn staged_files_gives_way_in_order_at_real_widths() {
    with_ui(|ui| {
        let t = Lang::En;
        let title = t.trf("details.wip.staged.header", &[("count", &3)]);
        let base = room(ui, &title);
        let labels = widths(ui, &actions(true, 3), t);
        assert_eq!(base.icon_w, 24.0, "a Small square");
        let at = |avail: f32| fit(Room { avail, ..base }, &labels, 1);
        let labelled = labels[0] + base.gap + base.title_w;
        assert_eq!(at(labelled), Fit { shown: 1, icons: false });
        assert_eq!(at(labelled - 1.0), Fit { shown: 1, icons: true }, "the label goes first");
        let least = base.icon_w + base.gap + base.title_min;
        assert!(least < labelled - 1.0);
        assert_eq!(at(least), Fit { shown: 1, icons: true }, "the title is cut while the square stays");
        assert_eq!(at(least - 1.0).shown, 0, "then the action moves into the menu");
    });
}

/// The square is the Small icon-only button, named by its label, with the new minus for
/// Unstage all.
#[test]
fn the_square_is_small_and_named_by_its_label() {
    assert!(HeaderAction::UnstageAll.icon() == Icon::Minus);
    assert!(HeaderAction::StageAll.icon() == Icon::Plus);
    with_ui(|ui| {
        let r = ui.add(HeaderAction::UnstageAll.shown(Lang::En, None, true));
        assert_eq!(r.rect.size(), vec2(24.0, 24.0));
    });
}

/// A cut title keeps its count whole, in English and in German (a clipping check).
#[test]
fn a_cut_title_keeps_its_count() {
    with_ui(|ui| {
        let font = type_scale::section(ui);
        for lang in [Lang::En, Lang::De] {
            let title = lang.trf("details.wip.unstaged.header", &[("count", &1)]);
            let least = title_min(ui, &title);
            let (g, cut) = counted_galley(ui.ctx(), &title, font.clone(), color::text(), least);
            assert!(cut, "{lang:?}: {title} is cut at {least}");
            let drawn: String = g.rows.iter().flat_map(|r| r.glyphs.iter().map(|g| g.chr)).collect();
            assert!(drawn.ends_with("… (1)") && drawn.chars().count() > 6, "{lang:?}: {drawn:?}");
            assert!(g.size().x <= least, "{lang:?}");
        }
    });
}

#[test]
fn widths_are_the_styled_buttons_own() {
    with_ui(|ui| {
        let t = Lang::En;
        let all = actions(false, 2);
        let w = widths(ui, &all, t);
        for (a, w) in all.iter().zip(&w) {
            assert_eq!(ui.add(a.button(t, None)).rect.width(), *w, "{a:?} is as wide as it said");
            assert_eq!(ui.add(a.button(t, None)).rect.height(), 24.0, "{a:?} is small");
        }
    });
}

#[test]
fn actions_follow_the_list_and_its_files() {
    use HeaderAction::*;
    assert_eq!(actions(true, 2), [UnstageAll]);
    assert!(actions(true, 0).is_empty());
    assert_eq!(actions(false, 2), [StageAll, DiscardAll, NewFile]);
    assert_eq!(actions(false, 0), [NewFile]);
}
