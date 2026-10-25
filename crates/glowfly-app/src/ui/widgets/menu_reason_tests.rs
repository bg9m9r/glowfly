use super::*;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;

const UNLOCK_WHY: &str = "It is locked. Unlock it first.";

#[test]
fn menu_items_keep_their_label_as_name() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        menu_button(ui, "Unlock", "", ItemState::Blocked(UNLOCK_WHY));
        menu_button(ui, "Open", "", ItemState::setup(Lang::En, "Opens the page"));
    });
    h.run();
    assert!(h.get_by_label("Unlock").is_disabled());
    assert!(!h.get_by_label("Open").is_disabled());
}

#[test]
fn blocked_items_take_focus_and_show_why() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        menu_button(ui, "Unlock", "", ItemState::Blocked(UNLOCK_WHY));
    });
    h.run();
    assert!(h.query_by_label(UNLOCK_WHY).is_none(), "no reason before hover or focus");
    h.get_by_label("Unlock").focus();
    h.run();
    let item = h.get_by_label("Unlock");
    assert!(item.is_focused() && item.is_disabled());
    assert!(h.query_by_label(UNLOCK_WHY).is_some(), "the reason shows under the focused item");
    assert_eq!(h.query_all_by_label("Unlock").count(), 1, "the tooltip doesn't repeat the label");
}

#[test]
fn blocked_items_stay_inert() {
    let mut h = crate::ui::snapshot_check::builder().build_ui_state(
        |ui, n: &mut u32| {
            if menu_button(ui, "Unlock", "", ItemState::Blocked(UNLOCK_WHY)).clicked() {
                *n += 1;
            }
            if menu_button(ui, "Copy", "", ItemState::Ready).clicked() {
                *n += 100;
            }
        },
        0,
    );
    h.run();
    h.get_by_label("Unlock").click();
    h.run();
    h.get_by_label("Unlock").focus();
    h.run();
    h.press_key(egui::Key::Enter);
    h.run();
    h.press_key(egui::Key::Space);
    h.run();
    assert_eq!(*h.state(), 0, "a blocked item never clicks");
    h.get_by_label("Copy").click();
    h.run();
    assert_eq!(*h.state(), 100, "a ready item does");
}

#[test]
fn blocked_items_describe_why() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        menu_button(ui, "Unlock", "", ItemState::Blocked(UNLOCK_WHY));
        menu_button(ui, "Copy", "", ItemState::Ready);
    });
    h.run();
    assert_eq!(h.get_by_label("Unlock").description().as_deref(), Some(UNLOCK_WHY));
    assert_eq!(h.get_by_label("Copy").description(), None);
}

#[test]
fn a_long_reason_does_not_widen_the_item() {
    let reason = "x ".repeat(60).trim_end().to_owned();
    assert!(reason.chars().count() >= 119);
    let mut widths = Vec::new();
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        let w = |r: Response| r.rect.width();
        let short = w(menu_button(ui, "Compare…", "", ItemState::Blocked("No")));
        let long = w(menu_button(ui, "Compare…", "", ItemState::Blocked(&reason)));
        let ready = w(menu_button(ui, "Compare…", "", ItemState::Ready));
        widths = vec![short, long, ready];
    });
    h.run();
    drop(h);
    let [short, long, ready] = widths[..] else { panic!("{widths:?}") };
    assert_eq!(short, long, "the reason isn't in the menu");
    assert!(short >= ready + INFO_ROOM - 0.5, "the info mark's room: {short} vs {ready}");
}

#[test]
fn setup_items_explain_on_focus() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        menu_button(ui, "Open", "", ItemState::setup(Lang::En, "Opens the page"));
    });
    h.run();
    h.get_by_label("Open").focus();
    h.run();
    assert!(h.query_by_label("Opens the page").is_some());
    assert!(!h.get_by_label("Open").is_disabled());
}

#[test]
fn setup_items_say_set_up_in_the_language() {
    let de = ItemState::setup(Lang::De, "x");
    let ItemState::Setup { note, suffix } = de else { panic!("{de:?}") };
    assert_eq!(note, "x");
    assert_eq!(suffix, Lang::De.tr("common.set_up.suffix"));
    assert_ne!(suffix, Lang::En.tr("common.set_up.suffix"));
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        menu_button(ui, "Öffnen", "", de);
    });
    h.run();
    assert!(!h.get_by_label("Öffnen").is_disabled(), "a setup item stays enabled");
}
