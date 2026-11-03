//! The Clone dialog: opening it (Clone submodules too follows Preferences > General), checking
//! its parent folder on disk as the user types (the view only reads the answer), its Your
//! repositories tab (`clone_service`), starting the clone and the end of a sparse one (its folders
//! or, after them, its submodules may fail). A plain clone is a git job; a sparse one is
//! `CloneEffect::Sparse`.

use super::jobs::{JobSpec, start_job};
use crate::effect::CloneEffect;
use crate::effect::Effect;
use crate::msg::{CloneMsg, Msg};
use crate::state::clone::{CloneForm, CloneRequest};
use crate::state::clone_service::listing_accounts;
use crate::state::trust::Created;
use crate::state::{AppState, Modal};
use glowfly_core::clone_ops::{clone_cmd, sparse_submodule_cmds};
use std::time::{Duration, Instant};

/// How long after an edit the parent folder is checked. Each edit asks again after this pause;
/// answers for text that has changed since are dropped.
pub const PARENT_CHECK_DELAY: Duration = Duration::from_millis(250);

pub(super) fn handle(st: &mut AppState, msg: CloneMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        CloneMsg::Dialog => open_with(st, None, fx),
        CloneMsg::ParentEdited => check_parent(st, PARENT_CHECK_DELAY, fx),
        CloneMsg::ParentChecked { parent, found } => {
            if let Some(Modal::Clone(form)) = st.modal.as_mut()
                && form.parent.trim() == parent
            {
                form.parent_checked = Some((parent, found));
            }
        }
        CloneMsg::Start(req) => clone_repo(st, req, fx),
        CloneMsg::Source(s) => super::clone_service::source(st, s, fx),
        CloneMsg::ServiceAccount(i) => super::clone_service::account(st, i, fx),
        CloneMsg::RetryRepos => super::clone_service::retry(st, fx),
        CloneMsg::PickRepo(i) => super::clone_service::pick(st, i),
        CloneMsg::UseSsh(ssh) => super::clone_service::use_ssh(st, ssh),
        CloneMsg::ReposLoaded { account, result } => super::clone_service::loaded(st, account, *result),
        CloneMsg::Finished(o) => {
            let made = st.job.as_ref().and_then(|j| j.then_open.as_ref()).map(|(p, _)| p.clone());
            let name = made.as_deref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let t = st.settings.lang();
            let why = super::job_errors::reason(t, &o.output);
            let args = [("name", &name as &dyn std::fmt::Display), ("why", &why.trim_end_matches('.'))];
            // The folders were set when only the submodule step failed after them: a short
            // toast with Retry, and git's reason in the activity log behind Details.
            let submodules = o.cloned && !o.ok && o.folders_set;
            let headline = (o.cloned && !o.ok).then(|| match submodules {
                true => match failed_submodules(&o.output) {
                    0 => t.tr("clone.toast.submodules_failed").to_owned(),
                    n => t.trn("clone.toast.submodules_failed_count", n, &[]),
                },
                false => t.trf("clone.toast.folders_failed", &args),
            });
            super::jobs::finish_made(st, o.cloned, o.ok, o.output, headline, now, fx);
            if submodules {
                st.log(false, t.trf("clone.log.submodules_failed", &args));
                retry_submodules(st, made);
            }
        }
    }
}

/// Opens the Clone dialog, with `url` filled in (a link's remote).
pub(super) fn open_with(st: &mut AppState, url: Option<String>, fx: &mut Vec<Effect>) {
    let parent = st.recent.first().and_then(|p| p.parent()).map(glowfly_core::native_path::shown).unwrap_or_default();
    let mut form = CloneForm::new(parent);
    // A new repository has no override of its own yet.
    form.submodules = st.settings.submodules.keep_updated;
    form.service.accounts = listing_accounts(&st.settings.connections);
    if let Some(url) = url {
        form.url = url;
    }
    st.modal = Some(Modal::Clone(form));
    check_parent(st, Duration::ZERO, fx);
}

