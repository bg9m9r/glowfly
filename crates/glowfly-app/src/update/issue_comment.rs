//! Posting the issue panel's comment box and quoting a comment into it; the reply appends the
//! posted comment or keeps the draft with the reason, also for a parked tab's panel.

use crate::effect::{Effect, ForgeError, IssueCall, IssueEffect, IssueJob};
use crate::state::AppState;
use crate::state::issue::{Details, IssueView};
use crate::state::issue_comment::{FIELD, comment_blocked, with_quote};
use crate::state::issue_words;
use glowfly_core::forge::{self, Entry, Target};
use std::path::Path;
use std::time::Instant;

/// Posts the open panel's draft, unless its button would be disabled. The raw text is sent;
/// glowfly-core normalises it.
pub(super) fn post(st: &mut AppState, fx: &mut Vec<Effect>) {
    let trusted = st.runs_git_unasked();
    let req = super::issue::request(st);
    let generation = st.issue_gen;
    let Some(v) = st.issue.as_mut() else { return };
    v.comment.touched.attempt();
    if comment_blocked(v, trusted).is_some() {
        return;
    }
    match req {
        Err(e) => {
            let text = format!("Commenting on {} {} failed: {e}", v.noun, v.mention);
            v.comment.error = Some(e);
            st.log(false, text);
        }
        Ok(req) => {
            v.comment.posting = true;
            v.comment.error = None;
            let call = IssueCall::Comment { key: v.key.clone(), body: v.comment.text.clone() };
            fx.push(Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation, call }))));
        }
    }
}

/// Quotes comment `i` of the loaded conversation at the end of the draft and focuses the field.
pub(super) fn quote(st: &mut AppState, i: usize) {
    let Some(v) = st.issue.as_mut() else { return };
    if v.comment.posting {
        return;
    }
    let Some(c) = v.loaded().and_then(|d| d.comments.get(i)) else { return };
    let q = forge::quote_comment(v.provider(), &c.author, &c.body);
    if q.is_empty() {
        return;
    }
    v.comment.text = with_quote(&v.comment.text, &q);
    v.comment.touched.mark(FIELD);
    st.view.issue_comment_focus = true;
}

/// A comment's answer for issue `key` on `target` in the repository at `workdir`. It is matched
/// by repository, tracker and key only, never by generation: a details reload in the meantime
/// must not make a posted comment's answer stale. It goes to the active panel, else to the
/// parked panel of the tab showing `workdir`, and only while that panel is posting: a panel
/// closed and reopened meanwhile has a new draft to keep. Without one it is logged and toasted.
pub(super) fn commented(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    key: &str,
    result: Result<Entry, ForgeError>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let here = st.workdir().as_deref() == Some(workdir);
    let active = here && st.forge.as_ref().and_then(|f| f.tracker.as_ref()) == Some(target);
    let mention = forge::mention(target.provider.ref_style(), key);
    let label = format!("{} {mention}", issue_words::noun(target.provider));
    let (ok, text) = match &result {
        Ok(_) => (true, format!("Commented on {label}")),
        Err(e) => (false, format!("Commenting on {label} failed: {}", e.message)),
    };
    let failure = result.as_ref().err().map(|e| e.message.clone());
    if active
        && result.as_ref().is_err_and(|e| e.auth)
        && let Some(fs) = st.forge.as_mut()
    {
        fs.auth.remove(&target.account().key());
    }
    let mine = |v: &&mut IssueView| v.comment.posting && v.key == key && &v.target == target;
    let view = if active {
        st.issue.as_mut().filter(mine)
    } else {
        st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()).and_then(|p| p.issue.as_mut()).filter(mine)
    };
    let found = view.map(|v| apply(v, result));
    if active {
        st.log(ok, text.clone());
    } else {
        st.log_in(Some(workdir), ok, text.clone());
    }
    match found {
        Some(reloading) if active && ok => {
            st.toast(true, "Comment posted", now);
            if reloading {
                super::issue::load(st, fx);
            }
        }
        Some(_) => {}
        // The tracker changed under the open repository: logged only.
        None if here && !active => {}
        None if ok => st.toast(true, text, now),
        None => st.toast(false, format!("Couldn't post the comment on {mention}: {}", failure.unwrap_or_default()), now),
    }
}

/// Applies the answer to panel `v`: the posted comment is appended and the draft cleared, or the
/// draft stays with the reason. Returns whether a details reload was in flight, which won't
/// have the new comment.
fn apply(v: &mut IssueView, result: Result<Entry, ForgeError>) -> bool {
    v.comment.posting = false;
    match result {
        Ok(entry) => {
            v.comment.text.clear();
            v.comment.touched = Default::default();
            v.comment.error = None;
            if let Details::Loaded(d) = &mut v.details {
                d.comments.push(entry);
            }
            v.reloading
        }
        Err(e) => {
            v.comment.error = Some(e.message);
            false
        }
    }
}
