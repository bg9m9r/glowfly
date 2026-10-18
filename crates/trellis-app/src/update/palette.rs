//! The command palette: opening and closing it, moving the selection, running a command
//! through the message that the rest of the app already uses for it (remembering plain
//! commands among the recently run ones), and the second steps that
//! two-step commands open: the file finder (loading its items in the background), the stash
//! sub-lists (apply, pop, delete and rename), the profile sub-list (switching profiles), the
//! assigned-issue sub-lists (starting a Git Flow branch from an issue), the stash message step
//! and the commit count of a shallow clone's fetch. The Stash dialog's entries open that dialog.

use crate::effect::Effect;
use crate::msg::BlameMsg;
use crate::msg::BranchMsg;
use crate::msg::ConflictMsg;
use crate::msg::DiffMsg;
use crate::msg::ForgeMsg;
use crate::msg::HistoryMsg;
use crate::msg::IssueMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::msg::SearchMsg;
use crate::msg::StageMsg;
use crate::msg::StashMsg;
use crate::msg::TabsMsg;
use crate::msg::TrustMsg;
use crate::state::conflict_files::{conflicted_paths, mark_conflicted_body, restorable_paths};
use crate::state::palette::{self, InputKind, Items, Mode, Palette, PaletteCommand, Pick, PickKind, StashAction};
use crate::state::{AppState, PrefPage, Selection};
use crate::tag::TagMsg;
use std::time::Instant;
use trellis_core::repo::StashInfo;
use trellis_core::shallow_ops::Deepen;

