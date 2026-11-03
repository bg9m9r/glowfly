//! The pixel goldens: the snapshots still compared as images, besides their text. Every other
//! view test keeps only a text snapshot (`text.rs`), which merges like code and doesn't change
//! when a token or the toolbar's look does. The goldens are the whole window in each built-in
//! theme, and the views whose look is the point: the graph, diffs, blame and conflicts, the
//! design sheets and colours, the window chrome, and a few dialogs, cropped to the dialog.
//!
//! Add a name here only for a view a text snapshot can't judge (colour, drawing, alignment);
//! `UPDATE_SNAPSHOTS=1` then writes its PNG. The test below keeps this list and the PNGs in
//! `tests/snapshots/` the same. A golden whose text is another snapshot's (the same view in
//! another theme) is listed in `IMAGE_ONLY` too: it keeps only its image.

/// How much of the window a golden keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Crop {
    /// The whole window (or the widget a small harness draws).
    Window,
    /// The open dialogs, with room for their shadow.
    Dialog,
    /// The top of the window, this many points tall: the title bar and toolbar.
    Top(f32),
    /// The window's top-left corner, this many points wide and tall: the toolbar's left end
    /// and a menu dropped from it.
    TopLeft(f32, f32),
}

use Crop::{Dialog, Top, TopLeft, Window};

