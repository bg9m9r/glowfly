//! Disabled actions that say why on screen: a dialog footer's reason line beside Cancel, cut when
//! long, above the buttons when extras and a note take the row's room, and for a danger button;
//! and menu items greyed with a painted info mark whose reason shows in the shared tooltip on
//! hover or keyboard focus, or with "Set up…" after an entry that opens a setting first.

use super::{menu_item, menu_item_if, menu_item_setup};
use crate::msg::Msg;
use crate::state::tool_compare::OPEN_IN_DIFF_TOOL;
use crate::tools::ToolsMsg;
use crate::ui::dialog_frame::{Cx, Dialog, Footer, Size};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::snapshot_tests_file_menu::menu_style;
use crate::ui::theme::{apply_theme, color};
use crate::ui::widgets::{buttons, tooltip};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::DiffTarget;

const NO_NAME: &str = "Enter a name for the branch.";

fn paint_theme(ui: &mut egui::Ui, dark: bool) {
    color::set_dark(dark);
    apply_theme(ui.ctx());
    ui.ctx().set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
}

/// A Small dialog in a 520 x 260 window with a body line and the footer `footer` builds.
fn dialog(dark: bool, footer: impl Fn() -> Footer<'static> + 'static) -> Harness<'static> {
    crate::ui::snapshot_check::builder().with_size(egui::vec2(520.0, 260.0)).build_ui(move |ui| {
        paint_theme(ui, dark);
        let cx = Cx { t: Lang::En, keyboard: false, key: egui::Id::new("reason-dialog") };
        egui::Frame::popup(ui.style()).inner_margin(18).corner_radius(8).show(ui, |ui| {
            let mut d = Dialog::new(cx, "New branch").size(Size::Small).begin(ui);
            d.body(ui, |ui| ui.label("The branch starts at main."));
            d.finish(ui, footer());
        });
    })
}

fn create_footer() -> Footer<'static> {
    Footer::new().cancel().primary(buttons::primary("Create branch").enabled(false).why(Some(NO_NAME)))
}

/// The bounds of the one node named `label`.
fn bounds<S>(h: &Harness<'_, S>, label: &str) -> egui::Rect {
    let b = h.get_by_label(label).bounding_box().expect("bounds");
    egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
}

#[test]
fn dialog_reason_beside_cancel() {
    for (dark, name) in [(true, "dialog_reason_beside_cancel"), (false, "dialog_reason_beside_cancel_light")] {
        let mut h = dialog(dark, create_footer);
        h.run();
        h.check_snapshot(name);
        assert!(h.get_by_label("Create branch").is_disabled());
        assert!(h.query_by_label(NO_NAME).is_some(), "the reason is on screen");
        let (why, cancel) = (bounds(&h, NO_NAME), bounds(&h, "Cancel"));
        assert!(why.right() < cancel.left(), "{why:?} ends left of {cancel:?}");
        assert!(why.top() < cancel.bottom() && why.bottom() > cancel.top(), "on Cancel's row");
    }
}

#[test]
fn dialog_reason_cut_when_long() {
    let long = "The branch name can't be used because another branch with almost exactly the same name is checked out in a worktree.";
    assert!(long.chars().count() >= 110);
    let mut h = dialog(true, move || Footer::new().cancel().primary(buttons::primary("Create branch").enabled(false).why(Some(long))));
    h.run();
    h.check_snapshot("dialog_reason_cut_when_long");
    let (why, cancel) = (bounds(&h, long), bounds(&h, "Cancel"));
    assert!(why.right() < cancel.left() && why.left() > 0.0, "cut to the row's room: {why:?}");
}

#[test]
fn dialog_reason_above_with_extras() {
    let footer = || {
        Footer::new()
            .extra(buttons::secondary("Use folder name"))
            .note("Draft saved")
            .cancel()
            .primary(buttons::primary("Create branch").enabled(false).why(Some(NO_NAME)))
    };
    let mut h = dialog(true, footer);
    h.run();
    h.check_snapshot("dialog_reason_above_with_extras");
    let (why, cancel) = (bounds(&h, NO_NAME), bounds(&h, "Cancel"));
    assert!(why.bottom() <= cancel.top(), "on its own line above: {why:?} {cancel:?}");
    assert!(cancel.top() - why.bottom() < 12.0, "just above the buttons: {why:?} {cancel:?}");
    assert!(bounds(&h, "Create branch").right() <= 520.0, "the row still fits");
}

#[test]
fn dialog_danger_reason() {
    let footer = || Footer::new().cancel().danger(buttons::danger("Delete branch").enabled(false).why(Some("Tick the box above first")));
    let mut h = dialog(true, footer);
    h.run();
    h.check_snapshot("dialog_danger_reason");
    assert!(h.get_by_label("Delete branch").is_disabled());
    assert!(h.query_by_label("Tick the box above first").is_some());
}

