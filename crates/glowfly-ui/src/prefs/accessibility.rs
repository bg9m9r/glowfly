//! Preferences > Accessibility: every accessibility option in one place. Vision: the interface,
//! diff and terminal text sizes (the rows their own pages draw, `appearance::zoom_row`,
//! `diff_viewer::font_size_row` and `terminal::font_size_row`), High contrast and the branch
//! colours. Motion: reduce motion and the blinking cursor. Keyboard: the focus indicator's
//! strength, and links to Preferences > Keyboard and the shortcut sheet. Screen readers: whether
//! notifications and the running job are announced. Timing: how long notifications stay and the
//! tooltip delay.
//!
//! Interface size, High contrast and Reduce motion each have a second check in their row that
//! follows the system's setting too (`follow_check`; the shell reads the system's,
//! `state/system_a11y.rs`).
//!
//! Every control is named for screen readers by its row's label (and a checkbox's text too),
//! with the row's help as its description (`name`). The High contrast check only asks (update
//! saves and restores the theme); the rest edit the settings in place.

use super::{combo_response, help, row, section};
use crate::keymap::Keymap;
use crate::msg::{Msg, PrefsMsg};
use crate::state::PrefPage;
use crate::ui::access;
use crate::ui::theme::{color, controls};
use crate::ui::widgets::buttons;
use eframe::egui::{Response, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{ColourPalette, FocusStyle, Settings, ToastTime, TooltipDelay};

/// What the page shows besides the settings.
pub struct A11yView<'a> {
    pub keys: &'a Keymap,
    /// The shortcut sheet is open (over Preferences).
    pub sheet_open: bool,
    /// The system has a text size to follow (not macOS): the Interface size row shows its
    /// follow check.
    pub text_size_followable: bool,
}

pub fn accessibility_page(ui: &mut Ui, s: &mut Settings, view: &A11yView, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    section(ui, t.tr("prefs.a11y.vision.section"));
    super::appearance::zoom_row(ui, t, s, view.keys, view.text_size_followable);
    super::diff_viewer::font_size_row(ui, s, t.tr("prefs.a11y.diff_size.label"));
    super::terminal::font_size_row(ui, t, &mut s.terminal, t.tr("prefs.a11y.terminal_size.label"));
    let on = s.theme.is_high_contrast();
    let (label, check, text) = (t.tr("prefs.a11y.contrast.label"), t.tr("prefs.a11y.contrast.check"), t.tr("prefs.a11y.contrast.help"));
    let mut wanted = on;
    let follow = (&mut s.a11y.follow_system_contrast, t.tr("prefs.a11y.contrast.follow"));
    if checked_row(ui, label, &mut wanted, check, text, Some(follow)).changed() {
        msgs.push(Msg::Prefs(PrefsMsg::SetHighContrast { on: wanted, dark: color::current().dark }));
    }
    let label = t.tr("prefs.a11y.colors.label");
    row(ui, label, |ui| {
        let options = [
            (ColourPalette::Standard, t.tr("prefs.a11y.colors.standard")),
            (ColourPalette::ColourBlind, t.tr("prefs.a11y.colors.color_blind")),
        ];
        let text = t.tr("prefs.a11y.colors.help");
        name(&combo_response(ui, "colour_palette", &mut s.colour_palette, &options), label, text);
        help(ui, text);
    });

    section(ui, t.tr("prefs.a11y.motion.section"));
    let (label, check, text) =
        (t.tr("prefs.a11y.reduce_motion.label"), t.tr("prefs.a11y.reduce_motion.check"), t.tr("prefs.a11y.reduce_motion.help"));
    let follow = (&mut s.a11y.follow_system_motion, t.tr("prefs.a11y.reduce_motion.follow"));
    checked_row(ui, label, &mut s.reduce_motion, check, text, Some(follow));
    let (label, check, text) = (t.tr("prefs.a11y.blink.label"), t.tr("prefs.a11y.blink.check"), t.tr("prefs.a11y.blink.help"));
    checked_row(ui, label, &mut s.a11y.cursor_blink, check, text, None);

    section(ui, t.tr("prefs.a11y.keyboard.section"));
    let label = t.tr("prefs.a11y.focus.label");
    row(ui, label, |ui| {
        let options = [(FocusStyle::Standard, t.tr("prefs.a11y.focus.standard")), (FocusStyle::Strong, t.tr("prefs.a11y.focus.strong"))];
        let text = t.tr("prefs.a11y.focus.help");
        name(&combo_response(ui, "focus_style", &mut s.a11y.focus_style, &options), label, text);
        help(ui, text);
    });
    shortcuts_row(ui, t, view.sheet_open, msgs);

    section(ui, t.tr("prefs.a11y.screen_readers.section"));
    let (label, check, text) = (t.tr("prefs.a11y.announce.label"), t.tr("prefs.a11y.announce.check"), t.tr("prefs.a11y.announce.help"));
    checked_row(ui, label, &mut s.a11y.announce_jobs, check, text, None);

    section(ui, t.tr("prefs.a11y.timing.section"));
    let label = t.tr("prefs.a11y.toast_time.label");
    row(ui, label, |ui| {
        let options = [
            (ToastTime::Standard, t.tr("prefs.a11y.toast_time.standard")),
            (ToastTime::Longer, t.tr("prefs.a11y.toast_time.longer")),
            (ToastTime::UntilClosed, t.tr("prefs.a11y.toast_time.until_closed")),
        ];
        let text = t.tr("prefs.a11y.toast_time.help");
        name(&combo_response(ui, "toast_time", &mut s.a11y.toast_time, &options), label, text);
        help(ui, text);
    });
    let label = t.tr("prefs.a11y.tooltip_delay.label");
    row(ui, label, |ui| {
        let options = [
            (TooltipDelay::Quick, t.tr("prefs.a11y.tooltip_delay.quick")),
            (TooltipDelay::Slow, t.tr("prefs.a11y.tooltip_delay.slow")),
            (TooltipDelay::Instant, t.tr("prefs.a11y.tooltip_delay.instant")),
        ];
        name(&combo_response(ui, "tooltip_delay", &mut s.a11y.tooltip_delay, &options), label, "");
    });
}

