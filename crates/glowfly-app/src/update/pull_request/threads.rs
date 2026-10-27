//! The pull request panel's code comment threads: replying and resolving (sent through
//! `calls::act`, one action at a time like the other calls), applying their answers in place,
//! which threads are shown and open, and going from a thread to its line in Files changed.

use super::calls::act;
use crate::effect::Effect;
use crate::pull_request::{PrCall, PrReply};
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction, PrTab};
use crate::state::pull_request_threads::{go_to, is_open, reply_blocked, resolve_blocked};
use glowfly_core::diff_rows::row_of_file_line;
use glowfly_core::forge::ReviewThread;
use std::time::Instant;

/// The loaded page's thread `id`.
fn find(st: &AppState, id: &str) -> Option<ReviewThread> {
    st.session.pull_request.view.as_ref()?.loaded()?.threads.iter().find(|t| t.id == id).cloned()
}

/// Posts thread `id`'s reply box.
pub(super) fn reply(st: &mut AppState, id: String, fx: &mut Vec<Effect>) {
    let (Some(v), Some(t)) = (st.session.pull_request.view.as_ref(), find(st, &id)) else { return };
    if reply_blocked(v, &t).is_some() {
        return;
    }
    let (Some(comment), Some(draft)) = (t.reply_to(), v.forms.replies.get(&id)) else { return };
    let call = PrCall::Reply { thread: id.clone(), comment, body: draft.trim().to_owned() };
    act(st, PrAction::Reply(id), call, fx);
}

/// Resolves (`resolve`) or reopens thread `id`.
pub(super) fn resolve(st: &mut AppState, id: String, resolve: bool, fx: &mut Vec<Effect>) {
    let (Some(v), Some(t)) = (st.session.pull_request.view.as_ref(), find(st, &id)) else { return };
    if resolve_blocked(v, &t).is_some() || t.resolved == resolve {
        return;
    }
    act(st, PrAction::Resolve(id.clone()), PrCall::Resolve { thread: id, resolve }, fx);
}

pub(super) fn show_resolved(st: &mut AppState, show: bool) {
    if let Some(v) = st.session.pull_request.view.as_mut() {
        v.threads.show_resolved = show;
    }
}

pub(super) fn show_outdated(st: &mut AppState, show: bool) {
    if let Some(v) = st.session.pull_request.view.as_mut() {
        v.threads.show_outdated = show;
    }
}

/// A card's Show/Hide.
pub(super) fn toggle(st: &mut AppState, id: String) {
    let Some(t) = find(st, &id) else { return };
    if let Some(v) = st.session.pull_request.view.as_mut() {
        let open = is_open(&v.threads, &t);
        v.threads.open.insert(id, !open);
    }
}

/// Opens thread `id` and asks the comment pane to scroll to it.
pub(super) fn focus(st: &mut AppState, id: String) {
    let Some(v) = st.session.pull_request.view.as_mut() else { return };
    if !v.loaded().is_some_and(|p| p.threads.iter().any(|t| t.id == id)) {
        return;
    }
    v.threads.open.insert(id.clone(), true);
    v.threads.focus = Some(id.clone());
    st.view.pr_thread_scroll = Some(id);
}

/// Shows thread `id`'s file in Files changed at its line: scrolls the open diff, or opens it
/// to land there once loaded. Nothing happens when it can't be placed (its link is disabled
/// then, with the reason).
pub(super) fn go_to_thread(st: &mut AppState, id: String, fx: &mut Vec<Effect>) {
    let (Some(v), Some(t)) = (st.session.pull_request.view.as_ref(), find(st, &id)) else { return };
    let Ok((target, side, n)) = go_to(v, &t) else { return };
    if let Some(v) = st.session.pull_request.view.as_mut() {
        v.tab = PrTab::Files;
    }
    focus(st, id);
    match &st.session.diff.view {
        Some((open, d)) if *open == target => {
            let (s, ex) = (&st.settings, &st.view.diff_expanded);
            match row_of_file_line(d, s.diff_mode, s.diff_layout, ex, side, n) {
                Some(row) => {
                    st.view.diff_scroll_to_row = Some(row);
                    st.view.diff_top_row = row;
                }
                None if n > 0 => st.log_quiet(false, format!("Line {n} isn't in the changed lines; switch to Whole file to see it")),
                None => {}
            }
        }
        _ => {
            super::super::repo::open_file_diff(st, target.clone(), fx);
            st.view.diff_open_line = Some((target, side, n));
        }
    }
}

/// A successful reply or resolve, applied in place (the page isn't loaded again). `open`: the
/// answer is for the open panel; `text` is its log line.
pub(super) fn applied(st: &mut AppState, open: bool, reply: PrReply, text: String, now: Instant) {
    let page = st.session.pull_request.view.as_mut().filter(|_| open);
    match reply {
        PrReply::RepliedToThread { thread, result: Ok(comment) } => {
            if let Some(v) = page {
                v.forms.replies.remove(&thread);
                if let Page::Loaded(p) = &mut v.page
                    && let Some(t) = p.threads.iter_mut().find(|t| t.id == thread)
                {
                    t.comments.push(comment);
                }
            }
            st.toast(true, "Reply posted", now);
        }
        PrReply::Resolved { thread, result: Ok(resolved), .. } => {
            if let Some(v) = page {
                v.threads.open.remove(&thread);
                if let Page::Loaded(p) = &mut v.page
                    && let Some(t) = p.threads.iter_mut().find(|t| t.id == thread)
                {
                    t.resolved = resolved;
                    if resolved {
                        t.can_unresolve = true;
                    } else {
                        t.can_resolve = true;
                    }
                }
            }
            st.toast(true, text, now);
        }
        _ => {}
    }
}
