//! Adding and removing submodules: the checks on the Add dialog's fields (the URL, the folder
//! typed for the submodule and the branch), the command lines for `git submodule add`,
//! `git submodule deinit` and `git rm`, and where git keeps a submodule's own repository
//! (`<git dir>/modules/<name>`; a linked worktree has its own store under
//! `.git/worktrees/<id>/modules`). The disk and index checks of a new submodule's folder
//! are in `repo/submodule_add.rs`; removing one is in `remove.rs`.

use crate::names::{WinNameProblem, blocks_here, is_git_dir_name, win_name_problem};
use crate::paths;
use crate::remote_ops::{UrlProblem, check_url};
use std::path::{Component, Path, PathBuf};

mod remove;
pub use remove::*;

/// Why a folder can't take a new submodule. `message` words it for the dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum AddPathProblem {
    Empty,
    /// The working tree's own folder.
    Root,
    /// An absolute path outside the working tree, or one with a `..` part.
    Outside,
    InsideGitDir,
    /// Windows can't use this name (refused on Windows only; elsewhere it is a warning).
    BadName(String, WinNameProblem),
    /// Already the path of this submodule.
    IsSubmodule(String),
    /// Inside this submodule's folder.
    InsideSubmodule(String),
    /// This submodule is inside the folder.
    ContainsSubmodule(String),
    /// The index has an entry at the path: a gitlink, or a tracked file.
    InIndex {
        gitlink: bool,
    },
    /// Tracked files are under the folder.
    TrackedInside,
    /// This parent folder of the path is a tracked file.
    UnderTrackedFile(String),
    /// Something is already there: a file (or link), or a folder that isn't empty.
    Exists {
        file: bool,
    },
    /// This parent folder is a symbolic link or junction.
    ParentLink(String),
    /// This parent folder is a file.
    ParentFile(String),
    /// An existing parent folder is spelled another way on disk (the sentence says how).
    ParentSpelling(String),
    /// .gitignore ignores the path, and git refuses to add a submodule there without -f.
    Ignored,
    /// A stored module from an earlier submodule of this name is still in the git dir.
    ModuleExists(PathBuf),
    /// The repository or the disk couldn't be read.
    Unreadable(String),
}

impl AddPathProblem {
    /// The sentence shown under the Path field; `path` is the folder as checked.
    pub fn message(&self, path: &str) -> String {
        use AddPathProblem as P;
        let choose = "Choose a folder that doesn't exist yet, or an empty one.";
        match self {
            P::Empty => "Enter the folder for the submodule, relative to the repository.".into(),
            P::Root => "The submodule needs a folder of its own, not the repository's root.".into(),
            P::Outside => "That path leads outside the working tree.".into(),
            P::InsideGitDir => "A submodule can't go inside the .git folder.".into(),
            P::BadName(name, why) => format!("{name} can't be used: {}.", why.reason()),
            P::IsSubmodule(p) => format!("{p} is already a submodule."),
            P::InsideSubmodule(p) => format!("{path} is inside the submodule {p}. Add it from that repository instead."),
            P::ContainsSubmodule(p) => format!("The submodule {p} is inside {path}. Choose another folder."),
            P::InIndex { gitlink: true } => format!("{path} is already a submodule entry in the index."),
            P::InIndex { gitlink: false } => format!("{path} is already tracked. {choose}"),
            P::TrackedInside => format!("{path} holds tracked files. {choose}"),
            P::UnderTrackedFile(p) => format!("{p} is a tracked file, so {path} can't be a folder inside it."),
            P::Exists { file: true } => format!("{path} is a file. {choose}"),
            P::Exists { file: false } => format!("{path} already exists. {choose}"),
            P::ParentLink(p) => format!("{p} is a symbolic link or junction; Trellis won't add a submodule through it."),
            P::ParentFile(p) => format!("{p} is a file, so {path} can't be created."),
            P::ParentSpelling(why) => sentence(why),
            P::Ignored => format!("{path} is ignored by .gitignore, so git won't add a submodule there."),
            P::ModuleExists(dir) => format!(
                "An earlier submodule at {path} left its repository in {}. Delete that folder first, or choose another path.",
                crate::native_path::shown(dir)
            ),
            P::Unreadable(e) => format!("Could not check {path}: {e}"),
        }
    }
}

