//! egui's keyboard events turned into terminal input.

use crate::state::terminal::Clip;
use eframe::egui::{Event, ImeEvent, Key};
use trellis_core::terminal::{KeyMods, TermInput, TermKey};

/// What the keyboard did in one frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Collected {
    /// Input for the shell.
    pub inputs: Vec<TermInput>,
    /// Copy the selection (and send nothing for that key).
    pub copy: bool,
}

/// The terminal input in this frame's `events`. `pending` carries copy and cut commands over
/// to the frame where their key is released.
///
/// Windows and Linux report Ctrl+C and Ctrl+X as egui's copy and cut, without the key, and on
/// Windows Ctrl+Insert and Shift+Delete do the same. The key's release tells them apart: C or
/// X sends ^C or ^X, Insert or Delete sends that key. Any other input first sends what is
/// waiting as ^C or ^X. (On macOS those are Cmd, and Ctrl+C arrives as a key.) While text is
/// selected, a copy command (Ctrl+C, Ctrl+Insert, Cmd+C) copies it instead, on every system.
pub(super) fn collect(events: &[Event], pending: &mut Option<(Clip, u32)>, has_selection: bool) -> Collected {
    let mut out = Vec::new();
    let mut copy = false;
    for e in events {
        match e {
            Event::Copy if has_selection => copy = true,
            Event::Copy | Event::Cut if !cfg!(target_os = "macos") => {
                let clip = if matches!(e, Event::Copy) { Clip::Copy } else { Clip::Cut };
                match pending {
                    Some((c, n)) if *c == clip => *n += 1,
                    _ => {
                        flush(pending, &mut out);
                        *pending = Some((clip, 1));
                    }
                }
            }
            Event::Key { key, pressed: false, modifiers, .. } => {
                let Some((clip, n)) = *pending else { continue };
                let mods = KeyMods { ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift };
                let sent = match (clip, key) {
                    (Clip::Copy, Key::C) | (Clip::Cut, Key::X) => Some(clip_char(clip)),
                    (Clip::Copy, Key::Insert) => Some(TermInput::Key { key: TermKey::Insert, mods: KeyMods { ctrl: true, ..mods } }),
                    (Clip::Cut, Key::Delete) => Some(TermInput::Key { key: TermKey::Delete, mods: KeyMods { shift: true, ..mods } }),
                    // A keyboard's own Copy or Cut key: nothing for the shell.
                    (_, Key::Copy | Key::Cut) => None,
                    _ => continue,
                };
                *pending = None;
                out.extend(sent.into_iter().flat_map(|i| std::iter::repeat_n(i, n as usize)));
            }
            _ => {
                if let Some(i) = map(e) {
                    flush(pending, &mut out);
                    out.push(i);
                }
            }
        }
    }
    Collected { inputs: out, copy }
}

/// Sends the waiting copy or cut commands as ^C or ^X.
fn flush(pending: &mut Option<(Clip, u32)>, out: &mut Vec<TermInput>) {
    if let Some((clip, n)) = pending.take() {
        out.extend(std::iter::repeat_n(clip_char(clip), n as usize));
    }
}

fn clip_char(clip: Clip) -> TermInput {
    ctrl(if clip == Clip::Copy { 'c' } else { 'x' })
}

fn ctrl(c: char) -> TermInput {
    TermInput::Key { key: TermKey::Char(c), mods: KeyMods { ctrl: true, ..Default::default() } }
}

