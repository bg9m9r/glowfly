//! Preferences > SSH: the key git uses for SSH remotes (core.sshCommand, globally or for the
//! open, trusted repository), the Use local SSH agent setting, and adding a key to the agent.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::ssh::{SshEffect, SshMsg};
use crate::state::AppState;
use crate::state::Modal;
use crate::state::ssh_use::{
    AGENT_TOAST, Choices, REMOVE_COMMAND_TITLE, REPLACE_COMMAND_TITLE, add_blocked, level_label, remove_command_body, replace_command_body,
    use_blocked,
};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{AgentStatus, KeyChoice};

/// Reads core.sshCommand again (globally, and in the page's repository).
pub(super) fn read_choice(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    page.choice_generation += 1;
    page.choice = None;
    let repo = page.repo.as_ref().map(|r| r.workdir.clone());
    fx.push(Effect::Ssh(SshEffect::ReadChoice { generation: page.choice_generation, repo }));
}

/// Checks the agent again.
pub(super) fn read_agent(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    page.agent_generation += 1;
    page.agent = None;
    fx.push(Effect::Ssh(SshEffect::Agent { generation: page.agent_generation }));
}

pub(super) fn choice_loaded(
    st: &mut AppState,
    generation: u64,
    global: Result<KeyChoice, String>,
    repo: Option<Result<KeyChoice, String>>,
) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh).filter(|p| p.choice_generation == generation) else { return };
    page.choice = Some(Choices { global, repo });
}

pub(super) fn agent_loaded(st: &mut AppState, generation: u64, status: AgentStatus) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh).filter(|p| p.agent_generation == generation) else { return };
    page.agent = Some(status);
}

/// Why writing the repository's own core.sshCommand can't go ahead: another repository is
/// open now, or the user no longer trusts this one.
fn repo_changed(st: &AppState) -> Option<&'static str> {
    let target = st.prefs.as_ref()?.ssh.repo.as_ref()?;
    let same = st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &target.real));
    if !same {
        return Some("Another repository is open now; reload this page.");
    }
    (!st.trusts(&target.real)).then_some(crate::state::ssh_use::NOT_TRUSTED)
}

/// The checks Use for git and Stop using share: the reason it can't run (shown on the row), or
/// the level's current value and label.
fn check(st: &mut AppState, repo: bool, use_agent: bool) -> Option<(KeyChoice, String)> {
    let blocked = st.prefs.as_ref().map(|p| use_blocked(&p.ssh, use_agent, repo))?;
    let why = blocked.or_else(|| if repo { repo_changed(st) } else { None });
    let page = &mut st.prefs.as_mut()?.ssh;
    if let Some(why) = why {
        page.choice_error = Some(why.into());
        return None;
    }
    let current = page.choice.as_ref().and_then(|c| c.at(repo)).cloned()?;
    let level = level_label(if repo { page.repo.as_ref().map(|r| r.name.as_str()) } else { None });
    Some((current, level))
}

/// Starts the core.sshCommand write at one level.
fn write(st: &mut AppState, repo: bool, key: Option<PathBuf>, fx: &mut Vec<Effect>) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let workdir = if repo { page.repo.as_ref().map(|r| r.workdir.clone()) } else { None };
    if repo && workdir.is_none() {
        return;
    }
    page.choice_busy = true;
    page.choice_error = None;
    page.choice_key = key.clone();
    page.choice_repo = repo;
    fx.push(Effect::Ssh(SshEffect::SetKeyChoice { repo: workdir, key }));
}

pub(super) fn use_key(st: &mut AppState, key: PathBuf, repo: bool, confirmed: bool, fx: &mut Vec<Effect>) {
    let Some(private) = st.prefs.as_ref().and_then(|p| p.ssh.key(&key)).map(|k| k.private.clone()) else { return };
    let use_agent = st.settings.use_ssh_agent;
    let Some((current, level)) = check(st, repo, use_agent) else { return };
    if let KeyChoice::Other { value, .. } = &current
        && !confirmed
    {
        st.modal = Some(Modal::Confirm {
            title: REPLACE_COMMAND_TITLE.into(),
            body: replace_command_body(&level, value),
            button: "Replace".into(),
            then: Box::new(Msg::Ssh(SshMsg::UseKey { key, repo, confirmed: true })),
        });
        return;
    }
    write(st, repo, Some(private), fx);
}

