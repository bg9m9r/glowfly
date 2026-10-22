//! Checks and command lines for adding, editing, fetching and removing a remote. Everything here
//! is pure; the app runs the commands through `gitcli` so credentials and the user's git config
//! apply.

use crate::gitcli::args;
use crate::repo::RemoteInfo;

/// Whether git accepts `name` as a remote name: `refs/remotes/<name>/x` must pass the
/// ref-format rules, and a name starting with '-' would read as an option. A trailing dot is
/// refused too: git allows it, but Windows drops it from the refs/remotes/<name> folder.
pub fn is_valid_remote_name(name: &str) -> bool {
    // Control characters first: git2 panics on an interior NUL when it builds its C string.
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('.')
        && !name.chars().any(char::is_control)
        && git2::Remote::is_valid_name(name)
}

/// Why a remote name can't be used.
#[derive(Debug, PartialEq)]
pub enum RemoteNameProblem {
    Empty,
    /// The name breaks a ref-format rule; the reason says which.
    Invalid(&'static str),
    /// A remote with this name exists, possibly with different letter case.
    Exists(String),
    /// A rename that changes only letter case: git's rename stops halfway where
    /// refs/remotes/<old> and refs/remotes/<new> are the same folder (Windows, macOS).
    CaseOnly,
}

impl RemoteNameProblem {
    /// The message shown under the name field.
    pub fn message(&self) -> String {
        match self {
            RemoteNameProblem::Empty => "Enter a remote name.".to_owned(),
            RemoteNameProblem::Invalid(reason) => (*reason).to_owned(),
            RemoteNameProblem::Exists(other) => format!("A remote named {other} already exists."),
            RemoteNameProblem::CaseOnly => {
                "Changing only the letter case isn't supported. Rename to another name first, then to this one.".to_owned()
            }
        }
    }
}

/// Why `name` isn't a valid remote name; only meaningful once `is_valid_remote_name` said no.
fn invalid_reason(name: &str) -> &'static str {
    let parts = || name.split('/');
    if name.chars().any(char::is_whitespace) {
        "Remote names can't contain spaces."
    } else if name.chars().any(char::is_control) {
        "Remote names can't contain control characters."
    } else if name.starts_with('-') {
        "Remote names can't start with a dash."
    } else if name.contains("..") {
        "Remote names can't contain two dots in a row."
    } else if name.contains(['~', '^', ':', '?', '*', '[', '\\']) {
        "Remote names can't contain ~ ^ : ? * [ or a backslash."
    } else if name.contains("@{") {
        "Remote names can't contain @{."
    } else if parts().any(|p| p.ends_with(".lock")) {
        "No part of a remote name can end in .lock."
    } else if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        "Remote names can't start or end with a slash, or have two in a row."
    } else if parts().any(|p| p.starts_with('.')) || name.ends_with('.') {
        "Parts of a remote name can't start with a dot, and the name can't end with one."
    } else {
        "Not a valid remote name."
    }
}

/// Checks `name` (already trimmed) against git's rules and the `existing` remote names, leaving
/// out `renaming` (the remote being edited). Names are compared case-insensitively because the
/// refs/remotes/<name> folders collide on Windows and macOS; for the same reason a rename that
/// changes only letter case is refused. Git allows `a` next to `a/b`, so there is no
/// folder-clash rule.
pub fn check_remote_name(name: &str, existing: &[&str], renaming: Option<&str>) -> Result<(), RemoteNameProblem> {
    if name.is_empty() {
        return Err(RemoteNameProblem::Empty);
    }
    if !is_valid_remote_name(name) {
        return Err(RemoteNameProblem::Invalid(invalid_reason(name)));
    }
    let lower = name.to_lowercase();
    if renaming.is_some_and(|r| r != name && r.to_lowercase() == lower) {
        return Err(RemoteNameProblem::CaseOnly);
    }
    match existing.iter().find(|o| Some(**o) != renaming && o.to_lowercase() == lower) {
        Some(other) => Err(RemoteNameProblem::Exists((*other).to_owned())),
        None => Ok(()),
    }
}

/// Why a remote URL can't be used.
#[derive(Debug, PartialEq)]
pub enum UrlProblem {
    Empty,
    /// Starts with '-', which git refuses because it could read as an option.
    Dash,
    Control,
}

impl UrlProblem {
    /// The message shown under the field. `push` is the optional push URL field, where an empty
    /// value is fine and never reported.
    pub fn message(&self, push: bool) -> String {
        match self {
            UrlProblem::Empty if push => String::new(),
            UrlProblem::Empty => "Enter a fetch URL.".to_owned(),
            UrlProblem::Dash => "URLs can't start with a dash.".to_owned(),
            UrlProblem::Control => "URLs can't contain control characters.".to_owned(),
        }
    }
}

