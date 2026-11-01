//! One row of Preferences > Keyboard: the action and where it works on the left; on the right
//! its keys as caps (click one to record a replacement), a weak × after each that strengthens
//! on hover, Add and Reset. A fixed row shows its keys dimmed and has no controls. Under the
//! row: "Esc cancels" while recording, and the warning or notice the row's last change left,
//! with its buttons.

use super::warn;
use crate::keymap::{Binding, Chord, Keymap, Slot};
use crate::msg::{KeysMsg, Msg};
use crate::state::keys_page::{KeysPage, Pending};
use crate::ui::shortcuts::Caps;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets;
use crate::ui::widgets::buttons;
use eframe::egui::{Align, CornerRadius, Layout, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};
use glowfly_core::i18n::Lang;

/// The most keys a row offers Add for.
const MAX_KEYS: usize = 4;

pub fn row(ui: &mut Ui, t: Lang, b: &'static Binding, keys: &Keymap, page: &KeysPage, msgs: &mut Vec<Msg>) {
    let recording = page.recording.filter(|r| r.id == b.id).map(|r| r.slot);
    let push = |msgs: &mut Vec<Msg>, m: KeysMsg| msgs.push(Msg::Keys(m));
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        let left = (ui.available_width() * 0.5).max(220.0);
        let wrap = Layout::left_to_right(Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(vec2(left, 0.0), wrap, |ui| {
            ui.label(RichText::new(b.label).color(color::text()));
            if let Some(note) = b.when.note() {
                ui.label(RichText::new(note).size(type_scale::CAPTION).color(color::weak()));
            }
            if !b.rebindable {
                let tip = match b.when.note() {
                    Some(note) => format!("Works only {note}; can't be changed"),
                    None => "Can't be changed".to_owned(),
                };
                ui.label(RichText::new("Fixed").size(type_scale::CAPTION).color(color::weak())).on_hover_text(tip);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !b.rebindable {
                Caps::new(ui, keys.chords(b.id), true).paint(ui, &keys.keys_text(b.id), Sense::hover());
                return;
            }
            let chords = keys.chords(b.id);
            let back = if b.defaults.is_empty() { "Back to no key".to_owned() } else { format!("Back to {}", keys.defaults_text(b.id)) };
            let reset = |ui: &mut Ui| ui.add(buttons::secondary("Reset").size(ControlSize::Small)).on_hover_text(&back);
            if !keys.is_default(b.id) && reset(ui).clicked() {
                push(msgs, KeysMsg::Reset(b.id));
            }
            if recording == Some(Slot::Add) {
                pill(ui);
            } else if chords.len() < MAX_KEYS
                && ui.add(buttons::secondary("Add").size(ControlSize::Small)).on_hover_text("Give this action another key").clicked()
            {
                push(msgs, KeysMsg::Record { id: b.id, slot: Slot::Add });
            }
            if chords.is_empty() && recording != Some(Slot::Add) {
                ui.label(RichText::new("No key").color(color::weak()));
            }
            for (n, chord) in chords.iter().enumerate().rev() {
                ui.add_space(4.0);
                if recording == Some(Slot::Replace(n)) {
                    pill(ui);
                    continue;
                }
                let remove = widgets::remove_icon(ui, "Remove this key");
                if remove.clicked() {
                    push(msgs, KeysMsg::Remove { id: b.id, index: n });
                }
                let label = format!("Change {} for {}", chord.text(), b.label);
                let cap = Caps::new(ui, std::slice::from_ref(chord), false).paint(ui, &label, Sense::click());
                if cap.on_hover_text("Click, then press the new key").clicked() {
                    push(msgs, KeysMsg::Record { id: b.id, slot: Slot::Replace(n) });
                }
            }
        });
    });
    if recording.is_some() {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("Esc cancels").size(type_scale::CAPTION).color(color::weak()));
        });
    }
    if let Some(p) = page.pending.as_ref().filter(|p| p.id() == b.id) {
        pending(ui, t, p, keys, msgs);
    }
    ui.add_space(2.0);
}

/// The warning or notice under a row, with what can be done about it.
fn pending(ui: &mut Ui, t: Lang, p: &Pending, keys: &Keymap, msgs: &mut Vec<Msg>) {
    let label = |id| crate::keymap::binding(id).label;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(12.0);
        match p {
            Pending::Refused { text, .. } => {
                warn(ui, text);
                ok(ui, msgs);
            }
            Pending::Notice { text, .. } => {
                ui.label(RichText::new(text).size(type_scale::SMALL).color(color::weak()));
                ok(ui, msgs);
            }
            Pending::Clash { chord, other, fixed: true, .. } => {
                warn(ui, &format!("{} is used by {}, which can't be changed", chord.text(), label(*other)));
                ok(ui, msgs);
            }
            Pending::Clash { chord, other, fixed: false, swap, .. } => {
                warn(ui, &format!("{} is already the key for {}.", chord.text(), label(*other)));
                let theirs = keys.chords(*other);
                if let Some(swap) = swap {
                    let after: Vec<String> = theirs.iter().map(|k| if k == chord { swap.text() } else { k.text() }).collect();
                    let tip = format!("{} gets {} instead", label(*other), after.join(" or "));
                    if ui.add(buttons::secondary("Swap")).on_hover_text(tip).clicked() {
                        msgs.push(Msg::Keys(KeysMsg::Swap));
                    }
                }
                let left: Vec<String> = theirs.iter().filter(|k| *k != chord).map(Chord::text).collect();
                let tip = if left.is_empty() {
                    format!("{} is left with no key", label(*other))
                } else {
                    format!("{} keeps {}", label(*other), left.join(" or "))
                };
                if ui.add(buttons::secondary("Use here")).on_hover_text(tip).clicked() {
                    msgs.push(Msg::Keys(KeysMsg::UseHere));
                }
                if ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
                    msgs.push(Msg::Keys(KeysMsg::DismissPending));
                }
            }
        }
    });
}

fn ok(ui: &mut Ui, msgs: &mut Vec<Msg>) {
    if ui.add(buttons::secondary("OK")).clicked() {
        msgs.push(Msg::Keys(KeysMsg::DismissPending));
    }
}

/// The slot waiting for a key: a highlighted pill.
fn pill(ui: &mut Ui) {
    let text = ui.painter().layout_no_wrap("Press keys…".to_owned(), type_scale::font(type_scale::SMALL), color::text());
    let size = text.size() + vec2(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, "Press keys…"));
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(radius::pill(rect.height())),
        color::selected(),
        Stroke::new(1.5_f32, color::primary()),
        StrokeKind::Inside,
    );
    p.galley(rect.center() - text.size() / 2.0, text, color::text());
}
