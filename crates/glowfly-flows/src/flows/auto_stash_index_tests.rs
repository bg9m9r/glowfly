//! Putting an auto-stash back keeps what was staged staged (`git stash pop --index`), and says
//! so when git can't restore it and everything comes back unstaged.

use super::auto_stash_tests::{ask, up};
use super::job_errors_tests::two_branches;
use super::tests::{clean_tree, drive, finish, git, scratch_repo, state};
use crate::msg::{AutoStashMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::auto_stash::back_unstaged;
use glowfly_core::i18n::Lang;
use std::path::Path;

fn stash_count(dir: &Path) -> usize {
    glowfly_core::stash_ops::list(dir).unwrap().len()
}

fn paths(rows: &[glowfly_core::repo::FileEntry]) -> Vec<&str> {
    rows.iter().map(|f| f.path.as_str()).collect()
}

fn checkout_feature_with_stash(st: &mut AppState) {
    assert!(up(st, Msg::git("Checkout", &["checkout", "feature", "--"])).is_empty());
    ask(st);
    let fx = up(st, Msg::AutoStash(AutoStashMsg::Stash));
    finish(st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"));
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

#[test]
fn stash_and_checkout_keeps_the_staged_file_staged() {
    let (mut st, dir) = two_branches("asi-staged");
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    checkout_feature_with_stash(&mut st);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(paths(&snap.staged), vec!["a.txt"], "still staged");
    assert_eq!(paths(&snap.unstaged), vec!["f.txt"], "still unstaged");
    let toast = st.toasts.iter().rev().find(|t| t.text.starts_with("Your changes from")).expect("the back toast");
    assert!(toast.ok && toast.text.ends_with("are back."), "{}", toast.text);
    assert_eq!(stash_count(&dir), 0);
    assert!(logged(&st, "$ git stash pop --index stash@{0}"));
    let _ = std::fs::remove_dir_all(&dir);
}

fn lines(change: &[(usize, &str)]) -> String {
    (1..=9)
        .map(|n| {
            let line = change.iter().find(|(i, _)| *i == n).map_or(n.to_string(), |(_, t)| (*t).to_owned());
            line + "\n"
        })
        .collect()
}

#[test]
fn a_put_back_that_cannot_restore_the_index_says_so() {
    let dir = scratch_repo("asi-fallback");
    clean_tree(&dir);
    std::fs::write(dir.join("n.txt"), lines(&[])).unwrap();
    git(&dir, &["add", "n.txt"]);
    git(&dir, &["commit", "-q", "-m", "numbers"]);
    // feature changes line 4: inside the context of a staged change to line 6.
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("n.txt"), lines(&[(4, "four")])).unwrap();
    git(&dir, &["commit", "-q", "-am", "four"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("n.txt"), lines(&[(6, "six")])).unwrap();
    git(&dir, &["add", "n.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    checkout_feature_with_stash(&mut st);
    let want = back_unstaged(Lang::En, "Auto-stash before checking out feature");
    let toast = st.toasts.iter().rev().find(|t| t.text.starts_with("Your changes from")).expect("the back toast");
    assert!(toast.ok, "{}", toast.text);
    assert_eq!(toast.text, want);
    assert!(logged(&st, "$ git stash pop --index stash@{0}") && logged(&st, "$ git stash pop stash@{0}"), "both commands are logged");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.is_empty(), "nothing is staged: {:?}", paths(&snap.staged));
    assert_eq!(paths(&snap.unstaged), vec!["n.txt"]);
    let text = std::fs::read_to_string(dir.join("n.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, lines(&[(4, "four"), (6, "six")]));
    assert!(st.auto_stash.pending.is_empty());
    assert_eq!(stash_count(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
