//! Saving and reverting the open Preferences page (the Save / Revert bar, Ctrl/Cmd+S and Esc,
//! and the "Save your changes?" answers), and the "Saved" tick's clock. A save's git config
//! values go out as one `Effect::Prefs(PrefsEffect::WriteGitConfig)` (`batch`), and the tick
//! waits for it to report success (`written`); a refused write keeps the page edited and shows
//! git's error on it. Profiles write through `profiles::set_identity`, and Git Flow and sparse
//! checkout through their own jobs.

use crate::effect::Effect;
use crate::effect::{ConfigWrite, PrefsEffect};
use crate::msg::Msg;
use crate::profile::{IdentityField, ProfileMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::prefs_save::{PendingSave, saving, unsaved};
use crate::state::{AppState, PrefPage};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Once per frame: the "Saved" tick's strength.
pub(super) fn tick(st: &mut AppState, now: Instant) {
    let reduce_motion = st.settings.reduce_motion;
    if let Some(p) = st.prefs.as_mut() {
        p.saved.tick(now, reduce_motion);
    }
}

/// Writes the open page's edits; false when they were refused (a problem shown on the page, or
/// a job or an earlier save still running), and the page stays edited. A page without edits
/// writes nothing.
pub(super) fn save_page(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let Some(p) = st.prefs.as_ref() else { return false };
    let page = p.page;
    if saving(p) {
        return false;
    }
    if !unsaved(p, &st.settings, st.snap.as_ref()) {
        return true;
    }
    if let Some(p) = st.prefs.as_mut()
        && p.save_error.as_ref().is_some_and(|(at, _)| *at == page)
    {
        p.save_error = None;
    }
    let start = fx.len();
    let mut written = None;
    let saved = match page {
        PrefPage::General => {
            written =
                write_git(st, true, fx, |g| vec![("core.longpaths", Some(g.longpaths.to_string())), ("core.autocrlf", g.autocrlf.value())]);
            true
        }
        PrefPage::RepoIdentity => {
            written = write_git(st, false, fx, |g| vec![("user.name", text(&g.repo_name)), ("user.email", text(&g.repo_email))]);
            true
        }
        PrefPage::RepoHooks => {
            written = write_git(st, false, fx, |g| vec![("core.hooksPath", text(&g.hooks_path))]);
            true
        }
        PrefPage::Signing => {
            written = write_git(st, true, fx, |g| {
                vec![
                    ("gpg.program", text(&g.gpg_program)),
                    ("gpg.ssh.program", text(&g.ssh_program)),
                    ("gpg.x509.program", text(&g.x509_program)),
                    ("gpg.ssh.allowedSignersFile", text(&g.ssh_allowed_signers)),
                ]
            });
            // The key list follows the program.
            super::prefs::load_signing_keys(st, fx);
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
    let Some(p) = st.prefs.as_mut() else { return saved };
    if let Some(ticket) = batch(fx, start) {
        // A settings save on the way (a profile's) marked the tick: it waits for git too.
        p.saved.pending = false;
        p.saving.push(PendingSave { ticket, page, written });
    } else if saved {
        if let Some(w) = written {
            let loaded = p.git_saved.get_or_insert_with(|| w.clone());
            copy_fields(page, &w, loaded);
        }
        p.saved.mark();
    }
    saved
}

/// Writes one value at once (a Commit signing switch or list) and shows "Saved" once git has.
pub(super) fn write_now(st: &mut AppState, global: bool, key: String, value: Option<String>, fx: &mut Vec<Effect>) {
    let cwd = st.any_dir();
    let Some(p) = st.prefs.as_mut() else { return };
    let ticket = next_ticket();
    fx.push(Effect::Prefs(PrefsEffect::WriteGitConfig { cwd, ticket, writes: vec![ConfigWrite { global, key, value }] }));
    p.saving.push(PendingSave { ticket, page: p.page, written: None });
}

/// A save's writes finished: on success the loaded copy takes what was written and the tick
/// shows; a refusal stays on its page (the edits kept) and goes to the activity log, with a
/// toast when its page isn't the one open.
pub(super) fn written(st: &mut AppState, ticket: u64, result: Result<(), String>, now: Instant) {
    let done = st.prefs.as_mut().and_then(|p| p.saving.iter().position(|s| s.ticket == ticket).map(|i| p.saving.remove(i)));
    match result {
        Ok(()) => {
            let (Some(p), Some(done)) = (st.prefs.as_mut(), done) else { return };
            if let Some(w) = done.written {
                let loaded = p.git_saved.get_or_insert_with(|| w.clone());
                copy_fields(done.page, &w, loaded);
            }
            if p.save_error.as_ref().is_some_and(|(at, _)| *at == done.page) {
                p.save_error = None;
            }
            p.saved.mark();
        }
        Err(e) => {
            let text = st.settings.lang().trf("prefs.save.failed", &[("error", &e)]);
            st.log(false, text.clone());
            let page = done.map(|d| d.page);
            let open = st.prefs.as_ref().map(|p| p.page);
            if let (Some(p), Some(page)) = (st.prefs.as_mut(), page) {
                p.save_error = Some((page, e));
            }
            if page.is_none() || open != page {
                st.toast(false, text, now);
            }
        }
    }
}

/// A new save's ticket: unique for the whole run, so a write that outlives its Preferences
/// window never answers a later one.
fn next_ticket() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Gathers the single config writes pushed onto `fx` since `start` into one batch, where the
/// first of them was; its ticket, or None when there were none.
fn batch(fx: &mut Vec<Effect>, start: usize) -> Option<u64> {
    let mut writes = Vec::new();
    let mut at = None;
    let mut cwd = None;
    let mut i = start;
    while i < fx.len() {
        if !matches!(fx[i], Effect::Prefs(PrefsEffect::SetGitConfig { .. })) {
            i += 1;
            continue;
        }
        if let Effect::Prefs(PrefsEffect::SetGitConfig { cwd: dir, global, key, value }) = fx.remove(i) {
            at.get_or_insert(i);
            cwd.get_or_insert(dir);
            writes.push(ConfigWrite { global, key, value });
        }
    }
    let (at, cwd) = (at?, cwd?);
    let ticket = next_ticket();
    fx.insert(at, Effect::Prefs(PrefsEffect::WriteGitConfig { cwd, ticket, writes }));
    Some(ticket)
}

/// A trimmed field's value to write; None (unset) when it is empty.
fn text(value: &str) -> Option<String> {
    Some(value.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Writes each of `keys(git)` whose value differs from the one loaded, trimming the page's text
/// fields; returns the fields as written, for the loaded copy once the write succeeds.
fn write_git(
    st: &mut AppState,
    global: bool,
    fx: &mut Vec<Effect>,
    keys: impl Fn(&GitConfigValues) -> Vec<(&'static str, Option<String>)>,
) -> Option<GitConfigValues> {
    let cwd = st.any_dir();
    let p = st.prefs.as_mut()?;
    let git = p.git.as_mut()?;
    let before = p.git_saved.as_ref().map(&keys);
    for (i, (key, value)) in keys(git).into_iter().enumerate() {
        if before.as_ref().and_then(|b| b.get(i)).is_some_and(|(_, old)| *old == value) {
            continue;
        }
        fx.push(Effect::Prefs(PrefsEffect::SetGitConfig { cwd: cwd.clone(), global, key: key.into(), value }));
    }
    let fields = [
        &mut git.repo_name,
        &mut git.repo_email,
        &mut git.hooks_path,
        &mut git.gpg_program,
        &mut git.ssh_program,
        &mut git.x509_program,
        &mut git.ssh_allowed_signers,
    ];
    for field in fields {
        *field = field.trim().to_owned();
    }
    Some(git.clone())
}

/// Copies the fields `page` edits from `from` to `to`.
fn copy_fields(page: PrefPage, from: &GitConfigValues, to: &mut GitConfigValues) {
    match page {
        PrefPage::General => (to.longpaths, to.autocrlf) = (from.longpaths, from.autocrlf.clone()),
        PrefPage::RepoIdentity => (to.repo_name, to.repo_email) = (from.repo_name.clone(), from.repo_email.clone()),
        PrefPage::RepoHooks => to.hooks_path = from.hooks_path.clone(),
        PrefPage::Signing => {
            to.gpg_program = from.gpg_program.clone();
            to.ssh_program = from.ssh_program.clone();
            to.x509_program = from.x509_program.clone();
            to.ssh_allowed_signers = from.ssh_allowed_signers.clone();
        }
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

/// Puts the open page's fields back to what was loaded, and forgets its refused save. Not while
/// its save is being written.
pub(super) fn revert_page(st: &mut AppState) {
    let Some(p) = st.prefs.as_mut() else { return };
    if saving(p) {
        return;
    }
    if p.save_error.as_ref().is_some_and(|(at, _)| *at == p.page) {
        p.save_error = None;
    }
    match p.page {
        PrefPage::General | PrefPage::RepoIdentity | PrefPage::RepoHooks | PrefPage::Signing => {
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
/// the page (with its problem shown) and drops the leave. A held switch to a profile other
/// than the edited one starts first, so the edited profile's identity waits for it
/// (`profiles::set_identity`) rather than racing its writes to the global git config.
pub(super) fn leave_save(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if switches_elsewhere(st) {
        replay(st, true, now, fx);
        save_page(st, fx);
        return;
    }
    let saved = save_page(st, fx);
    replay(st, saved, now, fx);
}

/// Whether the held leave switches to a profile other than the one the page edits.
fn switches_elsewhere(st: &AppState) -> bool {
    let Some(p) = st.prefs.as_ref() else { return false };
    matches!(p.leaving.as_deref(), Some(Msg::Profile(ProfileMsg::Switch(id))) if p.profiles.selected != Some(*id))
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
    fx.extend(super::update_sealed(st, *m, now));
}
