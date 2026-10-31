//! Config programs that run files from the working tree. A setting such as
//! `core.fsmonitor=./tools/fsm.sh`, `filter.x.smudge=python tools/smudge.py`,
//! `credential.helper=!./bin/creds`, `lfs.customtransfer.x.path=./bin/agent`,
//! `http.sslCAInfo=certs/ca.pem` or `include.path=../shared.cfg` names a file the branch
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

#[path = "programs_shell.rs"]
mod shell;

/// Characters that end a word inside a shell command (`a;./b`, `$(./x)`, `--file=./c`).
const BREAKS: &[char] = &[';', '|', '&', '(', ')', '<', '>', '`', '"', '\'', '=', ','];

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

/// The command a setting runs, or the file it reads (a CA bundle), as written; None for one
/// that names neither (core.hooksPath, whose hooks `follow.rs` follows itself; protocol.allow;
/// an include; git-lfs's choice of transfer agent, a name; a TLS switch or a proxy).
fn command_text(c: &ConfigCommand) -> Option<&str> {
    let key = c.key.to_ascii_lowercase();
    if key == "core.hookspath" || key.starts_with("protocol.") || key.starts_with("includeif.") || key == "lfs.standalonetransferagent" {
        return None;
    }
    if super::is_network_setting(&c.key) && !(key.ends_with(".sslcainfo") || key.ends_with(".sslcapath")) {
        return None;
    }
    // url.<base>.insteadOf: the command is in the URL the key names.
    if let Some(rest) = c.key.strip_prefix("url.") {
        return rest.rfind('.').map(|i| &rest[..i]);
    }
    Some(&c.value)
}

/// A word of a command that may name a file (`shell::Word`'s reading or one of its pieces).
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    text: String,
    /// sh's reading of a word that had a `\` outside quotes: followed even without a `/`, as
    /// what it names isn't what was written.
    escaped: bool,
}

/// The words of `text` that may name a file: each word as sh splits it (`shell::words`), its
/// Windows reading when a `\` was outside quotes (each `\` a folder separator), and each piece
/// of those between shell punctuation and spaces, with a leading `!` (an alias or credential
/// helper run by the shell) or `ext::` (a transport's command) dropped.
fn candidates(text: &str) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |w: &str, escaped: bool| {
        let w = w.trim_start_matches('!');
        let w = w.get(..5).filter(|p| p.eq_ignore_ascii_case("ext::")).map_or(w, |_| &w[5..]);
        let c = Candidate { text: w.to_owned(), escaped };
        if !w.is_empty() && !out.contains(&c) {
            out.push(c);
        }
    };
    for word in shell::words(text.trim().trim_start_matches('!')) {
        let windows = word.raw.as_ref().map(|r| r.replace('\\', "/"));
        push(&word.sh, windows.is_some());
        for form in std::iter::once(word.sh.as_str()).chain(windows.as_deref()) {
            push(form, false);
            for piece in form.split(|c: char| c.is_whitespace() || BREAKS.contains(&c)) {
                push(piece, false);
            }
        }
    }
    out
}

/// The glob characters, a substitution's mark among them (its value may be anything).
const GLOBS: &[char] = &['*', '?', '[', '{', shell::SUB];

/// Whether `text` is a path pattern sh would expand: a glob character with a name character or
/// `/` beside it (`*.sh`, `t*/x`, `lib/$NAME.sh`), a `[…]` beside one, or a `{a,b}` or
/// `{1..3}`; never bare shell syntax (`{`, `};`, `[[`, `]]`).
fn is_pattern(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let name = |i: Option<usize>| i.and_then(|i| chars.get(i)).is_some_and(|c| c.is_alphanumeric() || "._-/~".contains(*c));
    chars.iter().enumerate().any(|(i, &c)| match c {
        '*' | '?' | shell::SUB => name(i.checked_sub(1)) || name(Some(i + 1)),
        '[' => chars[i..].iter().position(|c| *c == ']').is_some_and(|j| j > 1 && (name(i.checked_sub(1)) || name(Some(i + j + 1)))),
        '{' => chars[i..].iter().position(|c| *c == '}').is_some_and(|j| {
            let inside: String = chars[i + 1..i + j].iter().collect();
            inside.contains(',') || inside.contains("..")
        }),
        _ => false,
    })
}

/// The readings of `text` to follow: as written, and from each substitution followed by a `/`
/// on, relative to the working tree (`$(git rev-parse --show-toplevel)/x`, `` `pwd`/x ``,
/// `${PWD%/}/x`, `$HOME/x`: its value may be the working folder). One that starts with a
/// substitution not followed by `/` names nothing knowable.
fn readings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !text.starts_with(shell::SUB) {
        out.push(text.to_owned());
    }
    for (i, _) in text.match_indices(shell::SUB) {
        let rest = &text[i + shell::SUB.len_utf8()..];
        if rest.starts_with('/') {
            out.push(format!(".{rest}"));
        }
    }
    // `name:stream` opens `name` on Windows (`x.sh::$DATA` is x.sh), where a `:` is no name
    // character: the part before it is what is named there. Elsewhere it is one (`tools:v2`),
    // so both readings count. A drive's `C:` stays.
    let cut = |r: &String| {
        let colon = r.char_indices().find(|&(i, c)| c == ':' && !(i == 1 && r.starts_with(|d: char| d.is_ascii_alphabetic())));
        colon.map(|(i, _)| r[..i].to_owned())
    };
    let mut all = Vec::new();
    for r in out {
        let short = cut(&r);
        if !(cfg!(windows) && short.is_some()) {
            all.push(r);
        }
        all.extend(short);
    }
    all.dedup();
    all
}

/// Where `word` points when it may be a file the command runs or reads: one with a `/` or `\`
/// (a quoted one with spaces too, but no shell punctuation), or one that names something in
/// the working tree now. Relative to the working tree, which is where git runs the
/// repository's commands. Nothing for a URL, an option, or the working tree itself.
///
/// A path pattern (`is_pattern`) also follows the nearest folder at or above it that the
/// working tree has now, whose version covers every match: the whole tree only when the
/// pattern is at its top or nothing above it is there.
fn path_of(workdir: &Path, home: Option<&Path>, word: &Candidate) -> Vec<PathBuf> {
    if word.text.contains("://") || word.text.starts_with(['-', '%']) {
        return Vec::new();
    }
    let at = |w: &str| if w.starts_with('~') { crate::paths::expand_home(w, home) } else { workdir.join(w) };
    let mut out: Vec<PathBuf> = Vec::new();
    for r in readings(&word.text) {
        let slashed = r.replace('\\', "/");
        let shown = shell::literal(&slashed);
        if !slashed.contains(shell::SUB) {
            let path = at(&shown);
            // A whole word with shell punctuation in it is a command line (`./a; ./b`): its
            // pieces count. The part before a `:` counts as the word it was cut from.
            let named = word.escaped || ((shown.contains('/') || word.text.contains('/')) && !shown.contains(BREAKS));
            let exists = std::fs::symlink_metadata(&path).is_ok();
            let root = crate::paths::resolve(&path) == crate::paths::resolve(workdir);
            if (named || exists) && !root && !out.contains(&path) {
                out.push(path);
            }
        }
        if is_pattern(&slashed) {
            // The literal part before the first glob, up to its last folder.
            let fixed = shell::literal(slashed.split(GLOBS).next().unwrap_or_default());
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
    }
    out
}

#[cfg(test)]
#[path = "programs_tests.rs"]
mod tests;
