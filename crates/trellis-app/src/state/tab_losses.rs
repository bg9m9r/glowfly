//! What closing tabs would lose: work that exists only in memory (a commit message draft, or
//! conflict choices not saved yet), and the confirmation that names it. Pure: the close itself
//! is in `update/tabs_close.rs`.

use super::{AppState, Modal};
use crate::msg::Msg;

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
        (&st.draft, st.conflict.as_ref())
    } else {
        let parked = tab.parked.as_ref()?;
        (&parked.draft, parked.conflict.as_ref())
    };
    let draft = !draft.is_blank();
    let conflict = conflict.map(|c| (c.path.clone(), c.user_choices())).filter(|(_, n)| *n > 0);
    (draft || conflict.is_some()).then(|| Loss { tab: super::menu_bar::tab_name(st, i).to_owned(), draft, conflict })
}

/// The confirmation before closing the tabs at `closing`, running `then` when accepted; `None`
/// when none of them would lose anything.
pub fn close_confirm(st: &AppState, closing: &[usize], then: Msg) -> Option<Modal> {
    let losses: Vec<Loss> = closing.iter().filter_map(|&i| loss_of(st, i)).collect();
    if losses.is_empty() {
        return None;
    }
    let n = closing.len();
    let mut body = Vec::new();
    if n == 1 {
        body.push(format!("{} holds work that isn't saved anywhere:", losses[0].tab));
        body.extend(losses[0].lines(""));
    } else {
        body.push("Some of these tabs hold work that isn't saved anywhere:".to_owned());
        for l in &losses {
            body.extend(l.lines(&format!("{}: ", l.tab)));
        }
    }
    let (title, button) =
        if n == 1 { ("Close tab?".to_owned(), "Close tab".to_owned()) } else { (format!("Close {n} tabs?"), format!("Close {n} tabs")) };
    Some(Modal::Confirm { title, body: body.join("\n"), button, then: Box::new(then) })
}
