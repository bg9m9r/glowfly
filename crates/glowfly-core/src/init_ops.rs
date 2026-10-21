//! Starting a new repository: the files Glowfly can add to it (a `.gitignore` from a short
//! template per language plus an optional section for operating system and editor files, and a
//! license from the usual set), what is already in the chosen folder, and the init job itself:
//! create the folder, `git init`, write each chosen file only if it isn't there yet, and commit
//! what was written. git runs through `gitcli`, so the user's config, hooks and signing apply.
//!
//! The texts live next to this file (`init_ops/gitignore`, `init_ops/licenses`). The license
//! texts are the official ones; the templates are Glowfly's own.

use crate::gitcli::{self, args};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A language's `.gitignore` template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitignoreTemplate {
    Rust,
    Node,
    Python,
    JavaKotlin,
    DotNet,
    Go,
    CCpp,
}

impl GitignoreTemplate {
    /// In the order the Init dialog lists them.
    pub const ALL: [GitignoreTemplate; 7] = [Self::Rust, Self::Node, Self::Python, Self::JavaKotlin, Self::DotNet, Self::Go, Self::CCpp];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Node => "Node.js",
            Self::Python => "Python",
            Self::JavaKotlin => "Java / Kotlin",
            Self::DotNet => ".NET",
            Self::Go => "Go",
            Self::CCpp => "C / C++",
        }
    }

    /// The template's lines (`\n` line endings, ending with one).
    pub fn body(self) -> String {
        lf(match self {
            Self::Rust => include_str!("init_ops/gitignore/rust.txt"),
            Self::Node => include_str!("init_ops/gitignore/node.txt"),
            Self::Python => include_str!("init_ops/gitignore/python.txt"),
            Self::JavaKotlin => include_str!("init_ops/gitignore/java_kotlin.txt"),
            Self::DotNet => include_str!("init_ops/gitignore/dotnet.txt"),
            Self::Go => include_str!("init_ops/gitignore/go.txt"),
            Self::CCpp => include_str!("init_ops/gitignore/c_cpp.txt"),
        })
    }
}

/// Files operating systems and editors leave in a folder.
pub const OS_EDITOR: &str = include_str!("init_ops/gitignore/os_editor.txt");

/// `text` with `\n` line endings, ending with exactly one. A checkout may have given the files
/// `include_str!` reads platform line endings.
fn lf(text: &str) -> String {
    let mut s = text.replace("\r\n", "\n").trim_end_matches('\n').to_owned();
    s.push('\n');
    s
}

/// The `.gitignore` for a template and the OS and editor section, each under a `# <name>`
/// heading and separated by a blank line; None when neither is chosen.
pub fn gitignore_text(template: Option<GitignoreTemplate>, os_editor: bool) -> Option<String> {
    let mut sections = Vec::new();
    if let Some(t) = template {
        sections.push(format!("# {}\n{}", t.label(), t.body()));
    }
    if os_editor {
        sections.push(format!("# Operating system and editor files\n{}", lf(OS_EDITOR)));
    }
    (!sections.is_empty()).then(|| sections.join("\n"))
}

/// A license for the new repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum License {
    Mit,
    Apache2,
    Bsd2,
    Bsd3,
    Gpl3,
    Mpl2,
    Unlicense,
}

impl License {
    /// In the order the Init dialog lists them.
    pub const ALL: [License; 7] = [Self::Mit, Self::Apache2, Self::Bsd2, Self::Bsd3, Self::Gpl3, Self::Mpl2, Self::Unlicense];

