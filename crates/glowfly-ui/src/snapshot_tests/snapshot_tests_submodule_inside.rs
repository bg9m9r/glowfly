//! The SUBMODULES row menu's items for working inside a submodule: Record this checkout, Fetch,
//! Pull and Discard changes inside…, enabled or greyed with their reasons by the submodule's
//! state, and the messages they push.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_submodules::{open_menu, pushed, with_submodules};
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::submodule::SubmoduleMsg;

const ITEMS: [&str; 4] = ["Record this checkout", "Fetch", "Pull", "Discard changes inside…"];

#[test]
fn submodule_inside_menu() {
    // Out of sync with changes inside: every item is offered.
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodule-inside-menu");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "vendor/zlib");
    h.check_snapshot("submodule_inside_menu");
    for item in ITEMS {
        assert!(!h.get_by_label(item).is_disabled(), "{item}");
    }
    h.get_by_label("Record this checkout").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Record(p)) if p == "vendor/zlib")));
    open_menu(&mut h, "vendor/zlib");
    h.get_by_label("Fetch").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Fetch(p)) if p == "vendor/zlib")));
    open_menu(&mut h, "vendor/zlib");
    h.get_by_label("Pull").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Pull(p)) if p == "vendor/zlib")));
    open_menu(&mut h, "vendor/zlib");
    h.get_by_label("Discard changes inside…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::DiscardInside { path, confirmed: false }) if path == "vendor/zlib")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_inside_menu_blocked() {
    // In sync and clean: nothing to record or discard; fetching and pulling are offered.
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodule-inside-blocked");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "libs/core");
    assert!(h.get_by_label("Record this checkout").is_disabled());
    assert!(h.get_by_label("Discard changes inside…").is_disabled());
    assert!(!h.get_by_label("Fetch").is_disabled());
    assert!(!h.get_by_label("Pull").is_disabled());
    h.press_key(eframe::egui::Key::Escape);
    h.run();

    // Not checked out: none of them.
    open_menu(&mut h, "tools/gen");
    h.check_snapshot("submodule_inside_menu_blocked");
    for item in ITEMS {
        assert!(h.get_by_label(item).is_disabled(), "{item}");
    }
    drop(h);
    remove_fixture(&dir);
}
