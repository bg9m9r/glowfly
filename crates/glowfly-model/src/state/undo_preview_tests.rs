//! The Undo and Redo previews: a commit, a checkout that made a branch, a pull over made-up
//! commits in the graph, a pop, several discarded files, a tag delete, renames, a removed remote,
//! a worktree made with a new branch, the Redo wording, and a preview for every kind of action.

use super::*;
use crate::state::op_fixture;
use crate::state::toolbar_tips::tests::diverge;
use glowfly_core::repo::ConflictIds;
use glowfly_core::undo::{BranchLink, Changes, Created, DeleteVia, MoveOp, RemoteConfig, ResetMode, Saved, UrlChange};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn lines(p: &TipPreview) -> Vec<String> {
    p.rows.iter().filter_map(|r| if let TipRow::Line(l) = r { Some(l.clone()) } else { None }).collect()
}

#[test]
fn a_commit_goes_back_to_its_parent() {
    let snap = op_fixture::snapshot("undo-preview-commit");
    let to = snap.head_oid.unwrap();
    let a = Action::Commit { branch: Some("main".into()), from: oid(7), to, message: "one".into(), amend: false };
    let p = preview(Lang::En, &a, Some(&snap), false);
    assert_eq!(p.heading, "Reverts");
    assert_eq!(p.rows[0], TipRow::Commit { sha: short_sha(to), summary: "one".into() });
    assert_eq!(lines(&p), [format!("main goes back to {}", short_sha(oid(7)))]);
    let redo = preview(Lang::En, &a, Some(&snap), true);
    assert_eq!(redo.heading, "Redoes");
    assert_eq!(lines(&redo), [format!("main moves to {} again", short_sha(to))]);
    let bare = preview(Lang::En, &a, None, false);
    assert_eq!(bare.rows[0], TipRow::Line(short_sha(to)), "no snapshot: the short SHA alone");
}

#[test]
fn a_checkout_that_made_a_branch_offers_to_delete_it() {
    let created = Created { name: "feature/x".into(), oid: oid(3), upstream: None };
    let a = Action::Checkout { from: HeadPos::Branch("dev".into()), to: HeadPos::Branch("feature/x".into()), created: Some(created) };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(lines(&p), ["Checks out dev again", "Asks whether to delete feature/x"]);
}

#[test]
fn a_pull_takes_off_the_commits_it_brought() {
    let mut snap = op_fixture::snapshot("undo-preview-pull");
    diverge(&mut snap, 3, 0, true);
    let base = snap.commits.last().unwrap().oid;
    let to = snap.head_oid.unwrap();
    let a = Action::Moved { op: MoveOp::Pull, detail: None, branch: Some("main".into()), from: base, to };
    let p = preview(Lang::En, &a, Some(&snap), false);
    assert_eq!(p.rows[0], TipRow::Line("Takes off 3 commits".into()));
    let commits = p.rows.iter().filter(|r| matches!(r, TipRow::Commit { .. })).count();
    assert_eq!(commits, 3);
    assert!(matches!(&p.rows[1], TipRow::Commit { summary, .. } if summary == "Ahead 3"), "newest first: {:?}", p.rows);
    assert_eq!(p.rows.last(), Some(&TipRow::Line(format!("main goes back to {}", short_sha(base)))));
    assert_eq!(p.note, None);
    assert_eq!(preview(Lang::En, &a, Some(&snap), true).rows[0], TipRow::Line("Puts back 3 commits".into()));
}

#[test]
fn a_pop_names_its_stash() {
    let a = Action::StashApply { pop: true, stash: oid(1), head: oid(2), message: "On main: wip".into(), left: Changes(Vec::new()) };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(lines(&p), ["On main: wip", "Takes the changes off and puts the stash back"]);
}

#[test]
fn a_tag_delete_says_the_remotes_keep_it_deleted() {
    let a =
        |remotes: &[&str]| Action::DeleteTag { name: "v1".into(), oid: oid(1), remotes: remotes.iter().map(|r| (*r).to_owned()).collect() };
    assert_eq!(lines(&preview(Lang::En, &a(&[]), None, false)), ["Creates tag v1 again"]);
    assert_eq!(
        lines(&preview(Lang::En, &a(&["origin", "backup"]), None, false)),
        ["Creates tag v1 again", "Stays deleted from origin and backup; push it to put it back there"]
    );
    assert_eq!(lines(&preview(Lang::En, &a(&["origin", "backup"]), None, true)), ["Deletes tag v1"], "redo deletes only here");
}

