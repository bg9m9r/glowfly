//! Rows Trellis won't act on: a name that isn't UTF-8 refuses every file action, and a path that
//! shares one file on disk with another index path refuses the destructive ones, before any
//! effect or confirmation.

use super::work_files_tests::{loaded, run};
use crate::msg::{FilesMsg, HistoryMsg, Msg, RepoMsg, StageMsg, WorkFileMsg};
use crate::state::path_hazards::warnings;
use trellis_core::repo::{Collision, DiffTarget, FileEntry, Folding, Hazard, HunkOp};

const SHOWN: &str = "caf\\xE9.txt";

fn odd(path: &str, status: char, hazard: Hazard) -> FileEntry {
    FileEntry { hazard: Some(hazard), ..FileEntry::new(path, status) }
}

fn refused(st: &mut crate::state::AppState, msg: Msg) -> bool {
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
        Msg::Blame(crate::msg::BlameMsg::Open { path: p(), at: None }),
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
    assert!(warnings(snap).is_empty());
    snap.collisions = vec![Collision { paths: vec!["A.txt".into(), "a.txt".into()], case: true, unicode: false }];
    snap.ignorecase = false;
    snap.folding = Folding { case: Some(false), unicode: Some(false) };
    let w = warnings(snap);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("can't both be checked out") && w[0].contains("A.txt = a.txt"), "{w:?}");
    snap.folding = Folding { case: Some(true), unicode: Some(false) };
    let w = warnings(snap);
    assert!(w[0].contains("One file on this disk"), "{w:?}");
    assert!(w[1].contains("core.ignorecase is false"), "{w:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
