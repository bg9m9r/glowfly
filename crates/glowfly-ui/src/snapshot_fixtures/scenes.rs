//! Scenes the view tests share, built on the fixture repository: a merge stopped on conflicts
//! (`merging`, `fill_lists`) and a history with two authors to search (`with_two_authors`,
//! `searching`). The app crate's view tests that run effects through the runner use them too.

use super::{git, reload, with_repo};
use crate::msg::{Msg, SearchMsg};
use crate::state::AppState;
use crate::state::Selection;
use crate::state::conflict_files::conflicted_paths;
use glowfly_core::gitcli::{args, run_with_input};
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DATE: &str = "2026-01-11T10:00:00+00:00";

/// `file`'s text on `side` ("base", "ours" or "theirs"): numbered lines, with `changed` lines
/// rewritten on the two sides.
fn text(file: &str, lines: usize, changed: &[usize], side: &str) -> String {
    (1..=lines)
        .map(|i| if side != "base" && changed.contains(&i) { format!("{file} {i} {side}\n") } else { format!("{file} {i}\n") })
        .collect()
}

/// The fixture mid-merge with alpha.txt (two conflicts), beta.txt (one) and gamma.txt (one,
/// resolved with `git add`). With `fix_beta`, beta.txt's markers are edited away.
pub fn merging(name: &str, fix_beta: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let files = [("alpha.txt", 10, vec![2, 8]), ("beta.txt", 4, vec![2]), ("gamma.txt", 4, vec![3])];
    let write = |side: &str| {
        for (f, n, changed) in &files {
            std::fs::write(dir.join(f), text(f, *n, changed, side)).unwrap();
        }
    };
    write("base");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: base"], DATE);
    git(&dir, &["checkout", "-q", "-b", "list-side"], DATE);
    write("theirs");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: theirs"], DATE);
    git(&dir, &["checkout", "-q", "main"], DATE);
    write("ours");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: ours"], DATE);
    let (ok, _) = run_with_input(&dir, &args(&["merge", "list-side"]), "", &[]);
    assert!(!ok, "the merge conflicts");
    if fix_beta {
        std::fs::write(dir.join("beta.txt"), "beta.txt 1\nbeta.txt 2 both\nbeta.txt 3\nbeta.txt 4\n").unwrap();
    }
    git(&dir, &["add", "gamma.txt"], DATE);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Wip;
    (st, dir)
}

pub fn fill_lists(st: &mut AppState, dir: &Path) {
    let paths = conflicted_paths(st.snap.as_ref().unwrap());
    let summary = repo::conflict_summary(dir, &paths, true).unwrap();
    st.session.conflict_files.counts = summary.counts.into_iter().collect();
    st.session.conflict_files.resolved = summary.resolved;
}

/// The fixture with one more commit on main, by Grace Hopper.
pub fn with_two_authors(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "notes.txt"], d);
    git(&dir, &["commit", "-q", "-m", "Write release notes", "--author", "Grace Hopper <grace@example.com>"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    let snap = st.snap.as_ref().unwrap();
    let grace = snap.commits.iter().find(|c| c.summary == "Write release notes").expect("the new commit");
    assert_eq!((grace.author.name(), grace.author.email()), ("Grace Hopper", "grace@example.com"));
    assert!(snap.graph.wip.is_some(), "README.md is still modified");
    (st, dir)
}

pub fn send(st: &mut AppState, msg: SearchMsg) {
    crate::update::update(st, Msg::Search(msg), Instant::now());
}

/// The bar shown with `q` typed into it.
pub fn searching(st: &mut AppState, q: &str) {
    st.view.show_search = true;
    st.session.tab_view.search = q.into();
    send(st, SearchMsg::QueryChanged);
}
