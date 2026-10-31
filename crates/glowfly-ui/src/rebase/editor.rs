//! The rebase panel's inline editors, drawn under their row: a commit's new message, the
//! message of the commit a squash group combines into (under the group's last row), and an
//! exec step's command. Each takes the keyboard focus when it opens, and ends in a small primary
//! Save or Update message and a small Cancel (the combined message also has Use git's message);
//! Save stays disabled, saying why, while the command is empty.

use super::UiCache;
use super::rows::{MARGIN, SHA_X};
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::{Editing, RebasePanel};
use crate::ui::spell_edit::spell_text_edit;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CornerRadius, Id, Response, RichText, Stroke, Ui};
use glowfly_core::i18n::Lang;

const COMBINED_LABEL: &str = "Message of the combined commit";
const COMMAND_HINT: &str = "Shell command, e.g. cargo test";

/// The open editor, under the SHA and summary and as wide as the row.
pub(super) fn show(ui: &mut Ui, t: Lang, panel: &mut RebasePanel, cache: &mut UiCache, spell: bool, msgs: &mut Vec<Msg>) {
    let Some(editing) = panel.editing else { return };
    let enabled = panel.editable();
    // The editor that had focus last frame: a new one takes it once.
    let open_id = Id::new("rebase-editor-open");
    let fresh = ui.data(|d| d.get_temp::<Option<Editing>>(open_id)) != Some(Some(editing));
    ui.data_mut(|d| d.insert_temp(open_id, Some(editing)));
    let width = ui.available_width() - 2.0 * MARGIN - SHA_X;
    let send = |msgs: &mut Vec<Msg>, m: RebaseMsg| msgs.push(Msg::Rebase(m));
    ui.horizontal(|ui| {
        ui.add_space(MARGIN + SHA_X);
        ui.vertical(|ui| {
            ui.set_width(width);
            egui::Frame::new()
                .fill(color::panel())
                .stroke(Stroke::new(1.0_f32, color::border()))
                .corner_radius(CornerRadius::same(radius::SM))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        let field = match editing {
                            Editing::Message(_) => {
                                ui.label(RichText::new("New commit message").size(type_scale::SMALL).color(color::weak()));
                                spell_text_edit(ui, &mut panel.draft, "Commit message", 5, &mut cache.speller, spell)
                            }
                            Editing::Combined(_) => {
                                ui.label(RichText::new(COMBINED_LABEL).size(type_scale::SMALL).color(color::weak()));
                                spell_text_edit(ui, &mut panel.draft, "Commit message", 6, &mut cache.speller, spell)
                            }
                            Editing::Command(_) => {
                                ui.label(RichText::new("Command").size(type_scale::SMALL).color(color::weak()));
                                command_field(ui, &mut panel.draft)
                            }
                        };
                        if fresh {
                            field.request_focus();
                        }
                        ui.horizontal(|ui| {
                            let save = match editing {
                                Editing::Message(_) | Editing::Combined(_) => buttons::primary("Update message").small(),
                                Editing::Command(_) => {
                                    let typed = !panel.draft.trim().is_empty();
                                    let why = (!typed).then_some(t.tr("rebase.editor.command_empty.why"));
                                    buttons::primary(t.tr("rebase.editor.save.button")).small().enabled(typed).why(why)
                                }
                            };
                            if ui.add(save).clicked() {
                                send(msgs, RebaseMsg::RebaseSaveMessage);
                            }
                            if matches!(editing, Editing::Combined(_))
                                && ui
                                    .add(buttons::secondary("Use git's message").small())
                                    .on_hover_text("Forget the typed message")
                                    .clicked()
                            {
                                send(msgs, RebaseMsg::RebaseResetCombined);
                            }
                            if ui.add(buttons::secondary(t.tr("common.cancel.button")).small()).clicked() {
                                send(msgs, RebaseMsg::RebaseCancelMessage);
                            }
                        });
                    });
                });
        });
    });
}

/// Forgets which editor was open, so the next one takes the focus again.
pub(super) fn forget(ui: &Ui) {
    ui.data_mut(|d| d.insert_temp::<Option<Editing>>(Id::new("rebase-editor-open"), None));
}

/// The one-line command field.
fn command_field(ui: &mut Ui, draft: &mut String) -> Response {
    controls::field(
        ui,
        egui::TextEdit::singleline(draft).hint_text(COMMAND_HINT).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY),
    )
}
