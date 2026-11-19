//! Scripts a hook runs from the working tree. A hook such as `exec ./scripts/check.sh` or
//! `node tools/lint.js` runs a file the branch carries: a checkout or pull that changes it
//! changes what the hook does without changing the hook. `scan` reads each hook's text with the
//! word scanner the config programs use (`programs.rs`), with its rules and `follow::far` guard,
//! so the trust scan follows those files as it follows a config program's. A bare word
//! (`npx lint-staged`, `git diff`, `npm test`) is no path here (`path_of`'s `hook`) unless an
//! interpreter, `.`, `source` or `exec` runs it (`sh check.sh`: `programs_hooks_runs.rs`).
//! `$(dirname "$0")` is read as the folder of the script it is in (husky's
//! `. "$(dirname "$0")/h"`), and a word from a home folder (`$HOME/x`, `$XDG_CONFIG_HOME/x`,
//! an unquoted `~/x`) names nothing the branch carries.
//!
//! A shell script a hook runs (`reads_as_shell`: one run by a shell, `.` or `source`, a command
//! whose `#!` line runs a shell or that is text with none, or any file that is a shell script by
//! its name or `#!` line) is read the same way, and so on up to `MAX_DEPTH` scripts below the
//! hook. Any other file (a JavaScript or Python file, a program, data) is followed by its bytes
//! without being read, and so is anything untracked under `node_modules`. A link is followed by
//! the regular file in the working tree it leads to; one leading elsewhere is named in `unread`
//! (or noted in `offsite`), never read. A hooks folder called `_` in the
//! working tree also runs the files beside it named as its hooks (`parent_scripts`): husky 9's
//! `.husky/_/h` runs `.husky/<hook>` by a path it builds at run time; those count as hooks.
//!
//! Bounded: at most `MAX_WORDS` words and `MAX_FILES` files in all, lines up to
//! `MAX_LINE_BYTES`, and `MAX_DEPTH` scripts down. Past any of them, and for a hook or shell
//! script too large to keep its text (`MAX_TEXT_BYTES`), one that can't be read, one with a NUL
//! byte in it (a compiled program, whose words can't be read) or one reached through a link
//! (which may change without a change here), the whole working tree is followed. A file that
//! isn't read never does.

#[path = "programs_hooks_files.rs"]
mod files;
#[path = "programs_hooks_runs.rs"]
mod runs;

use super::{candidates, path_of, shell};
use files::{LinkEnd, in_packages, in_tree, is_shell, link_end, rel_text, text_of_file};
use runs::Ran;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// The most bytes of a hook or script whose text is read for the files it runs.
pub(in crate::trust) const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// The most words read from all hooks and the scripts they run.
const MAX_WORDS: usize = 4096;

/// The longest line read; the word scanner's work grows with a line's words.
const MAX_LINE_BYTES: usize = 4096;

/// The most files followed for what hooks run.
const MAX_FILES: usize = 64;

/// How many scripts below a hook are read: a hook's own text is at 0.
const MAX_DEPTH: usize = 3;

/// Where a script's text named its own folder: put in before the text is split into words, so
/// a folder with spaces or shell characters in its name stays one part of one word.
const OWN: char = shell::private(32);

/// Where it named its folder's parent, as `OWN` does.
const PARENT: char = shell::private(33);

/// Where it named a home folder (`$HOME`, `${XDG_CONFIG_HOME:-$HOME/.config}`).
const HOME: char = shell::private(34);

/// The ways a script names the folder above its own (husky 9's `h` builds `../<hook>` so).
/// Replaced before `OWN_FOLDER`, whose forms these hold.
const PARENT_FOLDER: &[&str] =
    &["$(dirname \"$(dirname \"$0\")\")", "$(dirname -- \"$(dirname -- \"$0\")\")", "$(dirname $(dirname $0))", "${0%/*/*}"];

