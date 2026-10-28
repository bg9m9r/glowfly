//! Placeholders in command templates: checking them when a file loads, and filling them in when
//! a command runs. A `{word}` right after `@` (git's `@{u}`, `HEAD@{1}`, `stash@{0}`) or `$`
//! (the shell's `${HOME}`) is never a placeholder.
//!
//! git templates are split into arguments first (the External Tools rules: whitespace
//! separates, double quotes group, no escapes), then filled in, so a value never splits or
//! joins arguments; `{files}` is an argument of its own and becomes one per file, and a git
//! command with `{files}` starts with `--literal-pathspecs` so a `[` or `*` in a name matches
//! only that file. Shell templates get every value single-quoted for sh. Pure: the OS is the
//! `windows` parameter.

use super::{Command, Confirm, Kind, Needs, ProblemKind};
use crate::external_tools::split;
use crate::native_path::native;
use crate::rebase::{shell_quote, shell_quote_for};
use std::path::Path;

/// The placeholders a template may name.
pub const KNOWN: [&str; 4] = ["{branch}", "{sha}", "{files}", "{repo}"];

/// The placeholders in `s`: each one's byte offset and its text with braces (`{name}`).
fn placeholders(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            let start = i;
            i += 1;
            if bytes[start] != b'{' || (start > 0 && matches!(bytes[start - 1], b'@' | b'$')) {
                continue;
            }
            let len = bytes[start + 1..].iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'_').count();
            let close = start + 1 + len;
            if len > 0 && bytes.get(close) == Some(&b'}') {
                i = close + 1;
                return Some((start, &s[start..=close]));
            }
        }
        None
    })
}

/// Which placeholders `template` names, or why it can't be used: an unknown placeholder, an
/// unmatched quote, `{files}` inside a longer git argument, or (shell) a placeholder inside the
/// template's own quotes, where Glowfly's quoting would end them.
pub(super) fn check(kind: Kind, template: &str) -> Result<Needs, ProblemKind> {
    let mut needs = Needs::default();
    for (_, p) in placeholders(template) {
        match p {
            "{branch}" => needs.branch = true,
            "{sha}" => needs.sha = true,
            "{files}" => needs.files = true,
            "{repo}" => needs.repo = true,
            other => return Err(ProblemKind::UnknownPlaceholder(other.to_owned())),
        }
    }
    match kind {
        Kind::Git => {
            let args = split(strip_git_word(template)).map_err(|_| ProblemKind::UnmatchedQuote)?;
            if args.iter().any(|a| a != "{files}" && placeholders(a).any(|(_, p)| p == "{files}")) {
                return Err(ProblemKind::FilesNotAlone);
            }
        }
        Kind::Shell => sh_quotes(template)?,
    }
    Ok(needs)
}

/// Fails when a placeholder in the sh text `t` is inside quotes or escaped, or a quote is left
/// open. Single quotes take everything literally; double quotes allow `\` escapes; outside
/// quotes `\` escapes the next character.
fn sh_quotes(t: &str) -> Result<(), ProblemKind> {
    #[derive(PartialEq)]
    enum In {
        Plain,
        Single,
        Double,
    }
    let starts: Vec<(usize, &str)> = placeholders(t).collect();
    let mut state = In::Plain;
    let mut escaped = false;
    for (i, c) in t.char_indices() {
        if let Some((_, name)) = starts.iter().find(|(s, _)| *s == i)
            && (state != In::Plain || escaped)
        {
            return Err(ProblemKind::QuotedPlaceholder((*name).to_owned()));
        }
        if escaped {
            escaped = false;
            continue;
        }
        match (&state, c) {
            (In::Plain, '\\') | (In::Double, '\\') => escaped = true,
            (In::Plain, '\'') => state = In::Single,
            (In::Plain, '"') => state = In::Double,
            (In::Single, '\'') | (In::Double, '"') => state = In::Plain,
            _ => {}
        }
    }
    if state != In::Plain { Err(ProblemKind::UnmatchedQuote) } else { Ok(()) }
}

/// `t` without one leading `git` word, so `git log` and `log` are the same template.
fn strip_git_word(t: &str) -> &str {
    let t = t.trim_start();
    match t.strip_prefix("git") {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest,
        _ => t,
    }
}

/// git's options for how every pathspec is read. A git template naming one of them chose for
/// itself, and git refuses `--literal-pathspecs` alongside the others.
const PATHSPEC_OPTIONS: [&str; 4] = ["--literal-pathspecs", "--glob-pathspecs", "--noglob-pathspecs", "--icase-pathspecs"];

/// What the placeholders hold where a command runs.
#[derive(Clone, Copy, Debug)]
pub struct Vars<'a> {
    /// The repository's working folder.
    pub repo: &'a Path,
    pub branch: Option<&'a str>,
    pub sha: Option<&'a str>,
    pub files: &'a [String],
}

