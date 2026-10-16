//! The key git uses for SSH remotes, chosen through `core.sshCommand`. Trellis writes one form,
//! `ssh -i '<key>' -o IdentitiesOnly=yes`, and recognises it when reading the setting back;
//! anything else is someone else's command, which the page shows and asks before replacing.

use crate::gitcli::{self, NO_PROMPTS, READ_TIMEOUT, args};
use crate::rebase::shell_quote_for;
use std::path::{Path, PathBuf};

pub const SSH_COMMAND_KEY: &str = "core.sshCommand";

/// What core.sshCommand at one level says.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyChoice {
    Unset,
    /// The form Trellis writes, naming this key.
    Key(PathBuf),
    /// A command Trellis didn't write; `key` is the file after `-i`, when the command could be
    /// read as plain words.
    Other {
        value: String,
        key: Option<PathBuf>,
    },
}

/// The core.sshCommand value that makes ssh use only `key`. None when the path isn't UTF-8 (git
/// config can't hold it).
pub fn ssh_command_for(key: &Path) -> Option<String> {
    ssh_command_for_os(key, cfg!(windows))
}

/// [`ssh_command_for`] as written on Windows (`windows`: forward slashes, which git's sh reads
/// as they are) or elsewhere.
pub fn ssh_command_for_os(key: &Path, windows: bool) -> Option<String> {
    let native = crate::native_path::native(key);
    let s = native.to_str()?;
    Some(format!("ssh -i {} -o IdentitiesOnly=yes", shell_quote_for(s, windows)))
}

/// One word of a command line, and whether it starts with an unquoted `~` (which sh expands).
struct Word {
    text: String,
    tilde: bool,
}

/// The words sh would split `s` into. None when a quote is unbalanced or the line uses anything
/// sh would expand or run: an unquoted `$`, backquote, `;`, `|`, `&`, `<`, `>`, `(`, `)`, `*`, `?`
/// or `[`, a comment, or `$` or a backquote inside double quotes. A leading `~` stays as it is.
pub fn split_sh_words(s: &str) -> Option<Vec<String>> {
    words(s).map(|w| w.into_iter().map(|w| w.text).collect())
}

/// The word being read, started if there is none yet.
fn word(cur: &mut Option<Word>) -> &mut Word {
    cur.get_or_insert_with(|| Word { text: String::new(), tilde: false })
}

fn words(s: &str) -> Option<Vec<Word>> {
    let mut out = Vec::new();
    let mut cur: Option<Word> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => out.extend(cur.take()),
            '\'' => {
                let w = word(&mut cur);
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => w.text.push(c),
                    }
                }
            }
            '"' => {
                let w = word(&mut cur);
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            c @ ('"' | '\\' | '$' | '`') => w.text.push(c),
                            '\n' => {}
                            c => {
                                w.text.push('\\');
                                w.text.push(c);
                            }
                        },
                        '$' | '`' => return None,
                        c => w.text.push(c),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') => {}
                Some(c) => word(&mut cur).text.push(c),
                None => return None,
            },
            '$' | '`' | ';' | '|' | '&' | '<' | '>' | '(' | ')' | '*' | '?' | '[' => return None,
            // A comment, at the start of a word.
            '#' if cur.is_none() => return None,
            '~' if cur.is_none() => {
                let w = word(&mut cur);
                w.tilde = true;
                w.text.push('~');
            }
            c => word(&mut cur).text.push(c),
        }
    }
    out.extend(cur);
    Some(out)
}

/// What a core.sshCommand value means: unset, the form Trellis writes, or something else.
pub fn parse_ssh_command(value: &str) -> KeyChoice {
    if value.trim().is_empty() {
        return KeyChoice::Unset;
    }
    let Some(words) = words(value) else { return KeyChoice::Other { value: value.to_owned(), key: None } };
    let text: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    if let ["ssh", "-i", key, "-o", "IdentitiesOnly=yes"] = text[..]
        && !words[2].tilde
        && !key.is_empty()
    {
        return KeyChoice::Key(PathBuf::from(key));
    }
    let key = text.iter().position(|w| *w == "-i").and_then(|i| text.get(i + 1)).filter(|k| !k.is_empty()).map(PathBuf::from);
    KeyChoice::Other { value: value.to_owned(), key }
}

fn scope(global: bool) -> &'static str {
    if global { "--global" } else { "--local" }
}

/// core.sshCommand at one level: the global config (`global`, read from `cwd`) or the
/// repository at `cwd`. `env` is added to the command's environment (tests point
/// `GIT_CONFIG_GLOBAL` at a file of their own).
pub fn read_choice(cwd: &Path, global: bool, env: &[(&str, &str)]) -> Result<KeyChoice, String> {
    let mut vars: Vec<(&str, &str)> = NO_PROMPTS.to_vec();
    vars.extend_from_slice(env);
    let out = gitcli::output(cwd, &args(&["config", scope(global), "--get", SSH_COMMAND_KEY]), "", &vars, Some(READ_TIMEOUT));
    match (out.ok, out.code) {
        (true, _) => Ok(parse_ssh_command(out.stdout.trim_end_matches(['\r', '\n']))),
        // Exit code 1: the key isn't set.
        (false, Some(1)) => Ok(KeyChoice::Unset),
        _ => {
            let why = out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git config failed");
            Err(format!("Couldn't read {SSH_COMMAND_KEY}: {why}"))
        }
    }
}

/// Makes git use `key` for SSH remotes at one level (see [`read_choice`]), or with None stops
/// choosing a key there.
pub fn write_choice(cwd: &Path, global: bool, key: Option<&Path>, env: &[(&str, &str)]) -> Result<(), String> {
    let value = match key {
        Some(k) => Some(ssh_command_for(k).ok_or("This key's path can't be written into git config")?),
        None => None,
    };
    crate::identity::set_config(cwd, global, SSH_COMMAND_KEY, value.as_deref(), env)
}
