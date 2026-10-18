//! A chord: modifiers plus one key. Its display text ("Ctrl+Shift+S"; "Cmd+," and "Cmd+Option+1"
//! on macOS, chosen by `KeyLabels`) and its settings-file form ("Mod+Shift+S", the same on every system), which `Chord::parse` reads back.

/// Modifiers of a chord. `command` is Ctrl, or Cmd on macOS ("Mod" in the settings file);
/// `ctrl` is always the real Ctrl key (Ctrl+Tab).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub command: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyName {
    /// An uppercase ASCII letter.
    Letter(char),
    Digit(u8),
    /// Any digit from the first to the second (one row in the sheet, matched digit by digit).
    DigitRange(u8, u8),
    /// F1 to F12.
    F(u8),
    Enter,
    Escape,
    Tab,
    Backspace,
    Space,
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
    Comma,
    Period,
    Slash,
    Backslash,
    Semicolon,
    Quote,
    Backtick,
    Minus,
    Equals,
    /// The numpad +, and + on layouts that type it without Shift.
    Plus,
    OpenBracket,
    CloseBracket,
}

/// The keys with a fixed name: (key, settings-file name, display text).
const NAMED: [(KeyName, &str, &str); 27] = [
    (KeyName::Enter, "Enter", "Enter"),
    (KeyName::Escape, "Escape", "Esc"),
    (KeyName::Tab, "Tab", "Tab"),
    (KeyName::Backspace, "Backspace", "Backspace"),
    (KeyName::Space, "Space", "Space"),
    (KeyName::Up, "Up", "Up"),
    (KeyName::Down, "Down", "Down"),
    (KeyName::Left, "Left", "Left"),
    (KeyName::Right, "Right", "Right"),
    (KeyName::Home, "Home", "Home"),
    (KeyName::End, "End", "End"),
    (KeyName::PageUp, "PageUp", "PgUp"),
    (KeyName::PageDown, "PageDown", "PgDn"),
    (KeyName::Insert, "Insert", "Ins"),
    (KeyName::Delete, "Delete", "Del"),
    (KeyName::Comma, "Comma", ","),
    (KeyName::Period, "Period", "."),
    (KeyName::Slash, "Slash", "/"),
    (KeyName::Backslash, "Backslash", "\\"),
    (KeyName::Semicolon, "Semicolon", ";"),
    (KeyName::Quote, "Quote", "'"),
    (KeyName::Backtick, "Backtick", "`"),
    (KeyName::Minus, "Minus", "-"),
    (KeyName::Equals, "Equals", "="),
    // "Plus", not "+", so a chord reads "Ctrl+Plus" rather than "Ctrl++".
    (KeyName::Plus, "Plus", "Plus"),
    (KeyName::OpenBracket, "OpenBracket", "["),
    (KeyName::CloseBracket, "CloseBracket", "]"),
];

/// Other spellings `KeyName::parse` accepts in a hand-edited file.
const ALIASES: [(&str, KeyName); 6] = [
    ("esc", KeyName::Escape),
    ("return", KeyName::Enter),
    ("pgup", KeyName::PageUp),
    ("pgdn", KeyName::PageDown),
    ("ins", KeyName::Insert),
    ("del", KeyName::Delete),
];

const LETTERS: [&str; 26] =
    ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const FKEYS: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];

impl KeyName {
    /// The name in the settings file ("S", "F5", "PageUp", "Comma"); egui's key names agree.
    /// A digit range has none.
    pub fn config(self) -> &'static str {
        match self {
            KeyName::Letter(c) => LETTERS[(c.to_ascii_uppercase() as usize).saturating_sub('A' as usize).min(25)],
            KeyName::Digit(n) => DIGITS[(n as usize).min(9)],
            KeyName::DigitRange(..) => "",
            KeyName::F(n) => FKEYS[(n as usize).clamp(1, 12) - 1],
            k => NAMED.iter().find(|(n, ..)| *n == k).map_or("", |(_, name, _)| name),
        }
    }

    /// The key named `s` (any case; the settings-file names and a few aliases), if Trellis can
    /// bind it.
    pub fn parse(s: &str) -> Option<KeyName> {
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if c.is_ascii_alphabetic() {
                return Some(KeyName::Letter(c.to_ascii_uppercase()));
            }
            if let Some(d) = c.to_digit(10) {
                return Some(KeyName::Digit(d as u8));
            }
        }
        if let Some(n) = s.strip_prefix(['F', 'f']).and_then(|n| n.parse::<u8>().ok()) {
            return (1..=12).contains(&n).then_some(KeyName::F(n));
        }
        let named = NAMED.iter().find(|(_, name, _)| name.eq_ignore_ascii_case(s)).map(|(k, ..)| *k);
        named.or_else(|| ALIASES.iter().find(|(a, _)| a.eq_ignore_ascii_case(s)).map(|(_, k)| *k))
    }

    /// The text shown on the key's cap.
    fn display(self) -> &'static str {
        match self {
            // The only range in the table; a new one needs its own name here.
            KeyName::DigitRange(..) => "1–8",
            KeyName::Letter(_) | KeyName::Digit(_) | KeyName::F(_) => self.config(),
            k => NAMED.iter().find(|(n, ..)| *n == k).map_or("", |(.., text)| text),
        }
    }
}

