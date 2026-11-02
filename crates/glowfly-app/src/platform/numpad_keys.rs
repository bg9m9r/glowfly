//! Keeps Windows Alt codes (Alt held while typing digits on the numeric keypad, as Alt+0233
//! types é) away from the pane keys (Alt+1 to 5). egui names a keypad digit and a main-row digit
//! alike (`Key::Num1`), so a Windows message hook counts the keypad's digit presses before winit
//! sees them (`msg_hook`, which only looks), and the shell drops that many Alt+digit presses
//! from the frame's input (`drop_keypad_alt_digits`). The character the code types still comes
//! as text when Alt is let go. Elsewhere the keypad isn't told apart.

use eframe::egui::{Event, Key};
use std::sync::atomic::{AtomicU32, Ordering};

/// The keypad digit presses seen by the message hook and not yet matched to egui's, by digit.
type Pending = [AtomicU32; 10];

static PENDING: Pending = [const { AtomicU32::new(0) }; 10];

/// The digit of an egui key, if it is one.
fn digit(key: Key) -> Option<usize> {
    const DIGITS: [Key; 10] =
        [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    DIGITS.iter().position(|&k| k == key)
}

/// Drops this frame's presses of a keypad digit with Alt held, using the keypad presses the
/// hook counted in `pending` (a digit press with nothing pending is a main-row one, kept).
fn drop_keypad_alt_digits(events: &mut Vec<Event>, pending: &Pending) {
    events.retain(|e| {
        let Event::Key { key, pressed: true, modifiers, .. } = e else { return true };
        let Some(d) = digit(*key) else { return true };
        // The hook and the shell both run on the main thread.
        let keypad = pending[d].load(Ordering::SeqCst) > 0;
        if keypad {
            pending[d].fetch_sub(1, Ordering::SeqCst);
        }
        !(keypad && modifiers.alt)
    });
}

/// This frame's input with the keypad's Alt+digit presses taken out (`GlowflyApp`'s
/// `raw_input_hook`).
pub fn filter(events: &mut Vec<Event>) {
    drop_keypad_alt_digits(events, &PENDING);
}

/// The keypad digit a Windows key-down message is for: `message` WM_KEYDOWN or WM_SYSKEYDOWN
/// (Alt held), `vk` VK_NUMPAD0 to VK_NUMPAD9 (with Num Lock on; off, the keypad sends End,
/// Down and the like, which aren't digits).
#[cfg(any(windows, test))]
fn keypad_digit(message: u32, vk: usize) -> Option<usize> {
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_SYSKEYDOWN: u32 = 0x0104;
    const VK_NUMPAD0: usize = 0x60;
    let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    (down && (VK_NUMPAD0..VK_NUMPAD0 + 10).contains(&vk)).then(|| vk - VK_NUMPAD0)
}

/// Counts a keypad digit press in `pending` (the message hook's work, on any OS for the tests).
#[cfg(any(windows, test))]
fn note(message: u32, vk: usize, pending: &Pending) {
    if let Some(d) = keypad_digit(message, vk) {
        pending[d].fetch_add(1, Ordering::SeqCst);
    }
}

/// The message hook for winit's event loop: notes keypad digit presses and lets every message
/// through.
#[cfg(windows)]
pub fn msg_hook(msg: *const std::ffi::c_void) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::MSG;
    // SAFETY: winit hands the hook a pointer to the MSG it is about to dispatch.
    if let Some(m) = unsafe { msg.cast::<MSG>().as_ref() } {
        note(m.message, m.wParam.0, &PENDING);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Modifiers;

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    fn keys(events: &[Event]) -> Vec<Key> {
        events.iter().filter_map(|e| if let Event::Key { key, .. } = e { Some(*key) } else { None }).collect()
    }

    #[test]
    fn an_alt_code_on_the_keypad_never_reaches_the_pane_keys() {
        let pending: Pending = [const { AtomicU32::new(0) }; 10];
        // Alt+0233 on the keypad: four WM_SYSKEYDOWN for VK_NUMPAD0, 2, 3, 3.
        for vk in [0x60, 0x62, 0x63, 0x63] {
            note(0x0104, vk, &pending);
        }
        let mut events: Vec<Event> = [Key::Num0, Key::Num2, Key::Num3, Key::Num3].map(|k| press(k, Modifiers::ALT)).to_vec();
        events.push(Event::Text("é".into()));
        drop_keypad_alt_digits(&mut events, &pending);
        assert!(keys(&events).is_empty(), "no Alt+digit press left for the pane keys");
        assert!(matches!(&events[..], [Event::Text(t)] if t == "é"), "the typed character stays");
        assert!(pending.iter().all(|n| n.load(Ordering::SeqCst) == 0));
    }

    #[test]
    fn main_row_digits_stay_pane_keys() {
        let pending: Pending = [const { AtomicU32::new(0) }; 10];
        // The main row's 2 sends VK '2' (0x32), not a keypad code.
        note(0x0104, 0x32, &pending);
        let mut events = vec![press(Key::Num2, Modifiers::ALT), press(Key::Num3, Modifiers::ALT)];
        drop_keypad_alt_digits(&mut events, &pending);
        assert_eq!(keys(&events), [Key::Num2, Key::Num3]);
    }

    #[test]
    fn a_keypad_digit_without_alt_is_kept_and_counted_off() {
        let pending: Pending = [const { AtomicU32::new(0) }; 10];
        note(0x0100, 0x61, &pending);
        let mut events = vec![press(Key::Num1, Modifiers::NONE), press(Key::Num1, Modifiers::ALT)];
        drop_keypad_alt_digits(&mut events, &pending);
        assert_eq!(keys(&events), [Key::Num1, Key::Num1], "the plain press took the keypad count; the Alt one is the main row's");
        assert_eq!(keypad_digit(0x0101, 0x61), None, "a key-up isn't a press");
        assert_eq!(keypad_digit(0x0100, 0x23), None, "Num Lock off: End is no digit");
    }
}
