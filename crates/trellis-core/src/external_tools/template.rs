//! Argument templates: splitting one into arguments and filling in its placeholders.
//!
//! Templates are split before placeholders are filled in, so a path with spaces is always one
//! argument, and nothing goes through a shell. Whitespace separates arguments and double quotes
//! group text (the quotes themselves are dropped); there is no escape character, so backslashes
//! in Windows paths are taken literally. Each tool has its own list of placeholders.

use crate::native_path::native;
use std::ffi::OsString;
use std::path::Path;

/// Why a template can't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TemplateError {
    #[error("unmatched quote")]
    UnmatchedQuote,
}

/// The placeholders an editor or terminal template may name.
pub const EDITOR_PLACEHOLDERS: [&str; 3] = ["{path}", "{line}", "{repo}"];
/// The placeholders a diff tool template may name.
pub const DIFF_PLACEHOLDERS: [&str; 4] = ["{local}", "{remote}", "{path}", "{repo}"];
/// The placeholders a merge tool template may name.
pub const MERGE_PLACEHOLDERS: [&str; 5] = ["{base}", "{ours}", "{theirs}", "{output}", "{repo}"];

/// Splits `template` into arguments: whitespace separates them, double quotes group text with
/// spaces (`a"b c"d` is `ab cd`, `""` is one empty argument), and backslashes are plain text.
pub fn split(template: &str) -> Result<Vec<String>, TemplateError> {
    let mut out = Vec::new();
    let mut current = String::new();
    // Whether an argument has started (a quote starts one even if it stays empty).
    let mut started = false;
    let mut quoted = false;
    for ch in template.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(TemplateError::UnmatchedQuote);
    }
    if started {
        out.push(current);
    }
    Ok(out)
}

/// What a placeholder stands for: a path (filled in in this system's spelling) or text.
#[derive(Clone, Debug)]
pub enum Value<'a> {
    Path(&'a Path),
    Text(String),
}

/// The arguments of `template` with the placeholders named in `vars` (`("{path}", value)`)
/// filled in, in one pass (a value that itself contains a placeholder is not expanded again).
/// Unknown `{names}` are kept as typed. Paths go in in this system's spelling (`C:\x\y`, never
/// libgit2's `C:/x/y/`).
pub fn expand_with(template: &str, vars: &[(&str, Value)]) -> Result<Vec<OsString>, TemplateError> {
    Ok(split(template)?.iter().map(|a| substitute(a, vars)).collect())
}

fn substitute(arg: &str, vars: &[(&str, Value)]) -> OsString {
    let mut out = OsString::new();
    let mut rest = arg;
    'scan: while let Some(i) = rest.find('{') {
        out.push(&rest[..i]);
        let at = &rest[i..];
        for (name, value) in vars {
            if let Some(r) = at.strip_prefix(name) {
                match value {
                    Value::Path(p) => out.push(native(p)),
                    Value::Text(t) => out.push(t),
                }
                rest = r;
                continue 'scan;
            }
        }
        out.push("{");
        rest = &at[1..];
    }
    out.push(rest);
    out
}

/// What the editor and terminal placeholders stand for. For repository and terminal templates
/// `path` is the repository folder and `line` is 1.
#[derive(Clone, Copy, Debug)]
pub struct Vars<'a> {
    /// `{path}`: the file's full path.
    pub path: &'a Path,
    /// `{line}`: the line number.
    pub line: u32,
    /// `{repo}`: the repository folder.
    pub repo: &'a Path,
}

/// The arguments of an editor or terminal `template` with its placeholders filled in from `v`
/// (see [`expand_with`]).
pub fn expand(template: &str, v: &Vars) -> Result<Vec<OsString>, TemplateError> {
    expand_with(template, &[("{path}", Value::Path(v.path)), ("{line}", Value::Text(v.line.to_string())), ("{repo}", Value::Path(v.repo))])
}

/// What the diff tool placeholders stand for.
#[derive(Clone, Copy, Debug)]
pub struct DiffVars<'a> {
    /// `{local}`: the older version.
    pub local: &'a Path,
    /// `{remote}`: the newer version.
    pub remote: &'a Path,
    /// `{path}`: the file's full path in the working tree (it may not exist there).
    pub path: &'a Path,
    /// `{repo}`: the repository folder.
    pub repo: &'a Path,
}

impl DiffVars<'_> {
    /// The arguments of `template` with these filled in (see [`expand_with`]).
    pub fn expand(&self, template: &str) -> Result<Vec<OsString>, TemplateError> {
        let vars = [
            ("{local}", Value::Path(self.local)),
            ("{remote}", Value::Path(self.remote)),
            ("{path}", Value::Path(self.path)),
            ("{repo}", Value::Path(self.repo)),
        ];
        expand_with(template, &vars)
    }
}

/// What the merge tool placeholders stand for.
#[derive(Clone, Copy, Debug)]
pub struct MergeVars<'a> {
    /// `{base}`: the common ancestor (an empty file when there is none).
    pub base: &'a Path,
    /// `{ours}`: the checked-out side.
    pub ours: &'a Path,
    /// `{theirs}`: the side being merged in.
    pub theirs: &'a Path,
    /// `{output}`: the working file the result is saved to.
    pub output: &'a Path,
    /// `{repo}`: the repository folder.
    pub repo: &'a Path,
}

impl MergeVars<'_> {
    /// The arguments of `template` with these filled in (see [`expand_with`]).
    pub fn expand(&self, template: &str) -> Result<Vec<OsString>, TemplateError> {
        let vars = [
            ("{base}", Value::Path(self.base)),
            ("{ours}", Value::Path(self.ours)),
            ("{theirs}", Value::Path(self.theirs)),
            ("{output}", Value::Path(self.output)),
            ("{repo}", Value::Path(self.repo)),
        ];
        expand_with(template, &vars)
    }
}

/// Each `{name}` in `template` that is not one of `known`, once each, in the order they appear.
pub fn unknown_placeholders(template: &str, known: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        rest = &rest[i + 1..];
        let name_len = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        if name_len > 0 && rest[name_len..].starts_with('}') {
            let word = format!("{{{}}}", &rest[..name_len]);
            if !known.contains(&word.as_str()) && !out.contains(&word) {
                out.push(word);
            }
        }
    }
    out
}

/// Each of `wanted` that appears in no argument of `template`, in `wanted`'s order. A template
/// that can't be split names none (its unmatched quote is reported instead).
pub fn missing_placeholders(template: &str, wanted: &[&str]) -> Vec<String> {
    let Ok(args) = split(template) else { return Vec::new() };
    wanted.iter().filter(|w| !args.iter().any(|a| a.contains(*w))).map(|w| (*w).to_owned()).collect()
}
