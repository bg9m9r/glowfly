//! Which paths a cone-mode sparse checkout keeps, read from the worktree's
//! `info/sparse-checkout` file without running git, so a load of an untrusted repository can
//! use it. git's sparse checkout never leaves a gitlink out, so this is what tells a submodule
//! outside the chosen folders from one inside them.

use git2::Repository;

/// The folders a cone-mode sparse checkout of `repo` checks out in full (repo-relative, with
/// '/'), or None when sparse checkout is off or not in cone mode (patterns are not read here).
/// An empty list keeps only the files at the root.
pub fn cone_folders(repo: &Repository) -> Option<Vec<String>> {
    let config = repo.config().ok()?;
    if !config.get_bool("core.sparseCheckout").unwrap_or(false) || !config.get_bool("core.sparseCheckoutCone").unwrap_or(false) {
        return None;
    }
    let text = std::fs::read_to_string(repo.path().join("info").join("sparse-checkout")).ok()?;
    parse_cone(&text)
}

/// The fully kept folders of a cone pattern file: each `/dir/` line not followed by its own
/// `!/dir/*/` (those are the parents git adds, which keep only their files). None when a line
/// isn't one git writes in cone mode.
pub(super) fn parse_cone(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let mut dirs = Vec::new();
    let mut parents = Vec::new();
    for l in lines {
        match l {
            "/*" | "!/*/" => {}
            l if l.starts_with("!/") && l.ends_with("/*/") => parents.push(unescape(&l[2..l.len() - 3])),
            l if l.starts_with('/') && l.ends_with('/') && l.len() > 2 => dirs.push(unescape(&l[1..l.len() - 1])),
            _ => return None,
        }
    }
    Some(dirs.into_iter().filter(|d| !parents.contains(d)).collect())
}

/// A cone pattern's path with git's backslash escapes (`\*`, `\?`, `\[`, `\\`, `\ `) removed.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '\\' { chars.next().unwrap_or('\\') } else { c });
    }
    out
}

/// Whether the folder at `path` (a submodule's) is checked out by a cone keeping `folders`: it
/// is one of them, inside one, or holds one (git needs it to reach that folder).
pub fn in_cone(path: &str, folders: &[String]) -> bool {
    folders.iter().any(|f| path == f || path.starts_with(&format!("{f}/")) || f.starts_with(&format!("{path}/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn cone_files_give_the_kept_folders() {
        assert_eq!(parse_cone("/*\n!/*/\n"), Some(Vec::new()), "only the root's files");
        assert_eq!(parse_cone("/*\n!/*/\n/a/\n"), Some(s(&["a"])));
        let nested = "/*\n!/*/\n/src/\n!/src/*/\n/src/app/\n/docs/\n";
        assert_eq!(parse_cone(nested), Some(s(&["src/app", "docs"])), "src is a parent: its files only");
        assert_eq!(parse_cone("/*\n!/*/\n/my\\ dir/\n/a\\*b/\n"), Some(s(&["my dir", "a*b"])));
        assert_eq!(parse_cone("/*\n!/*/\n*.rs\n"), None, "a pattern git doesn't write in cone mode");
    }

    #[test]
    fn submodule_paths_in_and_out_of_the_cone() {
        let folders = s(&["libs", "src/app"]);
        for inside in ["libs", "libs/lib", "src/app/vendor/x", "src"] {
            assert!(in_cone(inside, &folders), "{inside}");
        }
        for outside in ["far/lib", "libsx/lib", "src/other", "top"] {
            assert!(!in_cone(outside, &folders), "{outside}");
        }
        assert!(!in_cone("top", &[]), "only the root's files");
    }
}
