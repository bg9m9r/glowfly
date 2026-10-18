//! The egui layer. Views draw `AppState` and push `Msg`s; the shell feeds those messages and
//! background results through `update`, runs the resulting effects, and handles the few
//! effects that need the window.

mod activity_log;
mod am_banner;
mod askpass;
mod avatars;
mod blame;
mod branch_dialogs;
mod branch_menu;
mod build_status;
mod central;
mod clone_dialog;
mod clone_service;
mod commit_box;
pub(crate) mod common;
mod conflict;
mod conflict_check;
mod conflict_files;
mod conflict_output;
mod conflict_panes;
mod dashboard;
mod dashboard_rows;
mod date_tip;
mod deep_link_dialog;
mod details;
mod details_multi;
mod diff;
mod file_buttons;
mod file_dialogs;
mod file_menu;
mod forge_filter_dialog;
mod forge_filter_menu;
mod git_banner;
mod gitflow_dialog;
mod gitflow_finish_dialog;
mod gitflow_panel;
mod gitflow_section;
mod graph;
mod history;
mod history_file;
mod history_folder;
mod icons;
mod init_dialog;
mod issue;
mod layout;
mod list_row;
mod menu_bar;
mod menus;
mod modals;
mod palette;
mod patch_apply_dialog;
mod patch_save_dialog;
mod path_warnings;
mod pr_menu_items;
mod prefs;
mod publish;
mod pull_request;
mod rebase;
mod ref_visibility;
mod release_notes;
mod remotes;
mod repo_finder;
mod repo_list;
mod repo_list_drag;
mod repo_menu;
mod repo_sync;
mod shortcuts;
mod sidebar;
mod signature_badge;
mod spell_edit;
mod stash_dialog;
mod stash_push_dialog;
mod status;
mod submodule_dialogs;
mod submodules;
mod tab_dialog;
mod tab_list;
mod tabs;
mod tag_dialog;
mod tags;
mod terminal;
pub mod theme;
mod theme_names;
mod toolbar;
mod tour;
mod trust_chip;
mod trust_dialog;
mod undo_dialog;
mod update_notice;
mod welcome;
mod welcome_steps;
pub mod widgets;
mod worktree_dialog;
mod worktree_manage_dialog;
mod worktree_move_dialog;
mod worktrees;

