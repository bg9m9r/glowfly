//! The bare words of a hook's line that name a file it runs: the script an interpreter is given
//! (`sh check.sh`, `node lint.js`, `pwsh -File check.ps1`) or that `.`, `source` or `exec` runs.
//! Any other bare word (`npx lint-staged`, `npm test`, `git diff`) is a command or its argument.
//! Such a word counts only when it names a file in the working tree now (a regular one, or a link
//! `programs_hooks.rs` checks), from its top (where git runs hooks) or from the folder of the
//! script it is in when that is in the tree.

use super::super::{placed, shell};
use std::path::{Path, PathBuf};

/// Programs that run a script file given as their first word that isn't an option.
const INTERPRETERS: &[&str] =
    &["sh", "bash", "zsh", "dash", "ksh", "node", "python", "python3", "ruby", "perl", "pwsh", "powershell", "deno", "bun"];

/// sh and the shells that read options as it does.
pub(super) const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];

/// Options after which an interpreter runs code from the next word, not a file.
const CODE_OPTIONS: &[&str] = &["-c", "-e", "-m", "-p", "--eval", "--print", "-command", "-encodedcommand", "-ec"];

/// Options that take the next word as their value, by interpreter (`python -W ignore x.py`).
const VALUE_OPTIONS: &[(&[&str], &[&str])] = &[
    (&["python", "python3"], &["-W", "-X"]),
    (SHELLS, &["-o", "-O", "+o", "+O"]),
    (&["perl"], &["-I", "-M"]),
    (&["node", "deno", "bun"], &["-r", "--require", "--import"]),
    (&["ruby"], &["-I", "-r"]),
];

/// How a script runs a file, which decides whether that file is read as shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Ran {
    /// As a hook (husky's `.husky/<hook>`): read as shell.
    AsHook,
    /// By `.`, `source` or a shell (`sh x`, `bash -e x`): read as shell whatever it is.
    Shell,
    /// As a command (`./x`, `exec x`): read as shell when its `#!` line runs one, or when it
    /// has none and is text (sh runs such a file itself).
    Direct,
    /// Any other way (`node x`, an argument): read as shell only when it is a shell script.
    Other,
}

/// A word of a line that names a script it runs.
#[derive(Debug, PartialEq)]
pub(super) struct RunWord {
    /// As sh reads it.
    pub text: String,
    pub ran: Ran,
}

/// Words that run the next word as a command (`exec ./x`, `env A=1 ./x`) or lead into one.
const LEADERS: &[&str] = &["exec", "env", "command", "nohup", "time", "if", "then", "else", "elif", "do", "while", "until", "!", "{", "("];

/// The words of `line` that sh runs as commands, as it reads them: the first of each command,
/// after `;`, `&&`, `||`, `|` or `&`, past assignments (`A=1 ./x`) and the words in `LEADERS`.
pub(super) fn command_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut at_command = true;
    for word in shell::words(line).into_iter().map(|w| w.sh) {
        let core = word.trim_end_matches([';', '&', '|']);
        let ends = core.len() < word.len();
        if at_command && !core.is_empty() {
            let assignment = core
                .split_once('=')
                .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            if !assignment && !LEADERS.contains(&core) {
                out.push(core.to_owned());
                at_command = false;
            }
        }
        if ends {
            at_command = true;
        }
    }
    out
}

/// The words of `line` that name a script it runs, as sh reads them, bare or not.
pub(super) fn run_words(line: &str) -> Vec<RunWord> {
    let words: Vec<String> = shell::words(line).into_iter().map(|w| w.sh).collect();
    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let name = word.rsplit(['/', '\\']).next().unwrap_or(word).to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        let builtin = matches!(name, "." | "source" | "exec");
        if !builtin && !INTERPRETERS.contains(&name) {
            continue;
        }
        let ran = match name {
            "exec" => Ran::Direct,
            _ if name == "." || name == "source" || SHELLS.contains(&name) => Ran::Shell,
            _ => Ran::Other,
        };
        let mut push = |w: Option<&String>| out.extend(w.map(|text| RunWord { text: text.clone(), ran }));
        // PowerShell's options may take values (`-ExecutionPolicy RemoteSigned`): its `-File`
        // names the script wherever it stands.
        let file = words[i + 1..].iter().position(|w| w.eq_ignore_ascii_case("-file"));
        if matches!(name, "pwsh" | "powershell")
            && let Some(at) = file
        {
            push(words.get(i + 1 + at + 1));
            continue;
        }
        let valued: Vec<&str> =
            VALUE_OPTIONS.iter().filter(|(names, _)| names.contains(&name)).flat_map(|(_, options)| options.iter().copied()).collect();
        let mut rest = words[i + 1..].iter();
        while let Some(next) = rest.next() {
            let lower = next.to_ascii_lowercase();
            if lower == "-file" || lower == "-f" {
                push(rest.next());
                break;
            }
            if valued.contains(&next.as_str()) && !builtin {
                rest.next();
                continue;
            }
            if next.starts_with(['-', '+']) && next.len() > 1 {
                // sh's `-c` (also among other flags, `-ec`) and the others' code options.
                let code = match lower.strip_prefix('-') {
                    Some(flags) if SHELLS.contains(&name) => !flags.starts_with('-') && flags.contains('c'),
                    _ => CODE_OPTIONS.contains(&lower.as_str()),
                };
                if code || builtin {
                    break;
                }
                continue;
            }
            if matches!(name, "deno" | "bun") && lower == "run" {
                continue;
            }
            push(Some(next));
            break;
        }
    }
    out
}

/// The files in the working tree that the bare `word` names, from its top and from `folder`
/// (the script's own, when in the tree): regular files and links. Nothing for a word with a `/`
/// or `\` (the word scanner reads those), a substitution, a quoted glob character or an option.
pub(super) fn bare_files(workdir: &Path, folder: Option<&Path>, word: &str) -> Vec<PathBuf> {
    let word = word.trim_end_matches([';', '&', '|', ')']);
    let plain = !word.is_empty() && !word.starts_with(['-', '~']) && !word.contains(['/', '\\']);
    // A quoted `~` (`sh "~x.sh"`) is part of a name; any other mark is no plain word.
    let marked = word.chars().any(|c| c != shell::TILDE && (shell::private(0)..=shell::private(0x18FF)).contains(&c));
    if !plain || marked || word.contains(['*', '?', '[', '{', '$', '`']) {
        return Vec::new();
    }
    let text = shell::literal(word);
    let text = if word.starts_with(shell::TILDE) { format!("./{text}") } else { text };
    let mut out: Vec<PathBuf> = Vec::new();
    for base in std::iter::once(workdir).chain(folder) {
        let Some(p) = placed(base, None, &text) else { continue };
        let file = !super::super::super::follow::far(&p, workdir)
            && std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file() || m.file_type().is_symlink());
        if file && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
#[path = "programs_hooks_runs_tests.rs"]
mod tests;
