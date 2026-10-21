//! Themes: the built-in choices and theme files (`<config dir>/themes/<stem>.json`).
//!
//! A theme file names a base theme and sets the colors it changes:
//!
//! ```json
//! { "name": "Ocean", "base": "dark", "colors": { "accent": "#2fa4ff", "hover": "selected" } }
//! ```
//!
//! A color is `#rrggbb` or the name of another color, which resolves to that color's final
//! value (the file's own, or the base theme's). The old names `green`, `red`, `amber`, `blue` and
//! `purple` still work, as keys and as values, for `added`, `removed`, `modified`, `info` and
//! `progress`; when a file sets both, the new name wins. Parsing never fails: everything that can't be
//! used is reported as a [`ThemeProblem`] and falls back to the base theme, key by key. This
//! module is pure; the I/O is in `themes_io.rs`.

use super::ThemeChoice;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub use super::themes_io::{MAX_FILE_BYTES, create, load_all, load_dir, themes_dir};

/// Every color a theme sets, in the palette's order.
#[rustfmt::skip]
pub const COLOR_KEYS: &[&str] = &[
    "bg", "panel", "toolbar", "raised", "input", "border", "input_border", "dialog_border", "hover", "selected", "accent",
    "text", "strong", "weak", "faint",
    "added", "removed", "modified", "info", "progress",
    "diff_add", "diff_del", "diff_add_sel", "diff_del_sel", "diff_add_word", "diff_del_word", "hunk_bg", "hunk_fg",
    "warn", "banner", "primary", "on_primary", "primary_off", "primary_border", "danger", "danger_off", "tag_bg", "tag_fg",
    "syn_comment", "syn_string", "syn_number", "syn_constant", "syn_keyword", "syn_type", "syn_function", "syn_attr",
];

/// Older color names and the keys they now stand for.
const LEGACY_COLOR_KEYS: &[(&str, &str)] =
    &[("green", "added"), ("red", "removed"), ("amber", "modified"), ("blue", "info"), ("purple", "progress")];

/// The palette name `s` stands for, as the `'static` key: an exact key, else an older name's key.
fn color_key(s: &str) -> Option<&'static str> {
    COLOR_KEYS.iter().copied().find(|k| *k == s).or_else(|| LEGACY_COLOR_KEYS.iter().find(|(old, _)| *old == s).map(|(_, k)| *k))
}

/// A palette that ships with Glowfly; theme files start from one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinTheme {
    Dark,
    Light,
    HighContrastDark,
    HighContrastLight,
    Sand,
}

impl BuiltinTheme {
    pub const ALL: [Self; 5] = [Self::Dark, Self::Light, Self::HighContrastDark, Self::HighContrastLight, Self::Sand];

    /// The name a theme file's `base` uses.
    pub fn id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::HighContrastDark => "high-contrast-dark",
            Self::HighContrastLight => "high-contrast-light",
            Self::Sand => "sand",
        }
    }

    /// The theme for a `base` value, spelled exactly as `id` gives it.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Glowfly Dark",
            Self::Light => "Glowfly Light",
            Self::HighContrastDark => "High Contrast Dark",
            Self::HighContrastLight => "High Contrast Light",
            Self::Sand => "Glowfly Sand",
        }
    }

    /// Light text on a dark background.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::HighContrastDark)
    }
}

impl ThemeChoice {
    /// The built-in choices, in the order Preferences lists them.
    pub const BUILT_IN: [ThemeChoice; 6] = [
        ThemeChoice::Dark,
        ThemeChoice::Light,
        ThemeChoice::System,
        ThemeChoice::HighContrastDark,
        ThemeChoice::HighContrastLight,
        ThemeChoice::Sand,
    ];

    /// The built-in palette this choice shows (`System` follows `system_dark`); None for a
    /// theme file.
    pub fn builtin(&self, system_dark: bool) -> Option<BuiltinTheme> {
        Some(match self {
            Self::Dark => BuiltinTheme::Dark,
            Self::Light => BuiltinTheme::Light,
            Self::System if system_dark => BuiltinTheme::Dark,
            Self::System => BuiltinTheme::Light,
            Self::HighContrastDark => BuiltinTheme::HighContrastDark,
            Self::HighContrastLight => BuiltinTheme::HighContrastLight,
            Self::Sand => BuiltinTheme::Sand,
            Self::Custom(_) => return None,
        })
    }

    /// The name Preferences shows; a theme file's id (its name is in the catalog).
    pub fn label(&self) -> String {
        match self {
            Self::System => "Match system".to_owned(),
            Self::Custom(id) => id.clone(),
            other => other.builtin(true).map(BuiltinTheme::label).unwrap_or_default().to_owned(),
        }
    }

