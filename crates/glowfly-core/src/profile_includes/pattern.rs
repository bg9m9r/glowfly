//! The text Glowfly writes for folder rules: the `gitdir:` pattern of a rule, a path as a git
//! config value, and the include file of a profile (git's value quoting). Pure.

use crate::identity::Identity;
use crate::paths;
use crate::settings::FolderRule;
use std::path::{Component, Path, PathBuf};

/// Why a folder can't be named in git config.
pub const NOT_TEXT: &str = "Git config can't name this folder: its path isn't valid text.";

/// The `includeIf` condition for `rule`: `gitdir:` (or `gitdir/i:` for a folder whose file
/// system folds case) and the folder with forward slashes on Windows, its glob characters
/// escaped and a trailing `/`, so it matches the folder and everything inside it.
pub fn gitdir_pattern(rule: &FolderRule) -> Result<String, String> {
    let path = paths::decode(&rule.path);
    let text = plain_text(&path)?;
    let mut out = String::with_capacity(text.len() + 12);
    out.push_str(if rule.ignore_case { "gitdir/i:" } else { "gitdir:" });
    for c in text.chars() {
        // On Windows a backslash was a separator and is a `/` by now; on Unix it is a character
        // of the name, which wildmatch would take as an escape.
        if matches!(c, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    if !out.ends_with('/') {
        out.push('/');
    }
    Ok(out)
}

/// `path` as a git config value (an include's `path`): its text, forward slashes on Windows.
pub fn config_path_value(path: &Path) -> Result<String, String> {
    plain_text(path)
}

/// The path's text as git config spells it: UTF-8 with no control characters; on Windows with
/// forward slashes, a UNC share as `//server/share/…`, and no `\\?\` prefix.
fn plain_text(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or(NOT_TEXT)?;
    if text.chars().any(char::is_control) {
        return Err(NOT_TEXT.into());
    }
    Ok(slashes(text))
}

#[cfg(windows)]
fn slashes(text: &str) -> String {
    let text = match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(text).to_owned(),
    };
    text.replace('\\', "/")
}

#[cfg(not(windows))]
fn slashes(text: &str) -> String {
    text.to_owned()
}

/// The first line of every include file. It names no profile, so a rename writes nothing.
pub const FILE_HEADER: &str = "# Written by Glowfly for a profile's folder rules. Glowfly rewrites this file: edit the profile in Glowfly.";

/// A profile's include file: the header, then `[user]` with the name, email and signing key
/// that aren't blank, each quoted as git reads it (no section when all are blank).
pub fn file_text(identity: &Identity) -> String {
    let mut out = format!("{FILE_HEADER}\n");
    let keys =
        [("name", Some(identity.name.as_str())), ("email", Some(identity.email.as_str())), ("signingkey", identity.signing_key.as_deref())];
    let set: Vec<(&str, &str)> = keys.into_iter().filter_map(|(k, v)| v.filter(|v| !v.trim().is_empty()).map(|v| (k, v))).collect();
    if !set.is_empty() {
        out.push_str("[user]\n");
        for (key, value) in set {
            out.push_str(&format!("\t{key} = {}\n", quoted(value)));
        }
    }
    out
}

/// `value` in double quotes with git's escapes, so `#`, `;` and spaces at either end stay as
/// they are; control characters git has no escape for are left out.
fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `path` with the ASCII case of its deepest part that has letters swapped (the probe of
/// whether its file system folds case); None when no part has a letter.
pub(crate) fn flip(path: &Path) -> Option<PathBuf> {
    let parts: Vec<Component> = path.components().collect();
    let k =
        parts.iter().rposition(|c| matches!(c, Component::Normal(n) if n.to_string_lossy().chars().any(|c| c.is_ascii_alphabetic())))?;
    let Component::Normal(name) = parts[k] else { return None };
    let name = name.to_str()?;
    let swapped: String =
        name.chars().map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() }).collect();
    let mut out: PathBuf = parts[..k].iter().collect();
    out.push(swapped);
    out.extend(&parts[k + 1..]);
    Some(out)
}
