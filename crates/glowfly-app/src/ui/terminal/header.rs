//! The terminal panel's 40 px header, drawn on the panel surface with a hairline under it: the
//! tab strip with + and the shell picker on the left (tabs.rs), and quiet icon buttons on the
//! right: Clear, Kill terminal, More (Kill all terminals, Rename terminal) | Terminal settings,
//! Maximize or Restore panel, Hide panel. Every control sits on the header's centre line.

use super::tabs;
use crate::keymap::BindingId;
use crate::msg::{Msg, PrefsMsg};
use crate::state::terminal::{self, SessionStatus, TermSession};
use crate::state::tip::Tip;
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use crate::ui::common::{menu_danger, menu_item_if};
use crate::ui::theme::color;
use crate::ui::widgets::Icon;
use crate::ui::widgets::ghost::{self, divider, ghost, icon_tip};
use crate::ui::widgets::menu::{popup_below, separator};
use eframe::egui::{self, Align, Layout, PopupCloseBehavior, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;

pub(super) const HEADER_H: f32 = 40.0;
/// The panel's side margin, which the hairline runs through.
pub(super) const SIDE: f32 = 8.0;
/// The right cluster: six buttons, the gaps between its seven items and the divider.
const RIGHT_W: f32 = 6.0 * ghost::HEIGHT + 6.0 * ghost::SPACE + ghost::DIVIDER_W;

/// Draws the header across the top of `ui`.
pub(super) fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let right = Rect::from_min_max(pos2(rect.right() - RIGHT_W, rect.top()), rect.max);
    let mut r = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
    r.spacing_mut().item_spacing.x = ghost::SPACE;
    buttons(&mut r, st, msgs);
    let left = Rect::from_min_max(rect.min, pos2(right.left() - SIDE, rect.bottom()));
    let mut l = ui.new_child(UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)));
    tabs::strip(&mut l, st, rect.center().y, msgs);
    let x = rect.x_range().expand(SIDE);
    ui.painter().hline(x, rect.bottom() - 0.5, Stroke::new(1.0_f32, color::border()));
}

/// The right-hand buttons, drawn right to left.
fn buttons(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let session = terminal::active(st);
    let push = |msgs: &mut Vec<Msg>, m| msgs.push(Msg::Terminal(m));

    let hide = t.tr("terminal.header.hide");
    if icon_tip(ghost(ui, Some(Icon::ChevronDown), None, hide, true), keys, t, Tip::new(hide).keys(BindingId::ToggleTerminal)).clicked() {
        push(msgs, TerminalMsg::Hide);
    }
    let (icon, max) = if st.terminal.maximized {
        (Icon::PanelRestore, t.tr("terminal.header.restore"))
    } else {
        (Icon::PanelMaximize, t.tr("terminal.header.maximize"))
    };
    if icon_tip(ghost(ui, Some(icon), None, max, true), keys, t, Tip::new(max)).clicked() {
        push(msgs, TerminalMsg::ToggleMaximize);
    }
    let settings = t.tr("terminal.header.settings");
    let tip = Tip::new(settings).body(t.tr("terminal.header.settings.tip"));
    if icon_tip(ghost(ui, Some(Icon::Gear), None, settings, true), keys, t, tip).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Terminal)));
    }
    divider(ui);
    more(ui, st, session, t, msgs);

    let kill = t.tr("terminal.header.kill");
    let blocked = match session {
        None => Some(t.tr("terminal.header.no_terminal")),
        Some(s) if s.status == SessionStatus::Held => Some(t.tr("terminal.header.held")),
        Some(s) if !s.alive() => Some(t.tr("terminal.header.ended")),
        Some(_) => None,
    };
    if button(ui, keys, t, Icon::Stop, kill, t.tr("terminal.header.kill.tip"), blocked).clicked() {
        push(msgs, TerminalMsg::Kill);
    }
    let clear = t.tr("terminal.header.clear");
    let blocked = match session {
        None => Some(t.tr("terminal.header.no_terminal")),
        Some(s) if s.screen.alternate() => Some(t.tr("terminal.header.full_screen")),
        Some(_) => None,
    };
    if button(ui, keys, t, Icon::Eraser, clear, t.tr("terminal.header.clear.tip"), blocked).clicked() {
        push(msgs, TerminalMsg::Clear);
    }
}

/// A ghost icon button named `name`, explaining itself with `body`, or with `blocked` (and
/// disabled) when it can't act.
fn button(ui: &mut Ui, keys: &crate::keymap::Keymap, t: Lang, icon: Icon, name: &str, body: &str, blocked: Option<&str>) -> egui::Response {
    let resp = ghost(ui, Some(icon), None, name, blocked.is_none());
    icon_tip(resp, keys, t, Tip::new(name).body(blocked.unwrap_or(body)))
}

/// More terminal actions: Rename terminal, then Kill all terminals… (it asks) set apart.
fn more(ui: &mut Ui, st: &AppState, session: Option<&TermSession>, t: Lang, msgs: &mut Vec<Msg>) {
    let name = t.tr("terminal.header.more");
    let id = ui.make_persistent_id("terminal-more");
    let r = icon_tip(ghost(ui, Some(Icon::More), None, name, true), &st.keymap, t, Tip::new(name));
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    let alive = st.snap.as_ref().map_or(0, |s| st.terminal.sessions_for(&s.workdir).filter(|t| t.alive()).count());
    popup_below(ui, id, &r, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(200.0);
        let before = msgs.len();
        let none = session.is_none().then(|| t.tr("terminal.header.no_terminal"));
        let rename = Msg::Terminal(TerminalMsg::StartRename(session.map_or(0, |s| s.id)));
        menu_item_if(msgs, ui, t.tr("terminal.header.rename"), none, rename);
        separator(ui);
        let few = (alive < 2).then(|| t.tr("terminal.header.fewer_running"));
        menu_danger(msgs, ui, t.tr("terminal.header.kill_all"), few, Msg::Terminal(TerminalMsg::AskKillAll));
        if msgs.len() > before {
            ui.memory_mut(|m| m.close_popup());
        }
    });
}
