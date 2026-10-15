//! The palette with recently run commands listed first, and the toolbar's Commands button: its
//! click and its tooltip. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::keymap::{self, BindingId};
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::state::palette::{Palette, PaletteCommand as C, shown};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

#[test]
fn palette_recent_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-recent");
    for cmd in [C::Refresh, C::Theme(ThemeChoice::Light)] {
        st.palette_recent.record(cmd.recall_key().unwrap());
    }
    st.palette = Some(Palette::default());
    let first: Vec<String> = shown(&st, "").into_iter().take(2).map(|s| s.entry.label).collect();
    assert_eq!(first, ["Theme: Light", "Refresh"]);
    render("palette_recent", st, true);
    remove_fixture(&dir);
}

#[test]
fn toolbar_palette_button() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "toolbar-palette-button");
    assert!(st.settings.toolbar_labels);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Command palette").click();
    h.run();
    let toggles = h.state().1.iter().filter(|m| matches!(m, Msg::Palette(PaletteMsg::TogglePalette))).count();
    assert_eq!(toggles, 1);
    remove_fixture(&dir);
}

#[test]
fn toolbar_palette_tooltip_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "toolbar-palette-tooltip");
    st.settings.toolbar_labels = false;
    let mut h = harness(st, false);
    h.run();
    h.get_by_label("Command palette").hover();
    for _ in 0..4 {
        h.run();
    }
    let want = format!("Command palette ({})", keymap::hint(BindingId::Palette));
    assert!(h.query_by_label_contains(&want).is_some(), "the tooltip shows {want}");
    h.check_snapshot("toolbar_palette_tooltip_light");
    remove_fixture(&dir);
}
