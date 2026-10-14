//! The command palette: opening and closing it, moving the selection, running a command
//! through the message that the rest of the app already uses for it, and the second steps that
//! two-step commands open: the file finder (loading its items in the background), the stash
//! sub-lists and the stash message step.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::palette::{self, InputKind, Items, Mode, Palette, PaletteCommand, Pick, PickKind, StashAction};
use crate::state::{AppState, PrefPage, Selection};
use std::time::Instant;
use trellis_core::repo::StashInfo;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::TogglePalette => {
            if st.palette.is_some() {
                st.palette = None;
            } else if may_open(st) {
                st.palette = Some(Palette::default());
            }
        }
        Msg::ClosePalette => st.palette = None,
        Msg::PaletteMove(step) => {
            let len = palette::row_count(st);
            let Some(p) = st.palette.as_mut() else { return };
            p.selected = if len == 0 { 0 } else { (p.selected.min(len - 1) as isize + step).rem_euclid(len as isize) as usize };
            p.scroll_to_selected = true;
        }
        Msg::RunPaletteCommand(cmd) => {
            if let Some(kind) = cmd.pick_kind() {
                enter_pick(st, kind, fx);
                return;
            }
            if let Some(kind) = cmd.input_kind() {
                enter_input(st, kind);
                return;
            }
            // A blank message stashes nothing and leaves the step open.
            if let PaletteCommand::StashWithMessage(t) = &cmd
                && t.trim().is_empty()
            {
                return;
            }
            st.palette = None;
            run(st, cmd, now, fx);
        }
        Msg::PaletteBack => {
            if let Some(p) = st.palette.as_mut()
                && !matches!(p.mode, Mode::Commands)
            {
                p.mode = Mode::Commands;
                p.query.clear();
                p.selected = 0;
            }
        }
        Msg::TrackedPathsLoaded { generation, result } => {
            if generation != st.palette_gen {
                return;
            }
            let Some(Palette { mode: Mode::Pick(pick), selected, .. }) = st.palette.as_mut() else { return };
            if pick.kind != PickKind::FileHistory {
                return;
            }
            *selected = 0;
            match result {
                Ok(paths) => pick.set_items(Items::Ready(paths)),
                Err(e) => {
                    pick.set_items(Items::Failed(e.clone()));
                    st.log(false, e);
                }
            }
        }
        _ => {}
    }
}

/// Whether the palette may open now: not over a dialog, Preferences or a credential prompt.
fn may_open(st: &AppState) -> bool {
    st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing()
}

/// Switches the palette to `mode` with an empty query, opening it if closed (unless something
/// covers the window). Returns whether the step opened.
fn open_step(st: &mut AppState, mode: Mode) -> bool {
    if st.palette.is_none() {
        if !may_open(st) {
            return false;
        }
        st.palette = Some(Palette::default());
    }
    let Some(p) = st.palette.as_mut() else { return false };
    p.mode = mode;
    p.query.clear();
    p.selected = 0;
    p.scroll_to_selected = false;
    st.palette_gen += 1;
    true
}

/// Switches the palette to the sub-list for `kind` (opening it if closed) and fills it: the
/// tracked files load in the background, the stashes come from the snapshot.
fn enter_pick(st: &mut AppState, kind: PickKind, fx: &mut Vec<Effect>) {
    match kind {
        PickKind::FileHistory => {
            if !palette::can_pick_file(st) {
                return;
            }
            let Some(workdir) = st.workdir() else { return };
            if open_step(st, Mode::Pick(Pick::new(kind))) {
                fx.push(Effect::LoadTrackedPaths { workdir, generation: st.palette_gen });
            }
        }
        PickKind::Stash(action) => {
            if !palette::can_pick_stash(st) {
                return;
            }
            let list = st.snap.as_ref().map(|s| s.stashes.clone()).unwrap_or_default();
            open_step(st, Mode::Pick(Pick::stashes(action, &list)));
        }
    }
}

/// Switches the palette to the free-text step `kind` (opening it if closed).
fn enter_input(st: &mut AppState, kind: InputKind) {
    let ok = match kind {
        InputKind::StashMessage => palette::can_stash(st),
    };
    if ok {
        open_step(st, Mode::Input(kind));
    }
}

