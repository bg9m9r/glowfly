//! Windows Terminal schemes: one scheme object, a list of them, or a settings.json (or a piece
//! of one) with a `schemes` list. settings.json may hold comments and trailing commas.

use super::{ImportError, Imported, Scheme, clean_name, short};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Windows Terminal's names for the terminal keys (`purple` is its magenta; `magenta` is taken
/// too).
const NAMES: &[(&str, &str)] = &[
    ("background", "terminal_background"),
    ("foreground", "terminal_foreground"),
    ("cursorColor", "terminal_cursor"),
    ("selectionBackground", "terminal_selection"),
    ("black", "terminal_black"),
    ("red", "terminal_red"),
    ("green", "terminal_green"),
    ("yellow", "terminal_yellow"),
    ("blue", "terminal_blue"),
    ("purple", "terminal_magenta"),
    ("magenta", "terminal_magenta"),
    ("cyan", "terminal_cyan"),
    ("white", "terminal_white"),
    ("brightBlack", "terminal_bright_black"),
    ("brightRed", "terminal_bright_red"),
    ("brightGreen", "terminal_bright_green"),
    ("brightYellow", "terminal_bright_yellow"),
    ("brightBlue", "terminal_bright_blue"),
    ("brightPurple", "terminal_bright_magenta"),
    ("brightMagenta", "terminal_bright_magenta"),
    ("brightCyan", "terminal_bright_cyan"),
    ("brightWhite", "terminal_bright_white"),
];

pub(super) fn parse(bytes: &[u8], fallback: &str) -> Result<Imported, ImportError> {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(strict) => {
            let text = std::str::from_utf8(bytes).map_err(|_| ImportError::Json(strict.to_string()))?;
            serde_json::from_str(&relaxed(text)).map_err(|_| ImportError::Json(strict.to_string()))?
        }
    };
    let list: Vec<&Value> = match &value {
        Value::Object(m) => match m.get("schemes") {
            Some(Value::Array(a)) => a.iter().collect(),
            Some(_) => return Err(ImportError::Json("\"schemes\" is not a list".into())),
            None => vec![&value],
        },
        Value::Array(a) => a.iter().collect(),
        _ => return Err(ImportError::NoScheme),
    };
    if list.is_empty() {
        return Err(ImportError::NoScheme);
    }
    let mut out = Imported::default();
    let several = list.len() > 1;
    for (i, v) in list.into_iter().enumerate() {
        let label = |name: Option<&str>, why: &str| match name {
            Some(n) => format!("scheme \"{n}\": {why}"),
            None if several => format!("scheme {}: {why}", i + 1),
            None => why.to_owned(),
        };
        let Value::Object(m) = v else {
            out.skipped.push(label(None, "it is not an object"));
            continue;
        };
        let name = m.get("name").and_then(Value::as_str).map(clean_name).filter(|n| !n.is_empty());
        match scheme(m) {
            Ok(colors) => {
                let name = name.unwrap_or_else(|| if several { format!("{fallback} {}", i + 1) } else { fallback.to_owned() });
                out.schemes.push(Scheme { name, colors });
            }
            Err(why) => out.skipped.push(label(name.as_deref(), &why)),
        }
    }
    if out.schemes.is_empty() {
        let first = out.skipped.first().cloned();
        return Err(first.map_or(ImportError::NoScheme, ImportError::Scheme));
    }
    Ok(out)
}

/// One scheme object's colours, or why it can't be used: a colour that isn't one, or no colours.
fn scheme(m: &Map<String, Value>) -> Result<BTreeMap<&'static str, [u8; 3]>, String> {
    let mut colors = BTreeMap::new();
    for (wt, key) in NAMES {
        let Some(v) = m.get(*wt) else { continue };
        match v.as_str().and_then(color) {
            Some(c) => {
                colors.insert(*key, c);
            }
            None => return Err(format!("\"{wt}\" is {}, not a #rrggbb color", short(&v.to_string()))),
        }
    }
    if colors.is_empty() {
        return Err("it sets no terminal colors (background, foreground, black … brightWhite)".into());
    }
    Ok(colors)
}

/// `#rrggbb`, `#rgb` or `#rrggbbaa` (the alpha is ignored).
pub(super) fn color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let nibble = |i: usize| u8::from_str_radix(&h[i..=i], 16).ok().map(|n| n * 17);
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    match h.len() {
        3 => Some([nibble(0)?, nibble(1)?, nibble(2)?]),
        6 | 8 => Some([byte(0)?, byte(2)?, byte(4)?]),
        _ => None,
    }
}

/// `text` without `//` and `/* */` comments and with trailing commas dropped, strings kept as
/// they are: the JSON-with-comments Windows Terminal's settings.json is written in.
pub(super) fn relaxed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
                out.push(' ');
            }
            (']' | '}', _) => {
                // A comma with only space after it before the closer goes.
                let trimmed = out.trim_end().len();
                if out[..trimmed].ends_with(',') {
                    out.truncate(trimmed - 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