/// The goldens and how each is cropped.
pub(crate) const GOLDENS: &[(&str, Crop)] = &[
    // The whole window in each built-in theme.
    ("main_dark", Window),
    ("main_light", Window),
    ("main_high_contrast_dark", Window),
    ("theme_graphite", Window),
    ("theme_midnight", Window),
    ("theme_mist", Window),
    ("theme_porcelain", Window),
    ("vivid_dark", Window),
    ("vivid_light", Window),
    ("vivid_porcelain", Window),
    ("vivid_high_contrast_dark", Window),
    ("vivid_high_contrast_light", Window),
    ("lineup_dawn", Window),
    ("lineup_dusk", Window),
    ("lineup_high_contrast_dark", Window),
    ("lineup_high_contrast_light", Window),
    ("lineup_porcelain", Window),
    ("lineup_welcome_dawn", Window),
    ("lineup_welcome_dusk", Window),
    ("mica_dark", Window),
    ("mica_light", Window),
    ("welcome", Window),
    ("welcome_start", Window),
    ("welcome_start_light", Window),
    ("welcome_start_high_contrast_dark", Window),
    // Help > Getting started: the cards, their groups and the in-use marks.
    ("getting_started_dark", Window),
    ("getting_started_light", Window),
    // Preferences > Appearance in the default light theme: the theme picker.
    ("prefs_theme_picker_porcelain", Window),
    // Match the system on (two rings), a theme file in use (the disclosure open), and German.
    ("prefs_theme_picker_system", TopLeft(1240.0, 520.0)),
    ("prefs_theme_custom_open", TopLeft(1240.0, 720.0)),
    ("prefs_theme_picker_de", TopLeft(1240.0, 520.0)),
    // Preferences > Accessibility in both default themes, and German and Spanish at 760 px.
    ("prefs_accessibility", Window),
    ("prefs_accessibility_light", Window),
    ("prefs_accessibility_de", Window),
    ("prefs_accessibility_es", Window),
    // The system asking for high contrast (shown in High contrast dark), less motion and larger text.
    ("prefs_accessibility_system_on", Window),
    // The window chrome: title bars, tabs, toolbar and menus.
    ("vivid_title_bar", Window),
    // The conflict check's popover below its toolbar chip with a release bot's long names (#523),
    // in the app's least window, with its Merge tooltip, and with 12 files; its report dialog.
    ("conflict_popover_long_dark", Top(420.0)),
    ("conflict_popover_long_light", Top(420.0)),
    ("conflict_popover_narrow_dark", Window),
    ("conflict_popover_merge_tip_dark", Top(480.0)),
    ("conflict_popover_many_dark", Top(600.0)),
    ("conflict_report_long_dark", Dialog),
    ("conflict_report_long_light", Dialog),
    ("title_bar_windows_dark", Top(108.0)),
    ("title_bar_windows_light", Top(108.0)),
    ("title_bar_mac_dark", Top(108.0)),
    ("tabs_restyled", Window),
    ("tabs_hover", Top(40.0)),
    // The status bar's What's new chip after an upgrade, in both default themes, and beside an
    // update's chip.
    ("status_whats_new_dark", Window),
    ("status_whats_new_light", Window),
    ("status_whats_new_beside_update", Window),
    // The trust chip waiting beside the conflict chip, then at rest.
    ("toolbar_trust_waiting_dark", Top(108.0)),
    ("toolbar_trust_rest_dark", Top(108.0)),
    ("tabs_new_tab_hover", Top(40.0)),
    ("tabs_close_focus", Top(40.0)),
    ("section_header_focus", TopLeft(280.0, 200.0)),
    // The left panel's empty states: an expanded empty Worktrees, and pull requests not connected.
    ("sidebar_empty_worktrees", TopLeft(300.0, 820.0)),
    ("sidebar_prs_connect", TopLeft(300.0, 820.0)),
    ("toolbar_restyled_dark", Window),
    ("toolbar_restyled_light", Window),
    ("menu_bar_file", Window),
    ("menu_bar_view_theme", Window),
    ("branch_colour_submenu", Window),
    ("branch_colour_main_submenu", Window),
    ("graph_commit_menu", Window),
    ("graph_multi_menu", Window),
    ("graph_commit_menu_copy_flipped", Window),
    ("toolbar_push_menu", Window),
    // Pull and Push with their count badges, the branch picker's counts, in both default themes.
    ("toolbar_ahead_behind_dark", Top(108.0)),
    ("toolbar_ahead_behind_light", Top(108.0)),
    ("wip_file_menu_danger_light", Window),
    ("repo_menu_dark", TopLeft(640.0, 620.0)),
    ("repo_menu_light", TopLeft(640.0, 620.0)),
    ("repo_menu_groups", TopLeft(640.0, 620.0)),
    ("welcome_repo_groups", Window),
    ("welcome_repo_groups_light", Window),
    ("welcome_repo_group_menu", Window),
    ("welcome_repo_group_menu_light", Window),
    ("prefs_ui_group_colour", Window),
    ("prefs_ui_group_colour_light", Window),
    ("welcome_repo_row_hover", Window),
    ("live_updates_states", Window),
    // The tips in context: under the graph's header, and in a narrow left panel.
    ("hint_row_undo", Top(280.0)),
    ("hint_row_undo_light", Top(280.0)),
    ("hint_row_connect", TopLeft(320.0, 560.0)),
    ("hint_row_tidy_de", Top(280.0)),
    // Narrow windows: the toolbar folded into More, the graph's columns given way.
    ("narrow_900_en", Window),
    ("graph_wip_row_floor", Window),
    ("graph_wip_row_floor_no_chips", Window),
    ("wip_header_icons", Top(220.0)),
    ("wip_header_icons_staged", Top(220.0)),
    ("narrow_900_de", Window),
    ("narrow_1100_de", Window),
    // The graph and the history table.
    ("graph_bands_dawn", Window),
    ("graph_bands_dusk", Window),
    ("branch_colours_graph_dark", Window),
    ("branch_colours_graph_light", Window),
    ("branch_colours_colour_blind_dark", Window),
    ("branch_colours_colour_blind_light", Window),
    ("person_colours_graph_dark", Window),
    ("person_colours_graph_light", Window),
    ("graph_wip_row", Window),
    ("graph_wip_row_light", Window),
    ("graph_wip_row_high_contrast", Window),
    ("graph_worktree_wips", Window),
    ("graph_ghost_branch", Window),
    ("graph_ghost_branch_narrow", Window),
    ("graph_shallow", Window),
    ("graph_two_commits_selected", Window),
    ("graph_search_dim", Window),
    ("graph_search_dim_light", Window),
    ("graph_search_dim_high_contrast_light", Window),
    ("graph_focus_inactive_dark", Window),
    ("history_table_dark", Window),
    ("history_table_light", Window),
    ("history_table_high_contrast_light", Window),
    ("history_table_compact_dark", Window),
    ("rebase_dragging", Window),
    // A commit body kept as written: hard line breaks, long lines soft-wrapped.
    ("details_hard_wrapped_body", Window),
    // Diffs, blame, conflicts and the pull request's line comments.
    ("diff_inline", Window),
    ("diff_split", Window),
    ("diff_hunks_german", Window),
    ("diff_hunk_header_narrow", Window),
    ("diff_syntax_inline", Window),
    ("diff_syntax_split_light", Window),
    ("diff_words_inline", Window),
    ("diff_words_split_light", Window),
    ("diff_high_contrast_light", Window),
    ("diff_binary_dark", Window),
    ("diff_image_before_after_dark", Window),
    ("diff_image_before_after_light", Window),
    ("diff_lines_gutter_selected_dark", Window),
    ("diff_lines_gutter_selected_light", Window),
    ("diff_commit_gutter_hover", Window),
    ("diff_commit_readonly_dark", Window),
    ("diff_text_selection_dark", Window),
    ("diff_text_selection_light", Window),
    ("diff_text_selection_hc_dark", Window),
    ("diff_split_text_selection", Window),
    ("diff_text_selection_unfocused_dark", Window),
    ("diff_row_menu_dark", Window),
    ("diff_row_menu_de", Window),
    ("diff_wrap_inline", Window),
    ("diff_whole_inline", Window),
    ("diff_gap_filled_split_light", Window),
    ("conflict_view", Window),
    ("conflict_line_picks", Window),
    ("conflict_line_picks_light", Window),
    ("conflict_both_sides_dark", Window),
    ("conflict_all_resolved_dark", Window),
    ("conflict_narrow_1100_de", Window),
    ("blame_view", Window),
    ("blame_view_light", Window),
    ("range_diff_changed", Window),
    ("pull_request_line_comment_dark", Window),
    // The design sheets, fonts, colours and icons.
    ("controls_dark", Window),
    // Every chip and badge kind: only the two chip heights.
    ("chips_catalog_dark", Window),
    ("controls_light", Window),
    ("controls_high_contrast_dark", Window),
    ("controls_high_contrast_light", Window),
    ("controls_focus_strong_dark", Window),
    ("controls_focus_strong_light", Window),
    ("design_buttons_dark", Window),
    ("design_buttons_light", Window),
    ("design_buttons_high_contrast_dark", Window),
    ("design_split_states_dark", Window),
    ("design_split_states_light", Window),
    ("design_surfaces_dark", Window),
    ("design_surfaces_light", Window),
    ("button_styles_dark", Window),
    ("button_styles_light", Window),
    ("fonts_dark", Window),
    ("fonts_light", Window),
    ("fonts_cjk", Window),
    ("type_scale_specimen_dark", Window),
    ("type_scale_specimen_light", Window),
    ("icon_branch", Window),
    ("terminal_ansi_dark", Window),
    ("terminal_ansi_light", Window),
    ("terminal_ansi_high_contrast_dark", Window),
    // The state ladder on rows and the targets in them (`snapshot_tests_row_states.rs`).
    ("dashboard_pr_row_hovered_dark", Window),
    ("dashboard_pr_row_hovered_light", Window),
    ("dashboard_pr_row_focused_dark", Window),
    ("dashboard_pr_row_focused_light", Window),
    ("sidebar_section_header_hovered_dark", TopLeft(360.0, 820.0)),
    ("sidebar_section_header_hovered_light", TopLeft(360.0, 820.0)),
    ("sidebar_section_header_focused_dark", TopLeft(360.0, 820.0)),
    ("sidebar_section_header_focused_light", TopLeft(360.0, 820.0)),
    ("sidebar_gitflow_group_plus_hovered_dark", TopLeft(360.0, 820.0)),
    ("sidebar_gitflow_group_plus_hovered_light", TopLeft(360.0, 820.0)),
    ("sidebar_gitflow_group_hovered_dark", TopLeft(360.0, 820.0)),
    ("sidebar_gitflow_group_hovered_light", TopLeft(360.0, 820.0)),
    ("sidebar_remote_group_pressed_dark", TopLeft(360.0, 820.0)),
    ("sidebar_remote_group_pressed_light", TopLeft(360.0, 820.0)),
    ("gitflow_panel_row_hovered_over_finish_dark", Window),
    ("gitflow_panel_row_hovered_over_finish_light", Window),
    ("pr_checks_header_hovered_dark", Window),
    ("pr_checks_header_hovered_light", Window),
    ("pr_checks_run_hovered_dark", Window),
    ("pr_checks_run_hovered_light", Window),
    ("prefs_nav_selected_focused_other_dark", TopLeft(640.0, 520.0)),
    ("prefs_nav_selected_focused_other_light", TopLeft(640.0, 520.0)),
    ("palette_cursor_and_hover_dark", Window),
    ("palette_cursor_and_hover_light", Window),
    ("palette_cursor_and_hover_high_contrast_dark", Window),
    ("diff_comment_add_hovered_dark", Window),
    ("diff_comment_add_hovered_light", Window),
    ("diff_draft_marker_rest_dark", Window),
    ("diff_draft_marker_rest_light", Window),
    ("diff_draft_marker_hovered_dark", Window),
    ("diff_draft_marker_hovered_light", Window),
    // Dialogs: the backdrop over the window, then dialogs on their own.
    ("modal_backdrop_dark", Window),
    ("modal_backdrop_light", Window),
    ("dialog_look_high_contrast_dark", Dialog),
    ("dialog_look_high_contrast_light", Dialog),
    ("about_dialog", Dialog),
    ("repo_group_add_dialog", Dialog),
    ("repo_group_add_dialog_light", Dialog),
    ("dialog_guide_beside", Dialog),
    ("confirm_dialog_danger", Dialog),
    ("quit_confirm_unsaved", Dialog),
    ("credential_prompt_sign_in", Dialog),
    ("trust_prompt_lfs", Dialog),
    ("trust_prompt_lfs_light", Dialog),
];

