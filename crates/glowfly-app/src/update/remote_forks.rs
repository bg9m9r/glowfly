//! View a remote on its hosting service, and the fork dialogs: Add a fork (list the
//! repository's forks, pick one, add it as a remote) and Fork (find the user's own fork or fork
//! into their account, then add and fetch it). The service calls are unscoped effects whose
//! results are matched against the open dialog's repository and target; adding a remote is a
//! plain git job through `ops::start_job`.

use super::ops::{JobSpec, start_job};
use super::remotes::configured;
use crate::effect::OsEffect;
use crate::effect::{Effect, ForgeError};
use crate::remote::{ForkRequest, RemoteEffect};
use crate::state::forks::{
    ForkAction, ForkCreate, ForkPicker, ForkStep, add_fork_label, add_job, added_as, error_text, fork_target, suggest,
};
use crate::state::remotes::{Job, find, gone_text};
use crate::state::{AppState, Modal};
use glowfly_core::forge::{self, CreatedFork, ForkList, ForkLookup, Target};
use glowfly_core::paths::same_spelling;
use glowfly_core::remote_ops::{add_cmds, check_remote_name, check_url};
use glowfly_core::repo::Snapshot;
use std::path::{Path, PathBuf};

/// Opens `name`'s page on its service, or says why there is none.
pub(super) fn view(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(info) = configured(st, name) else { return };
    match forge::web_page(info.fetch_url(), &st.settings.connections) {
        Ok(page) => fx.push(Effect::Os(OsEffect::OpenUrl(page.url))),
        Err(e) => st.log(false, format!("Can't open {name}: {}", e.message())),
    }
}

/// A request for `target` with this session's credential for it, if any.
fn req(st: &AppState, workdir: PathBuf, target: Target) -> ForkRequest {
    let account = target.account();
    let auth = st.forge.as_ref().and_then(|f| f.auth.get(&account.key())).cloned();
    let connected = forge::is_connected(&account, &st.settings.connections);
    ForkRequest { workdir, target, auth, connected }
}

/// The remote's repository for `action`, and the active repository, or None after logging why.
fn source(st: &mut AppState, name: &str, action: ForkAction) -> Option<(PathBuf, Target)> {
    let info = configured(st, name)?;
    match fork_target(&info, &st.settings.connections, action) {
        Ok(t) => Some((st.snap.as_ref()?.workdir.clone(), t)),
        Err(e) => {
            st.log(false, e);
            None
        }
    }
}

fn picker<'a>(st: &'a mut AppState, workdir: &Path, target: &Target) -> Option<&'a mut ForkPicker> {
    match st.modal.as_mut() {
        Some(Modal::AddFork(p)) if same_spelling(&p.workdir, workdir) && p.target == *target => Some(p),
        _ => None,
    }
}

fn creator<'a>(st: &'a mut AppState, workdir: &Path, target: &Target) -> Option<&'a mut ForkCreate> {
    match st.modal.as_mut() {
        Some(Modal::CreateFork(f)) if same_spelling(&f.workdir, workdir) && f.target == *target => Some(f),
        _ => None,
    }
}

fn names(snap: &Snapshot) -> Vec<&str> {
    snap.remote_info.iter().map(|r| r.name.as_str()).collect()
}

/// Runs an add-remote job; the dialog closes once it starts (busy: start_job said why).
fn start_and_close(st: &mut AppState, job: Result<Job, String>, fx: &mut Vec<Effect>) {
    match job {
        Err(e) => st.log(false, e),
        Ok((label, cmds)) => {
            let before = fx.len();
            start_job(st, JobSpec::new(label, cmds), fx);
            if fx.len() > before {
                st.modal = None;
            }
        }
    }
}

