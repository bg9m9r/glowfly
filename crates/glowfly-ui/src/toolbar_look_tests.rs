//! Snapshots of the toolbar's split buttons and toggles: hovering a dropdown caret lights the
//! caret alone, and the Terminal, Search and Activity toggles stay latched while their panel
//! is shown.

use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::snapshot_tests_remotes::with_remotes;
use crate::ui::snapshot_tests::{harness, remove_fixture};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::Tracking;
use glowfly_core::settings::ThemeChoice;

#[test]
fn toolbar_split_hover() {
    let (mut st, dir) = with_remotes(ThemeChoice::Dark, "toolbar-split-hover");
    st.settings.reduce_motion = true;
    let main = st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == "main").unwrap();
    main.upstream = Some("origin/main".into());
    main.tracking = Some(Tracking { remote: "origin".into(), merge: "main".into() });
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More pull actions").hover();
    h.run();
    h.check_snapshot("toolbar_split_hover");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_toggles_on() {
    let (mut st, dir) = with_remotes(ThemeChoice::Dark, "toolbar-toggles-on");
    st.settings.reduce_motion = true;
    st.view.show_log = true;
    st.view.show_search = true;
    st.terminal.open = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("toolbar_toggles_on");
    drop(h);
    remove_fixture(&dir);
}