#[cfg(test)]
mod hint_tests;
#[cfg(test)]
mod snapshot_check;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod snapshot_tests_askpass;
#[cfg(test)]
mod snapshot_tests_blame;
#[cfg(test)]
mod snapshot_tests_blame_menu;
#[cfg(test)]
mod snapshot_tests_branch;
#[cfg(test)]
mod snapshot_tests_clone;
#[cfg(test)]
mod snapshot_tests_clone_service;
#[cfg(test)]
mod snapshot_tests_columns;
#[cfg(test)]
mod snapshot_tests_commit;
#[cfg(test)]
mod snapshot_tests_conflict_check;
#[cfg(test)]
mod snapshot_tests_conflict_files;
#[cfg(test)]
mod snapshot_tests_dashboard;
#[cfg(test)]
mod snapshot_tests_dates;
#[cfg(test)]
mod snapshot_tests_deep_link;
#[cfg(test)]
mod snapshot_tests_diff;
#[cfg(test)]
mod snapshot_tests_editor;
#[cfg(test)]
mod snapshot_tests_esc;
#[cfg(test)]
mod snapshot_tests_file_actions;
#[cfg(test)]
mod snapshot_tests_file_dialogs;
#[cfg(test)]
mod snapshot_tests_file_menu;
#[cfg(test)]
mod snapshot_tests_file_menu_lfs;
#[cfg(test)]
mod snapshot_tests_forge_filters;
#[cfg(test)]
mod snapshot_tests_forks;
#[cfg(test)]
mod snapshot_tests_gitflow;
#[cfg(test)]
mod snapshot_tests_history;
#[cfg(test)]
mod snapshot_tests_init;
#[cfg(test)]
mod snapshot_tests_issue;
#[cfg(test)]
mod snapshot_tests_issue_create;
#[cfg(test)]
mod snapshot_tests_language_chrome;
#[cfg(test)]
mod snapshot_tests_lfs;
#[cfg(test)]
mod snapshot_tests_load_more;
#[cfg(test)]
mod snapshot_tests_logs;
#[cfg(test)]
mod snapshot_tests_onboarding;
#[cfg(test)]
mod snapshot_tests_palette;
#[cfg(test)]
mod snapshot_tests_palette_recent;
#[cfg(test)]
mod snapshot_tests_palette_stash;
#[cfg(test)]
mod snapshot_tests_patch;
#[cfg(test)]
mod snapshot_tests_path_warnings;
#[cfg(test)]
mod snapshot_tests_platform_dialogs;
#[cfg(test)]
mod snapshot_tests_pr_checks;
#[cfg(test)]
mod snapshot_tests_prefs_git;
#[cfg(test)]
mod snapshot_tests_prefs_keyboard;
#[cfg(test)]
mod snapshot_tests_prefs_notify;
#[cfg(test)]
mod snapshot_tests_prefs_terminal;
#[cfg(test)]
mod snapshot_tests_profiles;
#[cfg(test)]
mod snapshot_tests_pull_request;
#[cfg(test)]
mod snapshot_tests_pull_request_review;
#[cfg(test)]
mod snapshot_tests_pull_request_threads;
#[cfg(test)]
mod snapshot_tests_push_pr;
#[cfg(test)]
mod snapshot_tests_rebase;
#[cfg(test)]
mod snapshot_tests_rebase_edit;
#[cfg(test)]
mod snapshot_tests_rebase_keys;
#[cfg(test)]
mod snapshot_tests_rebase_steps;
#[cfg(test)]
mod snapshot_tests_remotes;
#[cfg(test)]
mod snapshot_tests_repo_finder;
#[cfg(test)]
mod snapshot_tests_repo_list;
#[cfg(test)]
mod snapshot_tests_repo_sync;
#[cfg(test)]
mod snapshot_tests_revert;
#[cfg(test)]
mod snapshot_tests_search;
#[cfg(test)]
mod snapshot_tests_select;
#[cfg(test)]
mod snapshot_tests_shallow;
#[cfg(test)]
mod snapshot_tests_signature;
#[cfg(test)]
mod snapshot_tests_sparse;
#[cfg(test)]
mod snapshot_tests_ssh;
#[cfg(test)]
mod snapshot_tests_stash;
#[cfg(test)]
mod snapshot_tests_stash_push;
#[cfg(test)]
mod snapshot_tests_submodule_dialogs;
#[cfg(test)]
mod snapshot_tests_submodules;
#[cfg(test)]
mod snapshot_tests_tag;
#[cfg(test)]
mod snapshot_tests_tag_annotate;
#[cfg(test)]
mod snapshot_tests_tag_menu;
#[cfg(test)]
mod snapshot_tests_terminal;
#[cfg(test)]
mod snapshot_tests_terminal_select;
#[cfg(test)]
mod snapshot_tests_terminal_tabs;
#[cfg(test)]
mod snapshot_tests_themes;
#[cfg(test)]
mod snapshot_tests_toasts;
#[cfg(test)]
mod snapshot_tests_tools;
#[cfg(test)]
mod snapshot_tests_trello;
#[cfg(test)]
mod snapshot_tests_trust;
#[cfg(test)]
mod snapshot_tests_undo;
#[cfg(test)]
mod snapshot_tests_updates;
#[cfg(test)]
mod snapshot_tests_whole;
#[cfg(test)]
mod snapshot_tests_worktree_branches;
#[cfg(test)]
mod snapshot_tests_worktree_manage;
#[cfg(test)]
mod snapshot_tests_worktree_move;
#[cfg(test)]
mod snapshot_tests_worktree_wip;
#[cfg(test)]
mod snapshot_tests_worktrees;
#[cfg(test)]
mod snapshot_tests_wrap;
#[cfg(test)]
mod tabs_dialog_tests;
#[cfg(test)]
mod tabs_list_tests;
#[cfg(test)]
mod tabs_tests;

use crate::effect::{Effect, UiEffect};
use crate::msg::AvatarMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RecentMsg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::runner::Runner;
use crate::state::AppState;
use crate::update::update;
#[cfg(test)]
use central::escape_key;
use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use trellis_core::settings::Settings;
use trellis_core::spell::Speller;