pub(super) fn open_picker(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some((workdir, target)) = source(st, name, ForkAction::List) else { return };
    st.modal = Some(Modal::AddFork(Box::new(ForkPicker {
        workdir: workdir.clone(),
        remote: name.to_owned(),
        target: target.clone(),
        loading: true,
        list: ForkList::default(),
        error: None,
        filter: String::new(),
        picked: None,
        name: String::new(),
        suggested: String::new(),
        fetch_after: true,
        touched: Default::default(),
    })));
    fx.push(Effect::Remote(RemoteEffect::ListForks(req(st, workdir, target))));
}

pub(super) fn forks_loaded(st: &mut AppState, workdir: &Path, target: &Target, result: Result<ForkList, ForgeError>) {
    let Some(p) = picker(st, workdir, target) else { return };
    p.loading = false;
    match result {
        Ok(list) => {
            p.list = list;
            p.picked = None;
            p.error = None;
        }
        Err(e) => p.error = Some(e),
    }
}

pub(super) fn pick(st: &mut AppState, index: usize) {
    let (Some(snap), Some(Modal::AddFork(p))) = (st.snap.as_ref(), st.modal.as_mut()) else { return };
    let Some(fork) = p.list.forks.get(index) else { return };
    if added_as(snap, fork, &st.settings.connections).is_some() {
        return;
    }
    p.picked = Some(index);
    // The name follows the pick until the user types their own.
    if p.name == p.suggested {
        p.suggested = suggest(snap, &fork.owner);
        p.name = p.suggested.clone();
    }
}

pub(super) fn add_fork(st: &mut AppState, fx: &mut Vec<Effect>) {
    let job = {
        let (Some(snap), Some(Modal::AddFork(p))) = (st.snap.as_ref(), st.modal.as_ref()) else { return };
        let Some(fork) = p.picked.and_then(|i| p.list.forks.get(i)) else { return };
        match find(snap, &p.remote) {
            None => Err(gone_text(&p.remote)),
            Some(src) => add_job(snap, src.fetch_url(), fork, &p.name, p.fetch_after, &st.settings.connections),
        }
    };
    start_and_close(st, job, fx);
}

pub(super) fn open_create(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some((workdir, target)) = source(st, name, ForkAction::Create) else { return };
    st.modal = Some(Modal::CreateFork(Box::new(ForkCreate {
        workdir: workdir.clone(),
        remote: name.to_owned(),
        target: target.clone(),
        step: ForkStep::Checking,
        name: String::new(),
        fetch_after: true,
        error: None,
        touched: Default::default(),
    })));
    fx.push(Effect::Remote(RemoteEffect::FindMyFork(req(st, workdir, target))));
}

pub(super) fn my_fork_found(st: &mut AppState, workdir: &Path, target: &Target, result: Result<ForkLookup, ForgeError>) {
    let existing: Vec<String> = st.snap.as_ref().map(|s| names(s).into_iter().map(str::to_owned).collect()).unwrap_or_default();
    let existing: Vec<&str> = existing.iter().map(String::as_str).collect();
    let Some(f) = creator(st, workdir, target) else { return };
    let lookup = match result {
        Ok(l) => l,
        Err(e) => {
            f.error = Some(e);
            return;
        }
    };
    f.error = None;
    let user = lookup.user;
    f.step = match lookup.mine {
        _ if lookup.own_repository => ForkStep::OwnRepo { user },
        Some(fork) => {
            f.name = forge::remote_name_for(&fork.owner, &existing);
            ForkStep::Mine { user, fork }
        }
        None => {
            f.name = forge::remote_name_for(&user, &existing);
            ForkStep::Ready { user }
        }
    };
}

pub(super) fn add_my_fork(st: &mut AppState, fx: &mut Vec<Effect>) {
    let job = {
        let (Some(snap), Some(Modal::CreateFork(f))) = (st.snap.as_ref(), st.modal.as_ref()) else { return };
        let ForkStep::Mine { fork, .. } = &f.step else { return };
        match find(snap, &f.remote) {
            None => Err(gone_text(&f.remote)),
            Some(src) => add_job(snap, src.fetch_url(), fork, &f.name, f.fetch_after, &st.settings.connections),
        }
    };
    start_and_close(st, job, fx);
}

