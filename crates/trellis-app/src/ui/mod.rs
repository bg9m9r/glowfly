//! The egui layer. Views draw `AppState` and push `Msg`s; the shell (`app.rs`) feeds those
//! messages and background results through `update`, runs the resulting effects, and handles
//! the few effects that need the window.

mod access;
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
mod conflict_cells;
mod conflict_check;
mod conflict_files;
mod conflict_fold;
mod conflict_output;
mod conflict_panes;
mod dashboard;
mod dashboard_rows;
mod date_tip;
mod deep_link_dialog;
mod details;
mod details_banner;
mod details_multi;
mod details_wip;
mod details_workdir;
mod diff;
mod file_buttons;
mod file_dialogs;
mod file_menu;
mod find_bar;
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
mod history_text_cache;
mod icons;
mod init_dialog;
mod issue;
mod layout;
mod lfs_badge;
mod lfs_banner;
mod list_row;
mod mainline_dialog;
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
mod remote_branch_menu;
mod remotes;
mod repo_finder;
mod repo_list;
mod repo_list_drag;
mod repo_menu;
mod repo_sync;
mod shortcuts;
mod sidebar;
mod sign_in_banner;
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
mod trash_offer_dialog;
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
mod snapshot_tests_blame_wrap;
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
mod snapshot_tests_compare;
#[cfg(test)]
mod snapshot_tests_conflict_check;
#[cfg(test)]
mod snapshot_tests_conflict_files;
#[cfg(test)]
mod snapshot_tests_conflict_layout;
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
mod snapshot_tests_graph_ref_menu;
#[cfg(test)]
mod snapshot_tests_history;
#[cfg(test)]
mod snapshot_tests_history_file;
#[cfg(test)]
mod snapshot_tests_init;
#[cfg(test)]
mod snapshot_tests_issue;
#[cfg(test)]
mod snapshot_tests_issue_create;
#[cfg(test)]
mod snapshot_tests_language_chrome;
#[cfg(test)]
mod snapshot_tests_language_panels;
#[cfg(test)]
mod snapshot_tests_lfs;
#[cfg(test)]
mod snapshot_tests_lfs_notes;
#[cfg(test)]
mod snapshot_tests_load_more;
#[cfg(test)]
mod snapshot_tests_logs;
#[cfg(test)]
mod snapshot_tests_onboarding;
#[cfg(test)]
mod snapshot_tests_palette;
#[cfg(test)]
mod snapshot_tests_palette_nav;
#[cfg(test)]
mod snapshot_tests_palette_recent;
#[cfg(test)]
mod snapshot_tests_palette_stash;
#[cfg(test)]
mod snapshot_tests_patch;
#[cfg(test)]
mod snapshot_tests_path_warnings;
#[cfg(test)]
mod snapshot_tests_pick;
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
mod snapshot_tests_rebase_merges;
#[cfg(test)]
mod snapshot_tests_rebase_steps;
#[cfg(test)]
mod snapshot_tests_remote_branch;
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
mod snapshot_tests_sign_in;
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
#[cfg(test)]
mod toolbar_names_tests;

use crate::msg::Msg;
use crate::state::AppState;
#[cfg(test)]
use central::escape_key;
use eframe::egui;
use trellis_core::spell::Speller;

mod app;
#[cfg(test)]
pub(crate) use app::TestStart;
pub use app::TrellisApp;
#[cfg(test)]
pub(crate) use palette::area_rect as palette_rect;

/// Resources owned by the UI thread (textures, the spell checker's COM object).
pub struct UiCache {
    pub avatars: avatars::Avatars,
    pub speller: Speller,
    pub diff_rows: diff::rows_cache::DiffRowsCache,
    /// The file history File view's line widths and wrapped row heights.
    pub history_text: history_text_cache::HistoryTextCache,
    /// The blame view's line widths and wrapped row heights.
    pub blame_rows: blame::rows::BlameRowsCache,
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
        if st.snap.is_some() && !st.dashboard.open {
            sign_in_banner::show(ctx, st, msgs);
            lfs_banner::show(ctx, st, msgs);
        }
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
