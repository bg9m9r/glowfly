//! The toolbar buttons' rich tooltips, one builder per button: the title, the shortcut or state
//! line, a sentence or the reason it is disabled, and a preview of what it would do. Pure over
//! `AppState` (the rules and previews are in `state/toolbar_tips.rs` and
//! `state/undo_preview.rs`); `widgets::tooltip` draws them.

use crate::keymap::BindingId;
use crate::state::AppState;
use crate::state::ForgeState;
use crate::state::tip::{Tip, short_sha};
use crate::state::toolbar_tips as rules;
use glowfly_core::forge::Provider;
use glowfly_core::i18n::Lang;

fn lang(st: &AppState) -> Lang {
    st.settings.lang()
}

fn fetched(st: &AppState) -> String {
    let at = st.snap.as_ref().and_then(|s| s.fetched_at);
    rules::fetched_line(lang(st), &st.settings, at, st.view.now, st.view.utc_offset)
}

/// Undo: "Undo commit 'Fix login'", its keys, and what undoing it reverts (or why it can't).
pub fn undo(st: &AppState) -> Tip {
    let t = lang(st);
    let (title, why) = st.session.undo.undo_entry(t, st.job.is_none());
    let tip = Tip::new(title).keys(BindingId::Undo);
    match (why, st.session.undo.undo.last()) {
        (Some(why), _) => tip.body(why),
        (None, Some(a)) => tip.preview(Some(crate::state::undo_preview::preview(t, a, st.snap.as_ref(), false))),
        (None, None) => tip,
    }
}

/// Redo, like `undo`.
pub fn redo(st: &AppState) -> Tip {
    let t = lang(st);
    let (title, why) = st.session.undo.redo_entry(t, st.job.is_none());
    let tip = Tip::new(title).keys(BindingId::Redo);
    match (why, st.session.undo.redo.last()) {
        (Some(why), _) => tip.body(why),
        (None, Some(a)) => tip.preview(Some(crate::state::undo_preview::preview(t, a, st.snap.as_ref(), true))),
        (None, None) => tip,
    }
}

/// Pull: "Pull origin/main into main", the ahead/behind line, the commits it brings in and when
/// the repository last fetched.
pub fn pull(st: &AppState) -> Tip {
    let t = lang(st);
    let Some(snap) = &st.snap else { return Tip::new(t.tr("toolbar.pull.name")) };
    let head = snap.head_branch.as_ref().and_then(|_| snap.head_info());
    let upstream = head.and_then(|b| Some((b, b.upstream.as_ref()?)));
    let title = match upstream {
        Some((b, up)) => t.trf("toolbar.tip.pull_title", &[("upstream", up), ("branch", &b.name)]),
        None => t.tr("toolbar.pull.name").to_owned(),
    };
    let tip = Tip::new(title).state_opt(rules::sync_line(t, snap)).footer(fetched(st));
    if let Some(why) = rules::pull_blocked(st) {
        return tip.body(why);
    }
    let body = match upstream {
        Some((b, up)) if b.behind == 0 => t.trf("toolbar.tip.pull_nothing", &[("upstream", up)]),
        _ => t.tr("toolbar.tip.pull_body").to_owned(),
    };
    tip.body(body).preview(rules::pull_preview(t, snap))
}

/// The pull menu's Fetch all: when the repository last fetched, and what fetching does.
pub fn fetch_all(st: &AppState) -> Tip {
    let t = lang(st);
    let tip = Tip::new(t.tr("toolbar.fetch_all")).state(fetched(st));
    tip.body(rules::fetch_blocked(st).unwrap_or_else(|| t.tr("toolbar.tip.fetch_all_body").to_owned()))
}

/// A pull menu item (`text` is its caption): the ahead/behind line, or why it can't run.
pub fn pull_variant(st: &AppState, text: &str) -> Tip {
    let tip = Tip::new(text).state_opt(st.snap.as_ref().and_then(|s| rules::sync_line(lang(st), s)));
    match rules::pull_variant_blocked(st, text) {
        Some(why) => tip.body(why),
        None => tip,
    }
}

/// Push: "Push main to origin/main" (or to a remote it will track, or Publish without a remote),
/// the ahead/behind line and the commits it sends.
pub fn push(st: &AppState) -> Tip {
    let t = lang(st);
    let Some(snap) = &st.snap else { return Tip::new(t.tr("toolbar.push.name")) };
    let head = snap.head_branch.as_ref().and_then(|_| snap.head_info());
    let blocked = rules::push_blocked(st);
    if snap.remotes.is_empty() {
        let tip = Tip::new(t.tr("toolbar.tip.publish_title")).state_opt(rules::sync_line(t, snap));
        return tip.body(blocked.unwrap_or_else(|| t.tr("toolbar.tip.publish_body").to_owned()));
    }
    let title = match head {
        Some(b) => match &b.upstream {
            Some(up) => t.trf("toolbar.tip.push_title", &[("branch", &b.name), ("upstream", up)]),
            None => {
                let remote = crate::state::branches::default_remote(snap).unwrap_or_default();
                t.trf("toolbar.tip.push_track_title", &[("branch", &b.name), ("remote", &remote)])
            }
        },
        None => t.tr("toolbar.push.name").to_owned(),
    };
    let tip = Tip::new(title).state_opt(rules::sync_line(t, snap));
    if let Some(why) = blocked {
        return tip.body(why);
    }
    let tip = match head {
        Some(b) if b.ahead == 0 && b.upstream.is_some() => {
            let up = b.upstream.as_deref().unwrap_or_default();
            tip.body(t.trf("toolbar.tip.push_nothing", &[("upstream", &up), ("branch", &b.name)]))
        }
        _ => tip,
    };
    tip.preview(rules::push_preview(t, snap))
}