    pub fn label(self) -> &'static str {
        match self {
            Self::Mit => "MIT",
            Self::Apache2 => "Apache 2.0",
            Self::Bsd2 => "BSD 2-Clause",
            Self::Bsd3 => "BSD 3-Clause",
            Self::Gpl3 => "GPL 3.0",
            Self::Mpl2 => "MPL 2.0",
            Self::Unlicense => "The Unlicense",
        }
    }

    /// The SPDX identifier.
    pub fn spdx(self) -> &'static str {
        match self {
            Self::Mit => "MIT",
            Self::Apache2 => "Apache-2.0",
            Self::Bsd2 => "BSD-2-Clause",
            Self::Bsd3 => "BSD-3-Clause",
            Self::Gpl3 => "GPL-3.0-only",
            Self::Mpl2 => "MPL-2.0",
            Self::Unlicense => "Unlicense",
        }
    }

    /// Whether the text has a copyright line naming a year and a holder.
    pub fn needs_holder(self) -> bool {
        matches!(self, Self::Mit | Self::Bsd2 | Self::Bsd3)
    }

    fn template(self) -> &'static str {
        match self {
            Self::Mit => include_str!("init_ops/licenses/mit.txt"),
            Self::Apache2 => include_str!("init_ops/licenses/apache2.txt"),
            Self::Bsd2 => include_str!("init_ops/licenses/bsd2.txt"),
            Self::Bsd3 => include_str!("init_ops/licenses/bsd3.txt"),
            Self::Gpl3 => include_str!("init_ops/licenses/gpl3.txt"),
            Self::Mpl2 => include_str!("init_ops/licenses/mpl2.txt"),
            Self::Unlicense => include_str!("init_ops/licenses/unlicense.txt"),
        }
    }
}

/// The file name every license is written as.
pub const LICENSE_FILE: &str = "LICENSE";

/// The text of `license`, with `year` and `holder` in the copyright line of those that have one.
pub fn license_text(license: License, year: i32, holder: &str) -> String {
    let text = lf(license.template());
    if license.needs_holder() { text.replace("{year}", &year.to_string()).replace("{holder}", holder) } else { text }
}

/// The message of the commit that adds the new files.
pub const INITIAL_COMMIT: &str = "Initial commit";

/// A file to add to the new repository (`name` at its root).
#[derive(Clone, Debug, PartialEq)]
pub struct NewFile {
    pub name: String,
    pub contents: String,
}

/// What the init job does after `git init`.
#[derive(Clone, Debug, PartialEq)]
pub struct InitPlan {
    /// `--initial-branch`; None is git's default.
    pub branch: Option<String>,
    /// Written only where no file of that name exists.
    pub files: Vec<NewFile>,
    /// Commit the files written, with this message.
    pub commit: Option<String>,
}

/// `git init` arguments (without the leading `git`), run in the folder.
pub fn init_cmd(branch: Option<&str>) -> Vec<String> {
    match branch {
        Some(b) => vec!["init".to_owned(), format!("--initial-branch={b}")],
        None => args(&["init"]),
    }
}

/// What the Init dialog's folder text names.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderFacts {
    /// The folder, resolved (its parent, when the folder doesn't exist yet).
    pub path: PathBuf,
    pub exists: bool,
    /// It has a `.git` (folder or file).
    pub repo: bool,
    /// It holds anything at all.
    pub has_files: bool,
    pub gitignore: bool,
    /// The name of the license file it has, in its own letter case.
    pub license: Option<String>,
}

const LICENSE_NAMES: [&str; 4] = ["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"];

/// Looks at the folder typed as `text` (`~` is `home`): None when it isn't a full path, isn't a
/// folder, or neither it nor its parent exists.
pub fn inspect_folder(text: &str, home: Option<&Path>) -> Option<FolderFacts> {
    let p = crate::paths::expand_home(text.trim(), home);
    if !p.is_absolute() {
        return None;
    }
    if p.is_dir() {
        let names: Vec<String> =
            std::fs::read_dir(&p).ok()?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        let license = names.iter().find(|n| LICENSE_NAMES.iter().any(|l| l.eq_ignore_ascii_case(n))).cloned();
        return Some(FolderFacts {
            path: crate::paths::resolve(&p),
            exists: true,
            repo: p.join(".git").exists(),
            has_files: !names.is_empty(),
            gitignore: p.join(".gitignore").exists(),
            license,
        });
    }
    if p.exists() {
        return None;
    }
    let (parent, name) = (p.parent()?, p.file_name()?);
    parent.is_dir().then(|| FolderFacts {
        path: crate::paths::resolve(parent).join(name),
        exists: false,
        repo: false,
        has_files: false,
        gitignore: false,
        license: None,
    })
}

