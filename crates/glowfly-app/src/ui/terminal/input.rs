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
    /// Alt with a letter, digit or punctuation key sends ESC and the character (on macOS only
    /// with Option as Meta, since Option composes characters there).
    pub alt_is_meta: bool,
    /// Alt with a digit, +, - or / may be a Windows Alt code typed on the numpad, which egui
    /// reports as the same keys as the main ones: such a press sends ESC only when its own
    /// text comes (Windows types none for numpad keys until Alt is released).
    pub alt_codes: bool,
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
/// Alt with a letter, digit or punctuation key (`opts.alt_is_meta`) is sent as ESC and the
/// character, once: the character the press types when it comes as text in the same frame (so
/// the keyboard layout decides Alt+_ or Alt+?), else the key's own (`meta_char`), which is what
/// macOS's Option gets, since it types a composed character instead. With `opts.alt_codes`
/// (Windows), a key the numpad shares sends nothing without its text: it is part of an Alt code.
pub(super) fn collect(events: &[Event], pending: &mut Option<(Clip, u32)>, opts: Opts) -> Collected {
    let mut out = Vec::new();
    let mut copy = false;
    let mut scroll = Vec::new();
    // An Alt+key press waiting for its text.
    let mut meta: Option<Meta> = None;
    for e in events {
        let text = match e {
            Event::Text(t) | Event::Ime(ImeEvent::Commit(t)) => Some(t),
            _ => None,
        };
        // A numpad-shared key whose text is something else (an Alt code's character) sends no
        // ESC: the text is typed as it is.
        if let Some(t) = text
            && meta.is_some_and(|m| m.needs_text && !single(t).is_some_and(|c| c.is_ascii_graphic()))
        {
            meta = None;
        }
        if let Some(t) = text
            && let Some(Meta { c, shift, .. }) = meta.take()
        {
            match single(t) {
                Some(typed) if typed.is_ascii_graphic() => out.push(meta_input(typed, shift)),
                // A character Option composed (macOS): the key's own goes instead.
                Some(_) => out.push(meta_input(c, shift)),
                None => out.extend([meta_input(c, shift), TermInput::Text(t.clone())]),
            }
            continue;
        }
        if !matches!(e, Event::Key { pressed: false, .. }) {
            send_meta(&mut meta, &mut out);
        }
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
                send_meta(&mut meta, &mut out);
                out.extend(sent.into_iter().flat_map(|i| std::iter::repeat_n(i, n as usize)));
            }
            Event::Key { key, pressed: true, modifiers, .. } => {
                let step = opts.history_keys.then(|| history_step(*key, *modifiers)).flatten();
                let alt = opts.alt_is_meta.then(|| meta_char(*key, *modifiers)).flatten();
                let sent = match (step, alt) {
                    (Some(step), _) => {
                        scroll.push(step);
                        None
                    }
                    (None, Some(c)) => {
                        flush(pending, &mut out);
                        let needs_text = opts.alt_codes && on_numpad_too(*key);
                        meta = Some(Meta { c, shift: modifiers.shift, needs_text });
                        None
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
    send_meta(&mut meta, &mut out);
    Collected { inputs: out, copy, scroll }
}

/// An Alt+key press waiting for its text.
#[derive(Clone, Copy)]
struct Meta {
    /// The key's own character.
    c: char,
    shift: bool,
    /// Sent only with its text: without it, the press is a numpad digit of a Windows Alt code.
    needs_text: bool,
}

/// Sends a waiting Alt+key with the key's own character: no text came for it. One that needs
/// its text is dropped.
fn send_meta(meta: &mut Option<Meta>, out: &mut Vec<TermInput>) {
    if let Some(m) = meta.take().filter(|m| !m.needs_text) {
        out.push(meta_input(m.c, m.shift));
    }
}

/// Whether egui reports the numpad's key as this one too (a digit, +, - or /).
fn on_numpad_too(key: Key) -> bool {
    matches!(
        key,
        Key::Num0
            | Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
            | Key::Plus
            | Key::Minus
            | Key::Slash
    )
}

/// ESC and `c`.
fn meta_input(c: char, shift: bool) -> TermInput {
    TermInput::Key { key: TermKey::Char(c), mods: KeyMods { alt: true, shift, ctrl: false } }
}

/// The one character of `text`, if it is a single character.
fn single(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
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

/// The character an Alt press of a letter, digit or punctuation key sends after ESC, from the
/// key alone: the letter (capital with Shift), else the key's character, with Shift as on a US
/// keyboard. Ctrl+Alt is AltGr on Windows, which types its character instead, and Cmd chords
/// are the app's.
pub(super) fn meta_char(key: Key, m: Modifiers) -> Option<char> {
    if !m.alt || m.ctrl || m.mac_cmd {
        return None;
    }
    if let Some(c) = letter(key) {
        return Some(if m.shift { c.to_ascii_uppercase() } else { c });
    }
    let (plain, shifted) = key_chars(key)?;
    Some(if m.shift { shifted } else { plain })
}

/// A digit or punctuation key's character, and the one Shift gives it on a US keyboard (keys
/// that are themselves a shifted character give it either way).
fn key_chars(key: Key) -> Option<(char, char)> {
    let pair = match key {
        Key::Num0 => ('0', ')'),
        Key::Num1 => ('1', '!'),
        Key::Num2 => ('2', '@'),
        Key::Num3 => ('3', '#'),
        Key::Num4 => ('4', '$'),
        Key::Num5 => ('5', '%'),
        Key::Num6 => ('6', '^'),
        Key::Num7 => ('7', '&'),
        Key::Num8 => ('8', '*'),
        Key::Num9 => ('9', '('),
        Key::Minus => ('-', '_'),
        Key::Equals => ('=', '+'),
        Key::OpenBracket => ('[', '{'),
        Key::CloseBracket => (']', '}'),
        Key::Backslash => ('\\', '|'),
        Key::Semicolon => (';', ':'),
        Key::Quote => ('\'', '"'),
        Key::Comma => (',', '<'),
        Key::Period => ('.', '>'),
        Key::Slash => ('/', '?'),
        Key::Backtick => ('`', '~'),
        Key::Plus => ('+', '+'),
        Key::Colon => (':', ':'),
        Key::Pipe => ('|', '|'),
        Key::Questionmark => ('?', '?'),
        Key::Exclamationmark => ('!', '!'),
        Key::OpenCurlyBracket => ('{', '{'),
        Key::CloseCurlyBracket => ('}', '}'),
        _ => return None,
    };
    Some(pair)
}

/// The lowercase letter of a letter key.
fn letter(key: Key) -> Option<char> {
    let mut chars = key.name().chars();
    match (chars.next(), chars.next()) {
        (Some(c @ 'A'..='Z'), None) => Some(c.to_ascii_lowercase()),
        _ => None,
    }
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
