//! The WIP panel's steps while an operation is in progress, in Dusk (each operation, with
//! conflicts left and once they are resolved) and Dawn (the merge); the plain panel in both;
//! German at the narrowest details panel; and a long list. Shares the fixtures of
//! `snapshot_tests`, `snapshot_tests_conflict_files`, `snapshot_tests_pick`,
//! `snapshot_tests_rebase_edit` and `snapshot_tests_patch`.

use super::snapshot_tests::{git, harness_palette_sized, reload, remove_fixture, with_repo};
use super::snapshot_tests_conflict_files::{fill_lists, merging};
use super::snapshot_tests_language_main::german;
use super::snapshot_tests_patch::{applying, with_files};
use super::snapshot_tests_pick::stopped_pick;
use super::snapshot_tests_rebase_edit::rebasing;
use super::theme::{self, color::Palette};
use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::op_steps::OpStep;
use crate::state::panel_layout::DETAILS;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::rebase::StopKind;
use glowfly_core::repo::{self, FileEntry, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

const DATE: &str = "2026-01-11T10:00:00+00:00";
const SIZE: eframe::egui::Vec2 = eframe::egui::vec2(1400.0, 820.0);
type H = Harness<'static, (AppState, Vec<Msg>)>;

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

/// The stopped cherry-pick, or (`resolved`) with app.txt fixed and staged.
fn pick(name: &str, resolved: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = stopped_pick(name);
    if resolved {
        std::fs::write(dir.join("app.txt"), "app\nhotfix\nfix\n").unwrap();
        git(&dir, &["add", "app.txt"], DATE);
        reload(&mut st, &dir, &LoadOptions::all(500));
    }
    (st, dir)
}

fn shot(st: AppState, p: Palette, name: &str) -> H {
    let mut h = harness_palette_sized(st, p, SIZE);
    h.run();
    h.check_snapshot(name);
    h
}

#[test]
fn op_steps_merge() {
    for (theme_name, p) in [("dusk", theme::DUSK), ("dawn", theme::DAWN)] {
        let (st, dir) = merge(&format!("op-steps-merge-{theme_name}"), false);
        let mut h = shot(st, p, &format!("op_steps_merge_conflicted_{theme_name}"));
        for text in ["Merge in progress", "Resolve conflicts", "Check what goes in", "Commit the merge", "2 left"] {
            assert!(h.get_all_by_label(text).next().is_some(), "{text}");
        }
        assert!(h.query_by_label("Commit merge").is_none(), "step 3 waits, folded, for the conflicts");
        assert!(!h.get_by_label("Abort merge").is_disabled());
        h.get_by_label("Show: Commit the merge").click();
        h.run();
        assert!(h.state().1.iter().any(|m| matches!(m, Msg::Layout(LayoutMsg::ToggleOpStep(OpStep::Finish)))));
        drop(h);
        remove_fixture(&dir);

        let (st, dir) = merge(&format!("op-steps-merge-ready-{theme_name}"), true);
        let h = shot(st, p, &format!("op_steps_merge_ready_{theme_name}"));
        assert_eq!(h.get_all_by_label("Commit merge").count(), 1, "one Commit merge: step 3's");
        assert!(!h.get_by_label("Commit merge").is_disabled());
        assert!(h.query_by_label("Conflicts resolved").is_some());
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn op_steps_rebase() {
    let (st, dir) = rebasing(ThemeChoice::Dark, "op-steps-rebase", StopKind::Conflict, true);
    let h = shot(st, theme::DUSK, "op_steps_rebase_conflicted");
    assert!(h.query_by_label("Continue").is_none(), "no Continue while a file is conflicted");
    assert!(!h.get_by_label("Skip commit").is_disabled(), "Skip stays at hand");
    assert!(h.query_by_label("Continue rebase \u{b7} step 2 of 4").is_some());
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = rebasing(ThemeChoice::Dark, "op-steps-rebase-ready", StopKind::Conflict, false);
    st.snap.as_mut().unwrap().staged.push(FileEntry::new("app.txt", 'M'));
    let h = shot(st, theme::DUSK, "op_steps_rebase_ready");
    assert!(!h.get_by_label("Continue").is_disabled());
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = rebasing(ThemeChoice::Dark, "op-steps-rebase-edit", StopKind::Edit, false);
    let h = shot(st, theme::DUSK, "op_steps_rebase_edit");
    assert!(h.query_by_label("Amend").is_some(), "a stop to edit opens the commit form");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_steps_pick_and_revert() {
    let (st, dir) = pick("op-steps-pick", false);
    let h = shot(st, theme::DUSK, "op_steps_pick_conflicted");
    assert!(h.query_by_label("Continue the cherry-pick").is_some());
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = pick("op-steps-pick-ready", true);
    let h = shot(st, theme::DUSK, "op_steps_pick_ready");
    assert!(!h.get_by_label("Continue").is_disabled());
    drop(h);
    remove_fixture(&dir);

    // A revert stops the same way; its words and commands differ.
    for (resolved, name) in [(false, "op_steps_revert_conflicted"), (true, "op_steps_revert_ready")] {
        let (mut st, dir) = pick(&format!("op-steps-revert-{resolved}"), resolved);
        st.snap.as_mut().unwrap().state = RepositoryState::Revert;
        let h = shot(st, theme::DUSK, name);
        assert!(h.query_by_label("Abort revert").is_some());
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn op_steps_patches() {
    for (conflicted, name) in [(true, "op_steps_am_conflicted"), (false, "op_steps_am_ready")] {
        let (st, dir) = applying(ThemeChoice::Dark, &format!("op-steps-am-{conflicted}"), conflicted);
        let h = shot(st, theme::DUSK, name);
        assert!(h.query_by_label("Continue applying \u{b7} patch 2 of 3").is_some());
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn wip_panel_plain() {
    for (theme_name, p) in [("dusk", theme::DUSK), ("dawn", theme::DAWN)] {
        let (st, dir) = with_repo(ThemeChoice::Dark, &format!("wip-plain-{theme_name}"));
        let h = shot(st, p, &format!("wip_plain_{theme_name}"));
        assert!(h.query_by_label("Amend").is_some());
        assert!(h.query_by_label("Stage").is_none(), "row actions wait for a hover or the current row");
        drop(h);
        remove_fixture(&dir);
    }
    // Many files: the lists scroll above the commit box, which stays at the bottom.
    let many: Vec<String> = (1..=60).map(|i| format!("src/module_{i:02}.rs")).collect();
    let unstaged: Vec<(&str, char)> = many.iter().map(|p| (p.as_str(), 'M')).collect();
    let staged: Vec<(&str, char)> = many[..25].iter().map(|p| (p.as_str(), 'A')).collect();
    let (st, dir) = with_files(ThemeChoice::Dark, "wip-plain-long", &unstaged, &staged);
    drop(shot(st, theme::DUSK, "wip_plain_long"));
    remove_fixture(&dir);
}

#[test]
fn op_steps_german_narrow() {
    for (resolved, name) in [(false, "op_steps_german_conflicted"), (true, "op_steps_german_ready")] {
        let (mut st, dir) = merge(&format!("op-steps-german-{resolved}"), resolved);
        german(&mut st);
        st.settings.panel_sizes.details = Some(DETAILS.min);
        drop(shot(st, theme::DUSK, name));
        remove_fixture(&dir);
    }
}
