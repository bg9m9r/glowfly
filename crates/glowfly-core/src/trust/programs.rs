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
            if let Some(p) = path_of(workdir, home.as_deref(), &word) {
                add(p);
            }
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

/// The words of `text` that may name a file: each word as a shell splits it (quotes taken
/// off), and each piece of it between shell punctuation and spaces, with a leading `!` (an
/// alias or credential helper run by the shell) or `ext::` (a transport's command) dropped.
fn candidates(text: &str) -> Vec<String> {
    let text = HERE.iter().fold(text.trim().trim_start_matches('!').to_owned(), |t, h| t.replace(h, "./"));
    let mut out = Vec::new();
    let mut push = |w: &str| {
        let w = w.trim_start_matches('!');
        let w = w.get(..5).filter(|p| p.eq_ignore_ascii_case("ext::")).map_or(w, |_| &w[5..]);
        if !w.is_empty() && !out.iter().any(|o: &String| o == w) {
            out.push(w.to_owned());
        }
    };
    for word in shell_words(&text) {
        push(&word);
        for piece in word.split(|c: char| c.is_whitespace() || BREAKS.contains(&c)) {
            push(piece);
        }
    }
    out
}

/// `text` split as a shell splits words: on spaces outside quotes, with the quotes removed.
fn shell_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            (_, c) => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Where `word` points when it may be a file the command runs or reads: one with a `/` or `\`
/// (a quoted one with spaces too, but no shell punctuation), or one that names something in
/// the working tree now. Relative to the working tree, which is where git runs the
/// repository's commands. None for a URL, an option, a variable other than the working
/// folder's, or the working tree itself.
fn path_of(workdir: &Path, home: Option<&Path>, word: &str) -> Option<PathBuf> {
    if word.contains("://") || word.starts_with(['-', '%', '$']) {
        return None;
    }
    let word = word.replace('\\', "/");
    let path = if word.starts_with('~') { crate::paths::expand_home(&word, home) } else { workdir.join(&word) };
    // A whole word with shell punctuation in it is a command line (`./a; ./b`): its pieces count.
    let named = word.contains('/') && !word.contains(BREAKS);
    let exists = std::fs::symlink_metadata(&path).is_ok();
    let root = crate::paths::resolve(&path) == crate::paths::resolve(workdir);
    ((named || exists) && !root).then_some(path)
}

#[cfg(test)]
#[path = "programs_tests.rs"]
mod tests;
