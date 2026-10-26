//! What closing tabs would lose: work that exists only in memory (a commit message draft, or
//! conflict choices not saved yet), and the confirmations that name it, before closing tabs or
//! restarting to update. Pure: the close itself is in `update/tabs_close.rs`.

use super::updates::install_label;
use super::{AppState, Modal};
use crate::msg::Msg;
use glowfly_core::app_update::InstallMethod;

/// The unsaved work in one tab.
#[derive(Debug, PartialEq)]
pub struct Loss {
    /// The tab's shown name.
    pub tab: String,
    /// A commit message draft that isn't blank.
    pub draft: bool,
    /// The open conflict's path and how many chunks the user decided there.
    pub conflict: Option<(String, usize)>,
}

impl Loss {
    /// One line per kind of work, each starting with "- ".
    fn lines(&self, prefix: &str) -> Vec<String> {
        let mut out = Vec::new();
        if self.draft {
            out.push(format!("- {prefix}its commit message draft"));
        }
        if let Some((path, n)) = &self.conflict {
            let choices = if *n == 1 { "1 conflict choice".to_owned() } else { format!("{n} conflict choices") };
            out.push(format!("- {prefix}{choices} in {path}"));
        }
        out
    }
}

/// What closing tab `i` would lose, if anything. The active tab's work is in `AppState`; another
/// tab's is parked with it (a restored tab that was never loaded has none).
pub fn loss_of(st: &AppState, i: usize) -> Option<Loss> {
    let tab = st.tabs.get(i)?;
    let (draft, conflict) = if i == st.active_tab {
        (&st.draft, st.session.conflict.view.as_ref())
    } else {
        let parked = tab.parked.as_ref()?;
        (&parked.draft, parked.session.conflict.view.as_ref())
    };
    let draft = !draft.is_blank();
    let conflict = conflict.map(|c| (c.path.clone(), c.user_choices())).filter(|(_, n)| *n > 0);
    (draft || conflict.is_some()).then(|| Loss { tab: super::menu_bar::tab_name(st, i).to_owned(), draft, conflict })
}

/// The lines naming what closing the tabs at `closing` would lose; None when nothing.
fn losses_body(st: &AppState, closing: &[usize]) -> Option<String> {
    let losses: Vec<Loss> = closing.iter().filter_map(|&i| loss_of(st, i)).collect();
    if losses.is_empty() {
        return None;
    }
    let mut body = Vec::new();
    if closing.len() == 1 {
        body.push(format!("{} holds work that isn't saved anywhere:", losses[0].tab));
        body.extend(losses[0].lines(""));
    } else {
        body.push("Some of these tabs hold work that isn't saved anywhere:".to_owned());
        for l in &losses {
            body.extend(l.lines(&format!("{}: ", l.tab)));
        }
    }
    Some(body.join("\n"))
}

/// The confirmation before closing the tabs at `closing`, running `then` when accepted; `None`
/// when none of them would lose anything.
pub fn close_confirm(st: &AppState, closing: &[usize], then: Msg) -> Option<Modal> {
    let body = losses_body(st, closing)?;
    let n = closing.len();
    let (title, button) =
        if n == 1 { ("Close tab?".to_owned(), "Close tab".to_owned()) } else { (format!("Close {n} tabs?"), format!("Close {n} tabs")) };
    Some(Modal::danger(title, body, button, then))
}

/// The confirmation before Install and restart (or, for `OpenAndQuit`, quitting to install)
/// closes every tab, running `then` when accepted; `None` when no tab would lose anything.
pub fn restart_confirm(st: &AppState, method: InstallMethod, then: Msg) -> Option<Modal> {
    let all: Vec<usize> = (0..st.tabs.len()).collect();
    let losses = losses_body(st, &all)?;
    let t = st.settings.lang();
    let quits = method == InstallMethod::OpenAndQuit;
    let first = if quits { "Quitting closes every tab." } else { "Restarting closes every tab." };
    let title = if quits { t.tr("updates.restart_confirm.quit_title") } else { t.tr("updates.restart_confirm.title") };
    Some(Modal::danger(title, format!("{first}\n{losses}"), install_label(t, method), then))
}
