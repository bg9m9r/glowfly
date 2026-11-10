//! The steps left in a rebase, cherry-pick or revert in progress that run a program when it
//! continues (`TrustScan::steps`): each `exec` step of its todo, and a merge strategy that
//! isn't one of git's own, which git runs as `git merge-<name>`. Read as bytes with nothing
//! run, and overmatched: a line git would refuse is listed too, and a file that is too large,
//! isn't a plain file or may not be followed is listed by its name with its digest.
//!
//! A step's text is kept byte for byte as git and sh take it: git drops only spaces and tabs
//! before a line and after its keyword, and one CR at its end, and sh splits words on nothing
//! else, so no other character is trimmed (`text_of` keeps bytes that aren't UTF-8 apart).
//!
//! The `exec` lines Glowfly's own rebases add around a reworded commit aren't listed while
//! they are exactly as Glowfly writes them for this repository (`rebase::own_files`) and the
//! files they name in its message folder are plain files, or not there yet: those lines only
//! record HEAD there and amend a message from there.

use super::digest::{self, Reading};
use super::keys::text_of;
use git2::Repository;
use std::path::Path;

/// The todo of a rebase, and of a cherry-pick or revert of several commits, in the git dir.
const TODOS: [&str; 2] = ["rebase-merge/git-rebase-todo", "sequencer/todo"];
/// Where a rebase keeps its merge strategy's name.
const STRATEGY: &str = "rebase-merge/strategy";
/// A cherry-pick's or revert's options, in git's config format.
const OPTS: &str = "sequencer/opts";

/// Every file `find` reads, relative to the git dir, for the scan's stamp.
pub(super) const FILES: [&str; 4] = [TODOS[0], TODOS[1], STRATEGY, OPTS];

/// The merge strategies that come with git.
const STRATEGIES: [&str; 6] = ["ort", "recursive", "resolve", "octopus", "ours", "subtree"];

/// The most bytes read of one of `FILES`.
pub(super) const MAX_BYTES: u64 = digest::MAX_BYTES;

/// One of `FILES` as read.
enum State {
    Missing,
    Bytes(Vec<u8>),
    /// There, but too large, not a plain file or not to be followed: the step that stands for it.
    Unread(String),
}

/// The file `name` in the git dir `git`: a plain file of at most `MAX_BYTES`, read only where
/// it may be followed from the git dir (docs/PLATFORMS.md).
fn read(git: &Path, name: &str) -> State {
    let file = git.join(name);
    if !crate::net_path::may_follow(&file, git) {
        return State::Unread(format!("{name} {}", digest::UNREADABLE));
    }
    let missing = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    match std::fs::symlink_metadata(&file) {
        Err(e) if missing(&e) => return State::Missing,
        // A link to nothing: git finds no file either.
        Ok(m) if m.file_type().is_symlink() && std::fs::metadata(&file).is_err_and(|e| missing(&e)) => return State::Missing,
        _ => {}
    }
    match crate::config_includes::read_file_bounded(&file, MAX_BYTES) {
        Some(bytes) => State::Bytes(bytes),
        // Its digest (past the cap, its length and time), so another file in its place asks again.
        None => State::Unread(format!("{name} {}", digest::of_file(&file, Reading::Capped))),
    }
}

