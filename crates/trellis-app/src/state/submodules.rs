//! The SUBMODULES section's text and rules, shared by the view and `update`: the status word,
//! the tooltip, why open, update and copy are blocked, and the update command.

use git2::Oid;
use trellis_core::gitcli::args;
use trellis_core::repo::{SubmoduleInfo, SubmoduleState};

/// The first 7 hex characters of `oid`.
fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The word at the right of a row; None when in sync.
pub fn state_word(s: SubmoduleState) -> Option<&'static str> {
    match s {
        SubmoduleState::InSync => None,
        SubmoduleState::OutOfSync => Some("out of sync"),
        SubmoduleState::Dirty => Some("changes"),
        SubmoduleState::DirtyOutOfSync => Some("changes · out of sync"),
        SubmoduleState::Uninitialized => Some("not initialized"),
    }
}

/// A row's hover text: what its state means, then its name, URL and commits.
pub fn tooltip(sm: &SubmoduleInfo) -> String {
    let recorded = short(sm.recorded);
    let checked = sm.checked_out.map(short);
    let mut out = match sm.state {
        SubmoduleState::InSync => "Checked out at the commit this repository records.".to_owned(),
        SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => {
            let now = checked.clone().unwrap_or_default();
            let mut s = format!(
                "Checked out at {now} while this repository records {recorded}. Updating checks out {recorded}; committing the \
                 submodule path here records {now} instead."
            );
            if sm.is_dirty() {
                s.push_str("\nIt also has uncommitted changes or untracked files inside; open it to commit or discard them.");
            }
            s
        }
        SubmoduleState::Dirty => "Uncommitted changes or untracked files inside; open it to commit or discard them.".to_owned(),
        SubmoduleState::Uninitialized => format!("Not checked out yet; Initialize clones it and checks out {recorded}."),
    };
    out.push('\n');
    if sm.name != sm.path {
        out.push_str(&format!("\nName: {}", sm.name));
    }
    match &sm.url {
        Some(u) => out.push_str(&format!("\nURL: {u}")),
        None => out.push_str("\nURL: none in .gitmodules"),
    }
    out.push_str(&format!("\nRecorded commit: {recorded}"));
    out.push_str(&format!("\nChecked-out commit: {}", checked.as_deref().unwrap_or("none")));
    out
}

/// Why the submodule can't be opened in a tab: its folder holds no repository yet, so opening
/// it would find the parent instead.
pub fn open_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    (!sm.is_initialized()).then_some("Not checked out yet. Initialize it first.")
}

/// Why Initialize / Update to recorded commit is unavailable.
pub fn update_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    if sm.url.is_none() {
        Some(".gitmodules has no entry for it, so git can't initialize or update it.")
    } else if sm.is_initialized() && !sm.is_out_of_sync() {
        Some("Already checked out at the recorded commit.")
    } else {
        None
    }
}

pub fn copy_url_blocked(sm: &SubmoduleInfo) -> Option<&'static str> {
    sm.url.is_none().then_some(".gitmodules has no URL for it.")
}

/// The menu item that runs `update_args`.
pub fn update_label(sm: &SubmoduleInfo) -> &'static str {
    if sm.is_initialized() { "Update to recorded commit" } else { "Initialize" }
}

/// The job's label, in the toast and the activity log.
pub fn job_label(sm: &SubmoduleInfo) -> String {
    if sm.is_initialized() { format!("Update submodule {}", sm.path) } else { format!("Initialize submodule {}", sm.path) }
}

/// The confirmation before updating a submodule that is checked out at another commit.
pub fn confirm_update_body(sm: &SubmoduleInfo) -> String {
    let now = sm.checked_out.map(short).unwrap_or_default();
    format!(
        "{} is at {now} now. Updating checks out the recorded commit {} (detached HEAD). Branches inside the submodule are \
         left as they are.",
        sm.path,
        short(sm.recorded)
    )
}

/// `git submodule update --init -- <path>`: clones it if needed and checks out the recorded commit.
pub fn update_args(path: &str) -> Vec<String> {
    args(&["submodule", "update", "--init", "--", path])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(state: SubmoduleState, name: &str, url: Option<&str>) -> SubmoduleInfo {
        let checked_out = match state {
            SubmoduleState::Uninitialized => None,
            SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => Some(Oid::from_str(&"b".repeat(40)).unwrap()),
            _ => Some(Oid::from_str(&"a".repeat(40)).unwrap()),
        };
        SubmoduleInfo {
            name: name.into(),
            path: "libs/core".into(),
            url: url.map(str::to_owned),
            recorded: Oid::from_str(&"a".repeat(40)).unwrap(),
            checked_out,
            state,
        }
    }

    #[test]
    fn state_words() {
        assert_eq!(state_word(SubmoduleState::InSync), None);
        assert_eq!(state_word(SubmoduleState::OutOfSync), Some("out of sync"));
        assert_eq!(state_word(SubmoduleState::Dirty), Some("changes"));
        assert_eq!(state_word(SubmoduleState::DirtyOutOfSync), Some("changes · out of sync"));
        assert_eq!(state_word(SubmoduleState::Uninitialized), Some("not initialized"));
    }

    #[test]
    fn tooltip_lines() {
        let t = tooltip(&info(SubmoduleState::OutOfSync, "libs/core", Some("../core.git")));
        assert!(t.starts_with("Checked out at bbbbbbb while this repository records aaaaaaa."), "{t}");
        assert!(t.contains("\nURL: ../core.git") && t.contains("\nRecorded commit: aaaaaaa"), "{t}");
        assert!(t.contains("\nChecked-out commit: bbbbbbb") && !t.contains("Name:"), "{t}");

        let t = tooltip(&info(SubmoduleState::Uninitialized, "core", None));
        assert!(t.contains("\nName: core") && t.contains("\nURL: none in .gitmodules"), "{t}");
        assert!(t.contains("\nChecked-out commit: none") && t.contains("checks out aaaaaaa"), "{t}");
    }

    #[test]
    fn labels_and_rules() {
        let un = info(SubmoduleState::Uninitialized, "libs/core", Some("u"));
        let ok = info(SubmoduleState::InSync, "libs/core", Some("u"));
        let dirty = info(SubmoduleState::Dirty, "libs/core", Some("u"));
        let out = info(SubmoduleState::DirtyOutOfSync, "libs/core", Some("u"));
        assert_eq!((update_label(&un), job_label(&un)), ("Initialize", "Initialize submodule libs/core".to_owned()));
        assert_eq!((update_label(&ok), job_label(&ok)), ("Update to recorded commit", "Update submodule libs/core".to_owned()));
        assert!(open_blocked(&un).is_some() && open_blocked(&dirty).is_none());
        assert_eq!(update_blocked(&un), None);
        assert_eq!(update_blocked(&out), None);
        assert_eq!(update_blocked(&ok), Some("Already checked out at the recorded commit."));
        assert_eq!(update_blocked(&dirty), Some("Already checked out at the recorded commit."));
        assert!(update_blocked(&info(SubmoduleState::Uninitialized, "x", None)).unwrap().contains(".gitmodules"));
        assert!(copy_url_blocked(&ok).is_none() && copy_url_blocked(&info(SubmoduleState::InSync, "x", None)).is_some());
        let body = confirm_update_body(&out);
        assert!(body.contains("bbbbbbb") && body.contains("aaaaaaa") && body.contains("Branches inside"), "{body}");
        assert_eq!(update_args("libs/core"), ["submodule", "update", "--init", "--", "libs/core"]);
    }
}
