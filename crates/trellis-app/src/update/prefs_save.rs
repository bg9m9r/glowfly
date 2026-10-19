//! Saving and reverting the open Preferences page (the Save / Revert bar and the "Save your
//! changes?" answers), and the "Saved" tick's clock. Git config is written with
//! `Effect::SetGitConfig`, profiles through `profiles::set_identity`, and Git Flow and sparse
//! checkout through their own jobs.

use crate::effect::Effect;
use crate::profile::IdentityField;
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage};
use std::time::Instant;

/// Once per frame: the "Saved" tick's strength.
pub(super) fn tick(st: &mut AppState, now: Instant) {
    let reduce_motion = st.settings.reduce_motion;
    if let Some(p) = st.prefs.as_mut() {
        p.saved.tick(now, reduce_motion);
    }
}

/// Writes the open page's edits; false when they were refused (a problem shown on the page, or
/// a job already running), and the page stays edited.
pub(super) fn save_page(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let Some(page) = st.prefs.as_ref().map(|p| p.page) else { return false };
    let saved = match page {
        PrefPage::General => {
            write_git(st, true, fx, |g| vec![("core.longpaths", Some(g.longpaths.to_string())), ("core.autocrlf", g.autocrlf.value())]);
            true
        }
        PrefPage::RepoIdentity => {
            write_git(st, false, fx, |g| vec![("user.name", text(&g.repo_name)), ("user.email", text(&g.repo_email))]);
            true
        }
        PrefPage::RepoHooks => {
            write_git(st, false, fx, |g| vec![("core.hooksPath", text(&g.hooks_path))]);
            true
        }
        PrefPage::Profile => {
            save_profile(st, fx);
            true
        }
        PrefPage::RepoGitFlow => super::gitflow::apply(st, fx),
        PrefPage::RepoSparse => super::sparse::apply(st, fx),
        _ => true,
    };
    if saved && let Some(p) = st.prefs.as_mut() {
        p.saved.mark();
    }
    saved
}

/// A trimmed field's value to write; None (unset) when it is empty.
fn text(value: &str) -> Option<String> {
    Some(value.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Writes each of `keys(git)` whose value differs from the one loaded, then makes the page's
/// fields and the loaded copy both hold what was written (trimmed).
fn write_git(
    st: &mut AppState,
    global: bool,
    fx: &mut Vec<Effect>,
    keys: impl Fn(&GitConfigValues) -> Vec<(&'static str, Option<String>)>,
) {
    let cwd = st.any_dir();
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(git) = p.git.as_mut() else { return };
    let before = p.git_saved.as_ref().map(&keys);
    for (i, (key, value)) in keys(git).into_iter().enumerate() {
        if before.as_ref().and_then(|b| b.get(i)).is_some_and(|(_, old)| *old == value) {
            continue;
        }
        fx.push(Effect::SetGitConfig { cwd: cwd.clone(), global, key: key.into(), value });
    }
    for field in [&mut git.repo_name, &mut git.repo_email, &mut git.hooks_path] {
        *field = field.trim().to_owned();
    }
    let saved = p.git_saved.get_or_insert_with(|| git.clone());
    copy_fields(p.page, git, saved);
}

/// Copies the fields `page` edits from `from` to `to`.
fn copy_fields(page: PrefPage, from: &GitConfigValues, to: &mut GitConfigValues) {
    match page {
        PrefPage::General => (to.longpaths, to.autocrlf) = (from.longpaths, from.autocrlf.clone()),
        PrefPage::RepoIdentity => (to.repo_name, to.repo_email) = (from.repo_name.clone(), from.repo_email.clone()),
        PrefPage::RepoHooks => to.hooks_path = from.hooks_path.clone(),
        _ => {}
    }
}

/// The selected profile's identity fields, each through `set_identity` (which writes the
/// current profile's to the global git config).
fn save_profile(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_ref() else { return };
    let Some(id) = p.profiles.selected else { return };
    let fields = [
        (IdentityField::AuthorName, p.profiles.author_name.clone()),
        (IdentityField::AuthorEmail, p.profiles.author_email.clone()),
        (IdentityField::SigningKey, p.profiles.signing_key.clone()),
    ];
    for (field, value) in fields {
        super::profiles::set_identity(st, id, field, value.trim(), fx);
    }
    if let Some(p) = st.prefs.as_mut() {
        p.profiles.fill(&st.settings, Some(id));
    }
}

/// Puts the open page's fields back to what was loaded.
pub(super) fn revert_page(st: &mut AppState) {
    let Some(p) = st.prefs.as_mut() else { return };
    match p.page {
        PrefPage::General | PrefPage::RepoIdentity | PrefPage::RepoHooks => {
            if let (Some(git), Some(saved)) = (p.git.as_mut(), p.git_saved.as_ref()) {
                copy_fields(p.page, saved, git);
            }
        }
        PrefPage::Profile => {
            let selected = p.profiles.selected;
            p.profiles.fill(&st.settings, selected);
        }
        PrefPage::RepoGitFlow => super::gitflow::discard(st),
        PrefPage::RepoSparse => p.sparse.fill(),
        _ => {}
    }
}

/// "Save your changes?" answered Save: save, then complete the leave. A refused save keeps
/// the page (with its problem shown) and drops the leave.
pub(super) fn leave_save(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let saved = save_page(st, fx);
    replay(st, saved, now, fx);
}

/// "Save your changes?" answered Discard: revert, then complete the leave.
pub(super) fn leave_discard(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    revert_page(st);
    replay(st, true, now, fx);
}

/// Runs the held leave (when `go`), letting it past the guard once.
fn replay(st: &mut AppState, go: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(m) = p.leaving.take().filter(|_| go) else { return };
    p.leave_approved = true;
    fx.extend(super::update(st, *m, now));
}