/// The steps left in `repo` that run a program, in file order, each once: an `exec` step as
/// `exec <command>`, another strategy as `--strategy=<name>`, a file that wasn't read as its
/// name and digest.
pub(super) fn find(repo: &Repository) -> Vec<String> {
    let git = repo.path();
    let msg_dir = git.join(crate::rebase::MSG_DIR);
    let mut out: Vec<String> = Vec::new();
    let mut add = |step: String| {
        if !out.contains(&step) {
            out.push(step);
        }
    };
    let strategy = |name: &[u8]| (!STRATEGIES.iter().any(|s| s.as_bytes() == name)).then(|| format!("--strategy={}", text_of(name)));
    for file in TODOS {
        match read(git, file) {
            State::Missing => {}
            State::Unread(step) => add(step),
            State::Bytes(bytes) => {
                for command in bytes.split(|b| *b == b'\n').filter_map(exec) {
                    let command = text_of(command);
                    if !own(&msg_dir, &command) {
                        add(format!("exec {command}"));
                    }
                }
            }
        }
    }
    match read(git, STRATEGY) {
        State::Missing => {}
        State::Unread(step) => add(step),
        State::Bytes(bytes) => {
            // As git writes it: the name and one line ending. Anything else is another name
            // to one of git's readers of this file.
            let name = bytes.strip_suffix(b"\n").map_or(&bytes[..], |b| b.strip_suffix(b"\r").unwrap_or(b));
            strategy(name).into_iter().for_each(&mut add);
        }
    }
    match read(git, OPTS) {
        State::Missing => {}
        State::Unread(step) => add(step),
        State::Bytes(bytes) => {
            // Git's config parser passes over a byte order mark.
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
            bytes.split(|b| *b == b'\n').flat_map(opts_strategies).filter_map(strategy).for_each(&mut add);
        }
    }
    out
}

fn blank(b: &u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// `bytes` without the bytes at its start that `drop` takes.
fn skip(bytes: &[u8], drop: impl Fn(&u8) -> bool) -> &[u8] {
    &bytes[bytes.iter().take_while(|b| drop(b)).count()..]
}

/// `bytes` without the bytes at its end that `drop` takes.
fn skip_end(bytes: &[u8], drop: impl Fn(&u8) -> bool) -> &[u8] {
    &bytes[..bytes.len() - bytes.iter().rev().take_while(|b| drop(b)).count()]
}

/// The command of a todo line that is an `exec` step, long or short, as git takes it: spaces
/// and tabs dropped before the line and after the keyword, one CR at the end, nothing else.
/// The keyword ends at a space, a tab, a CR or a NUL.
fn exec(line: &[u8]) -> Option<&[u8]> {
    let line = skip(line.strip_suffix(b"\r").unwrap_or(line), blank);
    let end = line.iter().position(|b| matches!(b, b' ' | b'\t' | b'\r' | 0)).unwrap_or(line.len());
    let (verb, rest) = line.split_at(end);
    matches!(verb, b"exec" | b"x").then(|| skip(rest, blank))
}

/// Whether the `exec` step's `command` is one Glowfly wrote for the message folder `msg_dir`,
/// whose files it names are plain files (or missing) in a plain folder: nothing is read or
/// written through a link.
fn own(msg_dir: &Path, command: &str) -> bool {
    let Some(files) = crate::rebase::own_files(msg_dir, command) else { return false };
    let plain_file = |p: &Path| match std::fs::symlink_metadata(p) {
        Ok(m) => m.is_file(),
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    };
    std::fs::symlink_metadata(msg_dir).is_ok_and(|m| m.is_dir()) && files.iter().all(|f| plain_file(f))
}

/// The value after each `=` on one line of the options file whose key, the run of letters,
/// digits and `-` right before it (spaces and tabs between), is `strategy` in any letter case:
/// whatever section the line is in and wherever on the line a header ends. Spaces, tabs and
/// CRs around the value and plain quotes around all of it are taken off; anything git would
/// read otherwise (a comment, a continued line, quotes or escapes inside) leaves a name that
/// isn't one of git's, so the reader lists at least what git's own config parser would find.
fn opts_strategies(line: &[u8]) -> Vec<&[u8]> {
    let space = |b: &u8| matches!(b, b' ' | b'\t' | b'\r');
    let mut out = Vec::new();
    for at in line.iter().enumerate().filter(|(_, b)| **b == b'=').map(|(i, _)| i) {
        let before = skip_end(&line[..at], blank);
        let key = &before[skip_end(before, |b| b.is_ascii_alphanumeric() || *b == b'-').len()..];
        if !key.eq_ignore_ascii_case(b"strategy") {
            continue;
        }
        let value = skip_end(skip(&line[at + 1..], space), space);
        let inside = value.strip_prefix(b"\"").and_then(|v| v.strip_suffix(b"\""));
        out.push(inside.filter(|v| !v.contains(&b'"') && !v.contains(&b'\\')).unwrap_or(value));
    }
    out
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
