//! Shows desktop notifications through the OS (notify-rust): WinRT toasts on Windows,
//! Notification Center on macOS, freedesktop notifications over D-Bus on Linux. The only code
//! that reaches the OS notification API (tests/layering.rs).

use crate::effect::{DesktopNote, NoteKind};
use crate::msg::{Msg, PrefsMsg};

/// Shows `note`; a refusal comes back as `PrefsMsg::DesktopNotificationFailed`.
pub(super) fn run(note: DesktopNote) -> Option<Msg> {
    deliver(note, show)
}

/// Shows `note` with `send`, injectable so tests never reach the OS. Ok gives no message.
fn deliver(note: DesktopNote, send: impl FnOnce(&DesktopNote) -> Result<(), String>) -> Option<Msg> {
    let text = send(&note).err()?;
    tracing::warn!("desktop notification: {text}");
    Some(Msg::Prefs(PrefsMsg::DesktopNotificationFailed { text, confirm: note.kind == NoteKind::Confirm }))
}

/// `show()` returns a handle on Linux and nothing elsewhere; `map` drops either, so no code
/// here depends on the OS.
fn show(note: &DesktopNote) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("Trellis")
        .summary(&note.title)
        .body(&note.body)
        .timeout(notify_rust::Timeout::Default)
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(kind: NoteKind) -> DesktopNote {
        DesktopNote { kind, title: "Pull succeeded".into(), body: "demo".into() }
    }

    fn failed(msg: Option<Msg>) -> Option<(String, bool)> {
        match msg? {
            Msg::Prefs(PrefsMsg::DesktopNotificationFailed { text, confirm }) => Some((text, confirm)),
            _ => None,
        }
    }

    #[test]
    fn shown_notes_report_nothing() {
        let mut seen = None;
        let msg = deliver(note(NoteKind::Job), |n| {
            seen = Some(n.clone());
            Ok(())
        });
        assert!(msg.is_none());
        assert_eq!(seen, Some(note(NoteKind::Job)));
    }

    #[test]
    fn refusals_say_whether_it_was_the_confirmation() {
        let refuse = |_: &DesktopNote| Err("no server".to_owned());
        assert_eq!(failed(deliver(DesktopNote::confirmation(), refuse)), Some(("no server".into(), true)));
        assert_eq!(failed(deliver(note(NoteKind::Job), refuse)), Some(("no server".into(), false)));
        assert_eq!(failed(deliver(note(NoteKind::BehindAlert), refuse)), Some(("no server".into(), false)));
    }
}
