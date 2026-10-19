//! Preferences > Keyboard: every binding by group with its keys, a filter, and Reset all (asked
//! for first). Each row (`keyboard_row.rs`) changes, adds, removes and resets keys. The page
//! edits only its filter; everything else is a `KeysMsg` for update/keys.rs.

use super::{help, keyboard_row, primary_button, warn};
use crate::keymap::Keymap;
use crate::msg::{KeysMsg, Msg};
use crate::state::keys_page::KeysPage;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{Align, Id, Layout, RichText, TextEdit, Ui, vec2};
use trellis_core::i18n::Lang;

/// The widest the list gets, so a row's keys stay near its label.
const MAX_W: f32 = 760.0;

const HELP: &str =
    "Click a key to change it, or Add to give an action another key. Fixed keys work in one place only and can't be changed.";

pub fn keyboard_page(ui: &mut Ui, t: Lang, page: &mut KeysPage, keys: &Keymap, msgs: &mut Vec<Msg>) {
    ui.set_max_width(MAX_W);
    help(ui, HELP);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let field = TextEdit::singleline(&mut page.filter)
            .id(Id::new("trellis-keys-filter"))
            .hint_text("Filter by action or key")
            .desired_width(320.0)
            .margin(vec2(8.0, 5.0));
        controls::field(ui, field);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let why = (!keys.customized()).then_some("Every shortcut has its default key");
            let reset = buttons::secondary("Reset all to defaults").enabled(keys.customized() && !page.confirm_reset_all).why(why);
            if ui.add(reset).clicked() {
                msgs.push(Msg::Keys(KeysMsg::AskResetAll));
            }
        });
    });
    if page.confirm_reset_all {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            warn(ui, "Put every shortcut back to its default key?");
            ui.add_space(8.0);
            if primary_button(ui, "Reset all", true, None).clicked() {
                msgs.push(Msg::Keys(KeysMsg::ResetAll));
            }
            if ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
                msgs.push(Msg::Keys(KeysMsg::CancelResetAll));
            }
        });
    }
    let groups = keys.filter(&page.filter);
    if groups.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No shortcuts match").color(color::weak()));
    }
    for (group, rows) in groups {
        ui.add_space(14.0);
        ui.label(RichText::new(group.title()).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak()));
        ui.add_space(2.0);
        for b in rows {
            keyboard_row::row(ui, t, b, keys, page, msgs);
        }
    }
}
