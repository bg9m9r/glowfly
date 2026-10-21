//! Custom commands: reading both commands files with each full load of the active repository
//! and when its `.glowfly/` (or older `.trellis/`) folder changes (generation-checked, for the
//! folder asked about), keeping their commands and keys, and telling the user about their
//! problems in the activity log, with one toast when they change. Running a command is in
//! custom_commands_run.rs.

use crate::custom_commands::{CustomCommandEffect, CustomCommandMsg};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::custom_commands::{chords, problem_text};
use glowfly_core::custom_commands::{FILE_NAME, FileRead, OLD_REPO_DIR, REPO_DIR, Source};
use glowfly_core::paths::same_spelling;
use std::path::PathBuf;
use std::time::Instant;

/// How the repository's file is named in problems: `.glowfly/commands.toml`, or
/// `.trellis/commands.toml` when that older folder is the one read.
fn repo_label(repo: &FileRead) -> String {
    let old = repo.path.as_deref().and_then(|p| p.parent()).and_then(|d| d.file_name()).is_some_and(|n| n == OLD_REPO_DIR);
    format!("{}/{FILE_NAME}", if old { OLD_REPO_DIR } else { REPO_DIR })
}

pub(super) fn handle(st: &mut AppState, msg: CustomCommandMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        CustomCommandMsg::Reload => request(st, fx),
        CustomCommandMsg::Loaded { generation, workdir, user, repo } => loaded_files(st, generation, workdir, user, repo, now),
        CustomCommandMsg::Run { command, at, confirmed } => super::custom_commands_run::run(st, command, at, confirmed, now, fx),
    }
}

/// The active repository was just loaded in full: read the commands files again.
pub(super) fn loaded(st: &mut AppState, fx: &mut Vec<Effect>) {
    request(st, fx);
}

/// The watcher saw these work-tree paths change (None: a change it couldn't name): reads the
/// files again when one is under `.glowfly/` or `.trellis/`.
pub(super) fn watched(st: &mut AppState, paths: Option<&[String]>, fx: &mut Vec<Effect>) {
    let ours =
        |p: &str| [REPO_DIR, OLD_REPO_DIR].iter().any(|d| p.strip_prefix(d).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')));
    if paths.is_some_and(|ps| ps.iter().any(|p| ours(p))) {
        request(st, fx);
    }
}

fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let cc = &mut st.custom_commands;
    cc.generation += 1;
    fx.push(Effect::CustomCommand(CustomCommandEffect::Load { generation: cc.generation, workdir }));
}

fn loaded_files(st: &mut AppState, generation: u64, workdir: PathBuf, user: FileRead, repo: FileRead, now: Instant) {
    let current = st.workdir().is_some_and(|w| same_spelling(&w, &workdir));
    if generation != st.custom_commands.generation || !current {
        tracing::debug!("dropped a stale custom commands load");
        return;
    }
    let t = st.settings.lang();
    let user_label =
        user.path.as_deref().map_or_else(|| glowfly_core::custom_commands::FILE_NAME.to_owned(), glowfly_core::native_path::shown);
    let mut problems: Vec<String> = user.problems.iter().map(|p| problem_text(t, &user_label, p)).collect();
    let repo_file = repo_label(&repo);
    problems.extend(repo.problems.iter().map(|p| problem_text(t, &repo_file, p)));
    let lists =
        [(Source::User, user_label.as_str(), user.commands.as_slice()), (Source::Repo, repo_file.as_str(), repo.commands.as_slice())];
    let (keys, key_problems) = chords(t, &st.keymap, &lists);
    problems.extend(key_problems);
    let cc = &mut st.custom_commands;
    cc.user = user.commands;
    cc.repo = repo.commands;
    cc.repo_for = Some(workdir);
    cc.keys = keys;
    if problems == cc.reported {
        return;
    }
    cc.reported = problems.clone();
    if problems.is_empty() {
        return;
    }
    for p in &problems {
        st.log(false, p.clone());
    }
    st.toast(false, t.trn("commands.problems.toast", problems.len(), &[]), now);
}