/// Checks a URL (already trimmed). Spaces are allowed: Windows folder paths can have them.
pub fn check_url(url: &str) -> Result<(), UrlProblem> {
    if url.is_empty() {
        Err(UrlProblem::Empty)
    } else if url.starts_with('-') {
        Err(UrlProblem::Dash)
    } else if url.chars().any(char::is_control) {
        Err(UrlProblem::Control)
    } else {
        Ok(())
    }
}

/// `s` as an anchored POSIX extended regex that matches only itself (git remote set-url's
/// <oldurl>): escapes \ . ^ $ * + ? ( ) [ ] { } | and wraps the result in ^…$.
pub fn exact_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('^');
    for c in s.chars() {
        if "\\.^$*+?()[]{}|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('$');
    out
}

/// `git remote add`, then a separate push URL when given, then a fetch when asked.
pub fn add_cmds(name: &str, url: &str, push_url: Option<&str>, fetch: bool) -> Vec<Vec<String>> {
    let mut cmds = vec![args(&["remote", "add", name, url])];
    if let Some(p) = push_url {
        cmds.push(args(&["remote", "set-url", "--push", name, p]));
    }
    if fetch {
        cmds.push(fetch_cmd(name));
    }
    cmds
}

/// What the Edit remote dialog submits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemoteEdit<'a> {
    pub name: &'a str,
    pub url: &'a str,
    /// None pushes to the fetch URL.
    pub push_url: Option<&'a str>,
}

/// The commands that turn `old` into `new`, only for what changed: rename first, then the
/// fetch URL, then the push URL, all under the new name. An old URL is named only when the
/// remote has several values: without one git replaces the single value, and git matches the
/// pattern against insteadOf-rewritten URLs, so naming a rewritten one would fail.
pub fn edit_cmds(old: &RemoteInfo, new: &RemoteEdit) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    let n = new.name;
    if old.name != n {
        cmds.push(args(&["remote", "rename", &old.name, n]));
    }
    if old.fetch_url() != new.url {
        let mut c = args(&["remote", "set-url", n, new.url]);
        if old.urls.len() > 1 {
            c.push(exact_regex(old.fetch_url()));
        }
        cmds.push(c);
    }
    match (old.push_url(), new.push_url) {
        (None, Some(p)) => cmds.push(args(&["remote", "set-url", "--push", n, p])),
        (Some(o), Some(p)) if o != p => {
            let mut c = args(&["remote", "set-url", "--push", n, p]);
            if old.push_urls.len() > 1 {
                c.push(exact_regex(o));
            }
            cmds.push(c);
        }
        (Some(o), None) => cmds.push(args(&["remote", "set-url", "--delete", "--push", n, &exact_regex(o)])),
        _ => {}
    }
    cmds
}

/// `git fetch --prune <name>`, like the toolbar's Fetch all.
pub fn fetch_cmd(name: &str) -> Vec<String> {
    args(&["fetch", "--prune", name])
}

/// Fetches `refname` from `remote` (a pull request's head ref) for its objects only: no tags,
/// no submodules, and with an empty refmap, so no remote-tracking ref changes. Without
/// `write_fetch_head` (git 2.29 and later) FETCH_HEAD isn't written either.
pub fn fetch_ref_cmd(remote: &str, refname: &str, write_fetch_head: bool) -> Vec<String> {
    let mut cmd = args(&["fetch", "--no-tags", "--recurse-submodules=no", "--refmap="]);
    if !write_fetch_head {
        cmd.push("--no-write-fetch-head".into());
    }
    cmd.extend([remote.to_owned(), refname.to_owned()]);
    cmd
}

/// `git remote remove <name>`.
pub fn remove_cmd(name: &str) -> Vec<String> {
    args(&["remote", "remove", name])
}

/// `url` with the password of a scheme://user:password@host URL replaced by ***.
pub fn display_url(url: &str) -> String {
    let Some(start) = url.find("://").map(|i| i + 3) else { return url.to_owned() };
    let rest = &url[start..];
    let authority = &rest[..rest.find('/').unwrap_or(rest.len())];
    let Some(at) = authority.rfind('@') else { return url.to_owned() };
    let Some(colon) = authority[..at].find(':') else { return url.to_owned() };
    format!("{}:***{}", &url[..start + colon], &url[start + at..])
}

#[cfg(test)]
mod tests;
