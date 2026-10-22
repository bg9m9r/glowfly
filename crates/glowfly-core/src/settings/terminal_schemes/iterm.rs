//! iTerm2 `.itermcolors` files: an XML property list whose top dict maps "Ansi 0 Color" …
//! "Ansi 15 Color", "Background Color", "Foreground Color", "Cursor Color" and "Selection Color"
//! to dicts of "Red/Green/Blue Component" numbers from 0 to 1. Components above 1 are read as
//! 0-255 (some converters write them so); alpha and the colour space are ignored. Only the
//! small part of XML property lists these files use is read.

use super::{ImportError, Scheme, ansi_key};
use std::collections::BTreeMap;

/// Nesting deeper than this is refused rather than followed.
const MAX_DEPTH: usize = 16;

pub(super) fn parse(text: &str, name: &str) -> Result<Scheme, ImportError> {
    let bad = |why: &str| ImportError::Plist(why.to_owned());
    let tokens = tokens(text).map_err(|e| bad(&e))?;
    let mut at = 0;
    // Up to and into <plist>.
    match tokens.get(at) {
        Some(Tok::Open(t)) if t == "plist" => at += 1,
        _ => return Err(bad("it doesn't start with <plist>")),
    }
    let root = value(&tokens, &mut at, 0).map_err(|e| bad(&e))?;
    let Val::Dict(entries) = root else { return Err(bad("its top level is not a dict")) };
    let mut colors = BTreeMap::new();
    for (k, v) in &entries {
        let Some(key) = key_for(k) else { continue };
        let Val::Dict(parts) = v else { return Err(bad(&format!("\"{k}\" is not a dict of components"))) };
        colors.insert(key, rgb(k, parts).map_err(|e| bad(&e))?);
    }
    if colors.is_empty() {
        return Err(bad("it has no Ansi, Background or Foreground colors"));
    }
    Ok(Scheme { name: name.to_owned(), colors })
}

/// The terminal key an iTerm2 colour name stands for.
fn key_for(k: &str) -> Option<&'static str> {
    Some(match k {
        "Background Color" => "terminal_background",
        "Foreground Color" => "terminal_foreground",
        "Cursor Color" => "terminal_cursor",
        "Selection Color" => "terminal_selection",
        _ => {
            let n: usize = k.strip_prefix("Ansi ")?.strip_suffix(" Color")?.parse().ok()?;
            return (n < 16).then(|| ansi_key(n));
        }
    })
}

/// A colour dict's RGB: three numbers from 0 to 1, or all of them 0 to 255 when one is above 1.
fn rgb(name: &str, parts: &[(String, Val)]) -> Result<[u8; 3], String> {
    let mut c = [0.0_f64; 3];
    for (i, part) in ["Red Component", "Green Component", "Blue Component"].iter().enumerate() {
        match parts.iter().find(|(k, _)| k == part).map(|(_, v)| v) {
            Some(Val::Num(n)) if n.is_finite() && *n >= 0.0 => c[i] = *n,
            Some(_) => return Err(format!("\"{name}\" has a {part} that isn't a number from 0 to 1")),
            None => return Err(format!("\"{name}\" has no {part}")),
        }
    }
    let scale = if c.iter().any(|v| *v > 1.0) { 1.0 } else { 255.0 };
    Ok(c.map(|v| (v * scale).round().clamp(0.0, 255.0) as u8))
}

#[derive(Debug, PartialEq)]
enum Tok {
    Open(String),
    Close(String),
    /// `<true/>` and the like.
    Empty(String),
    Text(String),
}

#[derive(Debug)]
enum Val {
    Dict(Vec<(String, Val)>),
    Num(f64),
    /// Strings, booleans, arrays and the rest: read past, never used.
    Other,
}

/// One value starting at `tokens[*at]`, leaving `*at` after it.
fn value(tokens: &[Tok], at: &mut usize, depth: usize) -> Result<Val, String> {
    if depth > MAX_DEPTH {
        return Err("it is nested too deeply".into());
    }
    let Some(tok) = tokens.get(*at) else { return Err("it ends early".into()) };
    *at += 1;
    match tok {
        Tok::Empty(_) => Ok(Val::Other),
        Tok::Open(tag) if tag == "dict" => {
            let mut entries = Vec::new();
            loop {
                match tokens.get(*at) {
                    Some(Tok::Close(t)) if t == "dict" => {
                        *at += 1;
                        return Ok(Val::Dict(entries));
                    }
                    Some(Tok::Open(t)) if t == "key" => {
                        *at += 1;
                        let key = text_until_close(tokens, at, "key")?;
                        entries.push((key, value(tokens, at, depth + 1)?));
                    }
                    _ => return Err("a dict holds something other than keys and values".into()),
                }
            }
        }
        Tok::Open(tag) if tag == "array" => {
            while !matches!(tokens.get(*at), Some(Tok::Close(t)) if t == "array") {
                if *at >= tokens.len() {
                    return Err("an array isn't closed".into());
                }
                value(tokens, at, depth + 1)?;
            }
            *at += 1;
            Ok(Val::Other)
        }
        Tok::Open(tag) => {
            let tag = tag.clone();
            let text = text_until_close(tokens, at, &tag)?;
            if tag == "real" || tag == "integer" {
                text.trim().parse::<f64>().map(Val::Num).map_err(|_| format!("<{tag}>{}</{tag}> is not a number", text.trim()))
            } else {
                Ok(Val::Other)
            }
        }
        Tok::Close(t) => Err(format!("</{t}> closes nothing")),
        Tok::Text(_) => Err("text stands outside an element".into()),
    }
}

/// The text inside an element just opened, through its closing tag.
fn text_until_close(tokens: &[Tok], at: &mut usize, tag: &str) -> Result<String, String> {
    let mut text = String::new();
    if let Some(Tok::Text(t)) = tokens.get(*at) {
        text = t.clone();
        *at += 1;
    }
    match tokens.get(*at) {
        Some(Tok::Close(t)) if t == tag => {
            *at += 1;
            Ok(text)
        }
        _ => Err(format!("<{tag}> isn't closed")),
    }
}

/// The tags and non-blank text of `s`, without the XML declaration, doctype and comments.
fn tokens(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").ok_or("a comment isn't closed")?;
            rest = &after[end + 3..];
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            let end = rest.find('>').ok_or("a declaration isn't closed")?;
            rest = &rest[end + 1..];
        } else if let Some(after) = rest.strip_prefix('<') {
            let end = after.find('>').ok_or("a tag isn't closed")?;
            let inner = after[..end].trim();
            rest = &after[end + 1..];
            let name = |t: &str| t.split_whitespace().next().unwrap_or("").to_owned();
            if let Some(closing) = inner.strip_prefix('/') {
                out.push(Tok::Close(name(closing)));
            } else if let Some(empty) = inner.strip_suffix('/') {
                out.push(Tok::Empty(name(empty)));
            } else {
                out.push(Tok::Open(name(inner)));
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..end];
            if !text.trim().is_empty() {
                out.push(Tok::Text(unescape(text)));
            }
            rest = &rest[end..];
        }
    }
    Ok(out)
}

/// XML's five named entities.
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}