/// Stop using: unsets the level's choice, asking first for a command Trellis didn't write. It
/// runs with the agent setting on too: an unset core.sshCommand is what that setting wants.
pub(super) fn stop_using(st: &mut AppState, repo: bool, confirmed: bool, fx: &mut Vec<Effect>) {
    let Some((current, level)) = check(st, repo, false) else { return };
    match &current {
        KeyChoice::Unset => {}
        KeyChoice::Other { value, .. } if !confirmed => {
            st.modal = Some(Modal::Confirm {
                title: REMOVE_COMMAND_TITLE.into(),
                body: remove_command_body(&level, value),
                button: "Remove".into(),
                then: Box::new(Msg::Ssh(SshMsg::StopUsing { repo, confirmed: true })),
            });
        }
        _ => write(st, repo, None, fx),
    }
}

/// The Use local SSH agent checkbox. Turning it on unsets the choices Trellis wrote (the
/// repository's only when it is trusted); a command Trellis didn't write stays, flagged.
pub(super) fn set_use_agent(st: &mut AppState, on: bool, now: Instant, fx: &mut Vec<Effect>) {
    st.settings.use_ssh_agent = on;
    fx.push(super::prefs::save_settings(st));
    if !on {
        return;
    }
    st.toast(true, AGENT_TOAST, now);
    let Some(page) = st.prefs.as_ref().map(|p| &p.ssh) else { return };
    let chosen = |repo: bool| matches!(page.choice.as_ref().and_then(|c| c.at(repo)), Some(KeyChoice::Key(_)));
    let levels: Vec<bool> =
        [false, true].into_iter().filter(|&repo| chosen(repo) && (!repo || page.repo.as_ref().is_some_and(|r| r.trusted))).collect();
    for repo in levels {
        if !repo || repo_changed(st).is_none() {
            write(st, repo, None, fx);
        }
    }
}

pub(super) fn key_choice_set(st: &mut AppState, repo: bool, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    let page = st.prefs.as_mut().map(|p| &mut p.ssh);
    let (key, name) = match page {
        Some(page) => {
            page.choice_busy = false;
            (page.choice_key.take(), page.repo.as_ref().map(|r| r.name.clone()))
        }
        None => (None, None),
    };
    let level = level_label(if repo { Some(name.as_deref().unwrap_or("this repository")) } else { None });
    match result {
        Ok(()) => {
            let text = match key.as_deref().and_then(Path::file_name) {
                Some(file) => format!("git now uses {} for SSH remotes ({level})", file.to_string_lossy()),
                None => format!("git no longer uses a chosen key ({level})"),
            };
            st.toast(true, text, now);
            read_choice(st, fx);
        }
        Err(e) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.choice_error = Some(e.clone());
            }
            st.log(false, format!("Could not change core.sshCommand: {e}"));
        }
    }
}

pub(super) fn add_to_agent(st: &mut AppState, key: &Path, fx: &mut Vec<Effect>) {
    let job = st.job.as_ref().map(|j| j.label.clone());
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let Some(private) = page.key(key).map(|k| k.private.clone()) else { return };
    if let Some(why) = add_blocked(page, job.as_deref()) {
        page.agent_error = Some(why);
        return;
    }
    page.agent_error = None;
    let Some(ssh_add) = page.agent.as_ref().and_then(|a| a.ssh_add.clone()) else { return };
    page.agent_adding = Some(private.clone());
    fx.push(Effect::Ssh(SshEffect::AddToAgent { ssh_add, key: private }));
}

pub(super) fn agent_added(st: &mut AppState, key: &Path, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut() {
        p.ssh.agent_adding = None;
    }
    let headline = super::askpass::job_finished(st, result.is_ok(), "Add key to SSH agent", None, "", fx);
    let file = key.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    match result {
        Ok(()) => {
            st.toast(true, format!("Added {file} to the SSH agent"), now);
            read_agent(st, fx);
        }
        Err(e) => {
            let text = headline.unwrap_or_else(|| format!("Could not add {file} to the SSH agent: {e}"));
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.agent_error = Some(text.clone());
            }
            st.log(false, text);
        }
    }
}
