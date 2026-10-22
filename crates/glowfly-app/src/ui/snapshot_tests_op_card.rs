//! The operation card over the WIP panel in Dusk and Dawn: a merge with conflicts left (View
//! conflicts leads), the same merge resolved (Commit merge leads), and a stopped cherry-pick in
//! German at the narrowest details panel. Shares the fixtures of `snapshot_tests`,
//! `snapshot_tests_conflict_files` and `snapshot_tests_pick`.

use super::snapshot_tests::{git, harness_palette_sized, remove_fixture};
use super::snapshot_tests_conflict_files::{fill_lists, merging};
use super::snapshot_tests_language_main::german;
use super::snapshot_tests_pick::stopped_pick;
use super::theme::{self, color::Palette};
use crate::state::AppState;
use crate::state::panel_layout::DETAILS;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, LoadOptions};
use std::path::PathBuf;

const DATE: &str = "2026-01-11T10:00:00+00:00";
const SIZE: eframe::egui::Vec2 = eframe::egui::vec2(1400.0, 820.0);

/// The merge fixture with alpha.txt and beta.txt conflicted, or (`resolved`) both fixed and staged.
fn merge(name: &str, resolved: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = merging(name, resolved);
    if resolved {
        std::fs::write(dir.join("alpha.txt"), (1..=10).map(|i| format!("alpha.txt {i}\n")).collect::<String>()).unwrap();
        git(&dir, &["add", "alpha.txt", "beta.txt"], DATE);
        st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
        let message = st.snap.as_ref().unwrap().merge_message.clone().expect("MERGE_MSG");
        st.draft.fill_merge(&message);
    }
    fill_lists(&mut st, &dir);
    (st, dir)
}

fn render(st: AppState, p: Palette, name: &str, check: impl FnOnce(&mut egui_kittest::Harness<'static, (AppState, Vec<crate::msg::Msg>)>)) {
    let mut h = harness_palette_sized(st, p, SIZE);
    h.run();
    h.check_snapshot(name);
    check(&mut h);
}

#[test]
fn op_card_merge_themes() {
    for (theme_name, p) in [("dusk", theme::DUSK), ("dawn", theme::DAWN)] {
        let (st, dir) = merge(&format!("op-card-conflicted-{theme_name}"), false);
        render(st, p, &format!("op_card_conflicted_{theme_name}"), |h| {
            assert!(!h.get_by_label("View conflicts").is_disabled());
            assert!(!h.get_by_label("Abort merge").is_disabled());
        });
        remove_fixture(&dir);

        let (st, dir) = merge(&format!("op-card-resolved-{theme_name}"), true);
        render(st, p, &format!("op_card_resolved_{theme_name}"), |h| {
            assert!(h.get_all_by_label("Commit merge").all(|n| !n.is_disabled()));
            assert!(!h.get_by_label("Abort merge").is_disabled());
        });
        remove_fixture(&dir);
    }
}

#[test]
fn op_card_german_narrow() {
    let (mut st, dir) = stopped_pick("op-card-german");
    german(&mut st);
    st.settings.panel_sizes.details = Some(DETAILS.min);
    render(st, theme::DUSK, "op_card_german_narrow", |_| {});
    remove_fixture(&dir);

    let (mut st, dir) = merge("op-card-german-merge", false);
    german(&mut st);
    st.settings.panel_sizes.details = Some(DETAILS.min);
    render(st, theme::DAWN, "op_card_german_merge_narrow", |_| {});
    remove_fixture(&dir);
}