/// Why the copyright holder can't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HolderProblem {
    Empty,
    NotOneLine,
    TooLong,
}

impl HolderProblem {
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "Enter the name for the copyright line.",
            Self::NotOneLine => "Use a single line of text.",
            Self::TooLong => "Use at most 200 characters.",
        }
    }
}

/// The copyright holder typed in the Init dialog, trimmed.
pub fn check_holder(text: &str) -> Result<String, HolderProblem> {
    let t = text.trim();
    if t.is_empty() {
        Err(HolderProblem::Empty)
    } else if t.chars().any(|c| c.is_control() || crate::text::is_bidi_control(c)) {
        Err(HolderProblem::NotOneLine)
    } else if t.chars().count() > 200 {
        Err(HolderProblem::TooLong)
    } else {
        Ok(t.to_owned())
    }
}

/// What the init job did.
#[derive(Clone, Debug, PartialEq)]
pub struct InitOutcome {
    /// `git init` succeeded, so the folder is a repository.
    pub initialized: bool,
    /// Every step succeeded.
    pub ok: bool,
    /// The step that failed after init, for the message ("the commit", "writing LICENSE").
    pub failed: Option<String>,
    /// The job log: each command line or file written, then git's output.
    pub output: String,
}

/// Appends `$ git <cmd>` and git's output to `log`, after running it in `cwd` with the extra
/// environment `env`.
fn git(cwd: &Path, cmd: &[String], env: &[(&str, &str)], log: &mut String) -> bool {
    let (ok, out) = gitcli::run_with_input(cwd, cmd, "", env);
    log.push_str(&format!("$ git {}\n", cmd.join(" ")));
    if !out.trim().is_empty() {
        log.push_str(out.trim_end());
        log.push('\n');
    }
    ok
}

/// Writes `file` into `folder` unless a file of that name is there: Ok(true) when written.
fn write_new(folder: &Path, file: &NewFile) -> std::io::Result<bool> {
    match std::fs::OpenOptions::new().write(true).create_new(true).open(folder.join(&file.name)) {
        Ok(mut f) => f.write_all(file.contents.as_bytes()).and(f.sync_all()).map(|()| true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

/// Runs the init job in `folder` (created if missing; see the module comment).
pub fn run(folder: &Path, plan: &InitPlan) -> InitOutcome {
    run_env(folder, plan, &[])
}

/// [`run`], with extra environment variables for git (tests give an identity this way).
fn run_env(folder: &Path, plan: &InitPlan, env: &[(&str, &str)]) -> InitOutcome {
    let mut log = String::new();
    let done = |initialized: bool, ok: bool, failed: Option<String>, log: String| InitOutcome {
        initialized,
        ok,
        failed,
        output: log.trim_end().to_owned(),
    };
    if let Err(e) = std::fs::create_dir_all(folder) {
        log.push_str(&format!("Could not create {}: {e}", crate::native_path::shown(folder)));
        return done(false, false, None, log);
    }
    if !git(folder, &init_cmd(plan.branch.as_deref()), env, &mut log) {
        return done(false, false, None, log);
    }
    let mut written = Vec::new();
    for file in &plan.files {
        match write_new(folder, file) {
            Ok(true) => {
                log.push_str(&format!("Wrote {}.\n", file.name));
                written.push(file.name.clone());
            }
            Ok(false) => log.push_str(&format!("Kept the existing {}.\n", file.name)),
            Err(e) => {
                log.push_str(&format!("Could not write {}: {e}\n", file.name));
                return done(true, false, Some(format!("writing {}", file.name)), log);
            }
        }
    }
    if let Some(message) = plan.commit.as_deref().filter(|_| !written.is_empty()) {
        let mut add = args(&["add", "--"]);
        add.extend(written.iter().cloned());
        // Only the files written go in, whatever else is staged.
        let mut commit = args(&["commit", "-m", message, "--"]);
        commit.extend(written.iter().cloned());
        if !git(folder, &add, env, &mut log) || !git(folder, &commit, env, &mut log) {
            return done(true, false, Some("the commit".into()), log);
        }
    }
    done(true, true, None, log)
}

#[cfg(test)]
mod tests;
