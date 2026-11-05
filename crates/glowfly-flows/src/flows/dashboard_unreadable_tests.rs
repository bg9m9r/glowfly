//! The dashboard's unreadable folders in the user's language (#367): a scan's `ReadFailed`
//! worded short by `typed::unreadable` (not a repository, owned by another user), the system's
//! own words kept, and a missing folder's reason.

use super::dashboard_tests::{deliver, path, run};
use super::tests::{said, state};
use glowfly_core::error::ReadFailed;
use glowfly_model::msg::DashboardMsg;
use std::path::PathBuf;

/// The dashboard's unreadable list in `code` after a scan that couldn't read three folders,
/// with one recent folder missing.
fn unreadable(code: &str) -> Vec<(PathBuf, String)> {
    let mut st = state();
    st.settings.language = code.into();
    let (a, gone) = (path("a"), path("gone"));
    st.recent = vec![a, gone.clone()];
    st.missing_repos = vec![gone];
    run(&mut st, DashboardMsg::Open);
    let shown = |p: &PathBuf| glowfly_core::native_path::shown(p);
    let (plain, theirs, locked) = (path("plain"), path("theirs"), path("locked"));
    deliver(
        &mut st,
        vec![
            (plain.clone(), Err(ReadFailed::NotARepository { path: shown(&plain) })),
            (theirs.clone(), Err(ReadFailed::NotOwned { root: shown(&theirs) })),
            (locked, Err(said("Access is denied."))),
        ],
    );
    st.dashboard.unreadable.clone()
}

fn reasons(code: &str) -> Vec<String> {
    unreadable(code).into_iter().map(|(_, why)| why).collect()
}

#[test]
fn unreadable_folders_read_short_in_each_language() {
    assert_eq!(reasons("en"), ["missing", "not a Git repository", "owned by another user", "Access is denied."]);
    assert_eq!(reasons("de"), ["fehlt", "kein Git-Repository", "gehört einem anderen Benutzer", "Access is denied."]);
    assert_eq!(reasons("es"), ["no se encuentra", "no es un repositorio Git", "pertenece a otro usuario", "Access is denied."]);
}

#[test]
fn each_reason_stays_with_its_folder() {
    let paths: Vec<PathBuf> = unreadable("de").into_iter().map(|(p, _)| p).collect();
    assert_eq!(paths, [path("gone"), path("plain"), path("theirs"), path("locked")]);
}
