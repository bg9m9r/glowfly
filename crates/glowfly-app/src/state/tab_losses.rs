//! What closing tabs or quitting would lose: work that exists only in memory (a commit message
//! draft, conflict choices not saved yet, an edited dialog, comments typed in the pull request or
//! issue panel and not posted, unsaved Preferences edits), and the confirmations that name it,
//! with the terminals the close would end while they run a program (state/terminal/busy.rs),
//! before closing tabs or restarting to update. The quit question uses `quit_lines`
//! (state/quit_confirm.rs). Pure: the close itself is in `update/tabs_close.rs`.

use super::dialog_guard::holds_edited_form;
use super::issue_form::IssueForm;
use super::prefs_save::unsaved;
use super::session::RepoSession;
use super::terminal::busy::{BusyTerminal, busy_text};
use super::updates::install_label;
use super::{AppState, Modal};
use crate::msg::Msg;
use glowfly_core::app_update::InstallMethod;
use glowfly_core::i18n::Lang;

/// The unsaved work in one tab.
#[derive(Debug, PartialEq)]
pub struct Loss {
    /// The tab's shown name.
    pub tab: String,
    /// A commit message draft that isn't blank.
    pub draft: bool,
    /// The open conflict's path and how many chunks the user decided there.
    pub conflict: Option<(String, usize)>,
    /// An edited dialog: the open one goes with the active tab; another tab can keep only a
    /// Create issue form.
    pub form: bool,
    /// Text typed in the pull request or issue panel and not posted.
    pub comments: bool,
}

impl Loss {
    /// One line per kind of work, each starting with "- " and `prefix`.
    fn lines(&self, t: Lang, prefix: &str) -> Vec<String> {
        let mut texts = Vec::new();
        if self.draft {
            texts.push(t.tr("losses.draft").to_owned());
        }
        if let Some((path, n)) = &self.conflict {
            texts.push(t.trn("losses.conflict", *n, &[("path", path)]));
        }
        if self.form {
            texts.push(t.tr("losses.form").to_owned());
        }
        if self.comments {
            texts.push(t.tr("losses.comments").to_owned());
        }
        texts.into_iter().map(|text| format!("- {prefix}{text}")).collect()
    }
}

/// Whether `s` holds a pull request or issue comment typed and not posted (a review's body, a
/// reply, a comment on lines). The merge's title and message are filled in, so they don't count.
fn unposted(s: &RepoSession) -> bool {
    let typed = |x: &str| !x.trim().is_empty();
    let pr = s.pull_request.view.as_ref().is_some_and(|v| {
        let f = &v.forms;
        typed(&f.comment)
            || typed(&f.review_body)
            || f.replies.values().any(|r| typed(r))
            || v.line_draft.as_ref().is_some_and(|d| typed(&d.text))
    });
    pr || s.issue.view.as_ref().is_some_and(|v| typed(&v.comment.text))
}

/// Whether a Create issue form kept with a parked tab has anything typed or chosen.
fn issue_typed(f: &IssueForm) -> bool {
    let typed = |x: &str| !x.trim().is_empty();
    typed(&f.title) || typed(&f.description) || !f.labels.is_empty() || !f.people.is_empty()
}

/// What closing tab `i` would lose, if anything. The active tab's work is in `AppState`; another
/// tab's is parked with it (a restored tab that was never loaded has none).
pub fn loss_of(st: &AppState, i: usize) -> Option<Loss> {
    let tab = st.tabs.get(i)?;
    let (session, form) = if i == st.active_tab {
        (&st.session, holds_edited_form(&st.dialog, st.modal.as_ref()))
    } else {
        let parked = tab.parked.as_ref()?;
        (&parked.session, parked.issue_form.as_deref().is_some_and(issue_typed))
    };
    let draft = !session.draft.is_blank();
    let conflict = session.conflict.view.as_ref().map(|c| (c.path.clone(), c.user_choices())).filter(|(_, n)| *n > 0);
    let comments = unposted(session);
    (draft || conflict.is_some() || form || comments).then(|| Loss {
        tab: super::menu_bar::tab_name(st, i).to_owned(),
        draft,
        conflict,
        form,
        comments,
    })
}

/// The lines naming what closing the tabs at `closing` would lose; None when nothing.
fn losses_body(st: &AppState, closing: &[usize]) -> Option<String> {
    let t = st.settings.lang();
    let losses: Vec<Loss> = closing.iter().filter_map(|&i| loss_of(st, i)).collect();
    if losses.is_empty() {
        return None;
    }
    let mut body = Vec::new();
    if closing.len() == 1 {
        body.push(t.trf("losses.tab_one", &[("tab", &losses[0].tab)]));
        body.extend(losses[0].lines(t, ""));
    } else {
        body.push(t.tr("losses.tabs_many").to_owned());
        for l in &losses {
            body.extend(l.lines(t, &format!("{}: ", l.tab)));
        }
    }
    Some(body.join("\n"))
}

/// What quitting would lose, one "- " line each: every tab's work (named by its tab when more
/// than one is open), then unsaved edits on the open Preferences page. Empty when nothing.
pub fn quit_lines(st: &AppState) -> Vec<String> {
    let t = st.settings.lang();
    let many = st.tabs.len() > 1;
    let mut lines: Vec<String> = (0..st.tabs.len())
        .filter_map(|i| loss_of(st, i))
        .flat_map(|l| l.lines(t, &if many { format!("{}: ", l.tab) } else { String::new() }))
        .collect();
    if let Some(p) = st.prefs.as_ref().filter(|p| unsaved(p, &st.settings, st.snap.as_ref())) {
        lines.push(format!("- {}", t.trf("losses.prefs", &[("page", &p.page.title(t))])));
    }
    lines
}

/// The losses' lines and the busy terminals' part, a blank line apart; None when neither.
fn body_with(st: &AppState, losses: Option<String>, busy: &[BusyTerminal]) -> Option<String> {
    let parts: Vec<String> = losses.into_iter().chain(busy_text(st.settings.lang(), busy)).collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// The confirmation before closing the tabs at `closing`, which would end the terminals `busy`,
/// running `then` when accepted; `None` when none of them would lose anything or end a program.
pub fn close_confirm(st: &AppState, closing: &[usize], busy: &[BusyTerminal], then: Msg) -> Option<Modal> {
    let body = body_with(st, losses_body(st, closing), busy)?;
    let t = st.settings.lang();
    let n = closing.len();
    let title = t.trn("tabs.close_confirm.title", n, &[]);
    let button = t.trn("tabs.close_confirm.button", n, &[]);
    Some(Modal::danger(title, body, button, then))
}

/// The confirmation before Install and restart (or, for `OpenAndQuit`, quitting to install)
/// closes every tab and ends the terminals `busy`, running `then` when accepted; `None` when no
/// tab would lose anything and no terminal runs a program.
pub fn restart_confirm(st: &AppState, method: InstallMethod, busy: &[BusyTerminal], then: Msg) -> Option<Modal> {
    let all: Vec<usize> = (0..st.tabs.len()).collect();
    let rest = body_with(st, losses_body(st, &all), busy)?;
    let t = st.settings.lang();
    let quits = method == InstallMethod::OpenAndQuit;
    let first = if quits { t.tr("updates.restart_confirm.quit_first") } else { t.tr("updates.restart_confirm.first") };
    let title = if quits { t.tr("updates.restart_confirm.quit_title") } else { t.tr("updates.restart_confirm.title") };
    Some(Modal::danger(title, format!("{first}\n{rest}"), install_label(t, method), then))
}