fn map(e: &Event) -> Option<TermInput> {
    match e {
        Event::Text(t) | Event::Ime(ImeEvent::Commit(t)) => Some(TermInput::Text(t.clone())),
        Event::Paste(t) => Some(TermInput::Paste(t.clone())),
        // Cmd chords (Cmd+T, Cmd+arrows...) are the app's on macOS, not the shell's; only real
        // Ctrl makes control codes. `mac_cmd` is never set elsewhere.
        Event::Key { modifiers, .. } if modifiers.mac_cmd => None,
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
        let one = |e: Event| collect(&[e], &mut None, false).inputs;
        assert_eq!(one(Event::Text("a".into())), [TermInput::Text("a".into())]);
        assert_eq!(one(key(Key::Enter, true, none)), [input(TermKey::Enter, false, false, false)]);
        assert!(one(key(Key::Enter, false, none)).is_empty());
        assert_eq!(one(key(Key::C, true, Modifiers::CTRL)), [input(TermKey::Char('c'), true, false, false)]);
        assert!(one(key(Key::Q, true, Modifiers::CTRL | Modifiers::ALT)).is_empty(), "AltGr types text");
        assert_eq!(one(Event::Paste("x".into())), [TermInput::Paste("x".into())]);
        assert_eq!(one(key(Key::ArrowUp, true, Modifiers::SHIFT)), [input(TermKey::Up, false, false, true)]);
        assert!(one(key(Key::A, true, none)).is_empty(), "plain letters come as text");
        assert_eq!(one(key(Key::OpenBracket, true, Modifiers::CTRL)), [input(TermKey::Char('['), true, false, false)]);
    }

    #[test]
    fn real_ctrl_on_every_platform_and_never_cmd() {
        let one = |e: Event| collect(&[e], &mut None, false).inputs;
        let ctrl_c = [input(TermKey::Char('c'), true, false, false)];
        // Windows and Linux report Ctrl as both `ctrl` and `command`; macOS only as `ctrl`.
        assert_eq!(one(key(Key::C, true, Modifiers::CTRL | Modifiers::COMMAND)), ctrl_c);
        assert_eq!(one(key(Key::C, true, Modifiers::CTRL)), ctrl_c);
        // macOS Cmd (`mac_cmd` and `command`) sends nothing to the shell.
        let cmd = Modifiers::MAC_CMD | Modifiers::COMMAND;
        assert!(one(key(Key::C, true, cmd)).is_empty());
        assert!(one(key(Key::ArrowLeft, true, cmd)).is_empty(), "not a plain Left");
        assert!(one(key(Key::Enter, true, cmd | Modifiers::SHIFT)).is_empty());
    }

    /// Feeds `frames` of events in turn, as the panel does, and returns all the input.
    fn frames(frames: &[&[Event]]) -> Vec<TermInput> {
        let mut pending = None;
        frames.iter().flat_map(|f| collect(f, &mut pending, false).inputs).collect()
    }

    #[test]
    fn copy_with_a_selection_copies_instead_of_interrupting() {
        let mut pending = None;
        let got = collect(&[Event::Copy], &mut pending, true);
        assert_eq!(got, Collected { inputs: Vec::new(), copy: true });
        assert_eq!(pending, None, "no ^C waits for the key's release");
        let up = key(Key::C, false, Modifiers::CTRL);
        assert_eq!(collect(&[up], &mut pending, false), Collected::default(), "the release sends nothing either");
        // Ctrl+Insert on Windows arrives the same way.
        let got = collect(&[Event::Copy, key(Key::Insert, false, Modifiers::CTRL)], &mut None, true);
        assert_eq!(got, Collected { inputs: Vec::new(), copy: true });
        // Without a selection, nothing changes: ^C waits for its key (not on macOS, where Cmd+C copies).
        let got = collect(&[Event::Copy], &mut pending, false);
        assert!(!got.copy && got.inputs.is_empty());
        assert_eq!(pending.is_some(), !cfg!(target_os = "macos"));
    }

    #[test]
    fn copy_and_cut_wait_for_their_key() {
        if cfg!(target_os = "macos") {
            return;
        }
        let up = |k, m| key(k, false, m);
        let (ctrl, shift) = (Modifiers::CTRL, Modifiers::SHIFT);
        let ctrl_c = input(TermKey::Char('c'), true, false, false);
        assert_eq!(frames(&[&[Event::Copy], &[up(Key::C, ctrl)]]), std::slice::from_ref(&ctrl_c));
        assert_eq!(frames(&[&[Event::Copy, up(Key::C, ctrl)]]), std::slice::from_ref(&ctrl_c), "in one frame");
        assert_eq!(frames(&[&[Event::Copy], &[Event::Copy], &[up(Key::C, ctrl)]]), [ctrl_c.clone(), ctrl_c.clone()], "held");
        assert_eq!(frames(&[&[Event::Cut], &[up(Key::X, ctrl)]]), [input(TermKey::Char('x'), true, false, false)]);

        // Ctrl+Insert and Shift+Delete don't interrupt the shell.
        assert_eq!(frames(&[&[Event::Copy], &[up(Key::Insert, ctrl)]]), [input(TermKey::Insert, true, false, false)]);
        assert_eq!(frames(&[&[Event::Cut], &[up(Key::Delete, shift)]]), [input(TermKey::Delete, false, false, true)]);
        assert!(frames(&[&[Event::Copy], &[up(Key::Copy, Modifiers::NONE)]]).is_empty(), "a Copy key");

        // Typing on before the release sends the ^C first, in order.
        let typed = frames(&[&[Event::Copy], &[Event::Text("a".into())], &[up(Key::C, ctrl)]]);
        assert_eq!(typed, [ctrl_c.clone(), TermInput::Text("a".into())]);
        // An unrelated release keeps it waiting.
        assert_eq!(frames(&[&[Event::Copy], &[up(Key::A, ctrl)]]), []);
        assert_eq!(frames(&[&[Event::Copy, up(Key::A, ctrl)], &[up(Key::C, ctrl)]]), [ctrl_c]);
    }
}