    /// The short name the command palette shows after "Theme: ".
    pub fn palette_label(&self) -> String {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "Follow system",
            Self::HighContrastDark => "High contrast dark",
            Self::HighContrastLight => "High contrast light",
            Self::Sand => "Sand",
            Self::Custom(id) => return id.clone(),
        }
        .to_owned()
    }
}

/// One color a theme file sets, aliases resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorValue {
    Rgb([u8; 3]),
    /// The base theme's value of this key (an alias to a key the file doesn't set).
    Base(&'static str),
}

/// Something in a theme file Glowfly couldn't use. Each one names its key or field.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ThemeProblem {
    #[error("could not be read: {0}")]
    Unreadable(String),
    #[error("is not valid JSON: {0}")]
    Invalid(String),
    #[error("does not hold a JSON object with name, base and colors")]
    NotAnObject,
    #[error("is larger than 256 KiB, so it was not read")]
    TooLarge,
    #[error("\"{field}\" should be {expected}; ignored")]
    WrongType { field: String, expected: &'static str },
    #[error("\"{0}\" is not a theme file field; ignored")]
    UnknownField(String),
    #[error("\"{0}\" is not a base theme (dark, light, high-contrast-dark, high-contrast-light or sand); using dark")]
    UnknownBase(String),
    #[error("\"{0}\" is not a theme color; ignored")]
    UnknownColor(String),
    #[error("\"{key}\": {value} is not a #rrggbb color or a color name; using the base color")]
    BadValue { key: String, value: String },
    #[error("\"{key}\" refers back to itself through other colors; using the base color")]
    Cycle { key: String },
}

impl ThemeProblem {
    /// Whether the file's own choices were lost (rather than one entry ignored).
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Unreadable(_) | Self::Invalid(_) | Self::NotAnObject | Self::TooLarge | Self::UnknownBase(_))
    }

    /// The color key the problem is about, if it is about one.
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::UnknownColor(k) | Self::BadValue { key: k, .. } | Self::Cycle { key: k } => Some(k),
            _ => None,
        }
    }
}

/// A theme file as loaded.
#[derive(Clone, PartialEq, Debug)]
pub struct ThemeFile {
    /// The file stem; the settings name the theme by it.
    pub id: String,
    pub path: PathBuf,
    /// The display name: `name` from the file, or the id.
    pub name: String,
    pub base: BuiltinTheme,
    /// Only the colors the file sets (and could use), aliases fully resolved.
    pub colors: BTreeMap<&'static str, ColorValue>,
    pub problems: Vec<ThemeProblem>,
}

impl ThemeFile {
    /// A file that couldn't be used at all: the Dark palette, with `problem`.
    pub fn failed(id: &str, path: &Path, problem: ThemeProblem) -> Self {
        Self {
            id: id.to_owned(),
            path: path.to_owned(),
            name: id.to_owned(),
            base: BuiltinTheme::Dark,
            colors: BTreeMap::new(),
            problems: vec![problem],
        }
    }

    /// "<id>.json", for messages.
    pub fn file_name(&self) -> String {
        format!("{}.json", self.id)
    }
}

/// The theme files in the themes folder.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ThemeCatalog {
    /// The folder read; None without a config folder.
    pub dir: Option<PathBuf>,
    /// Sorted by display name, ignoring case, then id.
    pub files: Vec<ThemeFile>,
    /// Why the folder couldn't be listed.
    pub error: Option<String>,
    /// Entries passed over that the user may have meant as themes (names that aren't UTF-8).
    pub skipped: Vec<String>,
}

impl ThemeCatalog {
    pub fn get(&self, id: &str) -> Option<&ThemeFile> {
        self.files.iter().find(|f| f.id == id)
    }

    /// The built-in palette `choice` starts from: a file's base, or Dark for a missing file.
    pub fn base_of(&self, choice: &ThemeChoice, system_dark: bool) -> BuiltinTheme {
        match choice {
            ThemeChoice::Custom(id) => self.get(id).map_or(BuiltinTheme::Dark, |f| f.base),
            other => other.builtin(system_dark).unwrap_or(BuiltinTheme::Dark),
        }
    }
}

