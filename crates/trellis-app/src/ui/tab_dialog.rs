//! The Rename tab dialog (`Modal::RenameTab`): the name the tab shows instead of its folder's.
//! `update` cleans the text again before setting it.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{Msg, TabsMsg};
use crate::state::tabs::{MAX_TAB_LABEL, RenameTabForm};
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};

/// Draws the dialog; returns true when it should close. Its field takes the keyboard focus
/// whenever nothing else in the dialog has it, unless `cx.keyboard` is false (a credential
/// prompt is open above it).
pub fn rename(ui: &mut egui::Ui, cx: Cx, form: &mut RenameTabForm, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, "Rename tab").subtitle(trellis_core::native_path::shown(&form.path)).size(Size::Small).begin(ui);
    let field = egui::TextEdit::singleline(&mut form.text).hint_text(form.folder.as_str()).char_limit(MAX_TAB_LABEL);
    let r = controls::field(ui, field.desired_width(f32::INFINITY));
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    ui.add_space(4.0);
    let help = "Shown in the tab bar, the Window menu and the window title. Leave it empty to use the folder name.";
    ui.label(RichText::new(help).color(color::weak()));
    let set = |label: Option<String>| Msg::Tabs(TabsMsg::SetTabLabel { tab: form.tab, label });
    let footer = Footer::new()
        .extra(buttons::secondary("Use folder name").enabled(form.current.is_some()))
        .cancel()
        .primary(buttons::primary("Rename").enabled(form.changed()))
        .submit(submit);
    let out = d.finish(ui, footer);
    if out.primary {
        msgs.push(set(form.label()));
    }
    if out.extra == Some(0) {
        msgs.push(set(None));
    }
    out.dismiss
}