#[test]
fn enabled_primary_has_no_reason() {
    let mut h = dialog(true, || Footer::new().cancel().primary(buttons::primary("Create branch").why(Some(NO_NAME))));
    h.run();
    assert!(!h.get_by_label("Create branch").is_disabled());
    assert!(h.query_by_label(NO_NAME).is_none());
}

const UNLOCK_WHY: &str = "It is locked. Unlock it first.";
const LONG_WHY: &str = "Choose the branch you want to compare against before starting";
const PR: &str = "Create pull request…";

/// A menu frame of five items (with `reasons`, four of them blocked or waiting for setup,
/// Compare… blocked by `compare_why`) in a `size` window, tooltips without motion; the state is
/// the messages sent and the frame's width.
fn menu_in(dark: bool, reasons: bool, compare_why: &'static str, size: egui::Vec2) -> Harness<'static, (Vec<Msg>, f32)> {
    crate::ui::snapshot_check::builder().with_size(size).build_ui_state(
        move |ui, (msgs, width): &mut (Vec<Msg>, f32)| {
            paint_theme(ui, dark);
            tooltip::configure(ui.ctx(), true);
            let r = egui::Frame::menu(ui.style()).show(ui, |ui| {
                menu_style(ui.style_mut());
                let why = |w| reasons.then_some(w);
                menu_item(msgs, ui, "Copy path", Msg::Copy("path".into()));
                menu_item_if(msgs, ui, "Unlock", why(UNLOCK_WHY), Msg::Copy("unlock".into()));
                menu_item_if(msgs, ui, "Compare…", why(compare_why), Msg::Copy("compare".into()));
                menu_item_if(msgs, ui, PR, why("Connect GitHub in Preferences first"), Msg::Copy("pr".into()));
                let open = Msg::Tools(ToolsMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into())));
                menu_item_setup(msgs, ui, Lang::En, OPEN_IN_DIFF_TOOL, why(Lang::En.tr("tools.diff_setup.tip")), open);
            });
            *width = r.response.rect.width();
        },
        (Vec::new(), 0.0),
    )
}

fn menu(dark: bool, reasons: bool) -> Harness<'static, (Vec<Msg>, f32)> {
    menu_in(dark, reasons, LONG_WHY, egui::vec2(480.0, 220.0))
}

#[test]
fn menu_disabled_info() {
    let mut plain = menu(true, false);
    plain.run();
    let plain_w = plain.state().1;
    let mut short = menu_in(true, true, "No", egui::vec2(480.0, 220.0));
    short.run();
    let short_w = short.state().1;
    for (dark, name) in [(true, "menu_disabled_info"), (false, "menu_disabled_info_light")] {
        let mut h = menu(dark, true);
        h.run();
        h.check_snapshot(name);
        let w = h.state().1;
        assert_eq!(w, short_w, "a long reason doesn't widen the menu");
        assert!(w > plain_w, "the info marks and Set up… take room: {w} vs {plain_w}");
        assert!(h.get_by_label("Unlock").is_disabled(), "found by its exact label");
        assert!(h.get_by_label(PR).is_disabled());
        assert!(!h.get_by_label("Copy path").is_disabled());
        h.get_by_label("Unlock").click();
        h.run();
        assert!(h.state().0.is_empty(), "a disabled item sends nothing");
        h.get_by_label("Unlock").hover();
        h.run();
        h.run();
        assert!(h.query_by_label(UNLOCK_WHY).is_some(), "the whole reason on hover");
        assert!(!h.get_by_label(OPEN_IN_DIFF_TOOL).is_disabled());
        h.get_by_label(OPEN_IN_DIFF_TOOL).click();
        h.run();
        assert!(matches!(&h.state().0[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));
    }
}

#[test]
fn menu_disabled_tooltip() {
    let mut h = menu_in(true, true, LONG_WHY, egui::vec2(520.0, 300.0));
    h.run();
    h.get_by_label("Compare…").hover();
    h.run();
    h.run();
    h.check_snapshot("menu_disabled_tooltip");
    assert!(h.query_by_label(LONG_WHY).is_some(), "the whole reason on hover");
    let why = bounds(&h, LONG_WHY);
    assert!(why.width() <= 340.0, "wrapped in the tooltip: {why:?}");
}

#[test]
fn menu_disabled_focus_shows_why() {
    let mut h = menu(true, true);
    h.run();
    h.get_by_label("Compare…").focus();
    h.run();
    assert!(h.get_by_label("Compare…").is_focused());
    assert!(h.query_by_label(LONG_WHY).is_some(), "the reason under the focused item");
    h.get_by_label("Unlock").focus();
    h.run();
    assert!(h.query_by_label(UNLOCK_WHY).is_some());
    assert!(h.query_by_label(LONG_WHY).is_none(), "one tooltip at a time");
    assert_eq!(h.get_by_label("Unlock").description().as_deref(), Some(UNLOCK_WHY));
}
