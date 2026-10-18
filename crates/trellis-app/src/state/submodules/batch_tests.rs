//! Planning batch updates over a mixed list, and their texts.

use super::*;
use git2::Oid;
use trellis_core::repo::SubmoduleState;

fn oid(c: char) -> Oid {
    Oid::from_str(&c.to_string().repeat(40)).unwrap()
}

fn sm(path: &str, state: SubmoduleState, url: bool) -> SubmoduleInfo {
    let checked_out = match state {
        SubmoduleState::Uninitialized | SubmoduleState::Missing => None,
        SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => Some(oid('b')),
        _ => Some(oid('a')),
    };
    SubmoduleInfo {
        name: path.into(),
        path: path.into(),
        url: url.then(|| "../x.git".to_owned()),
        clone_url: None,
        branch: None,
        added: false,
        recorded: oid('a'),
        checked_out,
        state,
        left_behind: Vec::new(),
    }
}

fn mixed() -> Vec<SubmoduleInfo> {
    let mut stranding = sm("c-strand", SubmoduleState::OutOfSync, true);
    stranding.left_behind = vec![(oid('c'), "work".into())];
    vec![
        sm("a-sync", SubmoduleState::InSync, true),
        sm("b-out", SubmoduleState::OutOfSync, true),
        stranding,
        sm("d-uninit", SubmoduleState::Uninitialized, true),
        sm("e-missing", SubmoduleState::Missing, true),
        sm("f-no-url", SubmoduleState::Uninitialized, false),
    ]
}

#[test]
fn plans_for_each_batch() {
    let list = mixed();
    let p = plan(Batch::InitAll, &list);
    assert_eq!(p.targets, ["d-uninit", "e-missing"]);
    assert!(!p.all && p.stranding.is_empty() && p.skipped.is_empty());

    let p = plan(Batch::UpdateAll, &list);
    assert_eq!(p.targets, ["b-out", "c-strand", "d-uninit", "e-missing"]);
    assert_eq!(p.stranding.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["c-strand"]);
    assert!(p.skipped.is_empty() && !p.all);

    let p = plan(Batch::Auto, &list);
    assert_eq!(p.targets, ["b-out", "d-uninit", "e-missing"]);
    assert_eq!(p.skipped.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["c-strand"]);
    assert!(p.stranding.is_empty());
    assert_eq!(cmds(Batch::Auto, &p)[0].iter().filter(|a| a.starts_with(":(literal)")).count(), 3);
    assert!(!cmds(Batch::Auto, &p)[0].contains(&"--checkout".to_owned()));
    assert!(cmds(Batch::UpdateAll, &p)[0].contains(&"--checkout".to_owned()));

    // Every listed submodule is a target: no pathspecs.
    let all = vec![sm("x", SubmoduleState::Uninitialized, true), sm("y", SubmoduleState::Missing, true)];
    let p = plan(Batch::InitAll, &all);
    assert!(p.all);
    assert_eq!(cmds(Batch::InitAll, &p), batch_update_cmds(None, true));
    assert!(plan(Batch::InitAll, &[]).targets.is_empty() && !plan(Batch::InitAll, &[]).all);
}

#[test]
fn blocked_and_labels() {
    let list = mixed();
    assert_eq!(init_all_blocked(&list), None);
    assert_eq!(update_all_blocked(&list), None);
    let synced = vec![sm("a", SubmoduleState::InSync, true), sm("b", SubmoduleState::Uninitialized, false)];
    assert!(init_all_blocked(&synced).is_some() && update_all_blocked(&synced).is_some());
    let out = vec![sm("a", SubmoduleState::OutOfSync, true)];
    assert!(init_all_blocked(&out).is_some() && update_all_blocked(&out).is_none());
    assert_eq!(job_label(Batch::InitAll), "Initialize submodules");
    assert_eq!(job_label(Batch::Auto), "Update submodules");
    assert_eq!(job_label(Batch::UpdateAll), "Update submodules");
}

#[test]
fn texts_name_counts_and_commits() {
    let mut a = sm("libs/a", SubmoduleState::OutOfSync, true);
    a.left_behind = vec![(oid('c'), "first".into())];
    let mut b = sm("libs/b", SubmoduleState::OutOfSync, true);
    b.left_behind = vec![(oid('d'), "one".into()), (oid('e'), "two".into())];
    let body = confirm_all_body(&[a.clone(), b]);
    assert!(body.contains("\n\nlibs/a: 1 commit\n    ccccccc first"), "{body}");
    assert!(body.contains("\n\nlibs/b: 2 commits\n    ddddddd one\n    eeeeeee two"), "{body}");
    assert!(body.contains("To keep them, cancel"), "{body}");
    let line = skipped_line(&a);
    assert!(line.contains("libs/a") && line.contains("1 commit") && line.contains("Update to recorded commit"), "{line}");
    assert_eq!(offer_text("app", 1), "1 submodule in app isn't checked out yet.");
    assert_eq!(offer_text("app", 3), "3 submodules in app aren't checked out yet.");
}
