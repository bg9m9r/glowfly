//! Scripts a hook runs from the working tree. A hook such as `exec ./scripts/check.sh` or
//! `node tools/lint.js` runs a file the branch carries: a checkout or pull that changes it
//! changes what the hook does without changing the hook. `scan` reads each hook's text with the
//! word scanner the config programs use (`programs.rs`), with its rules and `follow::far` guard,
//! so the trust scan follows those files as it follows a config program's. A bare word
//! (`npx lint-staged`, `git diff`, `npm test`) is no path here (`path_of`'s `hook`) unless an
//! interpreter, `.`, `source` or `exec` runs it (`sh check.sh`: `programs_hooks_runs.rs`), and
//! `$(dirname "$0")` is read as the folder of the script it is in (husky's
//! `. "$(dirname "$0")/h"`).
//!
//! A file a hook runs is read the same way, and so on up to `MAX_DEPTH` scripts down (a hook,
//! `.husky/pre-commit`, `scripts/check.sh`, a `lib.sh` it sources). A hooks folder called `_`
//! in the working tree also runs the files beside it named as its hooks (`parent_scripts`):
//! husky 9's `.husky/_/h` runs `.husky/<hook>` by a path it builds at run time.
//!
//! Bounded: at most `MAX_WORDS` words and `MAX_FILES` files in all, lines up to
//! `MAX_LINE_BYTES`, and `MAX_DEPTH` scripts down. Past any of them, for a hook or script too
//! large to keep its text (`MAX_TEXT_BYTES`) or one whose text can't be read, and for one with a
//! NUL byte in it (a compiled program, whose words can't be read), the whole working tree is
//! followed.

#[path = "programs_hooks_runs.rs"]
mod runs;

use super::{candidates, path_of};
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The most bytes of a hook or script whose text is read for the files it runs.
pub(in crate::trust) const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// The most words read from all hooks and the scripts they run.
const MAX_WORDS: usize = 4096;

/// The longest line read; the word scanner's work grows with a line's words.
const MAX_LINE_BYTES: usize = 4096;

/// The most files followed for what hooks run.
const MAX_FILES: usize = 64;

/// How many scripts down from a hook are read: a hook's own text is at 0.
const MAX_DEPTH: usize = 3;

/// Where a script's text named its own folder: put in before the text is split into words, so
/// a folder with spaces or shell characters in its name stays one part of one word.
const OWN: &str = "\u{E020}";

/// Where it named its folder's parent, as `OWN` does.
const PARENT: &str = "\u{E021}";

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
/// `offsite`; one whose bytes name no file here is named in `unread`.
pub(in crate::trust) fn scan(
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
        offsite,
        unread,
    };
    for path in parent_scripts(workdir, hooks_dir, names) {
        run.found(path, 1);
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

/// A text to read for the files it runs: a hook's (`depth` 0) or a script's a hook runs.
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
        let mut text = super::super::keys::text_of(bytes).into_owned();
        // A folder whose name isn't UTF-8 stays a substitution.
        let marks = [(PARENT_FOLDER, PARENT, &parent), (OWN_FOLDER, OWN, &folder)];
        for (forms, mark, _) in marks.iter().filter(|(_, _, text)| text.is_some()) {
            for form in *forms {
                text = text.replace(form, mark);
            }
        }
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if line.len() > MAX_LINE_BYTES {
                self.whole = true;
                return;
            }
            let mut paths = Vec::new();
            for mut word in candidates(line) {
                self.words += 1;
                if self.words > MAX_WORDS {
                    self.whole = true;
                    return;
                }
                for (_, mark, folder) in &marks {
                    if let Some(folder) = folder {
                        word.text = word.text.replace(mark, folder);
                    }
                }
                paths.extend(path_of(self.workdir, self.home.as_deref(), &word, true, self.unread));
            }
            let here = in_tree(self.workdir, &script.folder).then_some(script.folder.as_path());
            for word in runs::run_words(line) {
                paths.extend(runs::bare_files(self.workdir, here, &word));
            }
            for p in paths {
                if !self.found(p, script.depth + 1) {
                    return;
                }
            }
        }
    }

    /// Follows `path`, run from a script `depth - 1` down, and reads it in turn when it is a
    /// file in the working tree. False once `MAX_FILES` is reached.
    fn found(&mut self, path: PathBuf, depth: usize) -> bool {
        if super::super::follow::note_far(&path, self.workdir, self.offsite) || self.out.contains(&path) {
            return true;
        }
        if self.out.len() >= MAX_FILES {
            (self.whole, self.full) = (true, true);
            return false;
        }
        self.out.push(path.clone());
        let Some(folder) = in_tree(self.workdir, &path).then(|| path.parent().map(Path::to_path_buf)).flatten() else {
            return true;
        };
        match std::fs::symlink_metadata(&path) {
            // A link may lead anywhere: what it runs isn't read through it.
            Ok(m) if m.file_type().is_symlink() => self.whole = true,
            Ok(m) if m.is_file() && depth > MAX_DEPTH => self.whole = true,
            Ok(m) if m.is_file() => self.queue.push_back(Script { bytes: text_of_file(&path), folder, depth }),
            // A folder is followed whole; a missing file has no text yet.
            _ => {}
        }
        true
    }
}

/// The bytes of the file at `path`, or None when it can't be read or is over `MAX_TEXT_BYTES`.
fn text_of_file(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path).ok()?;
    file.take(MAX_TEXT_BYTES as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= MAX_TEXT_BYTES).then_some(bytes)
}

/// Whether `path` is in the working tree and not in a `.git` folder at its top: a file there is
/// one the branch may carry.
fn in_tree(workdir: &Path, path: &Path) -> bool {
    use std::path::Component;
    let Ok(rel) = path.strip_prefix(workdir) else { return false };
    // `..` parts applied as written (`.husky/_/../check.sh`); one that climbs out is outside.
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(name) => parts.push(name),
            Component::ParentDir if parts.pop().is_some() => {}
            Component::CurDir => {}
            _ => return false,
        }
    }
    parts.first().is_none_or(|first| !first.to_str().is_some_and(|s| s.eq_ignore_ascii_case(".git")))
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
    let text = rel.unwrap_or(folder).to_str()?.replace('\\', "/");
    // A relative `~x` names a folder here, never a home folder.
    let text = if rel.is_some() && text.starts_with('~') { format!("./{text}") } else { text };
    Some(super::shell::quoted(&text))
}

#[cfg(test)]
#[path = "programs_hooks_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "programs_hooks_chain_tests.rs"]
mod chain_tests;
