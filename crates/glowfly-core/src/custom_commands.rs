//! Custom commands: git and shell actions the user defines in `commands.toml` files, run from
//! the command palette, the commit, branch and file menus, and keys.
//!
//! Two files are read, both optional: the user's own `<config dir>/commands.toml`
//! (`user_file`) and the open repository's `<workdir>/.glowfly/commands.toml` (`repo_file`,
//! the working-tree file, so a team can commit and share it). Each is an array of tables:
//!
//! ```toml
//! [[command]]
//! name = "Log of this branch"              # shown in the palette and menus
//! kind = "git"                             # "git" or "shell"
//! template = "log --oneline -20 {branch}"  # what runs
//! show = ["palette", "branch"]             # optional; default ["palette"]
//! confirm = "Show the log of {branch}?"    # optional: false, true or a question
//! key = "Mod+Alt+L"                        # optional shortcut
//! ```
//!
//! `parse.rs` reads that format strictly, reporting each problem with its line; `expand.rs`
//! checks the placeholders (`{branch}`, `{sha}`, `{files}`, `{repo}`; a `{word}` after `@` or
//! `$` is git's or the shell's own) and fills them in, into git arguments (split first, so a
//! value is always one argument) or into sh text (every value single-quoted); `shell.rs` finds
//! POSIX sh (`/bin/sh`, or Git for Windows' `sh.exe`) and runs a shell command. Reading the
//! files and running a command are for runner threads only.

use std::path::{Path, PathBuf};

mod expand;
mod parse;
mod shell;

#[cfg(test)]
mod read_tests;

pub use expand::{ConfirmText, Expanded, KNOWN, Missing, Vars};
pub use parse::parse;
pub use shell::{MAX_OUTPUT_BYTES, SCRIPT_VAR, SH_ARGS, ShellMissing, find_sh, run_shell, sh_for};

/// The name of both commands files.
pub const FILE_NAME: &str = "commands.toml";
/// The largest commands file read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
/// The most commands one file may hold.
pub const MAX_COMMANDS: usize = 200;
/// The longest command name, in characters.
pub const MAX_NAME_CHARS: usize = 80;

/// What a command runs: git with arguments, or a POSIX sh script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Git,
    Shell,
}

/// Where a command is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Palette,
    Commit,
    Branch,
    File,
}

/// Whether a command asks before it runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirm {
    No,
    /// Asks "Run <name>?".
    Default,
    /// Asks this question (placeholders filled in plainly).
    Text(String),
}

/// Which file a command came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The user's own file in the config folder.
    User,
    /// The open repository's `.glowfly/commands.toml`.
    Repo,
}

/// Which placeholders a template names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub branch: bool,
    pub sha: bool,
    pub files: bool,
    pub repo: bool,
}

/// One command from a commands file, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub name: String,
    pub kind: Kind,
    /// As written, trimmed.
    pub template: String,
    pub show: Vec<Place>,
    pub confirm: Confirm,
    /// The shortcut as written; the app reads the chord.
    pub key: Option<String>,
    /// The line of `key` (0 without one).
    pub key_line: usize,
    /// The line of its `[[command]]` table.
    pub line: usize,
    needs: Needs,
}

impl Command {
    /// Which placeholders its template names.
    pub fn needs(&self) -> Needs {
        self.needs
    }

    /// Whether it is offered in `p`.
    pub fn shows_in(&self, p: Place) -> bool {
        self.show.contains(&p)
    }
}

/// What is wrong in a commands file, at `line` when it has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub line: Option<usize>,
    pub kind: ProblemKind,
}

/// The kinds of problem a commands file can have; the app words them.
#[derive(Clone, Debug, PartialEq)]
pub enum ProblemKind {
    /// The TOML parser's or the format's message (never the file's text around it).
    Syntax(String),
    TooLarge,
    NotUtf8,
    /// The file couldn't be read: the system's error.
    Unreadable(String),
    /// The file, or the `.glowfly` folder holding it, is a link (never followed).
    Link,
    TooMany,
    EmptyName,
    LongName,
    DuplicateName(String),
    EmptyTemplate,
    UnmatchedQuote,
    UnknownPlaceholder(String),
    /// `{files}` is part of a longer git argument.
    FilesNotAlone,
    /// This placeholder is written inside the template's own quotes (shell).
    QuotedPlaceholder(String),
    /// `show = []`.
    NoPlaces,
}