/// Resources owned by the UI thread (textures, the spell checker's COM object).
pub struct UiCache {
    pub avatars: avatars::Avatars,
    pub speller: Speller,
    pub diff_rows: diff::rows_cache::DiffRowsCache,
}

pub struct TrellisApp {
    st: AppState,
    cache: UiCache,
    runner: Runner<egui::Context>,
    results: Receiver<Msg>,
    /// The palette for the chosen theme, set at the start of every frame.
    theme: theme::ThemeCache,
}

impl TrellisApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start: Option<crate::StartArg>, logs: crate::logging::LogSetup) -> Self {
        let (tx, results) = mpsc::channel();
        let (settings, problem) = Settings::load();
        // The git chosen in Preferences > Git runs from the first git call on (no disk access here).
        trellis_core::git_program::set(trellis_core::git_program::from_setting(settings.git_program.as_deref()).as_deref());
        let recent = crate::platform::load_recent();
        // Restore the last session's tabs (or, before tabs were saved, the last repository).
        let (mut paths, labels, active) =
            if settings.reopen_last_repo { crate::platform::load_tabs() } else { (Vec::new(), Vec::new(), 0) };
        if paths.is_empty() && settings.reopen_last_repo {
            // Not checked here (a network share may not answer): a folder that is gone fails to
            // load like any other.
            paths.extend(recent.first().cloned());
        }
        let mut app = Self {
            st: AppState::new(settings, recent, Instant::now()),
            cache: UiCache { avatars: avatars::Avatars::default(), speller: Speller::new(), diff_rows: Default::default() },
            runner: Runner::new(tx, cc.egui_ctx.clone()),
            results,
            theme: theme::ThemeCache::default(),
        };
        // Read now, not through the runner, so the first frame already has the theme's colors;
        // the folder sits next to the settings file just read.
        let themes = trellis_core::settings::themes::load_all();
        crate::themes::trace_problems(&themes);
        app.dispatch(&cc.egui_ctx, Msg::Theme(crate::themes::ThemeMsg::Loaded(themes)));
        layout::start(&cc.egui_ctx, &app.st);
        // git and ssh ask for missing credentials through a dialog from now on.
        app.runner.start_askpass();
        // The log files, and earlier sessions' activity.
        app.runner.attach_logs(logs);
        app.dispatch(&cc.egui_ctx, Msg::Logs(crate::logs::LogsMsg::Start));
        // Links opened while this window runs come here instead of a new window.
        app.runner.start_instance();
        app.dispatch(&cc.egui_ctx, Msg::Lfs(crate::lfs::LfsMsg::Detect));
        app.dispatch(&cc.egui_ctx, Msg::GitEnv(crate::git_env::GitEnvMsg::Detect));
        app.dispatch(&cc.egui_ctx, Msg::Onboarding(crate::onboarding::OnboardingMsg::Startup));
        app.dispatch(&cc.egui_ctx, Msg::Updates(crate::msg::UpdateMsg::Startup));
        app.dispatch(&cc.egui_ctx, Msg::Recent(RecentMsg::CheckFolders));
        if let Some(problem) = problem {
            tracing::warn!("settings: {}", problem.message());
            app.dispatch(&cc.egui_ctx, Msg::Prefs(PrefsMsg::SettingsUnreadable(problem.message())));
        }
        if !paths.is_empty() {
            app.dispatch(&cc.egui_ctx, Msg::Tabs(TabsMsg::RestoreTabs { paths, labels, active }));
        }
        match start {
            Some(crate::StartArg::Path(path)) => app.dispatch(&cc.egui_ctx, Msg::Repo(RepoMsg::Open(path))),
            Some(crate::StartArg::Link(text)) => {
                app.dispatch(&cc.egui_ctx, Msg::DeepLink(crate::deep_link::DeepLinkMsg::Open(text)));
            }
            None => {}
        }
        app
    }

    fn dispatch(&mut self, ctx: &egui::Context, msg: Msg) {
        for effect in update(&mut self.st, msg, Instant::now()) {
            match effect {
                Effect::Ui(UiEffect::CopyText(text)) => ctx.copy_text(text),
                Effect::Ui(UiEffect::SetTitle(title)) => ctx.send_viewport_cmd(egui::ViewportCommand::Title(title)),
                Effect::Ui(UiEffect::RequestAttention) => request_attention(ctx),
                Effect::Ui(UiEffect::CloseWindow) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Effect::Ui(UiEffect::RequestPaste) => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                Effect::Ui(UiEffect::FocusWindow) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    request_attention(ctx);
                }
                other => self.runner.run(other),
            }
        }
    }

    /// Keyboard shortcuts and window events, turned into messages or view-state changes.
    fn input(&mut self, ctx: &egui::Context, msgs: &mut Vec<Msg>) {
        let focused = self.st.view.focused;
        if focused && !self.st.view.was_focused {
            msgs.push(Msg::Repo(RepoMsg::WindowFocused));
        }
        self.st.view.was_focused = focused;
        shortcuts::input(ctx, &mut self.st, msgs);
    }
}