/// Names `resp` `label` for screen readers, described by `text` (the row's help) when there is
/// one.
pub fn name(resp: &Response, label: &str, text: &str) {
    access::name_field(resp, label);
    access::from_label(resp);
    if !text.is_empty() {
        access::describe(resp, text);
    }
}

/// A row with a checkbox, then `follow`'s "Also when the system…" check when given, and its
/// help; each checkbox is named by the row's label and its text. Returns the first checkbox's
/// response.
fn checked_row(ui: &mut Ui, label: &str, value: &mut bool, check: &str, text: &str, follow: Option<(&mut bool, &str)>) -> Response {
    let mut out = None;
    row(ui, label, |ui| {
        let r = controls::checkbox(ui, value, check);
        name(&r, &format!("{label}, {check}"), text);
        out = Some(r);
        if let Some((on, check)) = follow {
            follow_check(ui, label, on, check, text);
        }
        help(ui, text);
    });
    out.expect("the row draws its contents")
}

/// A row's second check, which follows the system's setting too, named by the row's `label`
/// and its text `check` and described by the row's help `text`.
pub fn follow_check(ui: &mut Ui, label: &str, on: &mut bool, check: &str, text: &str) {
    let r = controls::checkbox(ui, on, check);
    name(&r, &format!("{label}, {check}"), text);
}

/// Where Show all shortcuts is in sending the keyboard focus back: asked (the sheet opens next
/// frame), then the sheet seen open; once it closes the button takes the focus again.
#[derive(Clone, Copy, PartialEq)]
enum SheetTrip {
    Asked,
    Open,
}

/// Shortcuts: Change shortcuts… (Preferences > Keyboard) and Show all shortcuts (the sheet over
/// Preferences, which gives the focus back to the button when it closes).
fn shortcuts_row(ui: &mut Ui, t: Lang, sheet_open: bool, msgs: &mut Vec<Msg>) {
    let text = t.tr("prefs.a11y.shortcuts.help");
    row(ui, t.tr("prefs.a11y.shortcuts.label"), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let change = ui.add(buttons::secondary(t.tr("prefs.a11y.shortcuts.change.button")));
            access::describe(&change, text);
            if change.clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::Navigate(PrefPage::Keyboard)));
            }
            let show = ui.add(buttons::secondary(t.tr("prefs.a11y.shortcuts.show.button")));
            access::describe(&show, text);
            let key = show.id.with("sheet-trip");
            let trip = ui.data(|d| d.get_temp::<SheetTrip>(key));
            match trip {
                Some(SheetTrip::Asked) if sheet_open => ui.data_mut(|d| d.insert_temp(key, SheetTrip::Open)),
                Some(SheetTrip::Open) if !sheet_open => {
                    show.request_focus();
                    ui.data_mut(|d| d.remove::<SheetTrip>(key));
                }
                _ => {}
            }
            if show.clicked() {
                ui.data_mut(|d| d.insert_temp(key, SheetTrip::Asked));
                msgs.push(Msg::Prefs(PrefsMsg::ShowShortcutSheet));
            }
        });
        help(ui, text);
    });
}

#[cfg(test)]
#[path = "accessibility_tests.rs"]
mod tests;
