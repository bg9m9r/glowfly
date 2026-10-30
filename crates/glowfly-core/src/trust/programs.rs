//! Config programs that run files from the working tree. A setting such as
//! `core.fsmonitor=./tools/fsm.sh`, `filter.x.smudge=python tools/smudge.py`,
//! `credential.helper=!./bin/creds` or `include.path=../shared.cfg` names a file the branch
//! carries: a checkout or pull that changes it changes what git runs (or which settings it
//! reads) without changing the setting's text. `scan` lists the files such settings name, so
//! the scan follows them as it follows a hooks folder in the working tree (`follow.rs`): the
//! version HEAD tracks is in the trust print, and a command that would bring in another is held.
//!
//! A security check, so it overmatches: every word of a command that may be a path (it has a
//! `/` or `\`, or names something in the working tree now) counts, not only the first, since
//! `sh tools/x.sh` runs a tracked script as surely as `./tools/x.sh` does.

use super::ConfigCommand;
use git2::Repository;
use std::path::{Path, PathBuf};

/// Characters that end a word inside a shell command (`a;./b`, `$(./x)`, `--file=./c`).
const BREAKS: &[char] = &[';', '|', '&', '(', ')', '<', '>', '`', '"', '\'', '=', ','];

/// How a command may write its working folder (the working tree): read as `./`.
const HERE: &[&str] = &["$PWD/", "${PWD}/", "$(pwd)/", "`pwd`/", "$(git rev-parse --show-toplevel)/"];

/// The files in `repo`'s working tree (absolute paths, maybe missing) that the settings
/// `config` run, and the files its config includes (`stamp::includes`); `follow::find` keeps
/// those inside the working tree. An include of a file in the working tree is added to
/// `config` as a setting of its own (`include.path`): the branch then chooses settings, so it
/// is a reason to ask by itself, like a setting that runs a program.
pub(super) fn scan(repo: &Repository, config: &mut Vec<ConfigCommand>) -> Vec<PathBuf> {
    let Some(workdir) = repo.workdir() else { return Vec::new() };
    let home = crate::settings::home_dir();
    let mut out: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for c in config.iter() {
        let Some(text) = command_text(c) else { continue };
        for word in candidates(text) {
            path_of(workdir, home.as_deref(), &word).into_iter().for_each(&mut add);
        }
    }
    for inc in super::stamp::include_entries(repo) {
        let in_tree = inc.paths.iter().any(|p| !super::follow::inside(repo, workdir, p).is_empty());
        let key = match inc.name.get(..10) {
            Some(head) if head.eq_ignore_ascii_case("includeif.") => format!("includeIf.{}", &inc.name[10..]),
            _ => "include.path".to_owned(),
        };
        let setting = ConfigCommand { key, value: inc.value };
        if in_tree && !config.contains(&setting) {
            config.push(setting);
        }
        inc.paths.into_iter().for_each(&mut add);
    }
    out
}

/// The command a setting runs, as written; None for one that names no command (core.hooksPath,
/// whose hooks `follow.rs` follows itself; protocol.allow; an include).
fn command_text(c: &ConfigCommand) -> Option<&str> {
    let key = c.key.to_ascii_lowercase();
    if key == "core.hookspath" || key.starts_with("protocol.") || key.starts_with("includeif.") {
        return None;
    }
    // url.<base>.insteadOf: the command is in the URL the key names.
    if let Some(rest) = c.key.strip_prefix("url.") {
        return rest.rfind('.').map(|i| &rest[..i]);
    }
    Some(&c.value)
}

/// A word of a command that may name a file, with what the shell would do to it unquoted.
#[derive(Clone, Debug, PartialEq)]
struct Word {
    text: String,
    /// It has a `*`, `?`, `[` or `{` outside quotes: the shell expands it to whatever matches.
    glob: bool,
    /// It has a `\` outside quotes: the shell drops it (`tools\x.sh` runs `toolsx.sh`), while a
    /// program given it on Windows reads a folder separator.
    escape: bool,
}

/// `text` with each way of writing the working folder (`HERE`) read as `./`.
fn here(text: &str) -> String {
    HERE.iter().fold(text.to_owned(), |t, h| t.replace(h, "./"))
}