/// Asks whether the open Clone dialog's parent folder exists, after `delay`. An empty field
/// needs no answer.
pub(super) fn check_parent(st: &AppState, delay: Duration, fx: &mut Vec<Effect>) {
    if let Some(Modal::Clone(form)) = &st.modal {
        let parent = form.parent.trim();
        if !parent.is_empty() {
            fx.push(Effect::Clone(CloneEffect::CheckParent { parent: parent.to_owned(), delay }));
        }
    }
}

/// Starts the Clone dialog's clone as the active job. A clone without its submodules is not
/// offered (or, keeping submodules updated, given) them when it opens: the box said no. Nor is a
/// sparse clone, whose own step handled them.
fn clone_repo(st: &mut AppState, req: CloneRequest, fx: &mut Vec<Effect>) {
    // The dialog blocks these; checked again so no other sender can start a bad clone.
    if req.name.is_empty() || glowfly_core::remote_ops::check_url(&req.url).is_err() {
        st.log(false, format!("Could not clone: {} isn't a URL or folder git can clone.", req.url));
        return;
    }
    // Never resolved against the process's working folder.
    if !req.parent.is_absolute() {
        st.log(false, format!("Could not clone: {} isn't the full path of a folder.", req.parent.display()));
        return;
    }
    let dest = req.parent.join(&req.name);
    // From a host with a connected account, over https: its SSH-named submodules come over https too.
    let https = super::clone_https::rewrites_for(&st.settings.connections, &req.url);
    let cmd = [https.clone(), clone_cmd(&req.url, &req.name, &req.options)].concat();
    let submodules = req.options.submodules;
    let sparse = req.options.sparse;
    if sparse.is_some()
        && let Some(why) = st.git.blocked(glowfly_core::git_version::Feature::SparseCheckout)
    {
        st.log(false, format!("Could not clone: {why}"));
        return;
    }
    // A sparse clone's own step did its folders' submodules (or said why not): the open doesn't
    // try them, nor the ones outside the folders, again.
    if !submodules || sparse.is_some() {
        st.submodules.offered.push(dest.clone());
    }
    if let Some(sparse) = sparse {
        let subs = if submodules { sparse_submodule_cmds(&sparse.folders) } else { Vec::new() };
        let subs = subs.into_iter().map(|c| [https.clone(), c].concat()).collect();
        let (then_open, folders) = (Some((dest.clone(), Created::Clone)), sparse.folders);
        let effect = |parent| Effect::Clone(CloneEffect::Sparse { parent, cmd, dest, folders, submodules: subs });
        super::jobs::start_effect_job(st, "Clone", req.parent, then_open, effect, fx);
        return;
    }
    start_job(
        st,
        JobSpec { cwd: Some(req.parent), then_open: Some((dest, Created::Clone)), ..JobSpec::new("Clone".into(), vec![cmd]) },
        fx,
    );
}

/// How many submodules git said it couldn't clone, each once though git tries twice (0 when
/// its words aren't English).
fn failed_submodules(output: &str) -> usize {
    let paths: std::collections::BTreeSet<&str> = output
        .lines()
        .filter(|l| l.contains("failed"))
        .filter_map(|l| l.split_once("into submodule path ").map(|(_, p)| p.split(" failed").next().unwrap_or(p)))
        .collect();
    paths.len()
}

/// Gives the newest toast (the clone's) Retry, which updates all submodules of the clone once
/// its tab is active; Details moves beside it.
fn retry_submodules(st: &mut AppState, made: Option<std::path::PathBuf>) {
    let label = st.settings.lang().tr("clone.toast.retry").to_owned();
    let msg =
        Box::new(Msg::Submodule(crate::submodule::SubmoduleMsg::Batch { batch: crate::submodule::Batch::UpdateAll, confirmed: false }));
    let Some(toast) = st.toasts.iter_mut().filter(|t| !t.ok).max_by_key(|t| t.shown) else { return };
    let details = toast.action.replace(crate::state::ToastAction { label, msg, workdir: made });
    if toast.details.is_none() {
        toast.details = details;
    }
}
