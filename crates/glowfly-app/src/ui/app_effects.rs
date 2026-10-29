//! The effects that need the window (`Effect::Ui`), carried out by the shell: the clipboard and
//! the Linux PRIMARY selection, the title, attention, closing, and the drawn frame's window
//! commands.

use super::GlowflyApp;
use crate::effect::UiEffect;
use crate::msg::Msg;
use crate::platform::primary_selection::{PasteFrom, middle_click_reads};
use crate::terminal::TerminalMsg;
use eframe::egui;
use glowfly_core::terminal::TermInput;

impl GlowflyApp {
    pub(super) fn ui_effect(&mut self, ctx: &egui::Context, effect: UiEffect) {
        match effect {
            UiEffect::CopyText(text) => ctx.copy_text(text),
            UiEffect::SetTitle(title) => ctx.send_viewport_cmd(egui::ViewportCommand::Title(title)),
            UiEffect::RequestAttention => request_attention(ctx),
            UiEffect::CloseWindow => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            // In the pass that saw the close request, so eframe keeps the window.
            UiEffect::CancelClose => ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose),
            UiEffect::RequestPaste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            UiEffect::SetPrimary(text) => self.primary.set(text),
            UiEffect::PastePrimary => match middle_click_reads(self.primary.available()) {
                // Nothing selected anywhere pastes nothing, as in other terminals.
                PasteFrom::Primary => {
                    if let Some(text) = self.primary.get() {
                        self.dispatch(ctx, Msg::Terminal(TerminalMsg::Input(vec![TermInput::Paste(text)])));
                    }
                }
                PasteFrom::Clipboard => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            },
            UiEffect::Window(cmd) => match crate::ui::window_frame::viewport_cmd(cmd) {
                Some(c) => ctx.send_viewport_cmd(c),
                // The system menu, the only window action without a viewport command.
                None => {
                    if let Some(n) = &self.native {
                        n.show_system_menu_at_cursor();
                    }
                }
            },
            UiEffect::FocusWindow => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                request_attention(ctx);
            }
        }
    }
}

/// Flashes the taskbar entry while the window isn't focused.
fn request_attention(ctx: &egui::Context) {
    if !ctx.input(|i| i.focused) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
    }
}
