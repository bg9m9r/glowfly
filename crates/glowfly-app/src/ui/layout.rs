//! The window zoom, applied to egui from the settings each frame, and the status bar's way back
//! from a zoom or a hidden toolbar. Which panels are shown is in `settings` too; `frame_layout.rs`
//! draws them, and a hidden side panel's edge strip is in `panel_rails.rs`.

use super::theme::color;
use crate::keymap::BindingId;
use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::layout::Panel;
use eframe::egui::{self, RichText};
use glowfly_core::settings::DEFAULT_ZOOM_PERCENT;

/// At start-up, before the first frame: hands the zoom keys to the keymap (egui's own would
/// still zoom on Ctrl+-/Ctrl+= after they are rebound) and applies the saved zoom.
pub(super) fn start(ctx: &egui::Context, st: &AppState) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    apply_zoom(ctx, st);
}

/// Makes egui's zoom the settings' (it takes effect from the next pass). The exact compare is
/// safe: both come from the same integer percent.
pub(super) fn apply_zoom(ctx: &egui::Context, st: &AppState) {
    let z = st.settings.zoom_factor();
    if ctx.zoom_factor() != z {
        ctx.set_zoom_factor(z);
    }
}

/// The status bar's right-hand items, inside its right-to-left layout: the zoom level while it
/// isn't 100% (a click resets it), then Show toolbar while the toolbar is hidden.
pub(super) fn status_items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let level = st.settings.zoom_level();
    if level != DEFAULT_ZOOM_PERCENT {
        let chip = egui::Button::new(RichText::new(t.trf("status.zoom", &[("percent", &level)])).color(color::weak())).frame(false);
        if ui.add(chip).on_hover_text(st.keymap.tip(t.tr("status.zoom_reset_tip"), BindingId::ZoomReset)).clicked() {
            msgs.push(Msg::Layout(LayoutMsg::ZoomReset));
        }
    }
    if !st.settings.show_toolbar
        && ui
            .small_button(t.tr("status.show_toolbar"))
            .on_hover_text(st.keymap.tip(t.tr("status.show_toolbar_tip"), BindingId::ToggleToolbar))
            .clicked()
    {
        msgs.push(Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: true }));
    }
}

#[cfg(test)]
#[path = "snapshot_tests_layout.rs"]
mod snapshot_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;
    use std::time::Instant;

    fn state(zoom_percent: u16) -> AppState {
        AppState::new(Settings { zoom_percent, ..Settings::default() }, Vec::new(), Instant::now())
    }

    #[test]
    fn start_turns_off_egui_zoom_keys_and_applies_the_saved_zoom() {
        let ctx = egui::Context::default();
        start(&ctx, &state(125));
        assert!(!ctx.options(|o| o.zoom_with_keyboard));
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        assert_eq!(ctx.zoom_factor(), 1.25);
        apply_zoom(&ctx, &state(100));
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        assert_eq!(ctx.zoom_factor(), 1.0);
    }
}