pub(super) fn handle(st: &mut AppState, msg: PaletteMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        PaletteMsg::TogglePalette => {
            if st.palette.is_some() {
                st.palette = None;
            } else if may_open(st) {
                st.palette = Some(Palette::default());
            }
        }
        PaletteMsg::ClosePalette => st.palette = None,
        PaletteMsg::PaletteMove(step) => {
            let len = palette::row_count(st);
            let Some(p) = st.palette.as_mut() else { return };
            p.selected = if len == 0 { 0 } else { (p.selected.min(len - 1) as isize + step).rem_euclid(len as isize) as usize };
            p.scroll_to_selected = true;
        }
        PaletteMsg::RunPaletteCommand(cmd) => {
            // Two-step commands count when their step opens; picks and typed text never do.
            if let Some(key) = cmd.recall_key() {
                st.palette_recent.record(key);
            }
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
            // Listed with the reason while creating is blocked; Enter leaves the palette open.
            if cmd == PaletteCommand::CreateIssue && crate::state::issue_form::create_blocked(st).is_some() {
                return;
            }
            st.palette = None;
            run(st, cmd, now, fx);
        }
        PaletteMsg::PaletteBack => {
            if let Some(p) = st.palette.as_mut()
                && !matches!(p.mode, Mode::Commands)
            {
                p.mode = Mode::Commands;
                p.query.clear();
                p.selected = 0;
            }
        }
        PaletteMsg::TrackedPathsLoaded { generation, result } => {
            if generation != st.palette_gen {
                return;
            }
            let Some(Palette { mode: Mode::Pick(pick), selected, .. }) = st.palette.as_mut() else { return };
            if !pick.kind.lists_files() {
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
/// tracked files load in the background, the stashes come from the snapshot, the profiles from
/// settings.
fn enter_pick(st: &mut AppState, kind: PickKind, fx: &mut Vec<Effect>) {
    match kind {
        PickKind::FileHistory | PickKind::Blame => {
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
        PickKind::RenameStash => {
            if !palette::can_pick_stash(st) {
                return;
            }
            let list = st.snap.as_ref().map(|s| s.stashes.clone()).unwrap_or_default();
            open_step(st, Mode::Pick(Pick::stashes_to_rename(&list)));
        }
        PickKind::Profile => {
            if st.settings.profiles.len() < 2 || st.profile_switch.is_some() {
                return;
            }
            let pick = Pick::profiles(&st.settings);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::GitFlowIssue(kind) => {
            if !crate::state::gitflow::issues_ready(st) {
                return;
            }
            let Some(f) = st.forge.as_ref() else { return };
            let pick = Pick::issues(kind, f);
            open_step(st, Mode::Pick(pick));
        }
        PickKind::ViewIssue => {
            if !crate::state::palette_commands::palette_issues::can_view(st) {
                return;
            }
            let Some(f) = st.forge.as_ref() else { return };
            let pick = Pick::issues_to_view(f);
            open_step(st, Mode::Pick(pick));
        }
    }
}

/// Switches the palette to the free-text step `kind` (opening it if closed).
fn enter_input(st: &mut AppState, kind: InputKind) {
    let ok = match kind {
        InputKind::StashMessage => palette::can_stash(st),
        InputKind::DeepenBy => st.can_fetch_history(),
    };
    if ok {
        open_step(st, Mode::Input(kind));
    }
}

/// Runs `cmd` through the message the toolbar, menus and sidebar already send for it.
fn run(st: &mut AppState, cmd: PaletteCommand, now: Instant, fx: &mut Vec<Effect>) {
    use PaletteCommand as C;
    let msg = match cmd {
        C::SearchCommits => Msg::Search(SearchMsg::Open),
        C::Fetch => Msg::git("Fetch all", &["fetch", "--all", "--prune"]),
        C::Pull => Msg::git("Pull", &["pull", "--ff"]),
        C::FetchFullHistory => Msg::Job(JobMsg::Deepen(Deepen::Full)),
        C::FetchMoreCommitsBy(n) => Msg::Job(JobMsg::Deepen(Deepen::By(n))),
        C::Push => Msg::Job(JobMsg::Push { force: false }),
        C::Stash => Msg::git("Stash", &["stash", "push", "-u"]),
        C::PopStash => Msg::git("Pop stash", &["stash", "pop"]),
        C::StashOp { action, oid } => {
            let Some(s) = st.snap.as_ref().and_then(|s| s.stashes.iter().find(|x| x.oid == oid)).cloned() else {
                st.log(false, "That stash no longer exists.");
                return;
            };
            stash_msg(action, &s)
        }
        C::RenameStash { oid } => {
            if !st.snap.as_ref().is_some_and(|s| s.stashes.iter().any(|x| x.oid == oid)) {
                st.log(false, "That stash no longer exists.");
                return;
            }
            Msg::Stash(StashMsg::RenameDialog { oid })
        }
        C::StashWithMessage(text) => {
            let t = text.trim();
            if t.is_empty() {
                return;
            }
            Msg::git("Stash", &["stash", "push", "-u", "-m", t])
        }
        C::StashDialog(scope) => Msg::Stash(StashMsg::PushDialog { scope, path: None }),
        C::StageAll => Msg::Stage(StageMsg::StageAll),
        C::UnstageAll => Msg::Stage(StageMsg::UnstageAll),
        // The handlers check the paths again.
        C::MarkAllResolved => {
            let Some(snap) = &st.snap else { return };
            Msg::Conflict(ConflictMsg::MarkResolved(conflicted_paths(snap)))
        }
        C::MarkAllConflicted => {
            let paths = restorable_paths(st);
            if paths.is_empty() {
                return;
            }
            mark_conflicted(paths)
        }
        C::MarkConflicted(path) => mark_conflicted(vec![path]),
        C::Undo => Msg::Job(JobMsg::Undo),
        C::Redo => Msg::Job(JobMsg::Redo),
        C::CreateBranch => Msg::Branch(BranchMsg::NewBranch { start: selected_commit(st).map(|o| o.to_string()), name: String::new() }),
        C::CreateTag { annotated } => {
            let Some(at) = selected_commit(st).or_else(|| st.snap.as_ref().and_then(|s| s.head_oid)) else { return };
            Msg::Tag(TagMsg::NewTag { at: at.to_string(), annotated })
        }
        C::RenameBranch(name) => Msg::Branch(BranchMsg::RenameBranchDialog(name)),
        C::CreatePr(b) => Msg::Forge(ForgeMsg::CreatePrDialog { source: b, target: None }),
        C::PushAndCreatePr(b) => Msg::Forge(ForgeMsg::PushAndStartPr { branch: b, target: None }),
        C::InitializeLfs => Msg::Lfs(crate::lfs::LfsMsg::InitializeWhenReady),
        C::ConfigureLfs => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)),
        C::ConfigureGitFlow => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)),
        C::ConfigureSparse => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)),
        C::StartGitFlow(k) => Msg::GitFlow(crate::gitflow::GitFlowMsg::StartDialog(k)),
        C::FinishGitFlow(b) => Msg::GitFlow(crate::gitflow::GitFlowMsg::FinishDialog(b)),
        C::OpenGitFlowPanel => Msg::GitFlow(crate::gitflow::GitFlowMsg::OpenPanel),
        C::StartGitFlowFromIssue { kind, name } => Msg::GitFlow(crate::gitflow::GitFlowMsg::StartFromIssue { kind, name }),
        C::CreateIssue => Msg::Issue(IssueMsg::CreateDialog),
        C::ViewIssue(key) => Msg::Issue(IssueMsg::Open(key)),
        C::Refresh => Msg::Repo(RepoMsg::Refresh),
        C::OpenTerminal => Msg::OpenTerminal,
        C::ToggleTerminal => Msg::Terminal(crate::terminal::TerminalMsg::Toggle),
        C::ShowInFileManager => {
            let Some(w) = st.workdir() else { return };
            Msg::OpenFolder(w)
        }
        C::DiffLayout(l) => Msg::Diff(DiffMsg::SetDiffLayout(l)),
        C::IgnoreWhitespace(on) => Msg::Diff(DiffMsg::SetIgnoreWhitespace(on)),
        C::OpenRepo => Msg::Repo(RepoMsg::PickRepo),
        C::NewTab => Msg::Tabs(TabsMsg::NewTab),
        C::ReopenClosedTab => Msg::Tabs(TabsMsg::ReopenClosedTab),
        C::CloseTab => Msg::Tabs(TabsMsg::CloseTab(usize::MAX)),
        C::OpenPrefs => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)),
        C::KeyboardShortcuts => Msg::Shortcut(crate::keymap::KeyAction::ToggleSheet),
        C::ToolbarLabels(on) => Msg::Prefs(PrefsMsg::SetToolbarLabels(on)),
        C::Theme(t) => Msg::Prefs(PrefsMsg::SetTheme(t)),
        C::Checkout(name) => Msg::Branch(crate::msg::BranchMsg::Checkout(name)),
        C::CheckoutRemote { remote, branch } => Msg::git("Checkout", &["checkout", "--track", &format!("{remote}/{branch}")]),
        C::CheckoutTag(name) => Msg::git("Checkout", &["checkout", &format!("refs/tags/{name}")]),
        C::OpenRecent(path) => Msg::Repo(RepoMsg::Open(path)),
        C::CopyLink(t) => Msg::DeepLink(crate::deep_link::DeepLinkMsg::Copy(t)),
        C::OpenFileHistory(path) => Msg::History(HistoryMsg::OpenFileHistory { path, at: None }),
        C::OpenBlame(path) => Msg::Blame(BlameMsg::Open { path, at: None }),
        C::Remote(m) => Msg::Remote(m),
        C::Submodule(m) => Msg::Submodule(m),
        C::Worktree(m) => Msg::Worktree(*m),
        C::Tools(m) => Msg::Tools(m),
        C::Layout(m) => Msg::Layout(m),
        C::Logs(m) => Msg::Logs(m),
        C::Onboarding(m) => Msg::Onboarding(m),
        C::Dashboard => Msg::Dashboard(crate::msg::DashboardMsg::Open),
        C::Updates(m) => Msg::Updates(m),
        C::ApplyPatch => Msg::Patch(crate::patch::PatchMsg::PickToApply),
        C::Patch(m) => Msg::Patch(m),
        C::SwitchProfile(id) => Msg::Profile(crate::profile::ProfileMsg::Switch(id)),
        C::ReviewTrust => {
            let Some(s) = &st.snap else { return };
            Msg::Trust(TrustMsg::Ask { real: s.real_workdir.clone() })
        }
        // Opens a step in `handle`; never run directly.
        C::FileHistory
        | C::Blame
        | C::ChooseStash(_)
        | C::ChooseStashToRename
        | C::ChooseProfile
        | C::ChooseGitFlowIssue(_)
        | C::ChooseIssueToView
        | C::StashMessage
        | C::FetchMoreCommits => return,
    };
    fx.extend(super::update(st, msg, now));
}

/// The Resolved Files list's question before Mark conflicted.
fn mark_conflicted(paths: Vec<String>) -> Msg {
    let body = mark_conflicted_body(&paths);
    Msg::confirm("Mark conflicted", body, "Mark conflicted", Msg::Conflict(ConflictMsg::MarkConflicted(paths)))
}

/// The same messages the sidebar's STASHES menu sends.
fn stash_msg(action: StashAction, s: &StashInfo) -> Msg {
    let git = Msg::Job(JobMsg::Stash { action, index: s.index, oid: s.oid });
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
