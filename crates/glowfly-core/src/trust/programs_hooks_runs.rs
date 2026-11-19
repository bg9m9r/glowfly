//! The bare words of a hook's line that name a file it runs: the script an interpreter is given
//! (`sh check.sh`, `node lint.js`, `pwsh -File check.ps1`) or that `.`, `source` or `exec` runs.
//! Any other bare word (`npx lint-staged`, `npm test`, `git diff`) is a command or its argument.
//! Such a word counts only when it names a regular file in the working tree now, from its top
//! (where git runs hooks) or from the folder of the script it is in when that is in the tree.

use super::super::{placed, shell};
use std::path::{Path, PathBuf};

/// Programs that run a script file given as their first word that isn't an option.
const INTERPRETERS: &[&str] =
    &["sh", "bash", "zsh", "dash", "ksh", "node", "python", "python3", "ruby", "perl", "pwsh", "powershell", "deno", "bun"];

/// Options after which an interpreter runs code from the next word, not a file.
const CODE_OPTIONS: &[&str] = &["-c", "-e", "-m", "-p", "--eval", "--print", "-command", "-encodedcommand", "-ec"];

/// The words of `line` that name a script it runs, as sh reads them, bare or not.
pub(super) fn run_words(line: &str) -> Vec<String> {
    let words: Vec<String> = shell::words(line).into_iter().map(|w| w.sh).collect();
    let mut out = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let name = word.rsplit(['/', '\\']).next().unwrap_or(word).to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        let sourced = matches!(name, "." | "source" | "exec");
        if !sourced && !INTERPRETERS.contains(&name) {
            continue;
        }
        let shell_like = matches!(name, "sh" | "bash" | "zsh" | "dash" | "ksh");
        // PowerShell's options may take values (`-ExecutionPolicy RemoteSigned`): its `-File` names
        // the script wherever it stands.
        let file = words[i + 1..].iter().position(|w| w.eq_ignore_ascii_case("-file"));
        if matches!(name, "pwsh" | "powershell")
            && let Some(at) = file
        {
            out.extend(words.get(i + 1 + at + 1).cloned());
            continue;
        }
        let mut rest = words[i + 1..].iter();
        while let Some(next) = rest.next() {
            let lower = next.to_ascii_lowercase();
            if lower == "-file" || lower == "-f" {
                out.extend(rest.next().cloned());
                break;
            }
            if let Some(flags) = lower.strip_prefix('-') {
                // sh's `-c` (also among other flags, `-ec`) and the others' code options.
                let code = if shell_like { !flags.starts_with('-') && flags.contains('c') } else { CODE_OPTIONS.contains(&lower.as_str()) };
                if code || sourced {
                    break;
                }
                continue;
            }
            if matches!(name, "deno" | "bun") && lower == "run" {
                continue;
            }
            out.push(next.clone());
            break;
        }
    }
    out
}

/// The regular files in the working tree that the bare `word` names, from its top and from
/// `folder` (the script's own, when in the tree). Nothing for a word with a `/` or `\` (the word
/// scanner reads those), a substitution, a quoted glob character or an option.
pub(super) fn bare_files(workdir: &Path, folder: Option<&Path>, word: &str) -> Vec<PathBuf> {
    let word = word.trim_end_matches([';', '&', '|', ')']);
    let plain = !word.is_empty() && !word.starts_with(['-', '~']) && !word.contains(['/', '\\']);
    if !plain || word.chars().any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c) || "*?[{$`".contains(c)) {
        return Vec::new();
    }
    let text = shell::literal(word);
    let mut out: Vec<PathBuf> = Vec::new();
    for base in std::iter::once(workdir).chain(folder) {
        let Some(p) = placed(base, None, &text) else { continue };
        let file = !super::super::super::follow::far(&p, workdir) && std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file());
        if file && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
#[path = "programs_hooks_runs_tests.rs"]
mod tests;
