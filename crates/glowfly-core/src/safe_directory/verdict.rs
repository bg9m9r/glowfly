//! Asking git whether it opens a folder libgit2 refused for its owner, and whom git says owns
//! it. The two disagree: on Windows libgit2 accepts a folder owned by the Administrators group
//! only from an elevated process (and compares `safe.directory` entries letter for letter),
//! while Git for Windows has its own rules. Glowfly follows git, never its own reading.

use crate::gitcli;
use std::path::Path;

/// What git says about a folder libgit2 refused for its owner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    /// git opens the repository at `root` from there.
    pub opens: bool,
    /// The folder's owner as git names it ("BUILTIN\Administrators"), when git said.
    pub owner: Option<String>,
    /// The owner is the Administrators group (by its SID, whatever the system's language calls it).
    pub admins: bool,
}

/// The environment the question is asked in: no prompts, and none of the variables that name a
/// repository outright, with which git skips its search and its owner check.
pub const ENV: &[(&str, &str)] = &[
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS", ""),
    ("SSH_ASKPASS_REQUIRE", "never"),
    ("GIT_DIR", gitcli::UNSET),
    ("GIT_WORK_TREE", gitcli::UNSET),
    ("GIT_COMMON_DIR", gitcli::UNSET),
];

/// Runs `git rev-parse --show-toplevel` in `path` with `env` and reads the answer: git opens
/// it when it succeeds and finds the repository at `root` (not one above or beside it).
pub fn ask(path: &Path, root: &Path, env: &[(&str, &str)]) -> Verdict {
    let out = gitcli::output(path, &gitcli::args(&["rev-parse", "--show-toplevel"]), "", env, Some(gitcli::READ_TIMEOUT));
    if !out.ok {
        return Verdict { opens: false, owner: owner_in(&out.stderr), admins: admins_in(&out.stderr) };
    }
    let top = crate::native_path::native(Path::new(out.stdout.trim_end_matches(['\r', '\n'])));
    Verdict { opens: same_folder(&top, root), ..Verdict::default() }
}

/// Whether git's top level `top` is the folder `root`, by identity when the disk gives one.
fn same_folder(top: &Path, root: &Path) -> bool {
    match (crate::paths::FolderId::of(top), crate::paths::FolderId::of(root)) {
        (Some(a), Some(b)) => a == b,
        _ => crate::paths::same_spelling(&crate::paths::resolve(top), root),
    }
}

/// The Administrators group's SID, the same in every language.
const ADMINISTRATORS_SID: &str = "(S-1-5-32-544)";

/// Whether the owner git names in its refusal is the Administrators group.
pub fn admins_in(stderr: &str) -> bool {
    let mut lines = stderr.lines();
    lines.find(|l| l.trim_end().ends_with("is owned by:")).is_some()
        && lines.next().is_some_and(|l| l.trim_end().ends_with(ADMINISTRATORS_SID))
}

/// The owner git names in its refusal: the line after "... is owned by:", without the SID
/// after it and with Windows' backslash between domain and name. Git for Windows writes it;
/// other gits don't, nor does any git for a drive that records no owner (FAT, exFAT).
pub fn owner_in(stderr: &str) -> Option<String> {
    let mut lines = stderr.lines();
    lines.find(|l| l.trim_end().ends_with("is owned by:"))?;
    let line = lines.next()?.trim();
    let name = match line.rfind(" (S-") {
        Some(i) if line.ends_with(')') => &line[..i],
        _ => line,
    };
    let name = name.trim().replace('/', "\\");
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owner_is_read_from_gits_refusal() {
        let refusal = "fatal: detected dubious ownership in repository at 'C:/work/app'\n\
                       'C:/work/app' is owned by:\n\
                       \tBUILTIN/Administrators (S-1-5-32-544)\n\
                       but the current user is:\n\
                       \tPC/ada (S-1-5-21-1-2-3-1001)\n\
                       To add an exception for this directory, call:\n\n\
                       \tgit config --global --add safe.directory C:/work/app\n";
        assert_eq!(owner_in(refusal).as_deref(), Some(r"BUILTIN\Administrators"));
        assert!(admins_in(refusal));
        let german =
            "'C:/w' is owned by:\n\tVORDEFINIERT/Administratoren (S-1-5-32-544)\nbut the current user is:\n\tPC/ada (S-1-5-21-9)\n";
        assert!(admins_in(german), "known by its SID in any language");
        let other = "'C:/w' is owned by:\n\tPC/bob (S-1-5-21-9)\nbut the current user is:\n\tPC/ada (S-1-5-32-544)\n";
        assert!(!admins_in(other), "only the owner's line counts");
        assert_eq!(owner_in("'C:/w' is owned by:\r\n\tOTHER\\bob\r\n").as_deref(), Some(r"OTHER\bob"), "no SID, CRLF");
        let linux = "fatal: detected dubious ownership in repository at '/srv/app'\nTo add an exception for this directory, call:\n";
        assert_eq!(owner_in(linux), None, "git names no owner");
        assert_eq!(owner_in("'E:/app' is on a file system that does not record ownership\n"), None);
        assert_eq!(owner_in("'C:/w' is owned by:\n"), None, "cut short");
    }

    #[test]
    fn git_opens_an_ordinary_repository_where_it_is() {
        let dir = crate::testutil::scratch_repo("owner-verdict");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let root = crate::paths::resolve(&dir);
        assert_eq!(ask(&dir.join("src"), &root, ENV), Verdict { opens: true, owner: None, admins: false });
        // Found, but not at the folder libgit2 refused: not git's word for that one.
        let inner = dir.join("src");
        assert!(!ask(&inner, &crate::paths::resolve(&inner), ENV).opens);
        // A GIT_DIR Glowfly was started with would skip git's search and its owner check.
        let unset = gitcli::output(&dir, &gitcli::args(&["rev-parse", "--absolute-git-dir"]), "", &[("GIT_DIR", gitcli::UNSET)], None);
        assert!(unset.ok, "{}", unset.stderr);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
