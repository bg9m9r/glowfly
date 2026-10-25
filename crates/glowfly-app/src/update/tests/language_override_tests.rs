//! The diff viewer highlights files in the language their `.gitattributes` name
//! (`linguist-language`), follows edits to that file on a reload, and highlights `.jsx`.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::AppState;
use glowfly_core::repo::DiffTarget;
use std::path::{Path, PathBuf};

/// The syntax of the diff on screen; `None` when it is plain or nothing is open.
fn syntax(st: &AppState) -> Option<String> {
    st.diff.as_ref().and_then(|(_, d)| d.highlight.as_ref()).map(|h| h.syntax.clone())
}

/// A repository with `.gitattributes` mapping `*.inc` to Rust and `lib.inc` committed, then
/// edited in the working tree.
fn repo_with_override(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::write(dir.join(".gitattributes"), "*.inc linguist-language=Rust\n").unwrap();
    std::fs::write(dir.join("lib.inc"), "fn main() {}\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "inc"]);
    std::fs::write(dir.join("lib.inc"), "fn main() { let y = 2; }\n").unwrap();
    dir
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

#[test]
fn commit_and_working_diffs_use_gitattributes_language() {
    let dir = repo_with_override("lang-override");
    let mut st = opened(&dir);
    let head = st.snap.as_ref().unwrap().commits[0].oid;

    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(head, "lib.inc".into()))));
    assert_eq!(syntax(&st).as_deref(), Some("Rust"), "the commit's diff");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("lib.inc".into()))));
    assert_eq!(syntax(&st).as_deref(), Some("Rust"), "the working tree's diff");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editing_gitattributes_changes_language_on_reload() {
    let dir = repo_with_override("lang-reload");
    let mut st = opened(&dir);
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("lib.inc".into()))));
    assert_eq!(syntax(&st).as_deref(), Some("Rust"));

    std::fs::write(dir.join(".gitattributes"), "*.inc linguist-language=Python\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.diff.as_ref().map(|(t, _)| t.path()), Some("lib.inc"), "still open");
    assert_eq!(syntax(&st).as_deref(), Some("Python"), "the Rust spans are not adopted");

    // Out of the index too: with no working tree copy, the index's attributes apply.
    git(&dir, &["rm", "-q", "-f", ".gitattributes"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(syntax(&st).as_deref(), Some("x86_64 Assembly"), "back to detection by extension");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jsx_working_diff_is_highlighted() {
    let dir = scratch_repo("lang-jsx");
    std::fs::write(dir.join("App.jsx"), "const App = () => <div />;\n").unwrap();
    git(&dir, &["add", "App.jsx"]);
    git(&dir, &["commit", "-q", "-m", "jsx"]);
    std::fs::write(dir.join("App.jsx"), "const App = () => <div className=\"x\">{1}</div>;\n").unwrap();
    let mut st = opened(&dir);
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("App.jsx".into()))));
    assert_eq!(syntax(&st).as_deref(), Some("TypeScriptReact"));
    let _ = std::fs::remove_dir_all(&dir);
}
