//! The terminals a close would end while they run a program: quitting, closing repository tabs
//! and Install and restart ask the runner what their shells are doing (update/terminal_busy.rs)
//! and name the busy ones, with the program where it is known, in their one confirmation.

use super::SessionStatus;
use crate::state::AppState;
use crate::state::menu_bar::tab_name;
use crate::state::terminal_names::{folder, name_of};
use glowfly_core::i18n::Lang;
use glowfly_core::terminal::ShellActivity;
use std::path::Path;

/// A terminal whose shell runs a program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyTerminal {
    /// Its session.
    pub id: u64,
    /// The repository tab it belongs to, by its shown name (the folder's when no tab shows it).
    pub tab: String,
    /// The terminal tab's name.
    pub terminal: String,
    /// The program, when the system tells it.
    pub program: Option<String>,
}

/// Whether session `id`'s shell is running (not starting, ended or held).
pub fn running(st: &AppState, id: u64) -> bool {
    st.terminal.sessions.iter().any(|s| s.id == id && s.status == SessionStatus::Running)
}

/// Every session whose shell runs, in any repository.
pub fn running_ids(st: &AppState) -> Vec<u64> {
    st.terminal.sessions.iter().filter(|s| s.status == SessionStatus::Running).map(|s| s.id).collect()
}

/// The running sessions that close with the tabs at `closing`: those of their repositories,
/// unless a tab that stays shows the same repository (update/terminal.rs `tab_closed`).
pub fn ids_for_tabs(st: &AppState, closing: &[usize]) -> Vec<u64> {
    let ends = |w: &Path| {
        let mine = closing.iter().any(|&j| st.tabs.get(j).and_then(|t| t.path.as_deref()) == Some(w));
        let kept = st.tabs.iter().enumerate().any(|(j, t)| !closing.contains(&j) && t.path.as_deref() == Some(w));
        mine && !kept
    };
    st.terminal.sessions.iter().filter(|s| s.status == SessionStatus::Running && ends(&s.workdir)).map(|s| s.id).collect()
}

/// The runner's answer as the terminals that still run and run a program, in tab order.
pub fn busy_from(st: &AppState, found: &[(u64, ShellActivity)]) -> Vec<BusyTerminal> {
    let program = |id: u64| {
        found.iter().find(|(f, _)| *f == id).and_then(|(_, a)| match a {
            ShellActivity::Running { program } => Some(program.clone()),
            ShellActivity::Idle | ShellActivity::Unknown => None,
        })
    };
    st.terminal
        .sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Running)
        .filter_map(|s| {
            let program = program(s.id)?;
            let tab = match st.tabs.iter().position(|t| t.path.as_deref() == Some(s.workdir.as_path())) {
                Some(i) => tab_name(st, i).to_owned(),
                None => folder(&s.workdir),
            };
            Some(BusyTerminal { id: s.id, tab, terminal: name_of(st, s.id).unwrap_or_default(), program })
        })
        .collect()
}

/// One line per busy terminal, naming its repository tab and its program.
pub fn busy_lines(t: Lang, busy: &[BusyTerminal]) -> Vec<String> {
    busy.iter()
        .map(|b| {
            let (terminal, tab) = (&b.terminal, &b.tab);
            match &b.program {
                Some(p) => t.trf("terminal.busy.line", &[("terminal", terminal), ("tab", tab), ("program", p)]),
                None => t.trf("terminal.busy.line_unnamed", &[("terminal", terminal), ("tab", tab)]),
            }
        })
        .collect()
}

/// The busy terminals' lines, a blank line and what closing does to them (`None` when there
/// are none): the part of a tab close's or a restart's confirmation about terminals.
pub fn busy_text(t: Lang, busy: &[BusyTerminal]) -> Option<String> {
    (!busy.is_empty()).then(|| format!("{}\n\n{}", busy_lines(t, busy).join("\n"), stops_text(t, busy.len())))
}

/// What closing does to `n` busy terminals.
pub fn stops_text(t: Lang, n: usize) -> &'static str {
    if n == 1 { t.tr("terminal.busy.stops_one") } else { t.tr("terminal.busy.stops_many") }
}
