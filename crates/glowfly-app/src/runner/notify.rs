//! Shows desktop notifications through the OS (notify-rust): WinRT toasts on Windows,
//! Notification Center on macOS, freedesktop notifications over D-Bus on Linux. The only code
//! that reaches the OS notification API (tests/layering.rs).
//!
//! The effect's worker thread then waits for the user's answer (at most `MAX_WAITING` at once)
//! and reports a click as `NotifyMsg::DesktopClicked`. Installed copies post under Glowfly's own
//! app id (`dress`), so the OS shows its name and icon.

use crate::effect::{DesktopNote, NoteKind};
use crate::msg::{Msg, NotifyMsg, PrefsMsg};
use notify_rust::{Notification, NotificationResponse};
use std::sync::atomic::{AtomicUsize, Ordering};

/// What became of a note the OS showed.
#[derive(Debug, PartialEq)]
enum Shown {
    /// Its body or a button was clicked.
    Clicked,
    /// Closed, timed out or not waited for.
    Closed,
}

/// Shows `note` and waits for the answer: a click comes back as `NotifyMsg::DesktopClicked`, a
/// refusal as `PrefsMsg::DesktopNotificationFailed`.
pub(super) fn run(note: DesktopNote) -> Option<Msg> {
    deliver(note, show)
}

/// Shows `note` with `send`, injectable so tests never reach the OS.
fn deliver(note: DesktopNote, send: impl FnOnce(&DesktopNote) -> Result<Shown, String>) -> Option<Msg> {
    match send(&note) {
        Ok(Shown::Clicked) => Some(Msg::Notify(NotifyMsg::DesktopClicked { workdir: note.workdir })),
        Ok(Shown::Closed) => None,
        Err(text) => {
            tracing::warn!("desktop notification: {text}");
            Some(Msg::Prefs(PrefsMsg::DesktopNotificationFailed { text, confirm: note.kind == NoteKind::Confirm }))
        }
    }
}

/// The most worker threads waiting for an answer at once: some servers keep a note until the
/// user dismisses it, so a wait can last long.
const MAX_WAITING: usize = 8;
static WAITING: AtomicUsize = AtomicUsize::new(0);

/// One of the `MAX_WAITING` waits, given back when dropped.
struct Slot;

impl Slot {
    fn take() -> Option<Slot> {
        WAITING.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < MAX_WAITING).then_some(n + 1)).ok().map(|_| Slot)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        WAITING.fetch_sub(1, Ordering::SeqCst);
    }
}

fn show(note: &DesktopNote) -> Result<Shown, String> {
    let mut n = Notification::new();
    n.appname("Glowfly").summary(&note.title).body(&note.body).timeout(notify_rust::Timeout::Default);
    dress(&mut n);
    let handle = n.show().map_err(|e| e.to_string())?;
    // Without a slot the handle is dropped unwaited (on macOS that delivers it in the background).
    let Some(_slot) = Slot::take() else { return Ok(Shown::Closed) };
    let mut clicked = false;
    let answer = |r: &NotificationResponse| clicked = matches!(r, NotificationResponse::Default | NotificationResponse::Action(_));
    if let Err(e) = handle.wait_for_response(answer) {
        tracing::debug!("desktop notification: no answer: {e}");
    }
    Ok(if clicked { Shown::Clicked } else { Shown::Closed })
}

/// This copy was installed (the MSI's folder or Glowfly.app), so the OS knows Glowfly's id.
#[cfg(any(windows, target_os = "macos"))]
fn installed() -> bool {
    use glowfly_core::app_update::{InstallKind, install_kind};
    static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *INSTALLED.get_or_init(|| std::env::current_exe().is_ok_and(|e| install_kind(&e) == InstallKind::Installer))
}

/// An installed copy posts under the app id its Start menu shortcut registers. An id no shortcut
/// registers makes the toasts vanish, so portable copies keep the default.
#[cfg(windows)]
fn dress(n: &mut Notification) {
    if installed() {
        n.app_id(glowfly_core::app_identity::APP_ID);
    }
}

/// Inside Glowfly.app, notes post under its bundle id (claimed once, before the first one).
#[cfg(target_os = "macos")]
fn dress(_n: &mut Notification) {
    static CLAIM: std::sync::Once = std::sync::Once::new();
    CLAIM.call_once(|| {
        if installed()
            && let Err(e) = notify_rust::set_application(glowfly_core::app_identity::APP_ID)
        {
            tracing::debug!("desktop notifications keep the default sender: {e:?}");
        }
    });
}

/// The desktop entry gives the server Glowfly's name and icon; the "default" action makes a
/// click on the body come back.
#[cfg(all(unix, not(target_os = "macos")))]
fn dress(n: &mut Notification) {
    use glowfly_core::app_identity::DESKTOP_ENTRY;
    n.icon(DESKTOP_ENTRY).hint(notify_rust::Hint::DesktopEntry(DESKTOP_ENTRY.into())).action("default", "Show");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn note(kind: NoteKind) -> DesktopNote {
        DesktopNote { kind, title: "Pull succeeded".into(), body: "demo".into(), workdir: Some(PathBuf::from("repo")) }
    }

    fn failed(msg: Option<Msg>) -> Option<(String, bool)> {
        match msg? {
            Msg::Prefs(PrefsMsg::DesktopNotificationFailed { text, confirm }) => Some((text, confirm)),
            _ => None,
        }
    }

    #[test]
    fn closed_notes_report_nothing() {
        let mut seen = None;
        let msg = deliver(note(NoteKind::Job), |n| {
            seen = Some(n.clone());
            Ok(Shown::Closed)
        });
        assert!(msg.is_none());
        assert_eq!(seen, Some(note(NoteKind::Job)));
    }

    #[test]
    fn clicked_notes_name_their_repository() {
        let msg = deliver(note(NoteKind::BehindAlert), |_| Ok(Shown::Clicked));
        assert!(
            matches!(&msg, Some(Msg::Notify(NotifyMsg::DesktopClicked { workdir: Some(p) })) if p == std::path::Path::new("repo")),
            "the click names the note's repository"
        );
        let msg = deliver(DesktopNote::confirmation(), |_| Ok(Shown::Clicked));
        assert!(matches!(msg, Some(Msg::Notify(NotifyMsg::DesktopClicked { workdir: None }))));
    }

    #[test]
    fn refusals_say_whether_it_was_the_confirmation() {
        let refuse = |_: &DesktopNote| Err("no server".to_owned());
        assert_eq!(failed(deliver(DesktopNote::confirmation(), refuse)), Some(("no server".into(), true)));
        assert_eq!(failed(deliver(note(NoteKind::Job), refuse)), Some(("no server".into(), false)));
        assert_eq!(failed(deliver(note(NoteKind::BehindAlert), refuse)), Some(("no server".into(), false)));
    }

    /// The only test that takes slots, so the count it starts from is its own.
    #[test]
    fn waiting_is_capped() {
        let start = WAITING.load(Ordering::SeqCst);
        let mut slots: Vec<Slot> = (start..MAX_WAITING).map(|_| Slot::take().expect("a free slot")).collect();
        assert!(Slot::take().is_none(), "all taken");
        slots.pop();
        slots.push(Slot::take().expect("one given back"));
        assert!(Slot::take().is_none());
        drop(slots);
        assert_eq!(WAITING.load(Ordering::SeqCst), start);
    }
}
