//! The toolbar in narrow windows: from 900 to 1400 px, in English and German, with and without
//! captions, a hosting service and the conflict chip, no two buttons overlap and none leaves the
//! window; the parts fold in their order, never Pull, Push or Refresh; and the More menu runs
//! what the folded buttons would.

use super::tests::{span, variant};
use crate::msg::{JobMsg, Msg};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::snapshot_tests_worktrees::H;
use crate::ui::snapshot_tests::{harness_sized, remove_fixture};
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::Queryable;

/// The toolbar's buttons left to right, with their edges: the ones level with Refresh.
fn toolbar_buttons(h: &H) -> Vec<(String, f64, f64)> {
    let refresh = h.state().0.settings.lang().tr("toolbar.refresh.name");
    let row = h.get_by_label(refresh).bounding_box().expect("bounds");
    let mid = |b: &egui::accesskit::Rect| (b.y0 + b.y1) / 2.0;
    let mut out: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| {
            let b = n.bounding_box()?;
            (mid(&b) > row.y0 && mid(&b) < row.y1).then(|| (n.label().unwrap_or_default(), b.x0, b.x1))
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

/// A harness for `variant`'s `case` in a window `width` wide, in `lang`. `test` names the
/// fixture folder with the rest, so tests running in parallel never share one.
fn narrow(test: &str, case: usize, labels: bool, lang: &str, width: f32) -> (H, std::path::PathBuf) {
    let (mut st, dir) = variant(case, labels, &format!("toolbar-{test}-{case}-{labels}-{lang}-{width}"));
    st.settings.language = lang.into();
    let mut h = harness_sized(st, true, egui::vec2(width, 700.0));
    h.run();
    (h, dir)
}

#[test]
fn narrow_windows_fold_the_toolbar_without_overlap() {
    for lang in ["en", "de"] {
        for width in [900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0] {
            // Plain, the 80-character branch with the conflict chip, and a hosting service.
            for (case, labels) in [(0, true), (4, true), (5, true), (5, false)] {
                let (h, dir) = narrow("overlap", case, labels, lang, width);
                let at = format!("{lang} {width} case {case} labels {labels}");
                let buttons = toolbar_buttons(&h);
                for pair in buttons.windows(2) {
                    assert!(pair[1].1 >= pair[0].2, "{at}: {:?} overlaps {:?}", pair[0], pair[1]);
                }
                let last = buttons.last().expect("buttons");
                assert!(last.2 <= f64::from(width), "{at}: {last:?} leaves the window");
                let t = h.state().0.settings.lang();
                for key in ["toolbar.pull.name", "toolbar.push.name", "toolbar.search.name", "toolbar.refresh.name"] {
                    assert!(buttons.iter().any(|b| b.0 == t.tr(key)), "{at}: {key} never folds");
                }
                drop(h);
                remove_fixture(&dir);
            }
        }
    }
}

#[test]
fn parts_fold_in_order() {
    let folded = |width: f32| {
        let (h, dir) = narrow("order", 5, true, "en", width);
        let shown: Vec<String> = toolbar_buttons(&h).into_iter().map(|b| b.0).collect();
        drop(h);
        remove_fixture(&dir);
        let order = [
            "Open the repository folder",
            "Undo the last action",
            "Create a branch",
            "PR: create a pull request",
            "Search and run commands",
            "Open settings",
        ];
        order.map(|name| !shown.iter().any(|s| s.starts_with(name)))
    };
    assert_eq!(folded(1400.0), [false; 6], "nothing folds in a wide window");
    let mut last = 0;
    for width in [1300.0, 1200.0, 1100.0, 1000.0, 900.0, 800.0] {
        let f = folded(width);
        let k = f.iter().take_while(|&&f| f).count();
        assert!(f[k..].iter().all(|&f| !f), "{width}: {f:?} folds out of order");
        assert!(k >= last, "{width}: a narrower window never shows more");
        last = k;
    }
    assert!(last >= 4, "an 800 px window folds most parts");
}

#[test]
fn the_more_menu_runs_the_folded_buttons() {
    let (mut h, dir) = narrow("menu", 5, true, "en", 900.0);
    assert!(h.query_by_label("Show the terminal panel").is_none(), "Terminal folded");
    h.get_by_label("More toolbar actions").click();
    h.run();
    for label in ["Terminal", "Folder", "Branch", "Stash", "Pop", "Pull request options"] {
        assert!(h.query_by_label(label).is_some(), "{label} in the menu");
    }
    assert!(h.get_by_label("Pop").is_disabled(), "no stash to pop");
    h.check_snapshot("toolbar_more_menu");
    h.get_by_label("Terminal").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Terminal(crate::terminal::TerminalMsg::Toggle))));
    assert!(h.query_by_label("Folder").is_none(), "a click closes the menu");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Undo))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_chip_folds_a_part_instead_of_squeezing_the_pickers() {
    // The 80-character branch without the chip (case 1) and with it (case 4).
    let branch_w = |case: usize| {
        let (h, dir) = narrow("chip", case, true, "en", 1300.0);
        let name = h.state().0.snap.as_ref().and_then(|s| s.head_branch.clone()).expect("a branch");
        let (l, r) = span(&h, &name);
        drop(h);
        remove_fixture(&dir);
        r - l
    };
    let (plain, chip) = (branch_w(1), branch_w(4));
    // The branch part at the pickers' comfortable width: its padding, dot and caret around 80 px.
    assert!(chip >= plain.min(134.0) - 0.5, "the branch picker went from {plain} to {chip}");
}