/// The ways a hook script names its own folder.
const OWN_FOLDER: &[&str] = &[
    "${0%/*}",
    "$(dirname \"$0\")",
    "$(dirname -- \"$0\")",
    "$(dirname $0)",
    "$(dirname -- $0)",
    "`dirname \"$0\"`",
    "`dirname -- \"$0\"`",
    "`dirname $0`",
];

/// The files in the working tree (absolute paths, maybe missing) that the hooks in `hooks_dir`
/// called `names` run, read from these `texts`: each hook's bytes, or None for one read but too
/// large to keep. A file at a place `net_path::may_follow` refuses is left out and noted in
/// `offsite`; one whose bytes name no file here, or a link that leads out of the working tree,
/// is named in `unread`.
pub(in crate::trust) fn scan(
    repo: &git2::Repository,
    workdir: &Path,
    hooks_dir: &Path,
    names: &[String],
    texts: &[Option<Vec<u8>>],
    offsite: &mut Vec<String>,
    unread: &mut Vec<String>,
) -> Vec<PathBuf> {
    let mut run = Run {
        workdir,
        home: crate::settings::home_dir(),
        out: Vec::new(),
        queue: texts.iter().map(|t| Script { bytes: t.clone(), folder: hooks_dir.to_path_buf(), depth: 0 }).collect(),
        words: 0,
        whole: false,
        full: false,
        index: repo.index().ok(),
        offsite,
        unread,
    };
    for path in parent_scripts(workdir, hooks_dir, names) {
        run.found(path, 0, Ran::AsHook);
    }
    while let Some(script) = run.queue.pop_front() {
        if run.full {
            break;
        }
        run.read(&script);
    }
    let mut out = run.out;
    if run.whole {
        out.push(workdir.to_path_buf());
    }
    out
}

/// A text to read for the files it runs: a hook's (`depth` 0) or a shell script's a hook runs.
struct Script {
    /// Its bytes; None for one that couldn't be read whole.
    bytes: Option<Vec<u8>>,
    /// The folder its `$(dirname "$0")` names.
    folder: PathBuf,
    depth: usize,
}

/// One scan's state: what is found, what is left to read, and the limits reached.
struct Run<'a> {
    workdir: &'a Path,
    home: Option<PathBuf>,
    out: Vec<PathBuf>,
    queue: VecDeque<Script>,
    words: usize,
    /// A limit was passed: the whole working tree is followed.
    whole: bool,
    /// `MAX_FILES` was reached: nothing more is read.
    full: bool,
    /// The repository's index, for what is tracked.
    index: Option<git2::Index>,
    offsite: &'a mut Vec<String>,
    unread: &'a mut Vec<String>,
}