/// The goldens compared only as images: their text would be the same as another snapshot's
/// (most are one view in several themes, whose text the first theme's snapshot keeps).
const IMAGE_ONLY: &[&str] = &[
    // `conflict_popover_long_dark` and `conflict_report_long_dark` in Porcelain.
    "conflict_popover_long_light",
    "conflict_report_long_light",
    // `welcome_repo_groups` and `repo_group_add_dialog` in Porcelain.
    "welcome_repo_groups_light",
    "repo_group_add_dialog_light",
    // `welcome_repo_group_menu` and `prefs_ui_group_colour` in Porcelain.
    "welcome_repo_group_menu_light",
    "prefs_ui_group_colour_light",
    // `conflict_line_picks` in Porcelain: the check marks' contrast on the light tints.
    "conflict_line_picks_light",
    // `graph_search_dim` in Dawn and High contrast light, for the search marks' sign-off.
    "graph_search_dim_light",
    "graph_search_dim_high_contrast_light",
    // The trust chip at rest: the same names as `toolbar_trust_waiting_dark`; only its look differs.
    "toolbar_trust_rest_dark",
    // `status_whats_new_dark` in Porcelain.
    "status_whats_new_light",
    // `hint_row_undo` in Porcelain.
    "hint_row_undo_light",
    // The same hovered ghost as `graph_ghost_branch_tooltip`'s text, icon-only in a narrow column.
    "graph_ghost_branch_narrow",
    "main_light",
    // Preferences > Accessibility's text is `prefs_accessibility`'s.
    "prefs_accessibility_light",
    // The same text with the system's settings followed: only the high-contrast look differs.
    "prefs_accessibility_system_on",
    "main_high_contrast_dark",
    "history_table_dark",
    "history_table_light",
    "history_table_high_contrast_light",
    // The same rows as `main_dark`'s text, 28 px tall.
    "history_table_compact_dark",
    "toolbar_restyled_dark",
    "toolbar_restyled_light",
    // `toolbar_ahead_behind_dark` in Light.
    "toolbar_ahead_behind_light",
    "theme_graphite",
    "theme_midnight",
    "theme_mist",
    "vivid_light",
    "vivid_porcelain",
    "vivid_high_contrast_dark",
    "vivid_high_contrast_light",
    "lineup_dawn",
    "lineup_dusk",
    "lineup_high_contrast_dark",
    "lineup_high_contrast_light",
    "lineup_porcelain",
    "diff_high_contrast_light",
    "lineup_welcome_dawn",
    "lineup_welcome_dusk",
    "mica_dark",
    "mica_light",
    "title_bar_windows_light",
    "welcome_start_light",
    "welcome_start_high_contrast_dark",
    // `getting_started_dark` in Light.
    "getting_started_light",
    "graph_bands_dusk",
    "branch_colours_graph_light",
    "branch_colours_colour_blind_dark",
    "branch_colours_colour_blind_light",
    "person_colours_graph_light",
    "graph_wip_row_light",
    "graph_wip_row_high_contrast",
    // `main_dark`'s text in a narrow details panel: the icon-only actions show only in the image.
    "wip_header_icons",
    "repo_menu_light",
    "diff_lines_gutter_selected_light",
    "diff_text_selection_light",
    "diff_text_selection_hc_dark",
    "diff_text_selection_unfocused_dark",
    "diff_image_before_after_light",
    "blame_view_light",
    "controls_light",
    "controls_high_contrast_dark",
    "controls_high_contrast_light",
    "controls_focus_strong_light",
    "design_buttons_light",
    "design_buttons_high_contrast_dark",
    "design_split_states_light",
    "design_surfaces_light",
    "button_styles_light",
    "fonts_light",
    "type_scale_specimen_light",
    "terminal_ansi_light",
    "terminal_ansi_high_contrast_dark",
    "modal_backdrop_light",
    "dialog_look_high_contrast_light",
    "trust_prompt_lfs_light",
    // The row states' hovers and focus: their text is their view's own snapshot's.
    "dashboard_pr_row_hovered_dark",
    "dashboard_pr_row_hovered_light",
    "dashboard_pr_row_focused_dark",
    "dashboard_pr_row_focused_light",
    "sidebar_section_header_hovered_dark",
    "sidebar_section_header_hovered_light",
    "sidebar_section_header_focused_dark",
    "sidebar_section_header_focused_light",
    "sidebar_gitflow_group_plus_hovered_dark",
    "sidebar_gitflow_group_plus_hovered_light",
    "sidebar_gitflow_group_hovered_dark",
    "sidebar_gitflow_group_hovered_light",
    "sidebar_remote_group_pressed_dark",
    "sidebar_remote_group_pressed_light",
    "gitflow_panel_row_hovered_over_finish_dark",
    "gitflow_panel_row_hovered_over_finish_light",
    "pr_checks_header_hovered_dark",
    "pr_checks_header_hovered_light",
    "pr_checks_run_hovered_dark",
    "pr_checks_run_hovered_light",
    "prefs_nav_selected_focused_other_dark",
    "prefs_nav_selected_focused_other_light",
    "palette_cursor_and_hover_dark",
    "palette_cursor_and_hover_light",
    "palette_cursor_and_hover_high_contrast_dark",
    "diff_comment_add_hovered_dark",
    "diff_comment_add_hovered_light",
    "diff_draft_marker_rest_dark",
    "diff_draft_marker_rest_light",
    "diff_draft_marker_hovered_dark",
    "diff_draft_marker_hovered_light",
];