/// Branch: where a new branch starts.
pub fn branch(st: &AppState) -> Tip {
    let t = lang(st);
    let tip = Tip::new(t.tr("toolbar.branch.name"));
    if let Some(why) = rules::branch_blocked(st) {
        return tip.body(why);
    }
    match st.snap.as_ref().and_then(|s| s.head_oid) {
        Some(o) => tip.body(t.trf("toolbar.tip.branch_body", &[("sha", &short_sha(o))])),
        None => tip,
    }
}

/// Stash: how many files changed, which ones it saves, and what stashing does.
pub fn stash(st: &AppState) -> Tip {
    let t = lang(st);
    let tip = Tip::new(t.tr("toolbar.stash.name"));
    let Some(snap) = &st.snap else { return tip };
    let files = rules::changed_files(snap).len();
    let tip = if files > 0 { tip.state(t.trn("toolbar.tip.files_changed", files, &[])) } else { tip };
    match rules::stash_blocked(st) {
        Some(why) => tip.body(why),
        None => tip.body(t.tr("toolbar.tip.stash_body")).preview(rules::stash_preview(t, snap)),
    }
}

/// Pop: how many stashes there are, the one it applies, and what popping does.
pub fn pop(st: &AppState) -> Tip {
    let t = lang(st);
    let tip = Tip::new(t.tr("toolbar.pop.name"));
    let Some(snap) = &st.snap else { return tip };
    let n = snap.stashes.len();
    let tip = if n > 0 { tip.state(t.trn("toolbar.tip.stashes", n, &[])) } else { tip };
    match rules::pop_blocked(st) {
        Some(why) => tip.body(why),
        None => tip.body(t.tr("toolbar.tip.pop_body")).preview(rules::pop_preview(t, snap)),
    }
}

/// PR: "Create a pull request on GitHub for this branch" and what the button opens.
pub fn pr(st: &AppState, fs: &ForgeState) -> Tip {
    let t = lang(st);
    let service = fs.host_provider().map(Provider::name).unwrap_or_default();
    let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
    let title = if fs.pr_noun() == "merge request" {
        t.trf("toolbar.pr.tip_merge_request", args)
    } else {
        t.trf("toolbar.pr.tip_pull_request", args)
    };
    Tip::new(title).body(rules::pr_blocked(st).unwrap_or_else(|| t.tr("toolbar.tip.pr_body").to_owned()))
}

/// Terminal: show or hide the panel, its keys, and what a right-click offers.
pub fn terminal(st: &AppState) -> Tip {
    let t = lang(st);
    let title = if st.terminal.open { t.tr("toolbar.terminal.name_hide") } else { t.tr("toolbar.terminal.name_show") };
    let body =
        if st.settings.terminal.open_in_panel { t.tr("toolbar.tip.terminal_tab_body") } else { t.tr("toolbar.tip.terminal_external_body") };
    Tip::new(title).keys(BindingId::ToggleTerminal).body(body)
}

pub fn folder(st: &AppState) -> Tip {
    let t = lang(st);
    Tip::new(t.tr("toolbar.folder.name")).body(t.tr("toolbar.tip.folder_body"))
}

pub fn refresh(st: &AppState) -> Tip {
    let t = lang(st);
    Tip::new(t.tr("toolbar.refresh.tip")).keys(BindingId::Refresh).body(t.tr("toolbar.tip.refresh_body"))
}

pub fn search(st: &AppState) -> Tip {
    let t = lang(st);
    Tip::new(t.tr("toolbar.search.tip")).keys(BindingId::Search).body(t.tr("toolbar.tip.search_body"))
}

pub fn palette(st: &AppState) -> Tip {
    let t = lang(st);
    Tip::new(t.tr("toolbar.commands.tip")).keys(BindingId::Palette).body(t.tr("toolbar.tip.commands_body"))
}

pub fn settings(st: &AppState) -> Tip {
    let t = lang(st);
    Tip::new(t.tr("toolbar.settings.tip")).keys(BindingId::Prefs).body(t.tr("toolbar.tip.settings_body"))
}

pub fn activity(st: &AppState) -> Tip {
    let t = lang(st);
    let title = if st.view.show_log { t.tr("toolbar.tip.activity_hide") } else { t.tr("toolbar.tip.activity_show") };
    Tip::new(title).body(t.tr("toolbar.tip.activity_body"))
}

#[cfg(test)]
#[path = "toolbar_tips_tests.rs"]
mod tests;
