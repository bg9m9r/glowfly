//! The header model: path parts, status and line counts, the context, the View label, the file
//! actions and their reasons, the primary action, binary sizes and the counter.

use super::*;
use crate::effect::Effect;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::state::diff_nav::Counter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use trellis_core::gitcli::{args, run};
use trellis_core::repo::{BinarySide, BinarySides};
use trellis_core::settings::Settings;

fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// Runs `msg` and everything it leads to, as the app would.
fn drive(st: &mut AppState, msg: Msg) {
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for e in crate::update::update(st, m, Instant::now()) {
            if matches!(e, Effect::LoadDiff { .. } | Effect::LoadBinarySides { .. } | Effect::LoadRepo { .. } | Effect::LoadStatus { .. })
                || matches!(e, Effect::LoadDetail { .. } | Effect::LoadCompare { .. } | Effect::LoadWorkdirCompare { .. })
            {
                queue.extend(crate::runner::execute(e));
            }
        }
    }
}

/// src/app.txt (20 lines), gone.txt, staged.txt and logo.png committed twice (so HEAD has a
/// parent); then src/app.txt edited at two places, gone.txt deleted, staged.txt changed and
/// staged, new.txt untracked.
fn fixture(name: &str) -> (AppState, PathBuf) {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-app-test-header-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&dir, &["config", k, v]);
    }
    let lines = |changed: &[usize]| -> String {
        (1..=20).map(|i| if changed.contains(&i) { format!("x{i}\n") } else { format!("{i}\n") }).collect()
    };
    let write = |f: &str, t: &[u8]| std::fs::write(dir.join(f), t).unwrap();
    write("src/app.txt", lines(&[]).as_bytes());
    write("gone.txt", b"gone\n");
    write("staged.txt", b"one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write("logo.png", b"\x89PNG\r\n\x1a\n\0\0\0 fake");
    write("staged.txt", b"one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "Second commit\n\nWith a body."]);
    write("src/app.txt", lines(&[2, 3, 18]).as_bytes());
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    write("staged.txt", b"one\ntwo\nthree\n");
    git(&dir, &["add", "staged.txt"]);
    write("new.txt", b"new\n");
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().head_oid.unwrap()
}

fn open(st: &mut AppState, t: DiffTarget) -> HeaderModel {
    drive(st, Msg::Repo(RepoMsg::OpenDiff(t)));
    model(st).expect("a diff is open")
}

#[test]
fn an_unstaged_text_diff() {
    let (mut st, dir) = fixture("unstaged");
    let m = open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    assert_eq!((m.folder.as_str(), m.name.as_str(), m.status), ("src/", "app.txt", Some('M')));
    assert_eq!(m.kind, FileKind::Text);
    assert_eq!(m.change, ChangeText::Lines { added: 3, removed: 3 });
    assert_eq!(m.context, Context::Unstaged);
    assert_eq!(m.primary, Some(Primary::Stage("src/app.txt".into())));
    assert_eq!(m.actions.open, Some(OpenAction { path: "src/app.txt".into(), missing: None }));
    assert!(m.actions.restore.is_none() && m.actions.diff_tool.is_some());
    assert_eq!(m.view.unwrap().label(Lang::En), "Inline \u{b7} Hunks");
    st.settings.diff_layout = DiffLayout::Split;
    st.settings.diff_mode = DiffMode::WholeFile;
    let v = model(&st).unwrap().view.unwrap();
    assert_eq!(v.label(Lang::En), "Split \u{b7} Whole file");
    let de = v.label(Lang::De);
    assert!(de != v.label(Lang::En) && de.contains(" \u{b7} "), "{de}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn working_tree_targets() {
    let (mut st, dir) = fixture("worktree");
    let m = open(&mut st, DiffTarget::Unstaged("new.txt".into()));
    assert_eq!(m.status, Some('A'), "untracked shows as added");
    let m = open(&mut st, DiffTarget::Unstaged("gone.txt".into()));
    assert_eq!(m.actions.open.unwrap().missing, Some(crate::state::files::NOT_IN_WORKDIR), "deleted from the working tree");
    let m = open(&mut st, DiffTarget::Staged("staged.txt".into()));
    assert_eq!((m.primary, m.context), (Some(Primary::Unstage("staged.txt".into())), Context::Staged));
    assert_eq!(m.actions.open.unwrap().missing, None);

    let head = head(&st);
    let parent = st.snap.as_ref().unwrap().commits[1].oid;
    let m = open(&mut st, DiffTarget::Between(parent, head, "staged.txt".into()));
    assert!(m.actions.open.is_none() && m.actions.restore.is_none() && m.actions.diff_tool.is_some());
    assert_eq!(m.primary, None);
    assert_eq!(m.context, Context::Between(format!("{}..{}", &parent.to_string()[..7], &head.to_string()[..7])));
    let m = open(&mut st, DiffTarget::ToWorkdir(head, "src/app.txt".into()));
    assert!(m.actions.open.is_some() && m.primary.is_none());
    assert_eq!(m.context, Context::ToWorkdir(format!("{}..working tree", &head.to_string()[..7])));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn swapped_and_empty_tree_targets() {
    let (mut st, dir) = fixture("swapped");
    let head = head(&st);
    let short = head.to_string()[..7].to_owned();
    drive(&mut st, Msg::Selection(crate::msg::SelectionMsg::CompareWithWorkdir(head)));
    drive(&mut st, Msg::Selection(crate::msg::SelectionMsg::SwapCompare));
    let m = open(&mut st, DiffTarget::FromWorkdir(head, "src/app.txt".into()));
    assert_eq!(m.context, Context::FromWorkdir(format!("working tree..{short}")));
    assert_eq!((m.status, m.primary), (Some('M'), None));
    assert_eq!(m.actions.open, Some(OpenAction { path: "src/app.txt".into(), missing: None }));
    // Listed as added this way round: the working tree lacks it.
    let m = open(&mut st, DiffTarget::FromWorkdir(head, "gone.txt".into()));
    assert_eq!(m.status, Some('A'));
    assert_eq!(m.actions.open.unwrap().missing, Some(crate::state::files::NOT_IN_WORKDIR));

    let empty = trellis_core::repo::empty_tree();
    let m = open(&mut st, DiffTarget::Between(empty, head, "staged.txt".into()));
    assert_eq!(m.context, Context::Between(format!("empty..{short}")));
    let m = open(&mut st, DiffTarget::Between(head, empty, "staged.txt".into()));
    assert_eq!(m.context, Context::Between(format!("{short}..empty")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_targets_with_and_without_the_detail() {
    let (mut st, dir) = fixture("commit");
    let head = head(&st);
    let t = DiffTarget::Commit(head, "staged.txt".into());
    let m = open(&mut st, t.clone());
    assert!(m.actions.restore.is_none() && m.actions.open.is_none(), "no detail lists it");
    assert_eq!(m.status, None);
    assert_eq!(m.context, Context::Commit { short: head.to_string()[..7].into(), summary: Some("Second commit".into()) });
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)));
    let m = open(&mut st, t);
    assert_eq!(m.actions.restore, Some(RestoreAction { oid: head, path: "staged.txt".into(), status: 'M' }));
    assert_eq!(m.actions.open, Some(OpenAction { path: "staged.txt".into(), missing: None }));
    assert_eq!(m.status, Some('M'));
    assert_eq!(m.context, Context::Commit { short: head.to_string()[..7].into(), summary: Some("Second commit".into()) });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn binary_and_lfs_diffs_have_no_view_menu() {
    let (mut st, dir) = fixture("binary");
    let head = head(&st);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)));
    let t = DiffTarget::Commit(head, "logo.png".into());
    let m = open(&mut st, t.clone());
    assert_eq!((m.kind, m.view, m.status), (FileKind::Image, None, Some('A')));
    assert!(matches!(m.change, ChangeText::Sizes { old: None, new: Some(_) }), "{:?}", m.change);
    // While the sides load the chip shows the letter only.
    st.diff_preview.as_mut().unwrap().sides = None;
    assert_eq!(model(&st).unwrap().change, ChangeText::Sizes { old: None, new: None });
    let sides = BinarySides { old: Some(BinarySide { size: 10, image: None }), new: Some(BinarySide { size: 12, image: None }) };
    st.diff_preview.as_mut().unwrap().sides = Some(Ok(Arc::new(sides)));
    assert_eq!(model(&st).unwrap().change, ChangeText::Sizes { old: Some(10), new: Some(12) });

    let change = trellis_core::lfs::pointer::LfsChange {
        old: trellis_core::lfs::pointer::LfsSide::Absent,
        new: trellis_core::lfs::pointer::LfsSide::Regular { size: 3 },
        new_in_workdir: false,
        filtered: false,
    };
    let m = open(&mut st, DiffTarget::Commit(head, "staged.txt".into()));
    assert!(m.view.is_some());
    st.lfs.diff =
        Some(crate::state::lfs::LfsDiffNote { target: DiffTarget::Commit(head, "staged.txt".into()), change, show_pointer: false });
    assert_eq!(model(&st).unwrap().view, None, "an LFS summary");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_counter_follows_changes_and_files() {
    let (mut st, dir) = fixture("counter");
    drive(&mut st, Msg::Repo(RepoMsg::SelectWip));
    open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    st.view.diff_visible_rows = 10;
    assert_eq!(diff_nav::counter(&st), Some(Counter { index: 1, total: 2, files: false }));
    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    st.view.diff_top_row = st.view.diff_scroll_to_row.unwrap_or(st.view.diff_top_row);
    assert_eq!(diff_nav::counter(&st), Some(Counter { index: 2, total: 2, files: false }));

    // A binary file listed second of the unstaged files.
    std::fs::write(dir.join("b.bin"), b"\0\x01bin\0").unwrap();
    git(&dir, &["add", "b.bin"]);
    git(&dir, &["commit", "-q", "-m", "bin"]);
    std::fs::write(dir.join("b.bin"), b"\0\x02bin2\0").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let unstaged: Vec<_> = st.snap.as_ref().unwrap().unstaged.iter().map(|f| f.path.clone()).collect();
    let i = unstaged.iter().position(|p| p == "b.bin").unwrap();
    let total = unstaged.len() + st.snap.as_ref().unwrap().staged.len();
    open(&mut st, DiffTarget::Unstaged("b.bin".into()));
    assert_eq!(diff_nav::counter(&st), Some(Counter { index: i + 1, total, files: true }));

    // A comparison not on screen has no list.
    let head = head(&st);
    let parent = st.snap.as_ref().unwrap().commits[2].oid;
    open(&mut st, DiffTarget::Between(parent, head, "b.bin".into()));
    assert_eq!(diff_nav::counter(&st), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paths_split_at_the_last_slash() {
    assert_eq!(split_path("src/ui/app.rs"), ("src/ui/".into(), "app.rs".into()));
    assert_eq!(split_path("README"), (String::new(), "README".into()));
}
