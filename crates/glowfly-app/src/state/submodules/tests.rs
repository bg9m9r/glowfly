//! The SUBMODULES section's words, tooltips, rules, labels and confirmations.

use super::*;

fn info(state: SubmoduleState, name: &str, url: Option<&str>) -> SubmoduleInfo {
    let checked_out = match state {
        SubmoduleState::Uninitialized | SubmoduleState::Missing => None,
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
        left_behind: Vec::new(),
        outside_sparse: false,
        clone_url: None,
        branch: None,
        added: false,
    }
}

fn word(state: SubmoduleState, added: bool) -> Option<String> {
    let mut sm = info(state, "libs/core", Some("u"));
    sm.added = added;
    row_word(Lang::En, &sm)
}

#[test]
fn state_words() {
    assert_eq!(word(SubmoduleState::InSync, false).as_deref(), Some("up to date"));
    assert_eq!(word(SubmoduleState::OutOfSync, false).as_deref(), Some("out of sync"));
    assert_eq!(word(SubmoduleState::Dirty, false).as_deref(), Some("changes"));
    assert_eq!(word(SubmoduleState::DirtyOutOfSync, false).as_deref(), Some("changes · out of sync"));
    assert_eq!(word(SubmoduleState::Uninitialized, false).as_deref(), Some("not initialized"));
    assert_eq!(word(SubmoduleState::Missing, false).as_deref(), Some("folder missing"));
    assert_eq!(word(SubmoduleState::InSync, true).as_deref(), Some("new"));
    assert_eq!(word(SubmoduleState::Uninitialized, true).as_deref(), Some("new · not initialized"));
}

#[test]
fn dot_tips_name_the_state() {
    let tip = |state, added| {
        let mut sm = info(state, "libs/core", Some("u"));
        sm.added = added;
        dot_tip(Lang::En, &sm)
    };
    assert_eq!(tip(SubmoduleState::InSync, false), "Up to date: checked out at the commit this repository records");
    assert_eq!(tip(SubmoduleState::OutOfSync, false), "Out of sync: checked out at another commit than this repository records");
    assert_eq!(tip(SubmoduleState::Dirty, false), "Changes inside");
    assert_eq!(tip(SubmoduleState::DirtyOutOfSync, false), "Changes inside, and out of sync");
    assert_eq!(tip(SubmoduleState::Uninitialized, false), "Not initialized");
    assert_eq!(tip(SubmoduleState::Missing, false), "Folder missing");
    assert_eq!(tip(SubmoduleState::Uninitialized, true), "Not initialized\nAdded, not committed yet");
}

#[test]
fn tooltip_states_urls_and_branch() {
    let mut sm = info(SubmoduleState::InSync, "libs/core", Some("../core.git"));
    sm.clone_url = Some("https://h/o/core.git".into());
    sm.branch = Some("dev".into());
    let t = tooltip(&sm);
    assert!(
        t.contains(
            "
URL: ../core.git
Resolves to: https://h/o/core.git"
        ) && t.contains(
            "
Branch: dev"
        ),
        "{t}"
    );
    sm.url = Some("https://h/o/core.git".into());
    assert!(!tooltip(&sm).contains("Resolves to"), "an absolute URL resolves to itself");
    sm.url = Some("./core.git".into());
    sm.clone_url = sm.url.clone();
    assert!(!tooltip(&sm).contains("Resolves to"), "nothing new to say");

    sm.added = true;
    let t = tooltip(&sm);
    assert!(
        t.starts_with(
            "Added and checked out, but not committed yet; commit .gitmodules and libs/core to record it.
"
        ),
        "{t}"
    );
    let mut un = info(SubmoduleState::Uninitialized, "libs/core", Some("u"));
    un.added = true;
    let t = tooltip(&un);
    assert!(
        t.starts_with(
            "Added but not committed yet, and not checked out; Initialize clones it.
"
        ),
        "{t}"
    );
    assert!(t.contains("An empty folder: never initialized, or deinitialized."), "{t}");

    let t = tooltip(&info(SubmoduleState::Missing, "libs/core", Some("u")));
    assert!(
        t.starts_with("The folder is gone, but this repository still records the submodule. Restore checks it out again at aaaaaaa"),
        "{t}"
    );
    assert!(t.contains("Remove submodule… drops it."), "{t}");
}

#[test]
fn missing_folder_rules() {
    let gone = info(SubmoduleState::Missing, "libs/core", Some("u"));
    assert_eq!((update_label(&gone), job_label(&gone)), ("Restore folder", "Restore submodule libs/core".to_owned()));
    assert_eq!(open_blocked(&gone), Some("Its folder is missing. Restore it first."));
    assert_eq!(update_blocked(&gone), None);
    assert!(!gone.is_initialized());
}

#[test]
fn copy_and_edit_rules() {
    let mut sm = info(SubmoduleState::InSync, "libs/core", Some("../core.git"));
    assert_eq!(copy_url_text(&sm), "../core.git");
    sm.clone_url = Some("https://h/o/core.git".into());
    assert_eq!(copy_url_text(&sm), "https://h/o/core.git", "the resolved URL");
    assert_eq!(edit_blocked(&sm, None), None);
    assert_eq!(edit_blocked(&sm, Some("too old")).as_deref(), Some("too old"));
    let bare = info(SubmoduleState::InSync, "x", None);
    assert_eq!(edit_blocked(&bare, None).as_deref(), Some(".gitmodules has no entry for it."));
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
    assert_eq!(confirm_update_button(&out), "Update");
    assert_eq!(update_args("libs/core"), ["submodule", "update", "--init", "--checkout", "--", ":(literal)libs/core"]);
}

fn with_left_behind(n: usize) -> SubmoduleInfo {
    let mut sm = info(SubmoduleState::OutOfSync, "libs/core", Some("u"));
    sm.left_behind = (0..n).map(|i| (Oid::from_str(&format!("{:x}", i + 1).repeat(40)[..40]).unwrap(), format!("work {i}"))).collect();
    sm
}

#[test]
fn left_behind_commits_are_named() {
    let one = with_left_behind(1);
    let body = confirm_update_body(&one);
    assert!(body.contains("No branch or tag inside it contains that commit") && !body.contains("Branches inside"), "{body}");
    assert!(body.contains("\n    1111111 work 0") && body.contains("To keep it, cancel"), "{body}");
    assert!(body.contains("create a branch at bbbbbbb"), "{body}");
    assert_eq!(confirm_update_button(&one), "Update and leave 1 commit behind");
    let t = tooltip(&one);
    assert!(t.contains("\n1 commit here is on no branch or tag; updating would leave it behind."), "{t}");

    let seven = with_left_behind(7);
    let body = confirm_update_body(&seven);
    assert!(body.contains("these commits") && body.contains("\n    5555555 work 4") && !body.contains("work 5"), "{body}");
    assert!(body.contains("\n    and 2 more\n"), "{body}");
    assert_eq!(confirm_update_button(&seven), "Update and leave 7 commits behind");

    let many = with_left_behind(LEFT_BEHIND_MAX);
    let body = confirm_update_body(&many);
    assert!(body.contains("\n    and at least 45 more\n"), "{body}");
    assert_eq!(confirm_update_button(&many), "Update and leave 50 or more commits behind");
    let t = tooltip(&many);
    assert!(t.contains("\n50 or more commits here are on no branch or tag; updating would leave them behind."), "{t}");
}
