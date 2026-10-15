//! Command lines and checks for creating a tag, annotating a lightweight one, pushing it and
//! deleting it from a remote.
//! Everything here is pure; the app runs the commands through `gitcli` so hooks, credentials and
//! the user's git config apply.

use crate::gitcli::args;

/// Whether git accepts `name` as a tag name: `refs/tags/<name>` must pass the ref-format rules,
/// and git itself refuses a tag starting with '-'.
pub fn is_valid_tag_name(name: &str) -> bool {
    // Control characters first: git2 panics on an interior NUL when it builds its C string.
    !name.is_empty()
        && !name.starts_with('-')
        && !name.chars().any(char::is_control)
        && git2::Reference::is_valid_name(&format!("refs/tags/{name}"))
}

/// Why a new tag name can't be used.
#[derive(Debug, PartialEq)]
pub enum TagNameProblem {
    Empty,
    /// The name breaks a ref-format rule; the reason says which.
    Invalid(&'static str),
    /// A tag with this name exists, possibly with different letter case.
    Exists(String),
    /// The name is a folder of an existing tag, or the other way round.
    Clashes(String),
}

impl TagNameProblem {
    /// The message shown for `name`.
    pub fn message(&self, name: &str) -> String {
        match self {
            TagNameProblem::Empty => "Enter a tag name.".to_owned(),
            TagNameProblem::Invalid(reason) => (*reason).to_owned(),
            TagNameProblem::Exists(other) => format!("Tag {other} already exists."),
            TagNameProblem::Clashes(other) => format!("{name} clashes with tag {other}."),
        }
    }
}

/// Why `name` isn't a valid tag name; only meaningful once `is_valid_tag_name` said no.
fn invalid_reason(name: &str) -> &'static str {
    let parts = || name.split('/');
    if name.chars().any(char::is_whitespace) {
        "Tag names can't contain spaces."
    } else if name.chars().any(char::is_control) {
        "Tag names can't contain control characters."
    } else if name.starts_with('-') {
        "Tag names can't start with a dash."
    } else if name.contains("..") {
        "Tag names can't contain two dots in a row."
    } else if name.contains(['~', '^', ':', '?', '*', '[', '\\']) {
        "Tag names can't contain ~ ^ : ? * [ or a backslash."
    } else if name.contains("@{") {
        "Tag names can't contain @{."
    } else if parts().any(|p| p.ends_with(".lock")) {
        "No part of a tag name can end in .lock."
    } else if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        "Tag names can't start or end with a slash, or have two in a row."
    } else if parts().any(|p| p.starts_with('.')) || name.ends_with('.') {
        "Parts of a tag name can't start with a dot, and the name can't end with one."
    } else {
        "Not a valid tag name."
    }
}

/// Checks `name` (already trimmed) against git's rules for a tag and the `existing` tag names.
/// Names are compared case-insensitively because loose ref files collide on Windows and macOS.
pub fn check_tag_name(name: &str, existing: &[&str]) -> Result<(), TagNameProblem> {
    if name.is_empty() {
        return Err(TagNameProblem::Empty);
    }
    if !is_valid_tag_name(name) {
        return Err(TagNameProblem::Invalid(invalid_reason(name)));
    }
    let lower = name.to_lowercase();
    for other in existing {
        let o = other.to_lowercase();
        if o == lower {
            return Err(TagNameProblem::Exists((*other).to_owned()));
        }
        if o.starts_with(&format!("{lower}/")) || lower.starts_with(&format!("{o}/")) {
            return Err(TagNameProblem::Clashes((*other).to_owned()));
        }
    }
    Ok(())
}

/// `git tag <name> <at>`, or with a message an annotated `git tag -a <name> <at> -m <message>`.
pub fn tag_cmd(name: &str, at: &str, message: &str) -> Vec<String> {
    if message.is_empty() { args(&["tag", name, at]) } else { args(&["tag", "-a", name, at, "-m", message]) }
}

/// Replaces tag `name` with an annotated tag at commit `at` (a full sha):
/// `git tag -a -f <name> <at> -m <message>`. Run through `gitcli`, so tag.gpgSign signs it and
/// hooks apply; if building or signing the tag object fails, git leaves the old ref untouched.
pub fn annotate_tag_cmd(name: &str, at: &str, message: &str) -> Vec<String> {
    args(&["tag", "-a", "-f", name, at, "-m", message])
}

/// Pushes tag `name` to `remote`. The full refspec keeps a branch with the same name from being
/// pushed instead.
pub fn push_tag_cmd(remote: &str, name: &str) -> Vec<String> {
    args(&["push", remote, &format!("refs/tags/{name}:refs/tags/{name}")])
}

/// Deletes tag `name` from `remote`. The full ref makes sure a branch named like the tag is never
/// deleted instead. It runs through `gitcli`, so credentials, hooks and config apply.
pub fn delete_remote_tag_cmd(remote: &str, name: &str) -> Vec<String> {
    args(&["push", remote, "--delete", &format!("refs/tags/{name}")])
}

#[cfg(test)]
mod tests;
