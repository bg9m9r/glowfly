//! The stash question's rules: the job on a clean tree, the question on a dirty one, the
//! refusal over conflicts, Pop naming the newest stash by position and commit, and the files
//! the stash and the working tree both change.

use super::*;
use glowfly_core::repo::FileEntry;

fn fixture(name: &str) -> (std::path::PathBuf, Snapshot) {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-stash-confirm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = git2::Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("T", "t@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "one", &tree, &[]).unwrap();
    let snap = glowfly_core::repo::load(&dir, &glowfly_core::repo::LoadOptions::all(100)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (dir, snap)
}

const EN: Lang = Lang::En;

fn oid() -> Oid {
    Oid::from_str("1234567890123456789012345678901234567890").unwrap()
}

fn listed(index: usize) -> StashRun {
    StashRun::Listed { index }
}

fn with_stash(mut snap: Snapshot) -> Snapshot {
    snap.stashes = vec![StashInfo { index: 0, message: "On main: wip".into(), oid: oid() }];
    snap
}

fn strings(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_clean_tree_runs_the_guarded_job() {
    let (_, clean) = fixture("clean");
    for action in [StashAction::Apply, StashAction::Pop] {
        let m = guarded(&clean, EN, action, oid(), "wip", listed(2));
        assert!(matches!(m, Msg::Job(JobMsg::Stash { action: a, index: 2, oid: o }) if a == action && o == oid()));
    }
    let m = guarded(&clean, EN, StashAction::Drop, oid(), "wip", listed(2));
    assert!(matches!(&m, Msg::ConfirmDanger { title, then, .. }
        if title == "Delete stash" && matches!(**then, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: 2, .. }))));
}

#[test]
fn a_dirty_tree_asks_after_reading_the_stash() {
    let (_, mut dirty) = fixture("dirty");
    dirty.unstaged.push(FileEntry::new("a.txt", 'M'));
    let m = guarded(&dirty, EN, StashAction::Pop, oid(), "wip", listed(0));
    let Msg::Stash(StashMsg::Ask(ask)) = m else { panic!("not the question") };
    assert_eq!(ask, StashAsk { action: StashAction::Pop, oid: oid(), message: "wip".into(), run: listed(0) });
    let paths = strings(&["a.txt", "b.txt"]);
    let m = ask_confirm(&dirty, EN, &ask, Some(&paths));
    assert!(matches!(&m, Msg::Confirm { title, button, body, then }
        if title == "Pop stash" && button == "Pop" && body.contains("a.txt") && !body.contains("b.txt")
        && matches!(**then, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, .. }))));
}

#[test]
fn conflicts_refuse_apply_and_pop() {
    let (_, mut snap) = fixture("conflicts");
    snap.unstaged.push(FileEntry::new("a.txt", '!'));
    let why = apply_blocked(&snap, EN).expect("refused over conflicts");
    assert!(why.contains("conflict"), "{why}");
    for action in [StashAction::Apply, StashAction::Pop] {
        let m = guarded(&snap, EN, action, oid(), "wip", listed(0));
        assert!(matches!(&m, Msg::Logged { ok: false, text } if text == why), "no question, a reason");
    }
    assert!(matches!(guarded(&snap, EN, StashAction::Drop, oid(), "wip", listed(0)), Msg::ConfirmDanger { .. }));
    let (_, clean) = fixture("no-conflicts");
    assert!(apply_blocked(&clean, EN).is_none());
}

#[test]
fn pop_latest_names_the_newest_stash_by_position_and_commit() {
    let (_, snap) = fixture("pop-latest");
    assert!(pop_latest(&snap, EN).is_none(), "no stashes");
    let snap = with_stash(snap);
    let m = pop_latest(&snap, EN).unwrap();
    assert!(matches!(m, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, oid: o }) if o == oid()));
}

#[test]
fn overlap_is_the_files_both_change() {
    let (_, mut snap) = fixture("overlap");
    snap.staged.push(FileEntry { renamed_from: Some("old.txt".into()), ..FileEntry::new("new.txt", 'R') });
    snap.unstaged.extend([FileEntry::new("b.txt", 'M'), FileEntry::new("u.txt", '?')]);
    let stash = strings(&["a.txt", "b.txt", "old.txt", "u.txt", "z.txt"]);
    assert_eq!(overlap(&stash, &snap), ["b.txt", "old.txt", "u.txt"]);
    assert!(overlap(&strings(&["a.txt"]), &snap).is_empty());
}

#[test]
fn dirty_body_lists_the_overlap_and_leaves_the_warning_out_without_one() {
    let start = "Apply \"wip on main\" on top of your uncommitted changes?";
    let apart = dirty_body(EN, StashAction::Apply, "wip on main", Some(&[]));
    assert_eq!(apart, format!("{start} It changes none of the files you have changed."));
    assert!(!apart.contains("conflict"), "{apart}");
    let unknown = dirty_body(EN, StashAction::Apply, "wip on main", None);
    assert_eq!(unknown, format!("{start} If they change the same files, git may stop with conflicts or refuse to apply."));
    let both = strings(&["a.txt", "b.txt"]);
    let listed = dirty_body(EN, StashAction::Pop, "wip on main", Some(&both));
    assert_eq!(
        listed,
        format!(
            "{start} It changes 2 files you have changed too, so git may stop with conflicts or refuse to apply:\n    a.txt\n    b.txt\n\nIf it conflicts, the stash is kept."
        )
    );
    let one = dirty_body(EN, StashAction::Apply, "w", Some(&both[..1]));
    assert!(one.contains("It changes a file you have changed too"), "{one}");
    let many: Vec<String> = (0..12).map(|i| format!("f{i:02}.txt")).collect();
    let capped = dirty_body(EN, StashAction::Apply, "w", Some(&many));
    assert!(capped.contains("f07.txt") && !capped.contains("f08.txt") && capped.ends_with("and 4 more"), "{capped}");
}
