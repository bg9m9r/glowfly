//! The actions a session takes and their case text: one action per line, a kebab-case keyword
//! and its arguments. Strings are double-quoted with `\"` and `\\` escapes, line counts are
//! written `+3`. `parse` reads back what `Display` writes.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    SelectWip,
    Stage(String),
    Unstage(String),
    StageAll,
    UnstageAll,
    StageHunk(String),
    Commit(String),
    Amend,
    CreateBranch(String),
    Checkout(String),
    DeleteBranch(String),
    Merge(String),
    Rebase(String),
    Stash,
    Unstash,
    DropStash,
    Undo,
    Redo,
    Discard(String),
    ResolveOurs(String),
    ResolveTheirs(String),
    Continue,
    Abort,
    Refresh,
    SwitchTab(String),
    CloseTab(String),
    /// A modal's button, by its label (Cancel, "Stash and check out", Delete...).
    Confirm(String),
    Outside(Outside),
}

/// A change made to the repository from outside the app, as another program would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outside {
    Write { path: String, lines: u8 },
    Append { path: String, lines: u8 },
    Create { path: String, lines: u8 },
    Delete { path: String },
    GitCommit(String),
    GitCheckout(String),
    GitBranch(String),
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Action::*;
        let (word, arg): (&str, Option<&String>) = match self {
            SelectWip => ("select-wip", None),
            Stage(a) => ("stage", Some(a)),
            Unstage(a) => ("unstage", Some(a)),
            StageAll => ("stage-all", None),
            UnstageAll => ("unstage-all", None),
            StageHunk(a) => ("stage-hunk", Some(a)),
            Commit(a) => ("commit", Some(a)),
            Amend => ("amend", None),
            CreateBranch(a) => ("create-branch", Some(a)),
            Checkout(a) => ("checkout", Some(a)),
            DeleteBranch(a) => ("delete-branch", Some(a)),
            Merge(a) => ("merge", Some(a)),
            Rebase(a) => ("rebase", Some(a)),
            Stash => ("stash", None),
            Unstash => ("unstash", None),
            DropStash => ("drop-stash", None),
            Undo => ("undo", None),
            Redo => ("redo", None),
            Discard(a) => ("discard", Some(a)),
            ResolveOurs(a) => ("resolve-ours", Some(a)),
            ResolveTheirs(a) => ("resolve-theirs", Some(a)),
            Continue => ("continue", None),
            Abort => ("abort", None),
            Refresh => ("refresh", None),
            SwitchTab(a) => ("switch-tab", Some(a)),
            CloseTab(a) => ("close-tab", Some(a)),
            Confirm(a) => ("confirm", Some(a)),
            Outside(o) => return write_outside(f, o),
        };
        match arg {
            Some(a) => write!(f, "{word} {}", quoted(a)),
            None => f.write_str(word),
        }
    }
}

fn write_outside(f: &mut fmt::Formatter<'_>, o: &Outside) -> fmt::Result {
    match o {
        Outside::Write { path, lines } => write!(f, "outside write {} +{lines}", quoted(path)),
        Outside::Append { path, lines } => write!(f, "outside append {} +{lines}", quoted(path)),
        Outside::Create { path, lines } => write!(f, "outside create {} +{lines}", quoted(path)),
        Outside::Delete { path } => write!(f, "outside delete {}", quoted(path)),
        Outside::GitCommit(m) => write!(f, "outside git-commit {}", quoted(m)),
        Outside::GitCheckout(b) => write!(f, "outside git-checkout {}", quoted(b)),
        Outside::GitBranch(b) => write!(f, "outside git-branch {}", quoted(b)),
    }
}

/// One word of a line: a bare word (keywords, `+3`) or a string.
#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Text(String),
}

fn tokens(line: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some(e @ ('"' | '\\')) => s.push(e),
                        _ => return Err("bad escape".into()),
                    },
                    Some(c) => s.push(c),
                    None => return Err("unterminated string".into()),
                }
            }
            out.push(Token::Text(s));
        } else {
            let mut w = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '"' {
                    break;
                }
                w.push(c);
                chars.next();
            }
            out.push(Token::Word(w));
        }
    }
    Ok(out)
}

/// The action written on `line`.
pub(super) fn parse(line: &str) -> Result<Action, String> {
    let bad = |why: &str| format!("{why}: {line:?}");
    let toks = tokens(line).map_err(|e| bad(&e))?;
    let mut it = toks.into_iter();
    let Some(Token::Word(word)) = it.next() else { return Err(bad("no action")) };
    let rest: Vec<Token> = it.collect();
    if word == "outside" {
        return parse_outside(&rest).ok_or_else(|| bad("unknown outside action"));
    }
    use Action::*;
    let action = match (word.as_str(), rest.is_empty()) {
        ("select-wip", true) => SelectWip,
        ("stage-all", true) => StageAll,
        ("unstage-all", true) => UnstageAll,
        ("amend", true) => Amend,
        ("stash", true) => Stash,
        ("unstash", true) => Unstash,
        ("drop-stash", true) => DropStash,
        ("undo", true) => Undo,
        ("redo", true) => Redo,
        ("continue", true) => Continue,
        ("abort", true) => Abort,
        ("refresh", true) => Refresh,
        (w, false) => {
            let ctor: fn(String) -> Action = match w {
                "stage" => Stage,
                "unstage" => Unstage,
                "stage-hunk" => StageHunk,
                "commit" => Commit,
                "create-branch" => CreateBranch,
                "checkout" => Checkout,
                "delete-branch" => DeleteBranch,
                "merge" => Merge,
                "rebase" => Rebase,
                "discard" => Discard,
                "resolve-ours" => ResolveOurs,
                "resolve-theirs" => ResolveTheirs,
                "switch-tab" => SwitchTab,
                "close-tab" => CloseTab,
                "confirm" => Confirm,
                _ => return Err(bad("unknown action")),
            };
            match rest.as_slice() {
                [Token::Text(s)] => ctor(s.clone()),
                _ => return Err(bad("expected one string")),
            }
        }
        _ => return Err(bad("unknown action")),
    };
    Ok(action)
}

fn parse_outside(rest: &[Token]) -> Option<Action> {
    use Token::{Text, Word};
    let count = |w: &str| w.strip_prefix('+')?.parse::<u8>().ok();
    let o = match rest {
        [Word(k), Text(path), Word(n)] => {
            let (path, lines) = (path.clone(), count(n)?);
            match k.as_str() {
                "write" => Outside::Write { path, lines },
                "append" => Outside::Append { path, lines },
                "create" => Outside::Create { path, lines },
                _ => return None,
            }
        }
        [Word(k), Text(s)] => match k.as_str() {
            "delete" => Outside::Delete { path: s.clone() },
            "git-commit" => Outside::GitCommit(s.clone()),
            "git-checkout" => Outside::GitCheckout(s.clone()),
            "git-branch" => Outside::GitBranch(s.clone()),
            _ => return None,
        },
        _ => return None,
    };
    Some(Action::Outside(o))
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
