//! The Delete tag form (its remotes, button and message) and its job's headline.

use super::*;
use crate::state::tags::RemoteTagOp;
use git2::Oid;

fn oid() -> Oid {
    Oid::from_str("1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b").unwrap()
}

/// A loaded snapshot (`name` keeps each test's folder apart) given a tag v1, remotes origin and
/// backup, and a tag on a tree.
fn snap(name: &str) -> Snapshot {
    let mut s = crate::state::op_fixture::snapshot(name);
    s.tags = vec![("v1".into(), oid())];
    s.remotes = vec![("origin".into(), vec![]), ("backup".into(), vec![])];
    s.other_tags = vec!["tree-tag".into()];
    s
}

/// The tag, commit and remotes a `TagMsg::Delete` carries.
fn sent(msg: Msg) -> (String, Oid, Vec<String>) {
    match msg {
        Msg::Tag(TagMsg::Delete { tag, at, remotes }) => (tag, at, remotes),
        _ => panic!("expected TagMsg::Delete"),
    }
}

fn job(remotes: &[&str]) -> RemoteTagJob {
    RemoteTagJob {
        label: "Delete local and remote tag".into(),
        op: RemoteTagOp::Delete,
        tag: "v1".into(),
        remotes: remotes.iter().map(|r| (*r).to_owned()).collect(),
        local: true,
    }
}

#[test]
fn the_form_lists_every_remote_unticked() {
    let snap = snap("tag-delete-form-new");
    let form = DeleteTagForm::new(&snap, "v1").expect("a form");
    assert_eq!(form.tag, "v1");
    assert_eq!(form.at, oid());
    assert_eq!(form.remotes, [("origin".to_owned(), false), ("backup".to_owned(), false)]);
    assert_eq!(DeleteTagForm::new(&snap, "v9"), None, "an unknown tag");
    assert_eq!(DeleteTagForm::new(&snap, "tree-tag"), None, "a tag on a tree is never listed");
}

#[test]
fn the_button_counts_the_ticked_remotes() {
    let t = Lang::En;
    let mut form = DeleteTagForm::new(&snap("tag-delete-form-button"), "v1").unwrap();
    assert_eq!(form.button(t), "Delete");
    form.remotes[1].1 = true;
    assert_eq!(form.button(t), "Delete here and on 1 remote");
    form.remotes[0].1 = true;
    assert_eq!(form.button(t), "Delete here and on 2 remotes");
}

#[test]
fn the_message_carries_only_the_ticked_remotes_in_list_order() {
    let mut form = DeleteTagForm::new(&snap("tag-delete-form-msg"), "v1").unwrap();
    assert_eq!(sent(form.delete_msg()), ("v1".to_owned(), oid(), vec![]));
    form.remotes[1].1 = true;
    form.remotes[0].1 = true;
    assert_eq!(sent(form.delete_msg()), ("v1".to_owned(), oid(), vec!["origin".to_owned(), "backup".to_owned()]));
}

const LOCAL: &str = "$ git tag -d -- v1\nDeleted tag 'v1' (was 1a2b3c4)\n";
const ORIGIN: &str = "$ git push --delete -- origin refs/tags/v1\nTo ../origin\n - [deleted]         v1\n";
const BACKUP_LACKS: &str =
    "$ git push --delete -- backup refs/tags/v1\nremote: warning: Deleting a non-existent ref.\nTo ../backup\n - [deleted]         v1\n";

#[test]
fn headlines_when_it_succeeded() {
    let t = Lang::En;
    let both = format!("{LOCAL}{ORIGIN}$ git push --delete -- backup refs/tags/v1\n - [deleted] v1\n");
    assert_eq!(headline(t, &job(&["origin", "backup"]), true, &both).as_deref(), Some("Deleted tag v1 here and from origin and backup."));
    let lacking = format!("{LOCAL}{ORIGIN}{BACKUP_LACKS}");
    assert_eq!(
        headline(t, &job(&["origin", "backup"]), true, &lacking).as_deref(),
        Some("Deleted tag v1 here and from origin. backup had no tag v1.")
    );
    let none = format!("{LOCAL}{BACKUP_LACKS}");
    assert_eq!(headline(t, &job(&["origin"]), true, &none).as_deref(), Some("Deleted tag v1 here. origin had no tag v1."));
}

#[test]
fn headlines_when_it_failed() {
    let t = Lang::En;
    let local = "$ git tag -d -- v1\nerror: tag 'v1' not found.\n";
    assert_eq!(
        headline(t, &job(&["origin", "backup"]), false, local).as_deref(),
        Some("Could not delete tag v1: tag 'v1' not found. No remote was changed.")
    );
    let second =
        format!("{LOCAL}{ORIGIN}$ git push --delete -- broken refs/tags/v1\nfatal: 'missing' does not appear to be a git repository\n");
    assert_eq!(
        headline(t, &job(&["origin", "broken", "backup"]), false, &second).as_deref(),
        Some(
            "Deleted tag v1 here, but could not delete it from broken: 'missing' does not appear to be a git repository. \
             Deleted from origin. Not tried: backup."
        )
    );
    let first = format!("{LOCAL}$ git push --delete -- origin refs/tags/v1\nerror: unable to delete 'v1': remote ref does not exist\n");
    assert_eq!(
        headline(t, &job(&["origin"]), false, &first).as_deref(),
        Some("Deleted tag v1 here, but could not delete it from origin: the remote has no such tag.")
    );
    // Refused before any command ran: the generic failure headline.
    assert_eq!(headline(t, &job(&["origin"]), false, "The repository changed since it was shown"), None);
}
