//! Apply and Unstash bring back what was staged: through the runner, a stash made of staged and
//! unstaged changes comes back the same (and undo and redo agree), says so when the staged part
//! no longer fits and everything comes back unstaged, keeps the staged selection that is already
//! there, and keeps the stash over a conflict.

use super::auto_stash_index_tests::{lines, logged, stash_count};
use super::job_errors_tests::two_branches;
use super::stash_push_options_tests::push_with;
use super::tests::{clean_tree, drive, git, scratch_repo, state};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::FileEntry;
use glowfly_core::stash_ops::StashScope;
use glowfly_model::msg::{JobMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::stash_actions::{self, StashOption};

fn paths(rows: &[FileEntry]) -> Vec<String> {
    rows.iter().map(|f| f.path.clone()).collect()
}

/// (staged paths, unstaged paths) as the snapshot shows them.
fn changes(st: &AppState) -> (Vec<String>, Vec<String>) {
    let snap = st.snap.as_ref().unwrap();
    (paths(&snap.staged), paths(&snap.unstaged))
}

fn stash_all(st: &mut AppState) {
    drive(st, push_with(StashScope::All, "wip", &[], false));
    assert_eq!(st.snap.as_ref().unwrap().stashes.len(), 1);
}

fn unstash(st: &mut AppState) {
    let snap = st.snap.as_ref().unwrap();
    let msg = stash_actions::pop_latest(snap, Lang::En).expect("a stash");
    drive(st, msg);
}

/// a.txt staged and f.txt (its last line) unstaged, on `two_branches`.
fn staged_and_unstaged(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = two_branches(name);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

#[test]
fn unstash_puts_the_staged_changes_back_as_staged_and_undo_and_redo_agree() {
    let (mut st, dir) = staged_and_unstaged("si-roundtrip");
    let before = changes(&st);
    assert_eq!(before, (vec!["a.txt".to_owned()], vec!["f.txt".to_owned()]));
    stash_all(&mut st);
    drive(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    drive(&mut st, Msg::git("Checkout", &["checkout", "main", "--"]));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    unstash(&mut st);
    assert_eq!(changes(&st), before);
    assert_eq!(stash_count(&dir), 0);
    assert!(logged(&st, "$ git stash pop --index stash@{0}"));
    assert!(!st.toasts.iter().any(|t| t.text.starts_with("The stashed changes are back, but unstaged")));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(changes(&st), (vec![], vec![]), "clean, nothing staged");
    assert_eq!(stash_count(&dir), 1);

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(changes(&st), before);
    assert_eq!(stash_count(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// n.txt (lines 1..9) with line 6 staged and stashed, then line 4 committed on main: the stash's
/// staged hunk no longer fits the index.
fn index_wont_fit(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    std::fs::write(dir.join("n.txt"), lines(&[])).unwrap();
    git(&dir, &["add", "n.txt"]);
    git(&dir, &["commit", "-q", "-m", "numbers"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    std::fs::write(dir.join("n.txt"), lines(&[(6, "six")])).unwrap();
    git(&dir, &["add", "n.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    stash_all(&mut st);
    std::fs::write(dir.join("n.txt"), lines(&[(4, "four")])).unwrap();
    git(&dir, &["commit", "-q", "-am", "four"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

#[test]
fn a_staged_part_that_no_longer_fits_says_so_for_unstash_and_apply() {
    for option in [StashOption::Pop, StashOption::Apply] {
        let (mut st, dir) = index_wont_fit("si-unfit");
        let oid = st.snap.as_ref().unwrap().stashes[0].oid;
        let msg = stash_actions::for_commit(st.snap.as_ref().unwrap(), Lang::En, option, oid).unwrap();
        drive(&mut st, msg);
        let want = Lang::En.tr("stash.back_unstaged.toast");
        let toast = st.toasts.iter().rev().find(|t| t.text == want).expect("the unstaged toast");
        assert!(toast.ok, "{}", toast.text);
        let verb = if option == StashOption::Pop { "pop" } else { "apply" };
        let both = format!("$ git stash {verb} --index stash@{{0}}");
        let plain = format!("$ git stash {verb} stash@{{0}}");
        assert!(st.log.iter().any(|l| l.text.contains(&both) && l.text.contains(&plain)), "both commands in one run");
        let (staged, unstaged) = changes(&st);
        assert!(staged.is_empty(), "{staged:?}");
        assert_eq!(unstaged, vec!["n.txt".to_owned()]);
        let text = std::fs::read_to_string(dir.join("n.txt")).unwrap().replace("\r\n", "\n");
        assert_eq!(text, lines(&[(4, "four"), (6, "six")]));
        assert_eq!(stash_count(&dir), usize::from(option == StashOption::Apply));

        drive(&mut st, Msg::Job(JobMsg::Undo));
        assert_eq!(changes(&st), (vec![], vec![]), "undo leaves a clean tree");
        assert_eq!(stash_count(&dir), 1, "the stash is back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn something_staged_already_stays_staged_and_runs_the_plain_form() {
    let (mut st, dir) = staged_and_unstaged("si-other-staged");
    stash_all(&mut st);
    std::fs::write(dir.join("x.txt"), "staged\n").unwrap();
    git(&dir, &["add", "x.txt"]);
    std::fs::write(dir.join("x.txt"), "work\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let msg = stash_actions::pop_latest(snap, Lang::En).unwrap();
    drive(&mut st, msg);
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(logged(&st, "$ git stash pop stash@{0}"));
    assert!(!logged(&st, "stash pop --index"));
    assert_eq!(stash_count(&dir), 0);
    let shown = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["show", ":x.txt"])).1;
    assert_eq!(shown.trim_end(), "staged");
    let (staged, _) = changes(&st);
    assert!(staged.contains(&"x.txt".to_owned()), "{staged:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_keeps_the_stash_and_runs_one_command() {
    let (mut st, dir) = staged_and_unstaged("si-conflict");
    stash_all(&mut st);
    let oid = st.snap.as_ref().unwrap().stashes[0].oid;
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\ntheirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    unstash(&mut st);
    assert!(st.toasts.last().is_some_and(|t| !t.ok), "{:?}", st.toasts.last().map(|t| t.text.clone()));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().chain(&snap.staged).any(|f| f.path == "f.txt" && f.status == '!'), "f.txt is conflicted");
    assert_eq!(snap.stashes.iter().map(|s| s.oid).collect::<Vec<_>>(), vec![oid], "the stash stays");
    let echoes: usize = st.log.iter().map(|l| l.text.matches("$ git stash pop").count()).sum();
    assert_eq!(echoes, 1, "no second run");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_entry_point_runs_the_same_job() {
    use glowfly_model::effect::{Effect, JobEffect};
    use glowfly_model::msg::PaletteMsg;
    use glowfly_model::state::palette::PaletteCommand as C;
    use glowfly_model::state::stash_confirm::StashAction;
    use std::time::Instant;
    let (mut st, dir) = staged_and_unstaged("si-entry-points");
    stash_all(&mut st);
    let snap = st.snap.as_ref().unwrap();
    let oid = snap.stashes[0].oid;
    let menu = stash_actions::items(snap, Lang::En, &snap.stashes[0]);
    let menu_pop = menu.into_iter().find(|i| i.option == StashOption::Pop).unwrap().msg;
    let toolbar = stash_actions::pop_latest(snap, Lang::En).unwrap();
    let palette = |cmd| Msg::Palette(PaletteMsg::RunPaletteCommand(cmd));
    let job = |st: &mut AppState, msg: Msg| {
        let fx = super::update(st, msg, Instant::now());
        let [Effect::Job(JobEffect::RunGit { cmds, expect, without_index, record, .. })] = &fx[..] else {
            panic!("expected one RunGit, got {} effects", fx.len())
        };
        let out = (cmds.clone(), expect.clone(), without_index.clone(), record.clone());
        st.job = None;
        out
    };
    let want_cmds = vec![vec!["stash".to_owned(), "pop".into(), "--index".into(), "stash@{0}".into()]];
    let want = (
        want_cmds,
        Some(("stash@{0}".to_owned(), oid)),
        Some(vec!["stash".to_owned(), "pop".into(), "stash@{0}".into()]),
        Some(glowfly_core::undo::UndoKind::StashApply { pop: true, rev: Some("stash@{0}".into()) }),
    );
    assert_eq!(job(&mut st, menu_pop), want);
    assert_eq!(job(&mut st, toolbar), want);
    assert_eq!(job(&mut st, palette(C::PopStash)), want);
    assert_eq!(job(&mut st, palette(C::StashOp { action: StashAction::Pop, oid })), want);

    let menu_apply = {
        let snap = st.snap.as_ref().unwrap();
        stash_actions::items(snap, Lang::En, &snap.stashes[0]).into_iter().find(|i| i.option == StashOption::Apply).unwrap().msg
    };
    let (cmds, _, plain, _) = job(&mut st, menu_apply);
    assert_eq!(cmds, vec![vec!["stash".to_owned(), "apply".into(), "--index".into(), "stash@{0}".into()]]);
    assert_eq!(plain, Some(vec!["stash".to_owned(), "apply".into(), "stash@{0}".into()]));

    let (cmds, _, plain, _) = job(&mut st, Msg::Stash(glowfly_model::msg::StashMsg::Run { action: StashAction::Drop, index: 0, oid }));
    assert_eq!(cmds, vec![vec!["stash".to_owned(), "drop".into(), "stash@{0}".into()]]);
    assert_eq!(plain, None);
    let _ = std::fs::remove_dir_all(&dir);
}