impl Run<'_> {
    /// Follows what `script`'s words run.
    fn read(&mut self, script: &Script) {
        let Some(bytes) = &script.bytes else {
            self.whole = true;
            return;
        };
        if bytes.contains(&0) {
            self.whole = true;
            return;
        }
        let folder = folder_text(self.workdir, &script.folder);
        let parent = script.folder.parent().and_then(|p| folder_text(self.workdir, p));
        // A folder whose name isn't UTF-8 stays a substitution.
        let marks = [(PARENT_FOLDER, PARENT, parent), (OWN_FOLDER, OWN, folder)];
        let mut text = home_marked(&super::super::keys::text_of(bytes));
        for (forms, mark, _) in marks.iter().filter(|(_, _, text)| text.is_some()) {
            for form in *forms {
                text = text.replace(form, &mark.to_string());
            }
        }
        let unmarked = |text: &str| {
            let mut text = text.replace(HOME, &shell::SUB.to_string());
            for (_, mark, folder) in marks.iter() {
                if let Some(folder) = folder {
                    text = text.replace(*mark, folder);
                }
            }
            text
        };
        let here = in_tree(self.workdir, &script.folder).then_some(script.folder.as_path());
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if line.len() > MAX_LINE_BYTES {
                self.whole = true;
                return;
            }
            let runs: Vec<(String, Ran)> = runs::run_words(line).into_iter().map(|w| (unmarked(&w.text), w.ran)).collect();
            let commands: Vec<String> = runs::command_words(line).iter().map(|w| unmarked(w)).collect();
            // The way that reads the most: a shell's, then a command's, then any other.
            let how = |text: &str| {
                let named = |r: Ran| runs.iter().any(|(w, ran)| w == text && *ran == r);
                if named(Ran::Shell) {
                    Ran::Shell
                } else if named(Ran::Direct) || commands.iter().any(|c| c == text) {
                    Ran::Direct
                } else {
                    Ran::Other
                }
            };
            let mut paths = Vec::new();
            for mut word in candidates(line) {
                self.words += 1;
                if self.words > MAX_WORDS {
                    self.whole = true;
                    return;
                }
                // A home folder's files are no part of the branch: a word or an assigned value
                // (`i=$HOME/x`, `PATH=~/bin:$HOME/bin`) that starts at one names nothing here.
                let from_home = |t: &str| t.starts_with([HOME, '~']);
                if from_home(&word.text) || word.text.split(['=', ':']).skip(1).any(from_home) {
                    continue;
                }
                word.text = unmarked(&word.text);
                let ran = how(&word.text);
                paths.extend(path_of(self.workdir, self.home.as_deref(), &word, true, self.unread).into_iter().map(|p| (p, ran)));
            }
            for (word, _) in &runs {
                let ran = how(word);
                paths.extend(runs::bare_files(self.workdir, here, word).into_iter().map(|p| (p, ran)));
            }
            for (p, ran) in paths {
                if !self.found(p, script.depth + 1, ran) {
                    return;
                }
            }
        }
    }

    /// Follows `path`, run as `ran` by a script `depth - 1` below a hook, and reads it in turn
    /// when it is a shell script in the working tree. False once `MAX_FILES` is reached.
    fn found(&mut self, path: PathBuf, depth: usize, ran: Ran) -> bool {
        if super::super::follow::note_far(&path, self.workdir, self.offsite) || self.out.contains(&path) {
            return true;
        }
        if self.out.len() >= MAX_FILES {
            (self.whole, self.full) = (true, true);
            return false;
        }
        let Some(folder) = in_tree(self.workdir, &path).then(|| path.parent().map(Path::to_path_buf)).flatten() else {
            self.out.push(path);
            return true;
        };
        let packages = in_packages(self.workdir, &path) && !self.tracked(&path);
        let meta = std::fs::symlink_metadata(&path);
        if meta.as_ref().is_ok_and(|m| m.file_type().is_symlink()) {
            self.link(path, ran, packages);
            return true;
        }
        self.out.push(path.clone());
        if packages || !meta.is_ok_and(|m| m.is_file()) {
            // Installed packages, a folder (followed whole) or a file not there yet.
            return true;
        }
        let text = text_of_file(&path, MAX_TEXT_BYTES);
        if !reads_as_shell(ran, &path, text.as_ref().map(|(b, _)| b.as_slice())) {
            return true;
        }
        match text {
            Some((bytes, true)) if depth <= MAX_DEPTH => self.queue.push_back(Script { bytes: Some(bytes), folder, depth }),
            _ => self.whole = true,
        }
        true
    }

    /// Whether `path` in the working tree is in the index: a tracked file under `node_modules`
    /// is the branch's own, not an installed package.
    fn tracked(&self, path: &Path) -> bool {
        let rel = rel_text(self.workdir, path);
        self.index.as_ref().zip(rel).is_some_and(|(index, rel)| index.get_path(Path::new(&rel), 0).is_some())
    }

    /// Follows the link at `path`, run as `ran`, by the file it leads to: one in the working
    /// tree is followed by the link's place (whose bytes are the file's), unless it is a script
    /// to read (`packages`: never), which a link may change without a change here, so the whole
    /// working tree follows. One that leads elsewhere is noted, never read.
    fn link(&mut self, path: PathBuf, ran: Ran, packages: bool) {
        match link_end(self.workdir, &path) {
            LinkEnd::File(real) => {
                let text = text_of_file(&real, MAX_TEXT_BYTES).map(|(b, _)| b);
                if !packages && (is_shell(&path, None) || reads_as_shell(ran, &real, text.as_deref())) {
                    self.whole = true;
                }
                self.out.push(path);
            }
            LinkEnd::Far(real) => _ = super::super::follow::note_far(&real, self.workdir, self.offsite),
            LinkEnd::Elsewhere => super::note(self.unread, &crate::native_path::shown(&path)),
        }
    }
}