/// Flashes the taskbar entry while the window isn't focused.
fn request_attention(ctx: &egui::Context) {
    if !ctx.input(|i| i.focused) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
    }
}

/// Draws one frame of the whole window from `st`, collecting the user's intents in `msgs`.
pub fn draw(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    status::status_bar(ctx, st, msgs);
    if st.prefs.is_some() {
        // The terminal isn't drawn, so it can't hold the keyboard (and Esc closes Preferences).
        st.terminal.focused = false;
        prefs::show(ctx, st, msgs);
    } else {
        menu_bar::show(ctx, st, msgs);
        tabs::show(ctx, st, msgs);
        if st.settings.show_toolbar {
            toolbar::show(ctx, st, msgs);
        }
        git_banner::show(ctx, st, msgs);
        activity_log::show(ctx, st, msgs);
        if st.snap.is_some() && !st.dashboard.open {
            if st.settings.show_left_panel {
                sidebar::show(ctx, st, msgs);
            }
            if st.settings.show_details_panel {
                details::show(ctx, st, cache, msgs);
            }
            terminal::show(ctx, st, msgs);
        } else {
            st.terminal.focused = false;
        }
        let central = central::show(ctx, st, cache, msgs);
        tour::show(ctx, st, central, msgs);
    }
    // Before the dialogs, so a dialog or credential prompt opened over them stacks on top.
    palette::show(ctx, st, msgs);
    welcome::show(ctx, st, msgs);
    modals::show(ctx, st, msgs);
    // After the other dialogs, so a credential prompt stacks on top of them.
    askpass::show(ctx, st, msgs);
    status::toasts(ctx, st, msgs);
    st.view.was_typing = shortcuts::text_field_focused(ctx);
}

impl eframe::App for TrellisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.apply(ctx, &self.st.settings, &self.st.themes);
        layout::apply_zoom(ctx, &self.st);
        self.st.view.focused = ctx.input(|i| i.focused);
        while let Ok(msg) = self.results.try_recv() {
            self.dispatch(ctx, msg);
        }
        for (email, img) in self.st.view.avatars_loaded.drain(..) {
            self.cache.avatars.insert(ctx, email, img);
        }
        self.st.view.hover_tip = self.st.view.hover_tip_next.take();
        self.st.view.now = chrono::Local::now().timestamp();
        self.st.view.utc_offset = trellis_core::dates::local_offset();

        let now = Instant::now();
        let mut msgs = vec![Msg::Tick(now)];
        self.input(ctx, &mut msgs);
        draw(ctx, &mut self.st, &mut self.cache, &mut msgs);
        build_status::after_draw(&mut self.st, ctx, now, &mut msgs);
        let wanted = self.cache.avatars.take_wanted();
        if !wanted.is_empty() {
            msgs.push(Msg::Avatars(AvatarMsg::WantAvatars(wanted)));
        }
        // Only Preferences edits settings in place; persist any change (also on the frame it closes).
        if self.st.prefs.is_some() {
            msgs.push(Msg::Prefs(PrefsMsg::SettingsEdited));
        }

        for msg in msgs {
            self.dispatch(ctx, msg);
        }
        // Keep polling while work is in flight or toasts are fading.
        if self.st.job.is_some() || self.st.loading || !self.st.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(150));
        } else if self.st.settings.auto_fetch_minutes > 0 {
            ctx.request_repaint_after(Duration::from_secs(5));
        }
        // The status is re-read on a timer where the watcher can be silent (see `poll_every`).
        if let Some(every) = self.st.watch.poll_interval() {
            ctx.request_repaint_after(every);
        }
    }
}