/// A placeholder the place a command runs from can't fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// No branch is checked out (a detached HEAD).
    Branch,
    /// No commit (a branch with none yet).
    Sha,
    /// No file.
    Files,
    /// The folder's name isn't valid Unicode, so it can't be a git argument.
    Repo,
}

/// A command filled in: git's arguments (after `git`), or the sh script.
#[derive(Clone, Debug, PartialEq)]
pub enum Expanded {
    Git(Vec<String>),
    Shell(String),
}

impl Expanded {
    /// The command as text for the screen: `git` and its arguments (one with spaces or quotes
    /// quoted for sh), or the script.
    pub fn preview(&self) -> String {
        match self {
            Expanded::Git(args) => {
                let shown = args.iter().map(|a| {
                    let plain = !a.is_empty() && !a.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '\\'));
                    if plain { a.clone() } else { shell_quote(a) }
                });
                std::iter::once("git".to_owned()).chain(shown).collect::<Vec<_>>().join(" ")
            }
            Expanded::Shell(s) => s.clone(),
        }
    }
}

/// What a confirmation asks: the app's own "Run <name>?", or the command's question.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmText {
    Default,
    Text(String),
}

impl Command {
    /// The first placeholder `v` can't fill: branch, then sha, files and (git) the folder.
    pub fn missing(&self, v: &Vars) -> Option<Missing> {
        let n = self.needs;
        if n.branch && v.branch.is_none() {
            Some(Missing::Branch)
        } else if n.sha && v.sha.is_none() {
            Some(Missing::Sha)
        } else if n.files && v.files.is_empty() {
            Some(Missing::Files)
        } else if n.repo && self.kind == Kind::Git && native(v.repo).to_str().is_none() {
            Some(Missing::Repo)
        } else {
            None
        }
    }

    /// The command with `v` filled in; `windows` gives `{repo}` forward slashes in sh text.
    pub fn expand(&self, v: &Vars, windows: bool) -> Result<Expanded, Missing> {
        if let Some(m) = self.missing(v) {
            return Err(m);
        }
        match self.kind {
            Kind::Git => {
                let repo = native(v.repo);
                let repo = repo.to_str().unwrap_or_default();
                let words = split(strip_git_word(&self.template)).unwrap_or_default();
                let mut out = Vec::new();
                if self.needs.files && !words.iter().any(|w| PATHSPEC_OPTIONS.contains(&w.as_str())) {
                    out.push("--literal-pathspecs".to_owned());
                }
                for arg in words {
                    if arg == "{files}" {
                        out.extend(v.files.iter().map(|f| safe_path(f, true)));
                        continue;
                    }
                    out.push(fill(&arg, |p| match p {
                        "{branch}" => v.branch.map(str::to_owned),
                        "{sha}" => v.sha.map(str::to_owned),
                        "{repo}" => Some(repo.to_owned()),
                        _ => None,
                    }));
                }
                Ok(Expanded::Git(out))
            }
            Kind::Shell => Ok(Expanded::Shell(fill(&self.template, |p| match p {
                "{branch}" => v.branch.map(|b| shell_quote_for(b, false)),
                "{sha}" => v.sha.map(|s| shell_quote_for(s, false)),
                "{files}" => Some(v.files.iter().map(|f| shell_quote_for(&safe_path(f, false), false)).collect::<Vec<_>>().join(" ")),
                "{repo}" => Some(shell_quote_for(&native(v.repo).to_string_lossy(), windows)),
                _ => None,
            }))),
        }
    }

    /// The confirmation to ask before running, with placeholders filled in plainly (files
    /// joined by ", "); None when it runs without asking.
    pub fn confirm_text(&self, v: &Vars) -> Option<ConfirmText> {
        match &self.confirm {
            Confirm::No => None,
            Confirm::Default => Some(ConfirmText::Default),
            Confirm::Text(t) => Some(ConfirmText::Text(fill(t, |p| match p {
                "{branch}" => Some(v.branch.unwrap_or_default().to_owned()),
                "{sha}" => Some(v.sha.unwrap_or_default().to_owned()),
                "{files}" => Some(v.files.join(", ")),
                "{repo}" => Some(crate::native_path::shown(v.repo)),
                _ => None,
            }))),
        }
    }
}

/// `s` with each placeholder `value` knows replaced, in one pass (a value is never filled in
/// again); others stay as written.
fn fill(s: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut done = 0;
    for (at, p) in placeholders(s) {
        if let Some(v) = value(p) {
            out.push_str(&s[done..at]);
            out.push_str(&v);
            done = at + p.len();
        }
    }
    out.push_str(&s[done..]);
    out
}

/// `p` as a path argument that can't be read as an option (a leading `-`) or, for git,
/// pathspec magic (a leading `:`): `./` goes in front.
fn safe_path(p: &str, git: bool) -> String {
    if p.starts_with('-') || (git && p.starts_with(':')) { format!("./{p}") } else { p.to_owned() }
}

#[cfg(test)]
#[path = "expand_tests.rs"]
mod tests;
