//! Destructive menu items: the working-tree file menu in a light theme puts Discard changes…
//! and Delete… last, after every other group, in the danger style, with the danger hover pill
//! on Discard (a pixel golden; `graph_commit_menu` and `toolbar_push_menu` cover the dark one).

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

#[test]
fn wip_file_menu_danger_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "menu-danger-light");
    st.settings.reduce_motion = true;
    let mut h = harness(st, false);
    h.run();
    let b = h.get_by_role_and_label(Role::Button, "README.md").bounding_box().expect("the row");
    click(&mut h, pos2(b.x1 as f32 - 12.0, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Secondary);
    let top = |label: &str| h.get_by_label(label).bounding_box().expect(label).y0;
    let (discard, delete) = (top("Discard changes…"), top("Delete…"));
    for safe in ["Stash this file…", "Rename…", "Track in Git LFS"] {
        assert!(top(safe) < discard, "{safe} comes before the destructive group");
    }
    assert!(discard < delete, "Discard changes…, then Delete…");
    h.get_by_label("Discard changes…").hover();
    h.run_steps(4);
    h.check_snapshot("wip_file_menu_danger_light");
    drop(h);
    remove_fixture(&dir);
}
