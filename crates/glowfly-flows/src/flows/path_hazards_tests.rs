//! Rows Glowfly won't act on: a name that isn't UTF-8 refuses every file action, and a path that
//! shares one file on disk with another index path refuses the destructive ones, before any
//! effect or confirmation. Unstaging everything (or a folder, or picked rows) leaves such a name
//! staged and says so.

use super::tests::{finish, git};
use super::work_files_tests::{loaded, run};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, Collision, DiffTarget, FileEntry, Folding, Hazard, HunkOp, LoadOptions};
use glowfly_model::effect::{Effect, FilesEffect, LocalOp};
use glowfly_model::msg::{FilesMsg, HistoryMsg, Msg, RepoMsg, StageMsg, WorkFileMsg};
use glowfly_model::state::path_hazards::warnings;

const SHOWN: &str = "caf\\xE9.txt";

fn odd(path: &str, status: char, hazard: Hazard) -> FileEntry {
    FileEntry { hazard: Some(hazard), ..FileEntry::new(path, status) }
}

fn refused(st: &mut glowfly_model::state::AppState, msg: Msg) -> bool {
    st.toasts.clear();
    let fx = run(st, msg);
    fx.is_empty() && st.modal.is_none() && st.toasts.len() == 1 && !st.toasts[0].ok
}

#[test]
fn a_name_that_is_not_utf8_refuses_every_file_action() {
    let (mut st, dir) = loaded("hazard-utf8");
    st.snap.as_mut().unwrap().unstaged.push(odd(SHOWN, '?', Hazard::NotUtf8(b"caf\xe9.txt".to_vec())));
    let p = || SHOWN.to_owned();
    for msg in [
        Msg::Stage(StageMsg::StageFile(p())),
        Msg::WorkFile(WorkFileMsg::DiscardUntracked(p())),
        Msg::WorkFile(WorkFileMsg::DeleteFile { path: p(), confirmed: true }),
        Msg::WorkFile(WorkFileMsg::RenameFileDialog(p())),
        Msg::Files(FilesMsg::OpenWorkingFile(p())),
        Msg::Files(FilesMsg::ShowWorkingFile(p())),
        Msg::History(HistoryMsg::OpenFileHistory { path: p(), at: None }),
        Msg::Blame(glowfly_model::msg::BlameMsg::Open { path: p(), at: None }),
        Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged(p()))),
    ] {
        assert!(refused(&mut st, msg), "{:?}", st.toasts.last().map(|t| &t.text));
    }
    assert!(st.toasts.last().unwrap().text.contains("isn't UTF-8"));
    // Other files are unaffected.
    assert!(!run(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into()))).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shared_file_refuses_only_what_destroys_contents() {
    let (mut st, dir) = loaded("hazard-twins");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.push(odd("a.txt", 'M', Hazard::Twins(vec!["A.txt".into()])));
    snap.unstaged.retain(|f| f.hazard.is_some());
    let discard = Msg::Stage(StageMsg::DiscardFile("a.txt".into()));
    let confirm = Msg::confirm("Discard changes", "body".into(), "Discard", discard);
    assert!(refused(&mut st, confirm), "no confirmation opens");
    let hunk = Msg::Stage(StageMsg::Hunk { op: HunkOp::Discard, path: "a.txt".into(), index: 0, generation: 0 });
    assert!(refused(&mut st, hunk));
    assert!(refused(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "a.txt".into(), confirmed: false })));
    assert!(st.toasts.last().unwrap().text.contains("A.txt"));
    assert!(!run(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into()))).is_empty(), "staging still works");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_banner_says_where_twins_share_a_file() {
    let (mut st, dir) = loaded("hazard-banner");
    let snap = st.snap.as_mut().unwrap();
    assert!(warnings(Lang::En, snap).is_empty());
    snap.collisions = vec![Collision { paths: vec!["A.txt".into(), "a.txt".into()], case: true, unicode: false }];
    snap.ignorecase = false;
    snap.folding = Folding { case: Some(false), unicode: Some(false) };
    let w = warnings(Lang::En, snap);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("can't both be checked out") && w[0].contains("A.txt = a.txt"), "{w:?}");
    snap.folding = Folding { case: Some(true), unicode: Some(false) };
    let w = warnings(Lang::En, snap);
    assert!(w[0].contains("One file on this disk"), "{w:?}");
    assert!(w[1].contains("core.ignorecase is false"), "{w:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The local op in `fx`, if any.
fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::Files(FilesEffect::LocalOp { op, .. }) => Some(op),
        _ => None,
    })
}