/// Whether golden `name` keeps no text snapshot.
pub(crate) fn image_only(name: &str) -> bool {
    IMAGE_ONLY.contains(&name)
}

/// How snapshot `name` is compared as an image, if it is a golden.
pub(crate) fn crop(name: &str) -> Option<Crop> {
    GOLDENS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

#[cfg(test)]
use std::collections::BTreeSet;

#[test]
fn goldens_match_the_approved_images() {
    let dir = std::path::Path::new(super::compare::DIR);
    let images: BTreeSet<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|f| f.strip_suffix(".png").map(str::to_owned))
        .filter(|n| ![".new", ".old", ".diff"].iter().any(|s| n.ends_with(s)))
        .collect();
    let goldens: BTreeSet<String> = GOLDENS.iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(goldens.len(), GOLDENS.len(), "a golden is listed twice");
    let stray: Vec<_> = images.difference(&goldens).collect();
    assert!(stray.is_empty(), "images in {} that are not goldens (delete them, or list them in GOLDENS): {stray:?}", dir.display());
    let missing: Vec<_> = goldens.difference(&images).collect();
    assert!(missing.is_empty(), "goldens without an image (run UPDATE_SNAPSHOTS=1 cargo test -p glowfly-ui -p glowfly): {missing:?}");
}

#[test]
fn image_only_goldens_have_no_text() {
    let dir = std::path::Path::new(super::compare::DIR);
    let only: BTreeSet<&str> = IMAGE_ONLY.iter().copied().collect();
    assert_eq!(only.len(), IMAGE_ONLY.len(), "a name is listed twice in IMAGE_ONLY");
    let not_goldens: Vec<_> = IMAGE_ONLY.iter().filter(|n| crop(n).is_none()).collect();
    assert!(not_goldens.is_empty(), "IMAGE_ONLY names that are not goldens: {not_goldens:?}");
    let with_text: Vec<_> = IMAGE_ONLY.iter().filter(|n| dir.join(format!("{n}.txt")).exists()).collect();
    assert!(with_text.is_empty(), "image-only goldens with a text snapshot (delete the .txt): {with_text:?}");
}
