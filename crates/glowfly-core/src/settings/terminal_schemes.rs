//! Terminal colour schemes: the theme-file keys for the built-in terminal's colours, the schemes
//! imported into `<config dir>/themes/terminal/<stem>.json`, and reading the files they are
//! imported from (a Windows Terminal scheme or settings.json, or an iTerm2 `.itermcolors` plist).
//!
//! A stored scheme is the terminal part of a theme file:
//!
//! ```json
//! { "name": "Campbell", "colors": { "terminal_background": "#0c0c0c", "terminal_red": "#c50f1f" } }
//! ```
//!
//! Keys it leaves out come from the app theme's terminal colours. Parsing is pure; the I/O is in
//! `terminal_schemes/io.rs`.

use super::themes::{ThemeProblem, parse_hex};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod io;
mod iterm;
mod wt;

pub use io::{MAX_IMPORT_BYTES, import_path, load_dir, save, schemes_dir};

/// The terminal's keys in theme files and schemes, in palette order: the body colours, then the
/// 16 basic colours (0-7, then their bright 8-15).
#[rustfmt::skip]
pub const TERMINAL_KEYS: [&str; 20] = [
    "terminal_background", "terminal_foreground", "terminal_cursor", "terminal_selection",
    "terminal_black", "terminal_red", "terminal_green", "terminal_yellow",
    "terminal_blue", "terminal_magenta", "terminal_cyan", "terminal_white",
    "terminal_bright_black", "terminal_bright_red", "terminal_bright_green", "terminal_bright_yellow",
    "terminal_bright_blue", "terminal_bright_magenta", "terminal_bright_cyan", "terminal_bright_white",
];

/// The key of basic colour `i` (0-15).
pub fn ansi_key(i: usize) -> &'static str {
    TERMINAL_KEYS[4 + i % 16]
}

/// The terminal key `s` names, as the `'static` key.
fn terminal_key(s: &str) -> Option<&'static str> {
    TERMINAL_KEYS.iter().copied().find(|k| *k == s)
}

/// A scheme read from a file being imported: its name and the colours it sets.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scheme {
    pub name: String,
    /// Terminal keys to colours; keys left out fall back to the app theme's.
    pub colors: BTreeMap<&'static str, [u8; 3]>,
}

/// What an import found: the schemes it can use, and why others in the same file were passed
/// over.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Imported {
    pub schemes: Vec<Scheme>,
    pub skipped: Vec<String>,
}

/// The schemes an import wrote to the schemes folder.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImportReport {
    /// (id, name) of each scheme written, in the file's order.
    pub saved: Vec<(String, String)>,
    pub skipped: Vec<String>,
}

/// A stored scheme as loaded.
#[derive(Clone, PartialEq, Debug)]
pub struct SchemeFile {
    /// The file stem; the settings name the scheme by it.
    pub id: String,
    pub path: PathBuf,
    /// `name` from the file, or the id.
    pub name: String,
    pub colors: BTreeMap<&'static str, [u8; 3]>,
    pub problems: Vec<ThemeProblem>,
}

impl SchemeFile {
    fn failed(id: &str, path: &Path, problem: ThemeProblem) -> Self {
        Self { id: id.to_owned(), path: path.to_owned(), name: id.to_owned(), colors: BTreeMap::new(), problems: vec![problem] }
    }

    /// "<id>.json", for messages.
    pub fn file_name(&self) -> String {
        format!("{}.json", self.id)
    }
}

/// The stored schemes in the schemes folder.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SchemeCatalog {
    /// Sorted by name, ignoring case, then id.
    pub files: Vec<SchemeFile>,
    /// Why the folder couldn't be listed.
    pub error: Option<String>,
}

impl SchemeCatalog {
    pub fn get(&self, id: &str) -> Option<&SchemeFile> {
        self.files.iter().find(|f| f.id == id)
    }
}