/// `#rrggbb`, any case, surrounding whitespace ignored.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// What one color entry says before aliases are followed.
#[derive(Clone, Copy)]
enum Raw {
    Rgb([u8; 3]),
    Alias(&'static str),
}

/// JSON text of `v`, shortened for a message.
fn shown_value(v: &Value) -> String {
    let text = v.to_string();
    if text.chars().count() <= 60 { text } else { format!("{}…", text.chars().take(60).collect::<String>()) }
}

/// Reads a theme file's bytes. `id` is its stem and `path` where it was read from.
pub fn parse(id: &str, path: &Path, bytes: &[u8]) -> ThemeFile {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return ThemeFile::failed(id, path, ThemeProblem::Invalid(e.to_string())),
    };
    let Value::Object(map) = value else { return ThemeFile::failed(id, path, ThemeProblem::NotAnObject) };
    let mut file = ThemeFile::failed(id, path, ThemeProblem::NotAnObject);
    file.problems.clear();
    let mut fields: Vec<(&String, &Value)> = map.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));
    let mut colors = None;
    for (field, v) in fields {
        let wrong = |expected| ThemeProblem::WrongType { field: field.clone(), expected };
        match (field.as_str(), v) {
            ("name", Value::String(s)) => {
                let name: String = crate::text::printable(s).chars().take(80).collect();
                if !name.is_empty() {
                    file.name = name;
                }
            }
            ("name", _) => file.problems.push(wrong("a string")),
            ("base", Value::String(s)) => match BuiltinTheme::from_id(s) {
                Some(b) => file.base = b,
                None => file.problems.push(ThemeProblem::UnknownBase(crate::text::printable(s).chars().take(60).collect())),
            },
            ("base", _) => file.problems.push(wrong("a string")),
            ("colors", Value::Object(c)) => colors = Some(c),
            ("colors", _) => file.problems.push(wrong("an object of colors")),
            _ => file.problems.push(ThemeProblem::UnknownField(crate::text::printable(field))),
        }
    }
    if let Some(c) = colors {
        let mut entries: Vec<(&String, &Value)> = c.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        // Current names first, then the older ones, which only fill keys the file left unset.
        let (legacy, current): (Vec<_>, Vec<_>) = entries.into_iter().partition(|(k, _)| LEGACY_COLOR_KEYS.iter().any(|(old, _)| old == k));
        let mut raw = BTreeMap::new();
        for (is_legacy, (k, v)) in current.into_iter().map(|e| (false, e)).chain(legacy.into_iter().map(|e| (true, e))) {
            let Some(key) = color_key(k) else {
                file.problems.push(ThemeProblem::UnknownColor(crate::text::printable(k)));
                continue;
            };
            let parsed = v.as_str().and_then(|s| parse_hex(s).map(Raw::Rgb).or_else(|| color_key(s.trim()).map(Raw::Alias)));
            match parsed {
                Some(r) if is_legacy => {
                    raw.entry(key).or_insert(r);
                }
                Some(r) => {
                    raw.insert(key, r);
                }
                None => file.problems.push(ThemeProblem::BadValue { key: key.to_owned(), value: shown_value(v) }),
            }
        }
        let (resolved, cycles) = resolve(&raw);
        file.colors = resolved;
        file.problems.extend(cycles.into_iter().map(|k| ThemeProblem::Cycle { key: k.to_owned() }));
    }
    file
}

/// Follows aliases: every key's final value, and the keys on loops (left out, sorted).
fn resolve(raw: &BTreeMap<&'static str, Raw>) -> (BTreeMap<&'static str, ColorValue>, BTreeSet<&'static str>) {
    let mut cycles = BTreeSet::new();
    for &start in raw.keys() {
        let mut path = vec![start];
        let mut cur = start;
        while let Some(Raw::Alias(next)) = raw.get(cur) {
            if cycles.contains(next) {
                break;
            }
            if let Some(i) = path.iter().position(|k| k == next) {
                cycles.extend(path[i..].iter().copied());
                break;
            }
            path.push(next);
            cur = next;
        }
    }
    let mut out = BTreeMap::new();
    for &start in raw.keys().filter(|k| !cycles.contains(*k)) {
        let mut cur = start;
        // Every chain ends: loops were found above, and a chain is at most every key long.
        let value = loop {
            match raw.get(cur) {
                Some(Raw::Rgb(c)) => break ColorValue::Rgb(*c),
                Some(Raw::Alias(next)) if raw.contains_key(next) && !cycles.contains(next) && *next != start => cur = next,
                Some(Raw::Alias(next)) => break ColorValue::Base(next),
                None => break ColorValue::Base(cur),
            }
        };
        out.insert(start, value);
    }
    (out, cycles)
}

/// A theme file setting every color in `colors` (in that order), based on `base`, named `name`.
pub fn starter_json(name: &str, base: BuiltinTheme, colors: &[(&str, [u8; 3])]) -> String {
    let name = serde_json::to_string(name).unwrap_or_else(|_| "\"\"".to_owned());
    let mut out = format!("{{\n  \"name\": {name},\n  \"base\": \"{}\",\n  \"colors\": {{\n", base.id());
    for (i, (key, [r, g, b])) in colors.iter().enumerate() {
        let comma = if i + 1 < colors.len() { "," } else { "" };
        out.push_str(&format!("    \"{key}\": \"#{r:02x}{g:02x}{b:02x}\"{comma}\n"));
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(test)]
#[path = "themes_tests.rs"]
mod tests;
