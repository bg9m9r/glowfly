//! egui's keyboard events turned into terminal input.

use eframe::egui::{Event, ImeEvent, Key};
use trellis_core::terminal::{KeyMods, TermInput, TermKey};

/// The terminal input in this frame's `events`.
pub(super) fn collect(events: &[Event]) -> Vec<TermInput> {
    events.iter().filter_map(map).collect()
}

fn ctrl(c: char) -> TermInput {
    TermInput::Key { key: TermKey::Char(c), mods: KeyMods { ctrl: true, ..Default::default() } }
}

fn map(e: &Event) -> Option<TermInput> {
    match e {
        Event::Text(t) | Event::Ime(ImeEvent::Commit(t)) => Some(TermInput::Text(t.clone())),
        Event::Paste(t) => Some(TermInput::Paste(t.clone())),
        // Windows and Linux report Ctrl+C and Ctrl+X as copy and cut; on macOS those are Cmd
        // and Ctrl+C arrives as a key.
        Event::Copy if !cfg!(target_os = "macos") => Some(ctrl('c')),
        Event::Cut if !cfg!(target_os = "macos") => Some(ctrl('x')),
        Event::Key { key, pressed: true, modifiers, .. } => {
            let mods = KeyMods { ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift };
            Some(TermInput::Key { key: term_key(*key, mods)?, mods })
        }
        _ => None,
    }
}

fn term_key(key: Key, mods: KeyMods) -> Option<TermKey> {
    let k = match key {
        Key::Enter => TermKey::Enter,
        Key::Backspace => TermKey::Backspace,
        Key::Tab => TermKey::Tab,
        Key::Escape => TermKey::Escape,
        Key::ArrowUp => TermKey::Up,
        Key::ArrowDown => TermKey::Down,
        Key::ArrowLeft => TermKey::Left,
        Key::ArrowRight => TermKey::Right,
        Key::Home => TermKey::Home,
        Key::End => TermKey::End,
        Key::PageUp => TermKey::PageUp,
        Key::PageDown => TermKey::PageDown,
        Key::Insert => TermKey::Insert,
        Key::Delete => TermKey::Delete,
        Key::F1 => TermKey::F(1),
        Key::F2 => TermKey::F(2),
        Key::F3 => TermKey::F(3),
        Key::F4 => TermKey::F(4),
        Key::F5 => TermKey::F(5),
        Key::F6 => TermKey::F(6),
        Key::F7 => TermKey::F(7),
        Key::F8 => TermKey::F(8),
        Key::F9 => TermKey::F(9),
        Key::F10 => TermKey::F(10),
        Key::F11 => TermKey::F(11),
        Key::F12 => TermKey::F(12),
        // Ctrl+character keys send control bytes. Ctrl+Alt is AltGr on Windows, whose
        // character comes as text instead.
        _ if mods.ctrl && !mods.alt => TermKey::Char(control_char(key)?),
        _ => return None,
    };
    Some(k)
}

/// The character of a key that has a control code with Ctrl.
fn control_char(key: Key) -> Option<char> {
    match key {
        Key::Space => Some(' '),
        Key::OpenBracket => Some('['),
        Key::Backslash => Some('\\'),
        Key::CloseBracket => Some(']'),
        _ => {
            let name = key.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c @ 'A'..='Z'), None) => Some(c.to_ascii_lowercase()),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Modifiers;

    fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }
    }

    fn input(key: TermKey, ctrl: bool, alt: bool, shift: bool) -> TermInput {
        TermInput::Key { key, mods: KeyMods { ctrl, alt, shift } }
    }

    #[test]
    fn maps_egui_events() {
        let none = Modifiers::NONE;
        let one = |e: Event| collect(&[e]);
        assert_eq!(one(Event::Text("a".into())), [TermInput::Text("a".into())]);
        assert_eq!(one(key(Key::Enter, true, none)), [input(TermKey::Enter, false, false, false)]);
        assert!(one(key(Key::Enter, false, none)).is_empty());
        assert_eq!(one(key(Key::C, true, Modifiers::CTRL)), [input(TermKey::Char('c'), true, false, false)]);
        assert!(one(key(Key::Q, true, Modifiers::CTRL | Modifiers::ALT)).is_empty(), "AltGr types text");
        if !cfg!(target_os = "macos") {
            assert_eq!(one(Event::Copy), [input(TermKey::Char('c'), true, false, false)]);
            assert_eq!(one(Event::Cut), [input(TermKey::Char('x'), true, false, false)]);
        }
        assert_eq!(one(Event::Paste("x".into())), [TermInput::Paste("x".into())]);
        assert_eq!(one(key(Key::ArrowUp, true, Modifiers::SHIFT)), [input(TermKey::Up, false, false, true)]);
        assert!(one(key(Key::A, true, none)).is_empty(), "plain letters come as text");
        assert_eq!(one(key(Key::OpenBracket, true, Modifiers::CTRL)), [input(TermKey::Char('['), true, false, false)]);
    }
}