/// The words of `text` that may name a file: each word as a shell splits it (quotes taken
/// off, then the working folder's spellings read as `./` again, so `"$PWD"/x` counts as
/// `"$PWD/x"` does), and each piece of it between shell punctuation and spaces, with a leading
/// `!` (an alias or credential helper run by the shell) or `ext::` (a transport's command)
/// dropped.
fn candidates(text: &str) -> Vec<Word> {
    let text = here(text.trim().trim_start_matches('!'));
    let mut out: Vec<Word> = Vec::new();
    let mut push = |w: &str, from: &Word| {
        let w = w.trim_start_matches('!');
        let w = w.get(..5).filter(|p| p.eq_ignore_ascii_case("ext::")).map_or(w, |_| &w[5..]);
        let word = Word { text: w.to_owned(), ..from.clone() };
        if !w.is_empty() && !out.contains(&word) {
            out.push(word);
        }
    };
    for word in shell_words(&text) {
        let word = Word { text: here(&word.text), ..word };
        push(&word.text, &word);
        for piece in word.text.split(|c: char| c.is_whitespace() || BREAKS.contains(&c)) {
            push(piece, &word);
        }
    }
    out
}

/// `text` split as a shell splits words: on spaces outside quotes, with the quotes removed.
fn shell_words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut word = Word { text: String::new(), glob: false, escape: false };
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !word.text.is_empty() {
                    out.push(std::mem::replace(&mut word, Word { text: String::new(), glob: false, escape: false }));
                }
            }
            (q, c) => {
                word.glob |= q.is_none() && matches!(c, '*' | '?' | '[' | '{');
                word.escape |= q.is_none() && c == '\\';
                word.text.push(c);
            }
        }
    }
    if !word.text.is_empty() {
        out.push(word);
    }
    out
}

/// Where `word` points when it may be a file the command runs or reads: one with a `/` or `\`
/// (a quoted one with spaces too, but no shell punctuation), or one that names something in
/// the working tree now. Relative to the working tree, which is where git runs the
/// repository's commands. Nothing for a URL, an option, a variable other than the working
/// folder's, or the working tree itself.
///
/// What the shell would change is overmatched: an unquoted `\` gives both readings (dropped,
/// and a folder separator; each is a real path, whose version is its nearest tracked folder's
/// when the file isn't tracked), and a glob the nearest folder at or above it that the working
/// tree has now (the whole tree when none), whose version covers every match.
fn path_of(workdir: &Path, home: Option<&Path>, word: &Word) -> Vec<PathBuf> {
    let text = word.text.as_str();
    if text.contains("://") || text.starts_with(['-', '%', '$']) {
        return Vec::new();
    }
    let at = |w: &str| if w.starts_with('~') { crate::paths::expand_home(w, home) } else { workdir.join(w) };
    let literal = |w: &str| {
        let path = at(w);
        // A whole word with shell punctuation in it is a command line (`./a; ./b`): its pieces count.
        let named = w.contains('/') && !w.contains(BREAKS);
        let exists = std::fs::symlink_metadata(&path).is_ok();
        let root = crate::paths::resolve(&path) == crate::paths::resolve(workdir);
        ((named || exists) && !root).then_some(path)
    };
    // `name:stream` opens `name` on Windows (`x.sh::$DATA` is x.sh), and a `:` elsewhere makes a
    // part only the file system resolves (follow.rs follows the whole tree for it): the part
    // before it is what is named. A drive's `C:` stays.
    let text = match text.char_indices().find(|&(i, c)| c == ':' && !(i == 1 && text.starts_with(|d: char| d.is_ascii_alphabetic()))) {
        Some((i, _)) => &text[..i],
        None => text,
    };
    let slashed = text.replace('\\', "/");
    let mut out: Vec<PathBuf> = literal(&slashed).into_iter().collect();
    if word.escape {
        let mut dropped = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            dropped.extend(if c == '\\' { chars.next() } else { Some(c) });
        }
        // The shell's reading, with or without a `/` left in it.
        let path = at(&dropped);
        if !dropped.is_empty() && crate::paths::resolve(&path) != crate::paths::resolve(workdir) && !out.contains(&path) {
            out.push(path);
        }
    }
    if word.glob {
        // The literal part before the first glob, up to its last folder.
        let fixed = slashed.split(['*', '?', '[', '{']).next().unwrap_or_default();
        let dir = fixed.rfind('/').map_or("", |i| &fixed[..i]);
        let mut p = if dir.is_empty() { workdir.to_path_buf() } else { at(dir) };
        while std::fs::symlink_metadata(&p).is_err() {
            if !p.pop() {
                break;
            }
        }
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
#[path = "programs_tests.rs"]
mod tests;