/// Runs `cmd` through the message the toolbar, menus and sidebar already send for it.
fn run(st: &mut AppState, cmd: PaletteCommand, now: Instant, fx: &mut Vec<Effect>) {
    use PaletteCommand as C;
    let msg = match cmd {
        C::SearchCommits => {
            if st.diff.is_some() {
                fx.extend(super::update(st, Msg::CloseDiff, now));
            }
            st.view.show_search = true;
            st.view.focus_search = true;
            return;
        }
        C::Fetch => Msg::git("Fetch all", &["fetch", "--all", "--prune"]),
        C::Pull => Msg::git("Pull", &["pull", "--ff"]),
        C::Push => Msg::Push { force: false },
        C::Stash => Msg::git("Stash", &["stash", "push", "-u"]),
        C::PopStash => Msg::git("Pop stash", &["stash", "pop"]),
        C::StashOp { action, oid } => {
            let Some(s) = st.snap.as_ref().and_then(|s| s.stashes.iter().find(|x| x.oid == oid)).cloned() else {
                st.log(false, "That stash no longer exists.");
                return;
            };
            stash_msg(action, &s)
        }
        C::StashWithMessage(text) => {
            let t = text.trim();
            if t.is_empty() {
                return;
            }
            Msg::git("Stash", &["stash", "push", "-u", "-m", t])
        }
        C::StageAll => Msg::StageAll,
        C::UnstageAll => Msg::UnstageAll,
        C::Undo => Msg::Undo,
        C::Redo => Msg::Redo,
        C::CreateBranch => Msg::NewBranch { start: selected_commit(st).map(|o| o.to_string()), name: String::new() },
        C::CreateTag { annotated } => {
            let Some(at) = selected_commit(st).or_else(|| st.snap.as_ref().and_then(|s| s.head_oid)) else { return };
            Msg::NewTag { at: at.to_string(), annotated }
        }
        C::RenameBranch(name) => Msg::RenameBranchDialog(name),
        C::InitializeLfs => Msg::Lfs(crate::lfs::LfsMsg::InitializeWhenReady),
        C::ConfigureLfs => Msg::OpenPrefs(PrefPage::RepoLfs),
        C::ConfigureGitFlow => Msg::OpenPrefs(PrefPage::RepoGitFlow),
        C::StartGitFlow(k) => Msg::GitFlow(crate::gitflow::GitFlowMsg::StartDialog(k)),
        C::Refresh => Msg::Refresh,
        C::OpenTerminal => Msg::OpenTerminal,
        C::ToggleTerminal => Msg::Terminal(crate::terminal::TerminalMsg::Toggle),
        C::ShowInFileManager => {
            let Some(w) = st.workdir() else { return };
            Msg::OpenFolder(w)
        }
        C::DiffLayout(l) => Msg::SetDiffLayout(l),
        C::IgnoreWhitespace(on) => Msg::SetIgnoreWhitespace(on),
        C::OpenRepo => Msg::PickRepo,
        C::NewTab => Msg::NewTab,
        C::ReopenClosedTab => Msg::ReopenClosedTab,
        C::CloseTab => Msg::CloseTab(usize::MAX),
        C::OpenPrefs => Msg::OpenPrefs(PrefPage::General),
        C::KeyboardShortcuts => Msg::Shortcut(crate::keymap::KeyAction::ToggleSheet),
        C::ToolbarLabels(on) => Msg::SetToolbarLabels(on),
        C::Theme(t) => Msg::SetTheme(t),
        C::Checkout(name) => Msg::git("Checkout", &["checkout", &name]),
        C::CheckoutRemote { remote, branch } => Msg::git("Checkout", &["checkout", "--track", &format!("{remote}/{branch}")]),
        C::CheckoutTag(name) => Msg::git("Checkout", &["checkout", &format!("refs/tags/{name}")]),
        C::OpenRecent(path) => Msg::Open(path),
        C::OpenFileHistory(path) => Msg::OpenFileHistory { path, at: None },
        // Opens a step in `handle`; never run directly.
        C::FileHistory | C::ChooseStash(_) | C::StashMessage => return,
    };
    fx.extend(super::update(st, msg, now));
}

/// The same messages the sidebar's STASHES menu sends.
fn stash_msg(action: StashAction, s: &StashInfo) -> Msg {
    let refname = format!("stash@{{{}}}", s.index);
    let git = Msg::git(action.job_label(), &["stash", action.git_verb(), &refname]);
    match action {
        StashAction::Drop => Msg::confirm("Delete stash", format!("Delete \"{}\"? This cannot be undone.", s.message), "Delete", git),
        StashAction::Apply | StashAction::Pop => git,
    }
}

/// The single selected commit, if any (WIP, nothing or several commits give None).
fn selected_commit(st: &AppState) -> Option<git2::Oid> {
    match &st.selection {
        Selection::Commit(o) => Some(*o),
        _ => None,
    }
}
