//! The Clone dialog's Your repositories tab: loading the chosen account's repositories (once
//! per account, again on Retry), dropping answers for an account no longer chosen or a dialog
//! since closed, and filling the URL from the picked repository.

use crate::effect::{CloneEffect, Effect, ForgeError};
use crate::state::clone::CloneForm;
use crate::state::clone_service::{CloneSource, url_for};
use crate::state::{AppState, Modal};
use trellis_core::forge::{Account, MyRepos};

fn form(st: &mut AppState) -> Option<&mut CloneForm> {
    match st.modal.as_mut() {
        Some(Modal::Clone(f)) => Some(f),
        _ => None,
    }
}

/// Switches the dialog's tab; the Your repositories tab loads the chosen account's list the
/// first time it shows.
pub(super) fn source(st: &mut AppState, s: CloneSource, fx: &mut Vec<Effect>) {
    let Some(f) = form(st) else { return };
    f.source = s;
    if s == CloneSource::Service && f.service.needs_load() {
        load(st, fx);
    }
}

pub(super) fn account(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(f) = form(st) else { return };
    let p = &mut f.service;
    if i >= p.accounts.len() || i == p.account {
        return;
    }
    p.account = i;
    p.list = MyRepos::default();
    p.error = None;
    p.picked = None;
    p.filter.clear();
    p.loaded = None;
    // A load still running for the previous account no longer counts.
    p.loading = false;
    load(st, fx);
}

pub(super) fn retry(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(f) = form(st) else { return };
    f.service.loaded = None;
    f.service.loading = false;
    load(st, fx);
}

/// Asks for the chosen account's repositories.
fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let cwd = st.any_dir();
    let Some(f) = form(st) else { return };
    let p = &mut f.service;
    let Some(account) = p.selected().cloned() else { return };
    p.loading = true;
    p.error = None;
    p.loaded = Some(account.clone());
    fx.push(Effect::Clone(CloneEffect::ListRepos { account, cwd }));
}

/// The answer for `account`, applied only while the dialog is open with that account chosen.
pub(super) fn loaded(st: &mut AppState, account: Account, result: Result<MyRepos, ForgeError>) {
    let Some(f) = form(st) else { return };
    let p = &mut f.service;
    if p.selected() != Some(&account) || p.loaded.as_ref() != Some(&account) {
        return;
    }
    p.loading = false;
    match result {
        Ok(list) => {
            p.list = list;
            p.picked = None;
        }
        Err(e) => p.error = Some(e),
    }
}

/// Picks repository `i` and fills the URL with its clone URL.
pub(super) fn pick(st: &mut AppState, i: usize) {
    let Some(f) = form(st) else { return };
    let Some(r) = f.service.list.repos.get(i) else { return };
    let url = url_for(r, f.service.ssh).to_owned();
    f.service.picked = Some(i);
    f.service.filled = url.clone();
    f.url = url;
}

/// Switches between the SSH and https URL, filling it in again while the URL is the picked one.
pub(super) fn use_ssh(st: &mut AppState, ssh: bool) {
    let Some(f) = form(st) else { return };
    let still_picked = f.service.picked_repo(&f.url).is_some();
    f.service.ssh = ssh;
    if still_picked && let Some(i) = f.service.picked {
        pick(st, i);
    }
}