/// `text` with a capital first letter and a full stop.
fn sentence(text: &str) -> String {
    let mut c = text.chars();
    let mut out: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// The typed folder for a new submodule as a repo path ('/'-separated), with Windows' naming
/// rules refused on Windows only.
pub fn parse_add_path(text: &str, workdir: &Path) -> Result<String, AddPathProblem> {
    parse_add_path_for(text, workdir, blocks_here())
}

/// [`parse_add_path`], with Windows' rules (`\` as a separator, its reserved names refused)
/// when `windows`. A path inside the working tree `workdir` may be typed in full; anything
/// else is relative to it. `./`, doubled separators and a trailing separator are dropped.
pub fn parse_add_path_for(text: &str, workdir: &Path, windows: bool) -> Result<String, AddPathProblem> {
    let text = text.trim();
    let relative: String;
    let rel = if paths::typed_is_absolute(text) {
        let full = Path::new(text);
        if !paths::is_within(full, workdir) {
            return Err(AddPathProblem::Outside);
        }
        if paths::same_spelling(full, workdir) {
            return Err(AddPathProblem::Root);
        }
        let skip = workdir.components().filter(|c| !matches!(c, Component::CurDir)).count();
        relative = full
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .skip(skip)
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        relative.as_str()
    } else {
        text
    };
    let seps: &[char] = if windows { &['/', '\\'] } else { &['/'] };
    // A leading separator (or "\\server") is a path from a drive's root, never inside.
    if rel.starts_with(seps) {
        return Err(AddPathProblem::Outside);
    }
    let parts: Vec<&str> = rel.split(seps).filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() {
        return Err(AddPathProblem::Empty);
    }
    if parts.contains(&"..") {
        return Err(AddPathProblem::Outside);
    }
    if parts.iter().any(|p| is_git_dir_name(p)) {
        return Err(AddPathProblem::InsideGitDir);
    }
    if windows && let Some((bad, why)) = parts.iter().find_map(|p| win_name_problem(p).map(|w| (*p, w))) {
        return Err(AddPathProblem::BadName(bad.to_owned(), why));
    }
    Ok(parts.join("/"))
}

/// The folder suggested for a submodule cloned from `url`: the repository's own name, made
/// usable on Windows (`con` becomes `con-repo`).
pub fn default_path(url: &str) -> String {
    crate::clone_ops::repo_name(url)
}

/// Checks the typed repository URL (trimmed first).
pub fn check_add_url(url: &str) -> Result<(), UrlProblem> {
    check_url(url.trim())
}

/// The Add dialog's words for a URL problem.
pub fn add_url_message(p: &UrlProblem) -> &'static str {
    match p {
        UrlProblem::Empty => "Enter the URL or path of the repository to add.",
        UrlProblem::Dash => "URLs can't start with a dash.",
        UrlProblem::Control => "URLs can't contain control characters.",
    }
}

/// Whether `url` reaches the repository through git's file transport (a path on this computer,
/// or `file://`), which git refuses for submodules unless it is allowed for the command.
pub fn needs_file_protocol(url: &str) -> bool {
    crate::clone_ops::is_local_source(url) || url.trim_start().to_ascii_lowercase().starts_with("file://")
}

/// `git submodule add [-b <branch>] -- <url> <path>`. git refuses the file transport for
/// submodules by default, because a cloned repository's .gitmodules could point at local paths;
/// here the user typed the URL, so a local one is allowed for this one command.
pub fn add_cmd(url: &str, path: &str, branch: Option<&str>) -> Vec<String> {
    let url = url.trim();
    let mut cmd = Vec::new();
    if needs_file_protocol(url) {
        cmd.extend(["-c".to_owned(), "protocol.file.allow=always".to_owned()]);
    }
    cmd.extend(["submodule".to_owned(), "add".to_owned()]);
    if let Some(b) = branch {
        cmd.extend(["-b".to_owned(), b.to_owned()]);
    }
    cmd.extend(["--".to_owned(), url.to_owned(), path.to_owned()]);
    cmd
}

/// `git submodule deinit -f -- :(literal)<path>`: empties its folder and drops its config
/// section. `--literal-pathspecs` doesn't reach `git submodule`, but the pathspec magic does.
pub fn deinit_cmd(path: &str) -> Vec<String> {
    ["submodule", "deinit", "-f", "--", &format!(":(literal){path}")].map(str::to_owned).to_vec()
}

/// `git --literal-pathspecs rm -f -- <path>`: drops the gitlink and the .gitmodules entry, both
/// staged.
pub fn rm_cmd(path: &str) -> Vec<String> {
    ["--literal-pathspecs", "rm", "-f", "--", path].map(str::to_owned).to_vec()
}

/// Where git stores the repository of the submodule called `name`: `<git_dir>/modules/<name>`,
/// joined one part at a time (`git_dir` is the repository's own git dir, which in a linked
/// worktree is `.git/worktrees/<id>`). A name that could lead elsewhere (empty, absolute, with an empty,
/// `.` or `..` part, a `\` on Windows, or a part that reaches `.git`) is refused.
pub fn module_dir(git_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let refuse = || Err(format!("the submodule name {name:?} can't be used as a folder name"));
    if name.is_empty() || name.starts_with('/') || (cfg!(windows) && (name.contains('\\') || name.contains(':'))) {
        return refuse();
    }
    let parts: Vec<&str> = name.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || is_git_dir_name(p)) {
        return refuse();
    }
    Ok(parts.iter().fold(git_dir.join("modules"), |dir, p| dir.join(p)))
}

#[cfg(test)]
mod tests;
