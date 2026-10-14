//! The New tag dialog: a plain tag, or an annotated one whose message is required.

use super::theme::color;
use crate::msg::Msg;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::gitcli::args;

/// The New tag dialog at commit `at` (a full sha); returns true when it should close. The name
/// field keeps the keyboard focus unless the message field has it, or `focus` is false (a
/// credential prompt is open above it).
pub fn new_tag(
    ui: &mut egui::Ui,
    at: &str,
    name: &mut String,
    message: &mut String,
    annotated: bool,
    focus: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut close = false;
    let short = &at[..7.min(at.len())];
    if annotated {
        ui.heading(format!("New annotated tag at {short}"));
        ui.label(RichText::new("Annotated tags keep a message, the tagger and the date.").color(color::weak()));
    } else {
        ui.heading(format!("New tag at {short}"));
    }
    ui.add_space(6.0);
    let msg_id = Id::new("trellis-tag-message");
    let r = ui.add(egui::TextEdit::singleline(name).id(Id::new("trellis-tag-name")).hint_text("v1.0.0").desired_width(f32::INFINITY));
    // Read before focus is requested again below, which would hide that Enter left the field.
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let hint = if annotated { "Message (required)" } else { "Message (optional, makes an annotated tag)" };
    ui.add(egui::TextEdit::multiline(message).id(msg_id).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY));
    ui.add_space(8.0);
    let valid = !name.trim().is_empty() && !name.contains(' ') && (!annotated || !message.trim().is_empty());
    if enter && !valid && annotated && message.trim().is_empty() {
        // The name is in; the message is still needed.
        ui.memory_mut(|m| m.request_focus(msg_id));
    } else if focus && !ui.memory(|m| m.has_focus(msg_id)) {
        r.request_focus();
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Create tag")).clicked() || (enter && valid) {
            msgs.push(Msg::Git { label: "Create tag".into(), cmds: vec![tag_cmd(name.trim(), at, message.trim())] });
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// `git tag <name> <at>`, or with a message an annotated `git tag -a <name> <at> -m <message>`.
pub(crate) fn tag_cmd(name: &str, at: &str, message: &str) -> Vec<String> {
    if message.is_empty() { args(&["tag", name, at]) } else { args(&["tag", "-a", name, at, "-m", message]) }
}

#[cfg(test)]
mod tests {
    use super::tag_cmd;

    #[test]
    fn tag_cmd_plain_and_annotated() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(tag_cmd("v1", sha, ""), ["tag", "v1", sha]);
        assert_eq!(tag_cmd("v1", sha, "notes"), ["tag", "-a", "v1", sha, "-m", "notes"]);
    }
}
