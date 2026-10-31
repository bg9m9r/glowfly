//! The links at the right of the issue and pull request panels' headers, after their Close:
//! Copy link and Open in browser as buttons, or, when they would leave the title less than
//! `TITLE_MIN`, one ⋯ button beside Close with Open in browser and Copy link in its menu.

use super::widgets::Icon;
use super::widgets::buttons;
use super::widgets::ghost::{self, icon_tip};
use super::widgets::menu::{self, popup_below};
use crate::keymap::Keymap;
use crate::msg::Msg;
use crate::state::tip::Tip;
use eframe::egui::{self, PopupCloseBehavior, Ui};
use glowfly_core::i18n::Lang;

/// The least width the title keeps before the links fold into the ⋯ menu.
pub const TITLE_MIN: f32 = 240.0;
/// The room the header gives the panel's icon before its label and title.
const ICON_ROOM: f32 = 8.0 + 16.0;

/// What the header's links open and copy.
pub struct Links<'a> {
    pub url: &'a str,
    /// Open in browser, or the service's own wording.
    pub open_label: &'a str,
    /// Why the links are off: the service sent no link.
    pub no_link: &'a str,
    /// The width the header's left side takes before its title (its label, a chip).
    pub lead_w: f32,
}

/// The links, in the header's right-to-left layout after Close.
pub fn show(ui: &mut Ui, t: Lang, keys: &Keymap, links: Links<'_>, msgs: &mut Vec<Msg>) {
    let has_link = !links.url.is_empty();
    let why = (!has_link).then_some(links.no_link);
    let copy = || buttons::secondary(t.tr("panel.header.copy_link")).enabled(has_link).why(why);
    let open = || buttons::secondary(links.open_label).enabled(has_link).why(why);
    let gap = ui.spacing().item_spacing.x;
    let buttons_w = copy().desired_width(ui) + open().desired_width(ui) + 2.0 * gap;
    if ui.available_width() - buttons_w >= ICON_ROOM + links.lead_w + TITLE_MIN {
        if ui.add(copy()).clicked() {
            msgs.push(Msg::Copy(links.url.to_owned()));
        }
        if ui.add(open()).clicked() {
            msgs.push(Msg::OpenUrl(links.url.to_owned()));
        }
        return;
    }
    let id = ui.make_persistent_id("panel-header-links");
    let name = t.tr("panel.header.more.name");
    let r = icon_tip(ghost::ghost(ui, Some(Icon::More), None, name, true), keys, t, Tip::new(name));
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    popup_below(ui, id, &r, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(200.0);
        if menu::item(links.open_label).blocked(why).show(ui).clicked() {
            msgs.push(Msg::OpenUrl(links.url.to_owned()));
        }
        if menu::item(t.tr("panel.header.copy_link")).blocked(why).show(ui).clicked() {
            msgs.push(Msg::Copy(links.url.to_owned()));
        }
    });
}

/// The width of `text` in the body font, for a header's `lead_w`.
pub fn text_w(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::PLACEHOLDER).size().x
}