pub(super) fn create(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (workdir, target, name) = {
        let (Some(snap), Some(Modal::CreateFork(f))) = (st.snap.as_ref(), st.modal.as_ref()) else { return };
        if !matches!(f.step, ForkStep::Ready { .. }) {
            return;
        }
        let name = f.name.trim().to_owned();
        if let Err(p) = check_remote_name(&name, &names(snap), None) {
            let text = p.message();
            return st.log(false, text);
        }
        (f.workdir.clone(), f.target.clone(), name)
    };
    let request = req(st, workdir, target);
    let Some(Modal::CreateFork(f)) = st.modal.as_mut() else { return };
    let ForkStep::Ready { user } = &f.step else { return };
    f.step = ForkStep::Creating { user: user.clone() };
    f.error = None;
    fx.push(Effect::Remote(RemoteEffect::CreateFork { req: request, name }));
}

pub(super) fn fork_created(
    st: &mut AppState,
    workdir: PathBuf,
    target: &Target,
    name: String,
    result: Result<CreatedFork, ForgeError>,
    fx: &mut Vec<Effect>,
) {
    let service = target.provider.name();
    let created = match result {
        Ok(c) => c,
        Err(e) => {
            match creator(st, &workdir, target) {
                Some(f) => {
                    let user = match &f.step {
                        ForkStep::Creating { user } | ForkStep::Ready { user } => user.clone(),
                        _ => String::new(),
                    };
                    f.step = ForkStep::Ready { user };
                    f.error = Some(e);
                }
                None => st.log(false, format!("Forking {} on {service} failed: {}", target.display(), error_text(&e))),
            }
            return;
        }
    };
    if creator(st, &workdir, target).is_some() {
        st.modal = None;
    }
    let fork = created.fork;
    let link = if fork.web_url.is_empty() { String::new() } else { format!(" ({})", fork.web_url) };
    st.log(true, format!("Created the fork {} on {service}{link}.", fork.full_name));
    // The remote it came from, when that repository is open: its URL style, and a name check.
    let connections = &st.settings.connections;
    let active = st.snap.as_ref().filter(|s| same_spelling(&s.workdir, &workdir));
    let src = active.and_then(|s| s.remote_info.iter().find(|r| forge::parse_remote(r.fetch_url(), connections).as_ref() == Some(target)));
    let (src_name, src_url) = src.map(|r| (r.name.clone(), r.fetch_url().to_owned())).unwrap_or_default();
    if active.is_some_and(|s| check_remote_name(&name, &names(s), None).is_err()) {
        let how = if src_name.is_empty() { "Add remote…".to_owned() } else { add_fork_label(&src_name) };
        return st.log(false, format!("Created the fork {}, but the remote name {name} is taken now. Add it with {how}.", fork.full_name));
    }
    let url = fork.url_like(&src_url).to_owned();
    if let Err(p) = check_url(&url) {
        return st.log(false, p.message(false));
    }
    // It runs in the forked repository even if another tab is active now, as that repository's
    // trust decision allows (update/trust.rs).
    let spec = JobSpec { cwd: Some(workdir), ..JobSpec::new(format!("Add remote {name}"), add_cmds(&name, &url, None, created.ready)) };
    start_job(st, spec, fx);
    if !created.ready {
        st.log(true, format!("{service} is still preparing the fork; fetch {name} in a minute."));
    }
}

pub(super) fn retry(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (workdir, target, list) = match st.modal.as_mut() {
        Some(Modal::AddFork(p)) if !p.loading => {
            p.loading = true;
            p.error = None;
            (p.workdir.clone(), p.target.clone(), true)
        }
        Some(Modal::CreateFork(f)) if f.step == ForkStep::Checking && f.error.is_some() => {
            f.error = None;
            (f.workdir.clone(), f.target.clone(), false)
        }
        _ => return,
    };
    let request = req(st, workdir, target);
    fx.push(Effect::Remote(if list { RemoteEffect::ListForks(request) } else { RemoteEffect::FindMyFork(request) }));
}
