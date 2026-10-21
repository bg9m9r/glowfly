//! egui's keyboard events turned into terminal input, and the keys that scroll the history.

use crate::state::terminal::Clip;
use crate::terminal::ScrollStep;
use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use glowfly_core::terminal::{KeyMods, TermInput, TermKey};

/// What the keyboard did in one frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Collected {
    /// Input for the shell.
    pub inputs: Vec<TermInput>,
    /// Copy the selection (and send nothing for that key).
    pub copy: bool,
    /// History scrolling from the keyboard (sent nothing).
    pub scroll: Vec<ScrollStep>,
}

/// How this frame's keys are read.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Opts {
    /// The copy command copies the selection instead of reaching the shell.
    pub has_selection: bool,
    /// Shift with Page Up/Down, Home and End scrolls the history (the main screen).
    pub history_keys: bool,
    /// Alt+letter sends ESC and the letter (not on macOS, where Option composes characters).
    pub alt_is_meta: bool,
}

/// The terminal input in this frame's `events`. `pending` carries copy and cut commands over
/// to the frame where their key is released.
///
/// Windows and Linux report Ctrl+C and Ctrl+X as egui's copy and cut, without the key, and on
/// Windows Ctrl+Insert and Shift+Delete do the same. The key's release tells them apart: C or
/// X sends ^C or ^X, Insert or Delete sends that key. Any other input first sends what is
/// waiting as ^C or ^X. (On macOS those are Cmd, and Ctrl+C arrives as a key.) While text is
/// selected (`opts.has_selection`), a copy command (Ctrl+C, Ctrl+Insert, Cmd+C) copies it
/// instead, on every system.
///
/// Alt+letter (`opts.alt_is_meta`) is sent as ESC and the letter, and the letter's own text in
/// the same frame is dropped so it isn't typed twice.
pub(super) fn collect(events: &[Event], pending: &mut Option<(Clip, u32)>, opts: Opts) -> Collected {
    let mut out = Vec::new();
    let mut copy = false;
    let mut scroll = Vec::new();
    // The letter of the last Alt+letter sent, until the next text.
    let mut meta_sent: Option<char> = None;
    for e in events {
        match e {
            Event::Copy if opts.has_selection => copy = true,
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
            Event::Text(t) | Event::Ime(ImeEvent::Commit(t)) if meta_sent.take().is_some_and(|c| same_letter(t, c)) => {}
            Event::Key { key, pressed: true, modifiers, .. } => {
                let step = opts.history_keys.then(|| history_step(*key, *modifiers)).flatten();
                let meta = opts.alt_is_meta.then(|| meta_letter(*key, *modifiers)).flatten();
                let sent = match (step, meta) {
                    (Some(step), _) => {
                        scroll.push(step);
                        None
                    }
                    (None, Some(c)) => {
                        meta_sent = Some(c);
                        let shown = if modifiers.shift { c.to_ascii_uppercase() } else { c };
                        Some(TermInput::Key { key: TermKey::Char(shown), mods: KeyMods { alt: true, shift: modifiers.shift, ctrl: false } })
                    }
                    (None, None) => map(e),
                };
                if step.is_some() || sent.is_some() {
                    flush(pending, &mut out);
                }
                out.extend(sent);
            }
            _ => {
                if let Some(i) = map(e) {
                    flush(pending, &mut out);
                    out.push(i);
                }
            }
        }
    }
    Collected { inputs: out, copy, scroll }
}

/// The history step of Shift (alone) with Page Up/Down, Home or End.
fn history_step(key: Key, m: Modifiers) -> Option<ScrollStep> {
    if !m.shift || m.ctrl || m.alt || m.mac_cmd {
        return None;
    }
    match key {
        Key::PageUp => Some(ScrollStep::PageUp),
        Key::PageDown => Some(ScrollStep::PageDown),
        Key::Home => Some(ScrollStep::Top),
        Key::End => Some(ScrollStep::Bottom),
        _ => None,
    }
}

/// The lowercase letter of an Alt+letter press (with or without Shift). Ctrl+Alt is AltGr on
/// Windows, which types its character instead.
fn meta_letter(key: Key, m: Modifiers) -> Option<char> {
    if !m.alt || m.ctrl || m.mac_cmd {
        return None;
    }
    letter(key)
}

/// The lowercase letter of a letter key.
fn letter(key: Key) -> Option<char> {
    let mut chars = key.name().chars();
    match (chars.next(), chars.next()) {
        (Some(c @ 'A'..='Z'), None) => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

/// Whether `text` is just the letter `c`, in either case.
fn same_letter(text: &str, c: char) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(t), None) if t.eq_ignore_ascii_case(&c))
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

/// The character of a key that has a control code with Ctrl. The digits follow xterm: Ctrl+2
/// is NUL, Ctrl+3 to Ctrl+7 are ESC and 0x1c to 0x1f, Ctrl+8 is DEL, and Ctrl+1 and Ctrl+9 send
/// the digit (Ctrl+0 is the app's zoom reset).
fn control_char(key: Key) -> Option<char> {
    match key {
        Key::Space => Some(' '),
        Key::OpenBracket => Some('['),
        Key::Backslash => Some('\\'),
        Key::CloseBracket => Some(']'),
        Key::Num1 => Some('1'),
        Key::Num2 => Some('@'),
        Key::Num3 => Some('['),
        Key::Num4 => Some('\\'),
        Key::Num5 => Some(']'),
        Key::Num6 => Some('^'),
        Key::Num7 => Some('_'),
        Key::Num8 => Some('?'),
        Key::Num9 => Some('9'),
        _ => letter(key),
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