#[test]
fn every_action_has_a_preview() {
    let saved = Saved::file(oid(5), false);
    let ids = ConflictIds { ancestor: None, ours: Some(oid(1)), theirs: Some(oid(2)) };
    let branch = Some("main".to_owned());
    let all = [
        Action::Checkout { from: HeadPos::Detached(oid(1)), to: HeadPos::Branch("main".into()), created: None },
        Action::Commit { branch: branch.clone(), from: oid(1), to: oid(2), message: "m".into(), amend: true },
        Action::Reset { mode: ResetMode::Hard, branch: branch.clone(), from: oid(1), to: oid(2), stash: None },
        Action::DeleteBranch { name: "x".into(), oid: oid(1), upstream: None },
        Action::DiscardAll { stash: oid(1), head: oid(2) },
        Action::Discard { path: "a.txt".into(), what: Discarded::Hunk, before: Some(saved), after: None },
        Action::Restore { path: "a.txt".into(), oid: oid(1), before: None, after: Some(saved) },
        Action::CreateBranch { name: "x".into(), oid: oid(1), upstream: None },
        Action::CreateTag { name: "v1".into(), oid: oid(1) },
        Action::DeleteTag { name: "v1".into(), oid: oid(1), remotes: vec!["origin".into()] },
        Action::Moved { op: MoveOp::Merge, detail: Some("dev".into()), branch: None, from: oid(1), to: oid(2) },
        Action::StashPush { stash: oid(1), head: oid(2), message: "m".into(), replay: None },
        Action::StashApply { pop: false, stash: oid(1), head: oid(2), message: "m".into(), left: Changes(Vec::new()) },
        Action::StashDrop { stash: oid(1), message: "m".into() },
        Action::Resolve { path: "a.txt".into(), ids, head: None, staged: None, before: None, after: None },
        Action::DeleteFile { path: "a.txt".into(), before: saved, via: DeleteVia::Permanent, staged: None },
        Action::AbortKept {
            op: MoveOp::CherryPick,
            saved: oid(1),
            head: oid(2),
            branch: branch.clone(),
            left: Changes(Vec::new()),
            during: None,
        },
        Action::RenameBranch { old: "a".into(), new: "b".into(), remote: Some("origin".into()) },
        Action::RenameRemote { old: "origin".into(), new: "up".into(), urls: None },
        Action::RemoveRemote { name: "origin".into(), config: remote_config(&["main"]) },
        Action::AddWorktree {
            path: glowfly_core::paths::temp_dir().join("feature-wt"),
            created: Created { name: "feature".into(), oid: oid(1), upstream: None },
        },
    ];
    for a in &all {
        for redo in [false, true] {
            let p = preview(Lang::En, a, None, redo);
            assert!(!p.rows.is_empty(), "{a:?} (redo {redo})");
            for l in lines(&p) {
                assert!(!l.contains('{') && !l.starts_with("undo."), "{a:?}: {l}");
            }
        }
    }
}

#[test]
fn batch_preview_lists_its_files_and_the_restore_line() {
    let saved = Saved::file(oid(5), false);
    let discard = |p: &str| Action::Discard { path: p.into(), what: Discarded::File, before: Some(saved), after: None };
    let untracked = Action::DeleteFile { path: "new.txt".into(), before: saved, via: DeleteVia::Trash, staged: None };
    let a = Action::Batch { op: glowfly_core::undo::BatchOp::Discard, parts: vec![discard("a.txt"), untracked] };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(p.rows[0], TipRow::File { status: 'M', path: "a.txt".into() });
    assert_eq!(p.rows[1], TipRow::File { status: 'D', path: "new.txt".into() });
    assert_eq!(lines(&p), ["Brings back what was discarded"]);
    assert_eq!(lines(&preview(Lang::En, &a, None, true)), ["Discards them again"]);

    let many: Vec<Action> = (0..PREVIEW_ROWS + 2).map(|i| discard(&format!("f{i}.txt"))).collect();
    let a = Action::Batch { op: glowfly_core::undo::BatchOp::Discard, parts: many };
    let p = preview(Lang::En, &a, None, false);
    let files = p.rows.iter().filter(|r| matches!(r, TipRow::File { .. })).count();
    assert_eq!(files, PREVIEW_ROWS);
    assert_eq!(lines(&p), ["and 2 more", "Brings back what was discarded"]);
}

