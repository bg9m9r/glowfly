//! What typing, named keys and pasting send to the program, as xterm encodes them.

/// One piece of keyboard input for the terminal.
#[derive(Clone, Debug, PartialEq)]
pub enum TermInput {
    /// Typed text, sent as UTF-8.
    Text(String),
    /// Pasted text: line breaks become CR, bracketed when the program asked for that.
    Paste(String),
    Key {
        key: TermKey,
        mods: KeyMods,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct KeyMods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TermKey {
    Enter,
    Backspace,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// F1–F12.
    F(u8),
    /// A character key, for Ctrl and Alt combinations.
    Char(char),
}

/// Input modes the program has switched on.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Modes {
    /// DECCKM: arrows send ESC O instead of ESC [.
    pub application_cursor: bool,
    /// Pastes are wrapped in ESC [200~ … ESC [201~.
    pub bracketed_paste: bool,
}

const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

/// The bytes `input` sends to the program in `modes`.
pub fn encode(input: &TermInput, modes: Modes) -> Vec<u8> {
    match input {
        TermInput::Text(t) => t.as_bytes().to_vec(),
        TermInput::Paste(t) => {
            let text = t.replace("\r\n", "\r").replace('\n', "\r");
            if modes.bracketed_paste {
                format!("{PASTE_START}{}{PASTE_END}", text.replace(PASTE_END, "")).into_bytes()
            } else {
                text.into_bytes()
            }
        }
        TermInput::Key { key, mods } => key_bytes(*key, *mods, modes),
    }
}

/// xterm's modifier parameter: 1 plus Shift 1, Alt 2, Ctrl 4.
fn modifier(mods: KeyMods) -> u8 {
    1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.ctrl)
}

fn key_bytes(key: TermKey, mods: KeyMods, modes: Modes) -> Vec<u8> {
    let m = modifier(mods);
    let cursor = |x: char| {
        if m > 1 {
            format!("\x1b[1;{m}{x}").into_bytes()
        } else if modes.application_cursor {
            format!("\x1bO{x}").into_bytes()
        } else {
            format!("\x1b[{x}").into_bytes()
        }
    };
    let tilde = |n: u8| if m > 1 { format!("\x1b[{n};{m}~") } else { format!("\x1b[{n}~") }.into_bytes();
    let alt = |bytes: Vec<u8>| if mods.alt { [vec![0x1b], bytes].concat() } else { bytes };
    match key {
        TermKey::Enter => alt(b"\r".to_vec()),
        TermKey::Backspace => alt(vec![if mods.ctrl { 0x08 } else { 0x7f }]),
        TermKey::Tab if mods.shift => b"\x1b[Z".to_vec(),
        TermKey::Tab => b"\t".to_vec(),
        TermKey::Escape => vec![0x1b],
        TermKey::Up => cursor('A'),
        TermKey::Down => cursor('B'),
        TermKey::Right => cursor('C'),
        TermKey::Left => cursor('D'),
        TermKey::Home => cursor('H'),
        TermKey::End => cursor('F'),
        TermKey::Insert => tilde(2),
        TermKey::Delete => tilde(3),
        TermKey::PageUp => tilde(5),
        TermKey::PageDown => tilde(6),
        TermKey::F(n @ 1..=4) => format!("\x1bO{}", char::from(b'P' + n - 1)).into_bytes(),
        TermKey::F(n @ 5..=12) => tilde([15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 5)]),
        TermKey::F(_) => Vec::new(),
        TermKey::Char(c) => alt(char_bytes(c, mods.ctrl)),
    }
}

fn char_bytes(c: char, ctrl: bool) -> Vec<u8> {
    let control = match c.to_ascii_lowercase() {
        l @ 'a'..='z' if ctrl => Some(l as u8 - b'a' + 1),
        '@' | ' ' if ctrl => Some(0),
        '[' if ctrl => Some(27),
        '\\' if ctrl => Some(28),
        ']' if ctrl => Some(29),
        '^' if ctrl => Some(30),
        '_' if ctrl => Some(31),
        '?' if ctrl => Some(127),
        _ => None,
    };
    control.map(|b| vec![b]).unwrap_or_else(|| c.to_string().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: TermKey, ctrl: bool, alt: bool, shift: bool) -> TermInput {
        TermInput::Key { key, mods: KeyMods { ctrl, alt, shift } }
    }

    fn plain(k: TermKey) -> TermInput {
        key(k, false, false, false)
    }

    const NORMAL: Modes = Modes { application_cursor: false, bracketed_paste: false };
    const APP: Modes = Modes { application_cursor: true, bracketed_paste: false };

    #[test]
    fn encodes_named_keys() {
        let cases: [(TermInput, Modes, &[u8]); 13] = [
            (plain(TermKey::Enter), NORMAL, b"\r"),
            (plain(TermKey::Backspace), NORMAL, b"\x7f"),
            (key(TermKey::Backspace, true, false, false), NORMAL, b"\x08"),
            (plain(TermKey::Tab), NORMAL, b"\t"),
            (key(TermKey::Tab, false, false, true), NORMAL, b"\x1b[Z"),
            (plain(TermKey::Up), NORMAL, b"\x1b[A"),
            (plain(TermKey::Up), APP, b"\x1bOA"),
            (key(TermKey::Right, true, false, false), APP, b"\x1b[1;5C"),
            (plain(TermKey::Delete), NORMAL, b"\x1b[3~"),
            (key(TermKey::PageUp, false, false, true), NORMAL, b"\x1b[5;2~"),
            (plain(TermKey::F(1)), NORMAL, b"\x1bOP"),
            (plain(TermKey::F(5)), NORMAL, b"\x1b[15~"),
            (plain(TermKey::F(12)), NORMAL, b"\x1b[24~"),
        ];
        for (input, modes, want) in cases {
            assert_eq!(encode(&input, modes), want, "{input:?}");
        }
    }

    #[test]
    fn encodes_control_and_alt_chars() {
        let ctrl = |c| encode(&key(TermKey::Char(c), true, false, false), NORMAL);
        assert_eq!(ctrl('c'), [0x03]);
        assert_eq!(ctrl('d'), [0x04]);
        assert_eq!(ctrl('['), [0x1b]);
        assert_eq!(ctrl(' '), [0x00]);
        assert_eq!(encode(&key(TermKey::Enter, false, true, false), NORMAL), b"\x1b\r");
        assert_eq!(encode(&TermInput::Text("é".into()), NORMAL), "é".as_bytes());
    }

    #[test]
    fn paste_normalises_newlines_and_brackets() {
        let paste = TermInput::Paste("a\nb\r\nc".into());
        assert_eq!(encode(&paste, NORMAL), b"a\rb\rc");
        let bracketed = Modes { bracketed_paste: true, ..NORMAL };
        assert_eq!(encode(&paste, bracketed), b"\x1b[200~a\rb\rc\x1b[201~");
        let sneaky = TermInput::Paste("x\x1b[201~y".into());
        assert_eq!(encode(&sneaky, bracketed), b"\x1b[200~xy\x1b[201~");
    }
}
