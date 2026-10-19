//! The Rename tab dialog (`Modal::RenameTab`): the name the tab shows instead of its folder's.
//! `update` cleans the text again before setting it.

use super::theme::color;
use crate::msg::{Msg, TabsMsg};
use crate::state::tabs::{MAX_TAB_LABEL, RenameTabForm};
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};

/// Draws the dialog; returns true when it should close. Its field keeps the keyboard focus
/// unless `focus` is false (a credential prompt is open above it).
pub fn rename(ui: &mut egui::Ui, form: &mut RenameTabForm, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Rename tab");
    ui.label(RichText::new(trellis_core::native_path::shown(&form.path)).color(color::weak()));
    ui.add_space(6.0);
    let field = egui::TextEdit::singleline(&mut form.text).hint_text(form.folder.as_str()).char_limit(MAX_TAB_LABEL);
    let r = controls::field(ui, field.desired_width(f32::INFINITY));
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus {
        r.request_focus();
    }
    ui.add_space(4.0);
    let help = "Shown in the tab bar, the Window menu and the window title. Leave it empty to use the folder name.";
    ui.label(RichText::new(help).color(color::weak()));
    ui.add_space(10.0);
    let set = |label: Option<String>| Msg::Tabs(TabsMsg::SetTabLabel { tab: form.tab, label });
    ui.horizontal(|ui| {
        let changed = form.changed();
        if ui.add_enabled(changed, egui::Button::new("Rename")).clicked() || (submit && changed) {
            msgs.push(set(form.label()));
        }
        if ui.add_enabled(form.current.is_some(), egui::Button::new("Use folder name")).clicked() {
            msgs.push(set(None));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
