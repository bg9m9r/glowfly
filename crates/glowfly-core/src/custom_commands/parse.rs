//! Reading a commands file: strict TOML (unknown keys and wrong types are errors of the whole
//! file), then each command checked on its own, so a mistake in one drops only that one. Every
//! problem carries the line of the field it is about.

use super::{Command, Confirm, Kind, MAX_COMMANDS, MAX_NAME_CHARS, Needs, Place, Problem, ProblemKind};
use serde::Deserialize;
use toml::Spanned;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    command: Vec<Spanned<RawCommand>>,
}

/// `name` and `template` are optional here so a missing one is a problem of that command (at
/// its table's line) rather than of the whole file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCommand {
    name: Option<Spanned<String>>,
    kind: RawKind,
    template: Option<Spanned<String>>,
    show: Option<Spanned<Vec<RawPlace>>>,
    confirm: Option<RawConfirm>,
    key: Option<Spanned<String>>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawKind {
    Git,
    Shell,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RawPlace {
    Palette,
    Commit,
    Branch,
    File,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawConfirm {
    Ask(bool),
    Text(String),
}

/// The commands `text` holds and its problems. A TOML or format error loads nothing; a
/// command with a problem is left out. A leading UTF-8 byte order mark is skipped.
pub fn parse(text: &str) -> (Vec<Command>, Vec<Problem>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let raw: RawFile = match toml::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            let line = e.span().map(|s| line_of(text, s.start));
            return (Vec::new(), vec![Problem { line, kind: ProblemKind::Syntax(e.message().trim().to_owned()) }]);
        }
    };
    let mut commands = Vec::new();
    let mut problems = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (i, table) in raw.command.into_iter().enumerate() {
        let line = table_line(text, table.span().start);
        if i == MAX_COMMANDS {
            problems.push(Problem { line: Some(line), kind: ProblemKind::TooMany });
            break;
        }
        match command(text, line, table.into_inner(), &mut names) {
            Ok(c) => commands.push(c),
            Err(p) => problems.push(p),
        }
    }
    (commands, problems)
}

/// One command checked, or its first problem.
fn command(text: &str, line: usize, raw: RawCommand, names: &mut Vec<String>) -> Result<Command, Problem> {
    let at = |offset: usize| Some(line_of(text, offset));
    let problem = |line: Option<usize>, kind| Problem { line, kind };
    let Some(name) = raw.name else { return Err(problem(Some(line), ProblemKind::EmptyName)) };
    let name_line = at(name.span().start);
    let name = crate::text::printable(name.get_ref());
    if name.is_empty() {
        return Err(problem(name_line, ProblemKind::EmptyName));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(problem(name_line, ProblemKind::LongName));
    }
    if names.contains(&name) {
        return Err(problem(name_line, ProblemKind::DuplicateName(name)));
    }
    names.push(name.clone());
    let Some(template) = raw.template else { return Err(problem(Some(line), ProblemKind::EmptyTemplate)) };
    let template_line = at(template.span().start);
    let template = template.get_ref().trim().to_owned();
    if template.is_empty() {
        return Err(problem(template_line, ProblemKind::EmptyTemplate));
    }
    let kind = match raw.kind {
        RawKind::Git => Kind::Git,
        RawKind::Shell => Kind::Shell,
    };
    let needs: Needs = super::expand::check(kind, &template).map_err(|k| problem(template_line, k))?;
    let show = match raw.show {
        None => vec![Place::Palette],
        Some(s) => {
            let mut places = Vec::new();
            for p in s.get_ref() {
                let p = match p {
                    RawPlace::Palette => Place::Palette,
                    RawPlace::Commit => Place::Commit,
                    RawPlace::Branch => Place::Branch,
                    RawPlace::File => Place::File,
                };
                if !places.contains(&p) {
                    places.push(p);
                }
            }
            if places.is_empty() {
                return Err(problem(at(s.span().start), ProblemKind::NoPlaces));
            }
            places
        }
    };
    let confirm = match raw.confirm {
        None | Some(RawConfirm::Ask(false)) => Confirm::No,
        Some(RawConfirm::Ask(true)) => Confirm::Default,
        Some(RawConfirm::Text(s)) if s.trim().is_empty() => Confirm::Default,
        Some(RawConfirm::Text(s)) => Confirm::Text(s.trim().to_owned()),
    };
    let (key, key_line) = match raw.key {
        Some(k) => (Some(k.get_ref().trim().to_owned()), line_of(text, k.span().start)),
        None => (None, 0),
    };
    Ok(Command { name, kind, template, show, confirm, key, key_line, line, needs })
}

/// The line of a `[[command]]` table whose span starts at `offset`.
fn table_line(text: &str, offset: usize) -> usize {
    line_of(text, offset)
}

/// The 1-based line of the byte `offset` in `text` (a CRLF ends one line).
pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end].iter().filter(|b| **b == b'\n').count() + 1
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