/// Reads a stored scheme's bytes. Like theme files, it never fails: what can't be used is a
/// problem, and that key falls back.
pub fn parse_stored(id: &str, path: &Path, bytes: &[u8]) -> SchemeFile {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return SchemeFile::failed(id, path, ThemeProblem::Invalid(e.to_string())),
    };
    let Value::Object(map) = value else { return SchemeFile::failed(id, path, ThemeProblem::NotAnObject) };
    let mut file = SchemeFile::failed(id, path, ThemeProblem::NotAnObject);
    file.problems.clear();
    let mut fields: Vec<(&String, &Value)> = map.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));
    for (field, v) in fields {
        match (field.as_str(), v) {
            ("name", Value::String(s)) => {
                let name = clean_name(s);
                if !name.is_empty() {
                    file.name = name;
                }
            }
            ("colors", Value::Object(c)) => {
                let mut entries: Vec<(&String, &Value)> = c.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in entries {
                    let Some(key) = terminal_key(k) else {
                        file.problems.push(ThemeProblem::UnknownColor(crate::text::printable(k)));
                        continue;
                    };
                    match v.as_str().and_then(parse_hex) {
                        Some(c) => {
                            file.colors.insert(key, c);
                        }
                        None => file.problems.push(ThemeProblem::BadValue { key: key.to_owned(), value: short(&v.to_string()) }),
                    }
                }
            }
            ("name", _) => file.problems.push(ThemeProblem::WrongType { field: field.clone(), expected: "a string" }),
            ("colors", _) => file.problems.push(ThemeProblem::WrongType { field: field.clone(), expected: "an object of colors" }),
            _ => file.problems.push(ThemeProblem::UnknownField(crate::text::printable(field))),
        }
    }
    file
}

/// The stored form of `s`: its name and every colour it sets, in key order.
pub fn scheme_json(s: &Scheme) -> String {
    let name = serde_json::to_string(&s.name).unwrap_or_else(|_| "\"\"".to_owned());
    let set: Vec<(&str, [u8; 3])> = TERMINAL_KEYS.iter().filter_map(|k| s.colors.get(k).map(|c| (*k, *c))).collect();
    let mut out = format!("{{\n  \"name\": {name},\n  \"colors\": {{\n");
    for (i, (key, [r, g, b])) in set.iter().enumerate() {
        let comma = if i + 1 < set.len() { "," } else { "" };
        out.push_str(&format!("    \"{key}\": \"#{r:02x}{g:02x}{b:02x}\"{comma}\n"));
    }
    out.push_str("  }\n}\n");
    out
}

/// Why a file couldn't be imported at all.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the file could not be read: {0}")]
    Unreadable(String),
    #[error("the file is larger than 1 MiB, so it was not read")]
    TooLarge,
    #[error("the file is empty")]
    Empty,
    #[error("it is a binary property list; export the scheme from iTerm2 as an XML .itermcolors file")]
    BinaryPlist,
    #[error("it is not a Windows Terminal scheme (JSON) or an iTerm2 .itermcolors file")]
    UnknownFormat,
    #[error("it is not valid JSON: {0}")]
    Json(String),
    #[error("it is not a valid .itermcolors file: {0}")]
    Plist(String),
    #[error("it holds no color scheme")]
    NoScheme,
    #[error("it could not be saved to the themes folder: {0}")]
    Save(String),
    /// Every scheme in it had a problem; the first one's.
    #[error("{0}")]
    Scheme(String),
}

/// Reads a file to import: Windows Terminal JSON (one scheme, a list of them, or a settings.json
/// with a `schemes` list, comments and trailing commas allowed) or an iTerm2 XML plist. `stem`
/// names a scheme the file doesn't name.
pub fn import(stem: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).ok_or(ImportError::Empty)?;
    let body = &bytes[start..];
    if body.starts_with(b"bplist") {
        return Err(ImportError::BinaryPlist);
    }
    let fallback = clean_name(stem);
    let fallback = if fallback.is_empty() { "Imported scheme".to_owned() } else { fallback };
    match body[0] {
        b'<' => {
            let text = std::str::from_utf8(body).map_err(|_| ImportError::Plist("it isn't UTF-8 text".into()))?;
            iterm::parse(text, &fallback).map(|s| Imported { schemes: vec![s], skipped: Vec::new() })
        }
        // settings.json may open with a comment.
        b'{' | b'[' | b'/' => wt::parse(body, &fallback),
        _ => Err(ImportError::UnknownFormat),
    }
}

/// A scheme name as shown: printable, at most 80 characters, trimmed.
fn clean_name(s: &str) -> String {
    crate::text::printable(s).chars().take(80).collect::<String>().trim().to_owned()
}

/// `text` shortened for a message.
fn short(text: &str) -> String {
    if text.chars().count() <= 40 { text.to_owned() } else { format!("{}…", text.chars().take(40).collect::<String>()) }
}

#[cfg(test)]
mod tests;