#[test]
fn unstage_all_leaves_a_name_that_is_not_utf8_staged_and_says_so() {
    // Written into the index only, so it runs where such a file can't be made (Windows, macOS).
    let (mut st, dir) = loaded("hazard-unstage-all");
    git(&dir, &["add", "a.txt"]);
    let r = git2::Repository::open(&dir).unwrap();
    let mut index = r.index().unwrap();
    let base = index.get_path(std::path::Path::new("a.txt"), 0).unwrap();
    let path = b"caf\xe9.txt".to_vec();
    index.add(&git2::IndexEntry { flags: path.len() as u16, path, ..base }).unwrap();
    index.write().unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    let staged: Vec<_> = st.snap.as_ref().unwrap().staged.iter().map(|f| f.path.clone()).collect();
    assert_eq!(staged, ["a.txt", SHOWN], "both staged");

    let fx = run(&mut st, Msg::Stage(StageMsg::UnstageAll));
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["a.txt".into()])), "the shown name names no file: {fx:?}");
    assert!(
        matches!(&st.toasts[..], [t] if !t.ok && t.text.contains("1 file whose name isn't UTF-8 stays staged")),
        "{:?}",
        st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    finish(&mut st, fx);
    let index = git2::Repository::open(&dir).unwrap().index().unwrap();
    let head = r.head().unwrap().peel_to_tree().unwrap().get_name("a.txt").unwrap().id();
    assert_eq!(index.get_path(std::path::Path::new("a.txt"), 0).unwrap().id, head, "a.txt is unstaged");
    assert!(index.iter().any(|e| e.path == b"caf\xe9.txt"), "the odd name is still staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_a_folder_or_picked_rows_says_which_names_stay_staged() {
    let (mut st, dir) = loaded("hazard-unstage-picked");
    let snap = st.snap.as_mut().unwrap();
    snap.staged = vec![FileEntry::new("src/a.txt", 'M'), odd("src/caf\\xE9.txt", 'A', Hazard::NotUtf8(b"src/caf\xe9.txt".to_vec()))];
    for msg in [
        Msg::Stage(StageMsg::UnstageFolder("src".into())),
        Msg::Stage(StageMsg::UnstagePaths(vec!["src/a.txt".into(), "src/caf\\xE9.txt".into()])),
    ] {
        st.toasts.clear();
        let fx = run(&mut st, msg);
        assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["src/a.txt".into()])), "{fx:?}");
        assert!(
            matches!(&st.toasts[..], [t] if !t.ok && t.text.contains("isn't UTF-8")),
            "{:?}",
            st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }
    st.toasts.clear();
    run(&mut st, Msg::Stage(StageMsg::UnstagePaths(vec!["src/a.txt".into()])));
    assert!(st.toasts.is_empty(), "nothing left behind, nothing said");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_banner_says_when_submodules_are_not_shown() {
    let (mut st, dir) = loaded("hazard-gitmodules");
    assert!(warnings(Lang::En, st.snap.as_ref().unwrap()).is_empty());
    std::fs::write(dir.join(".gitmodules"), "[submodule \"a\"]\n\tpath = a\n\turl = ./a\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    assert!(warnings(Lang::En, st.snap.as_ref().unwrap()).is_empty(), "a plain file");
    std::fs::write(dir.join(".gitmodules"), "[submodule \"a\"]\n\tpath = a\n\turl = ./a\n[include]\n\tpath = x\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    let w = warnings(Lang::En, st.snap.as_ref().unwrap());
    assert_eq!(w, ["Submodules aren't shown: a .gitmodules file here or in a submodule isn't a plain file Glowfly can read."]);
    let _ = std::fs::remove_dir_all(&dir);
}
