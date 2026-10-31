//! The menus' mnemonics on screen and from the keyboard: each title's letter
//! (`state/menu_keys.rs` picks it from the translated title, past the letters Alt keys of the
//! keymap and custom commands use), the title with that letter underlined while Alt alone is
//! held or a menu opened from the keyboard is open, and taking an Alt+letter press out of the
//! frame's input (with the text it would type) for the view that opens the menu.

use crate::keymap::KeyName;
use crate::state::AppState;
use crate::state::menu_keys::{self, Mnemonic, TopMenu};
use eframe::egui::{self, Color32, Event, InputState, Modifiers, Stroke, TextFormat, TextStyle, WidgetText, text::LayoutJob};
use glowfly_core::i18n::Lang;

/// Alt alone: no Ctrl, Shift or Cmd (AltGr reads as Ctrl+Alt on Windows).
fn alt_only(m: Modifiers) -> bool {
    m.alt && !m.ctrl && !m.shift && !m.command && !m.mac_cmd
}

/// Whether Alt alone is held now.
fn alt_held(ctx: &egui::Context) -> bool {
    ctx.input(|i| alt_only(i.modifiers))
}

/// Each menu's mnemonic, in `TopMenu::ALL` order, in the current language.
pub(crate) fn marks(st: &AppState) -> [Option<Mnemonic>; 5] {
    let t = st.settings.lang();
    menu_keys::mnemonics(TopMenu::ALL.map(|m| m.title(t)), &menu_keys::reserved(&st.keymap, &st.custom_commands.keys))
}

/// Whether the titles show their letters now.
pub(super) fn shown(ctx: &egui::Context, st: &AppState) -> bool {
    st.view.chrome.mnemonics() && (alt_held(ctx) || st.view.menu_keys.keyboard)
}

/// `m`'s title; with `show`, its mnemonic underlined. The text, and so the widget's id and
/// accessible name, is the same either way.
pub(super) fn title(ui: &egui::Ui, t: Lang, m: TopMenu, marks: &[Option<Mnemonic>; 5], show: bool) -> WidgetText {
    let text = m.title(t);
    let mark = TopMenu::ALL.iter().position(|&x| x == m).and_then(|i| marks[i]);
    let Some(mark) = mark.filter(|_| show) else { return text.into() };
    let Some((start, c)) = text.char_indices().nth(mark.index) else { return text.into() };
    let end = start + c.len_utf8();
    // Placeholder colours: the button paints the text, and the line under it, in its own colour.
    let plain = TextFormat { font_id: TextStyle::Button.resolve(ui.style()), color: Color32::PLACEHOLDER, ..Default::default() };
    let marked = TextFormat { underline: Stroke::new(1.0_f32, Color32::PLACEHOLDER), ..plain.clone() };
    let mut job = LayoutJob::default();
    job.append(&text[..start], 0.0, plain.clone());
    job.append(&text[start..end], 0.0, marked);
    job.append(&text[end..], 0.0, plain);
    job.into()
}

/// Takes the first Alt+letter press of a menu's mnemonic out of `i`'s events, with any text of
/// that letter typed this frame (so a focused field doesn't get it), and returns its menu.
pub(crate) fn take(i: &mut InputState, marks: &[Option<Mnemonic>; 5]) -> Option<TopMenu> {
    let (n, letter, menu) = i.events.iter().enumerate().find_map(|(n, e)| {
        let Event::Key { key, pressed: true, modifiers, .. } = e else { return None };
        if !alt_only(*modifiers) {
            return None;
        }
        let KeyName::Letter(c) = KeyName::parse(key.name())? else { return None };
        let c = c.to_ascii_lowercase();
        menu_keys::menu_for(c, marks).map(|m| (n, c, m))
    })?;
    i.events.remove(n);
    i.events.retain(|e| {
        let Event::Text(s) = e else { return true };
        let mut chars = s.chars();
        !matches!((chars.next(), chars.next()), (Some(ch), None) if ch.to_ascii_lowercase() == letter)
    });
    Some(menu)
}