/// Whether a file run as `ran`, whose bytes are `text` (None: can't be read), is read as shell:
/// always by a shell or as a hook; as a command when its `#!` line runs a shell, or when it has
/// none and no NUL byte (sh runs a text file it can't execute itself); otherwise when it is a
/// shell script by its name or `#!` line.
fn reads_as_shell(ran: Ran, path: &Path, text: Option<&[u8]>) -> bool {
    match ran {
        Ran::AsHook | Ran::Shell => true,
        Ran::Direct => is_shell(path, text) || text.is_some_and(|t| !t.starts_with(b"#!") && !t.contains(&0)),
        Ran::Other => is_shell(path, text),
    }
}

/// `text` with each expansion of a home folder's variable (`$HOME`, `${HOME}`, and the XDG
/// ones such as `${XDG_CONFIG_HOME:-$HOME/.config}`) as one `HOME` mark. Another name ending
/// in `HOME` (`$PROJECT_HOME`, gradlew's `$APP_HOME`) may be anywhere, the working tree too.
fn home_marked(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let braced = after.strip_prefix('{');
        let body = braced.unwrap_or(after);
        let name_len = body.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(body.len());
        let end = match braced {
            Some(b) => b.find('}').map(|i| 1 + 1 + i + 1),
            None => Some(1 + name_len),
        };
        let name = &body[..name_len];
        let home = name == "HOME" || (name.starts_with("XDG_") && name.ends_with("_HOME"));
        match end.filter(|_| home) {
            Some(end) => {
                out.push(HOME);
                rest = &rest[at + end..];
            }
            None => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The files a hooks folder called `_` in the working tree runs beside it: its parent's files
/// named as its hooks. husky 9's hooks there each source `_/h`, which runs `../<hook>` by a
/// path it builds from `$0` at run time, so the words can't name it.
fn parent_scripts(workdir: &Path, hooks_dir: &Path, names: &[String]) -> Vec<PathBuf> {
    if hooks_dir.file_name().is_none_or(|n| n != "_") {
        return Vec::new();
    }
    let Some(parent) = hooks_dir.parent().filter(|p| in_tree(workdir, p)) else {
        return Vec::new();
    };
    names.iter().map(|name| parent.join(name)).collect()
}

/// `folder` as the part of a word that a script's `$(dirname "$0")` stands for: relative to the
/// working tree when it is there, `/`-separated, read as quoted (spaces and glob characters are
/// part of its name). None for a folder whose name isn't UTF-8.
fn folder_text(workdir: &Path, folder: &Path) -> Option<String> {
    let rel = folder.strip_prefix(workdir).ok().map(|r| if r.as_os_str().is_empty() { Path::new(".") } else { r });
    // Quoted, a `~` is a folder of that name, never a home folder.
    Some(shell::quoted(&rel.unwrap_or(folder).to_str()?.replace('\\', "/")))
}

#[cfg(test)]
#[path = "programs_hooks_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "programs_hooks_chain_tests.rs"]
mod chain_tests;

#[cfg(test)]
#[path = "programs_hooks_noise_tests.rs"]
mod noise_tests;

#[cfg(test)]
#[path = "programs_hooks_shell_tests.rs"]
mod shell_tests;