#[test]
fn a_delete_batch_lists_its_files_and_says_it_puts_them_back() {
    let saved = Saved::file(oid(6), false);
    let gone = |p: &str| Action::DeleteFile { path: p.into(), before: saved, via: DeleteVia::Trash, staged: None };
    let a = Action::Batch { op: glowfly_core::undo::BatchOp::Delete, parts: vec![gone("a.txt"), gone("c.txt")] };
    let p = preview(Lang::En, &a, None, false);
    assert_eq!(p.rows[0], TipRow::File { status: 'D', path: "a.txt".into() });
    assert_eq!(p.rows[1], TipRow::File { status: 'D', path: "c.txt".into() });
    assert_eq!(lines(&p), ["Puts the files back"]);
    assert_eq!(lines(&preview(Lang::En, &a, None, true)), ["Deletes them again"]);

    let many: Vec<Action> = (0..PREVIEW_ROWS + 3).map(|i| gone(&format!("f{i}.txt"))).collect();
    let a = Action::Batch { op: glowfly_core::undo::BatchOp::Delete, parts: many };
    assert_eq!(lines(&preview(Lang::En, &a, None, false)), ["and 3 more", "Puts the files back"]);
}

/// A removed remote's recorded config, with `tracking` branches that tracked it.
fn remote_config(tracking: &[&str]) -> RemoteConfig {
    let link = |b: &str, key: &str, value: &str| BranchLink { branch: b.into(), key: key.into(), value: value.into() };
    let mut branches: Vec<BranchLink> = tracking.iter().map(|b| link(b, "remote", "origin")).collect();
    branches.push(link("dev", "pushremote", "origin"));
    RemoteConfig { entries: vec![("url".into(), "https://example.invalid/a.git".into())], branches, push_default: false }
}

#[test]
fn a_branch_rename_with_remote_says_the_remote_stays() {
    let a = |remote: Option<&str>| Action::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote: remote.map(Into::into) };
    assert_eq!(lines(&preview(Lang::En, &a(None), None, false)), ["Renames branch feature-2 back to feature"]);
    assert_eq!(
        lines(&preview(Lang::En, &a(Some("origin")), None, false)),
        ["Renames branch feature-2 back to feature", "Stays feature-2 on origin: renaming it there isn't undone"]
    );
    assert_eq!(lines(&preview(Lang::En, &a(Some("origin")), None, true)), ["Renames branch feature to feature-2 again"]);
}

#[test]
fn a_remote_rename_with_urls_says_they_come_back() {
    let pair = |v: &str| vec![("url".to_owned(), v.to_owned())];
    let urls = Some(UrlChange { before: pair("https://example.invalid/a.git"), after: pair("https://example.invalid/b.git") });
    let a = Action::RenameRemote { old: "origin".into(), new: "upstream".into(), urls };
    assert_eq!(lines(&preview(Lang::En, &a, None, false)), ["Renames remote upstream back to origin", "Puts its previous URLs back"]);
    assert_eq!(lines(&preview(Lang::En, &a, None, true)), ["Renames remote origin to upstream again", "Sets its edited URLs again"]);
    let plain = Action::RenameRemote { old: "origin".into(), new: "upstream".into(), urls: None };
    assert_eq!(lines(&preview(Lang::En, &plain, None, false)), ["Renames remote upstream back to origin"]);
}

#[test]
fn a_removed_remote_offers_the_fetch_line_and_its_branches() {
    let a = Action::RemoveRemote { name: "origin".into(), config: remote_config(&["main", "dev"]) };
    assert_eq!(
        lines(&preview(Lang::En, &a, None, false)),
        [
            "Adds remote origin back with its URLs and settings",
            "main and dev track it again",
            "Fetch it afterwards to bring back its remote-tracking branches"
        ]
    );
    assert_eq!(lines(&preview(Lang::En, &a, None, true)), ["Removes remote origin again"]);
    let untracked = Action::RemoveRemote { name: "origin".into(), config: remote_config(&[]) };
    assert_eq!(lines(&preview(Lang::En, &untracked, None, false)).len(), 2, "no branch line without tracking branches");
}

#[test]
fn a_worktree_undo_keeps_a_moved_branch() {
    let snap = op_fixture::snapshot("undo-preview-worktree");
    let local = snap.locals.first().expect("a branch");
    let (name, tip) = (local.name.clone(), local.oid.unwrap());
    let path = glowfly_core::paths::temp_dir().join("app-wt");
    let a = |oid| Action::AddWorktree { path: path.clone(), created: Created { name: name.clone(), oid, upstream: None } };
    assert_eq!(
        lines(&preview(Lang::En, &a(tip), Some(&snap), false)),
        ["Removes worktree app-wt".to_owned(), format!("Deletes branch {name}")]
    );
    assert_eq!(
        lines(&preview(Lang::En, &a(oid(9)), Some(&snap), false)),
        ["Removes worktree app-wt".to_owned(), format!("Keeps branch {name}: it has commits made since")]
    );
    assert_eq!(lines(&preview(Lang::En, &a(tip), Some(&snap), true)), [format!("Creates worktree app-wt on branch {name} again")]);
}
