//! Headless tests for the Delete tag dialog: opening it with every remote unticked, the one job a
//! delete starts (the local delete, then one per ticked remote), the refusals, the headlines, and
//! undo, which brings back only the local tag (also after a remote failed, and for an annotated
//! tag, the same tag object).

use super::tag_remote_tests::with_two;
use super::tests::{drive, finish, git, state};
use super::update;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::undo::UndoKind;
use glowfly_model::effect::{Effect, JobEffect};
use glowfly_model::msg::{JobMsg, Msg, RepoMsg};
use glowfly_model::state::tag_delete::DeleteTagForm;
use glowfly_model::state::{AppState, Modal};
use glowfly_model::tag::TagMsg;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn names(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

/// The commit tag `name` points at in the snapshot.
fn tag_at(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().tags.iter().find(|(n, _)| n == name).map(|(_, o)| *o).expect("the tag is listed")
}

/// The ref's own target: the tag object for an annotated tag.
fn tag_ref(dir: &Path, name: &str) -> Option<Oid> {
    git2::Repository::open(dir).unwrap().find_reference(&format!("refs/tags/{name}")).ok().and_then(|r| r.target())
}

fn has_tag(bare: &Path) -> bool {
    git2::Repository::open_bare(bare).unwrap().find_reference("refs/tags/v1").is_ok()
}

fn delete(st: &AppState, tag: &str, remotes: &[&str]) -> Msg {
    Msg::Tag(TagMsg::Delete { tag: tag.into(), at: tag_at(st, tag), remotes: names(remotes) })
}

fn open(tag: &str) -> Msg {
    Msg::Tag(TagMsg::OpenDelete { tag: tag.into() })
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(Lang::En, true).0
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn toast(st: &AppState) -> String {
    st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
}

fn label(st: &AppState) -> String {
    st.job.as_ref().map(|j| j.label.clone()).unwrap_or_default()
}

/// What a job checks before it runs: the revision and the commit it must still be.
type Expect = Option<(String, Oid)>;

/// The single job `fx` starts: its commands, undo record and expected revision.
fn job(fx: &[Effect]) -> (Vec<Vec<String>>, Option<UndoKind>, Expect) {
    match fx {
        [Effect::Job(JobEffect::RunGit { cmds, record, expect, .. })] => (cmds.clone(), record.clone(), expect.clone()),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

#[test]
fn local_only_delete_is_undoable() {
    let (mut st, work, origin, backup) = with_two("tag-delete-local");
    let at = tag_at(&st, "v1");
    update(&mut st, open("v1"), Instant::now());
    let Some(Modal::DeleteTag(form)) = &st.modal else { panic!("expected the Delete tag dialog") };
    // In the snapshot's order, all unticked.
    assert_eq!(form, &DeleteTagForm { tag: "v1".into(), at, remotes: vec![("backup".into(), false), ("origin".into(), false)] });

    let msg = delete(&st, "v1", &[]);
    let fx = update(&mut st, msg, Instant::now());
    let (cmds, record, expect) = job(&fx);
    assert_eq!(cmds, [names(&["tag", "-d", "--", "v1"])]);
    assert_eq!(record, Some(UndoKind::DeleteTag { name: "v1".into(), remotes: vec![] }));
    assert_eq!(expect, Some(("refs/tags/v1".to_owned(), at)));
    assert_eq!(label(&st), "Delete tag");
    assert!(st.modal.is_none(), "the dialog closes once the job starts");
    assert!(st.tag_job.is_none(), "a local delete has the generic headline");

    finish(&mut st, fx);
    assert_eq!(tag_ref(&work, "v1"), None);
    assert_eq!(toast(&st), "Delete tag succeeded");
    assert!(undo_label(&st).starts_with("Undo delete of tag v1"), "{}", undo_label(&st));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tag_ref(&work, "v1"), Some(at), "undo brings the tag back where it was");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(tag_ref(&work, "v1"), None, "redo deletes it again");
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn local_and_remote_delete() {
    let (mut st, work, origin, backup) = with_two("tag-delete-remotes");
    git(&work, &["push", "-q", "origin", "refs/tags/v1:refs/tags/v1"]);
    let at = tag_at(&st, "v1");
    update(&mut st, open("v1"), Instant::now());
    let msg = delete(&st, "v1", &["origin", "backup"]);
    let fx = update(&mut st, msg, Instant::now());
    let (cmds, record, _) = job(&fx);
    assert_eq!(
        cmds,
        [
            names(&["tag", "-d", "--", "v1"]),
            names(&["push", "--delete", "--", "origin", "refs/tags/v1"]),
            names(&["push", "--delete", "--", "backup", "refs/tags/v1"]),
        ]
    );
    assert_eq!(record, Some(UndoKind::DeleteTag { name: "v1".into(), remotes: names(&["origin", "backup"]) }));
    assert_eq!(label(&st), "Delete local and remote tag");
    assert!(st.tag_job.as_ref().is_some_and(|j| j.local));
    assert!(st.modal.is_none());

    finish(&mut st, fx);
    assert_eq!(tag_ref(&work, "v1"), None);
    assert!(!has_tag(&origin));
    // Stock git warns about a ref the remote lacks; a stricter server refuses it.
    let headline = toast(&st);
    assert!(
        headline == "Deleted tag v1 here and from origin. backup had no tag v1."
            || headline == "Deleted tag v1 here, but could not delete it from backup: the remote has no such tag. Deleted from origin.",
        "{headline}"
    );
    assert_eq!(undo_label(&st), "Undo delete of tag v1 (here only, not on origin and backup)");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tag_ref(&work, "v1"), Some(at), "the local tag comes back");
    assert!(!has_tag(&origin), "the remote copy stays deleted");
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn remote_failure_keeps_the_local_undo() {
    let (mut st, work, origin, backup) = with_two("tag-delete-broken");
    let missing: PathBuf = glowfly_core::paths::temp_dir().join(format!("glowfly-missing-remote-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    git(&work, &["remote", "add", "broken", &missing.display().to_string()]);
    refresh(&mut st);
    let at = tag_at(&st, "v1");
    let msg = delete(&st, "v1", &["broken", "backup"]);
    drive(&mut st, msg);
    assert!(st.job.is_none(), "the job ended");
    assert_eq!(tag_ref(&work, "v1"), None, "the local delete ran first");
    let headline = toast(&st);
    assert!(
        headline.starts_with("Deleted tag v1 here, but could not delete it from broken: ") && headline.ends_with(" Not tried: backup."),
        "{headline}"
    );
    assert_eq!(undo_label(&st), "Undo delete of tag v1 (here only, not on broken and backup)");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tag_ref(&work, "v1"), Some(at));
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn delete_refusals() {
    let mut bare = state();
    assert!(update(&mut bare, open("v1"), Instant::now()).is_empty());
    let none = Msg::Tag(TagMsg::Delete { tag: "v1".into(), at: Oid::zero(), remotes: vec![] });
    assert!(update(&mut bare, none, Instant::now()).is_empty());
    assert!(bare.log.is_empty() && bare.modal.is_none(), "no snapshot: nothing");

    let (mut st, work, origin, backup) = with_two("tag-delete-refusals");
    update(&mut st, open("v9"), Instant::now());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Tag v9 no longer exists.");

    update(&mut st, open("v1"), Instant::now());
    let at = tag_at(&st, "v1");
    let other = Oid::from_bytes(&[7; 20]).unwrap();
    let short = &at.to_string()[..7];
    let moved = format!("Tag v1 has moved to {short}. Open Delete again.");
    let msg = |tag: &str, at: Oid, remotes: &[&str]| Msg::Tag(TagMsg::Delete { tag: tag.into(), at, remotes: names(remotes) });
    let cases = [
        (msg("v9", at, &[]), "Tag v9 no longer exists.".to_owned()),
        (msg("v1", other, &[]), moved),
        (msg("v1", at, &["origin", "nope"]), "There is no remote named nope.".to_owned()),
        (msg("-x", at, &[]), "Not a valid tag name.".to_owned()),
    ];
    for (m, want) in cases {
        let fx = update(&mut st, m, Instant::now());
        assert!(fx.is_empty() && st.job.is_none() && st.tag_job.is_none(), "{want}");
        assert_eq!(last_log(&st), want);
        assert!(matches!(st.modal, Some(Modal::DeleteTag(_))), "the dialog stays open: {want}");
    }

    st.job = Some(super::tag_remote_tests::running());
    assert!(update(&mut st, msg("v1", at, &[]), Instant::now()).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(matches!(st.modal, Some(Modal::DeleteTag(_))));

    st.job = None;
    let fx = update(&mut st, msg("v1", at, &["origin", "origin"]), Instant::now());
    assert_eq!(job(&fx).0.len(), 2, "duplicate remotes are dropped");
    assert_eq!(st.tag_job.as_ref().map(|j| j.remotes.clone()), Some(names(&["origin"])));
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn annotated_tag_delete_round_trip() {
    let (mut st, work, origin, backup) = with_two("tag-delete-annotated");
    git(&work, &["tag", "-a", "v2", "-m", "Notes"]);
    refresh(&mut st);
    let object = tag_ref(&work, "v2").expect("the tag object");
    assert_ne!(object, tag_at(&st, "v2"), "an annotated tag's ref holds the tag object");
    let msg = delete(&st, "v2", &[]);
    drive(&mut st, msg);
    assert_eq!(tag_ref(&work, "v2"), None);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tag_ref(&work, "v2"), Some(object), "the same tag object comes back");
    let repo = git2::Repository::open(&work).unwrap();
    assert_eq!(repo.find_tag(object).unwrap().message().map(str::trim), Some("Notes"));
    cleanup(&[&work, &origin, &backup]);
}