/// How modifiers are named on screen: PC style ("Ctrl", "Alt") or macOS style ("Cmd", "Option").
/// The real Ctrl key reads "Ctrl" in both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyLabels {
    Pc,
    Mac,
}

impl KeyLabels {
    /// The style of the system Trellis runs on.
    pub const fn native() -> Self {
        if cfg!(target_os = "macos") { KeyLabels::Mac } else { KeyLabels::Pc }
    }

    /// The name of the command modifier ("Mod" in the settings file).
    pub fn command(self) -> &'static str {
        match self {
            KeyLabels::Pc => "Ctrl",
            KeyLabels::Mac => "Cmd",
        }
    }

    /// The name of the Alt key.
    pub fn alt(self) -> &'static str {
        match self {
            KeyLabels::Pc => "Alt",
            KeyLabels::Mac => "Option",
        }
    }

    /// The modifiers that make a key safe to use while typing, as a phrase.
    pub fn needs_modifier(self) -> &'static str {
        match self {
            KeyLabels::Pc => "Ctrl or Alt",
            KeyLabels::Mac => "Cmd, Ctrl or Option",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Chord {
    pub mods: Mods,
    pub key: KeyName,
}

const NONE: Mods = Mods { command: false, ctrl: false, shift: false, alt: false };

impl Chord {
    pub const fn plain(key: KeyName) -> Self {
        Self { mods: NONE, key }
    }
    pub const fn shift(key: KeyName) -> Self {
        Self { mods: Mods { shift: true, ..NONE }, key }
    }
    pub const fn alt(key: KeyName) -> Self {
        Self { mods: Mods { alt: true, ..NONE }, key }
    }
    pub const fn cmd(key: KeyName) -> Self {
        Self { mods: Mods { command: true, ..NONE }, key }
    }
    pub const fn cmd_shift(key: KeyName) -> Self {
        Self { mods: Mods { command: true, shift: true, ..NONE }, key }
    }
    /// Ctrl+Alt (Cmd+Option on macOS).
    pub const fn cmd_alt(key: KeyName) -> Self {
        Self { mods: Mods { command: true, alt: true, ..NONE }, key }
    }
    pub const fn ctrl(key: KeyName) -> Self {
        Self { mods: Mods { ctrl: true, ..NONE }, key }
    }
    pub const fn ctrl_shift(key: KeyName) -> Self {
        Self { mods: Mods { ctrl: true, shift: true, ..NONE }, key }
    }

    /// The modifier and key names in display order on this system, e.g. ["Ctrl", "Shift", "S"].
    pub fn parts(&self) -> Vec<&'static str> {
        self.parts_in(KeyLabels::native())
    }

    /// The modifier and key names in display order in style `l`: command, Ctrl, Shift, Alt, then
    /// the key (["Cmd", "Option", "1"] on macOS).
    pub fn parts_in(&self, l: KeyLabels) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.mods.command {
            out.push(l.command());
        }
        if self.mods.ctrl {
            out.push("Ctrl");
        }
        if self.mods.shift {
            out.push("Shift");
        }
        if self.mods.alt {
            out.push(l.alt());
        }
        out.push(self.key.display());
        out
    }

    /// The chord as text on this system, e.g. "Ctrl+Shift+S".
    pub fn text(&self) -> String {
        self.text_in(KeyLabels::native())
    }

    /// The chord as text in style `l`, e.g. "Cmd+Shift+S".
    pub fn text_in(&self, l: KeyLabels) -> String {
        self.parts_in(l).join("+")
    }

    /// The settings-file form: "Mod", "Ctrl", "Shift" and "Alt" in that order, then the key
    /// ("Mod+Shift+S", "Ctrl+Tab", "F5").
    pub fn config(&self) -> String {
        let m = self.mods;
        let names = [(m.command, "Mod"), (m.ctrl, "Ctrl"), (m.shift, "Shift"), (m.alt, "Alt")];
        let mut out: Vec<&str> = names.iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
        out.push(self.key.config());
        out.join("+")
    }

    /// Reads the settings-file form: parts split on '+', any case and order of modifiers
    /// ("Cmd" for Mod, "Option" for Alt too), exactly one key. None for anything else.
    pub fn parse(text: &str) -> Option<Chord> {
        let mut mods = NONE;
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            let flag = match part.to_ascii_lowercase().as_str() {
                "mod" | "cmd" | "command" => &mut mods.command,
                "ctrl" | "control" => &mut mods.ctrl,
                "shift" => &mut mods.shift,
                "alt" | "option" => &mut mods.alt,
                _ => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(KeyName::parse(part)?);
                    continue;
                }
            };
            if std::mem::replace(flag, true) {
                return None;
            }
        }
        Some(Chord { mods, key: key? })
    }

    /// Whether the chord holds Ctrl, Cmd or Alt, so it can't be typed text.
    pub fn has_command_mod(&self) -> bool {
        self.mods.command || self.mods.ctrl || self.mods.alt
    }
}