/// What one commands file gave: its commands and its problems.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileRead {
    /// The file read, if there is a place for it.
    pub path: Option<PathBuf>,
    pub commands: Vec<Command>,
    pub problems: Vec<Problem>,
}

/// The user's commands file, in the config folder.
pub fn user_file() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join(FILE_NAME))
}

/// The folder in a working folder that holds the repository's commands file.
pub const REPO_DIR: &str = ".glowfly";
/// The same folder's name from before the app was renamed (it was called Trellis), read when a
/// repository has no `.glowfly`.
pub const OLD_REPO_DIR: &str = ".trellis";

/// The repository's commands folder under its working folder `workdir`: `.glowfly`, or
/// `.trellis` when only that one is there. Looks at the disk: runner threads only.
pub fn repo_dir(workdir: &Path) -> PathBuf {
    let (new, old) = (workdir.join(REPO_DIR), workdir.join(OLD_REPO_DIR));
    // `symlink_metadata`: a link in either place counts as there (and is refused when read).
    if std::fs::symlink_metadata(&new).is_err() && std::fs::symlink_metadata(&old).is_ok() { old } else { new }
}

/// The repository's commands file under its working folder `workdir` (see [`repo_dir`]).
pub fn repo_file(workdir: &Path) -> PathBuf {
    repo_dir(workdir).join(FILE_NAME)
}

/// Reads the user's commands file. Runner threads only.
pub fn read_user() -> FileRead {
    match user_file() {
        Some(path) => read_file(path, None),
        None => FileRead::default(),
    }
}

/// Reads the commands file of the repository at `workdir`, refusing a link for the file or
/// its `.glowfly` (or `.trellis`) folder. Runner threads only.
pub fn read_repo(workdir: &Path) -> FileRead {
    let dir = repo_dir(workdir);
    read_file(dir.join(FILE_NAME), Some(&dir))
}

/// Reads `path`; with `folder`, neither it nor the file may be a link.
fn read_file(path: PathBuf, folder: Option<&Path>) -> FileRead {
    use std::io::ErrorKind::NotFound;
    let empty = || FileRead { path: Some(path.clone()), ..FileRead::default() };
    let fail = |kind| FileRead { path: Some(path.clone()), problems: vec![Problem { line: None, kind }], ..FileRead::default() };
    if let Some(dir) = folder {
        match std::fs::symlink_metadata(dir) {
            Ok(m) if m.file_type().is_symlink() => return fail(ProblemKind::Link),
            Ok(m) if m.is_dir() => {}
            Ok(_) => return empty(),
            Err(e) if e.kind() == NotFound => return empty(),
            Err(e) => return fail(ProblemKind::Unreadable(e.to_string())),
        }
    }
    let meta = if folder.is_some() { std::fs::symlink_metadata(&path) } else { std::fs::metadata(&path) };
    match meta {
        Err(e) if e.kind() == NotFound => return empty(),
        Err(e) => return fail(ProblemKind::Unreadable(e.to_string())),
        Ok(m) if m.file_type().is_symlink() => return fail(ProblemKind::Link),
        Ok(m) if !m.is_file() => return empty(),
        Ok(m) if m.len() > MAX_FILE_BYTES => return fail(ProblemKind::TooLarge),
        Ok(_) => {}
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == NotFound => return empty(),
        Err(e) => return fail(ProblemKind::Unreadable(e.to_string())),
    };
    // It may have grown since its size was read.
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return fail(ProblemKind::TooLarge);
    }
    let Ok(text) = String::from_utf8(bytes) else { return fail(ProblemKind::NotUtf8) };
    let (commands, problems) = parse(&text);
    FileRead { path: Some(path.clone()), commands, problems }
}
