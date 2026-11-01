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
    ("main_sand", Window),
    ("main_high_contrast_dark", Window),
    ("theme_ember", Window),
    ("theme_graphite", Window),
    ("theme_linen", Window),
    ("theme_midnight", Window),
    ("theme_mist", Window),
    ("theme_pine", Window),
    ("vivid_dark", Window),
    ("vivid_light", Window),
    ("vivid_sand", Window),
    ("vivid_garden_dark", Window),
    ("vivid_garden_light", Window),
    ("vivid_high_contrast_dark", Window),
    ("vivid_high_contrast_light", Window),
    ("lineup_dawn", Window),
    ("lineup_dusk", Window),
    ("lineup_garden_dark", Window),
    ("lineup_garden_light", Window),
    ("lineup_high_contrast_dark", Window),
    ("lineup_high_contrast_light", Window),
    ("lineup_sand", Window),
    ("lineup_welcome_dawn", Window),
    ("lineup_welcome_dusk", Window),
    ("mica_dark", Window),
    ("mica_light", Window),
    ("welcome", Window),
    ("welcome_start", Window),
    ("welcome_start_light", Window),
    ("welcome_start_high_contrast_dark", Window),
    // The window chrome: title bars, tabs, toolbar and menus.
    ("vivid_title_bar", Window),
    ("title_bar_windows_dark", Top(108.0)),
    ("title_bar_windows_light", Top(108.0)),
    ("title_bar_mac_dark", Top(108.0)),
    ("tabs_restyled", Window),
    ("toolbar_restyled_dark", Window),
    ("toolbar_restyled_light", Window),
    ("menu_bar_file", Window),
    ("menu_bar_view_theme", Window),
    ("branch_colour_submenu", Window),
    ("graph_commit_menu", Window),
    ("graph_multi_menu", Window),
    ("graph_commit_menu_copy_flipped", Window),
    ("toolbar_push_menu", Window),
    ("wip_file_menu_danger_light", Window),
    ("repo_menu_dark", TopLeft(640.0, 620.0)),
    ("repo_menu_light", TopLeft(640.0, 620.0)),
    ("repo_menu_groups", TopLeft(640.0, 620.0)),
    ("welcome_repo_groups", Window),
    ("live_updates_states", Window),
    // Narrow windows: the toolbar folded into More, the graph's columns given way.
    ("narrow_900_en", Window),
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
    ("graph_shallow", Window),
    ("graph_two_commits_selected", Window),
    ("graph_search_dim", Window),
    ("history_table_dark", Window),
    ("history_table_light", Window),
    ("history_table_high_contrast_light", Window),
    ("rebase_dragging", Window),
    // Diffs, blame, conflicts and the pull request's line comments.
    ("diff_inline", Window),
    ("diff_split", Window),
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
    ("diff_wrap_inline", Window),
    ("diff_whole_inline", Window),
    ("diff_gap_filled_split_light", Window),
    ("conflict_view", Window),
    ("conflict_line_picks", Window),
    ("blame_view", Window),
    ("blame_view_light", Window),
    ("range_diff_changed", Window),
    ("pull_request_line_comment_dark", Window),
    // The design sheets, fonts, colours and icons.
    ("controls_dark", Window),
    ("controls_light", Window),
    ("controls_high_contrast_dark", Window),
    ("controls_high_contrast_light", Window),
    ("controls_sand", Window),
    ("design_buttons_dark", Window),
    ("design_buttons_light", Window),
    ("design_buttons_high_contrast_dark", Window),
    ("design_buttons_sand", Window),
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
    // Dialogs: the backdrop over the window, then dialogs on their own.
    ("modal_backdrop_dark", Window),
    ("modal_backdrop_light", Window),
    ("dialog_look_high_contrast_dark", Dialog),
    ("dialog_look_high_contrast_light", Dialog),
    ("dialog_look_sand", Dialog),
    ("about_dialog", Dialog),
    ("dialog_guide_beside", Dialog),
    ("confirm_dialog_danger", Dialog),
    ("quit_confirm_unsaved", Dialog),
    ("credential_prompt_sign_in", Dialog),
];

/// The goldens compared only as images: their text would be the same as another snapshot's
/// (most are one view in several themes, whose text the first theme's snapshot keeps).
const IMAGE_ONLY: &[&str] = &[
    "main_light",
    "main_sand",
    "main_high_contrast_dark",
    "history_table_dark",
    "history_table_light",
    "history_table_high_contrast_light",
    "toolbar_restyled_dark",
    "toolbar_restyled_light",
    "theme_graphite",
    "theme_linen",
    "theme_midnight",
    "theme_mist",
    "theme_pine",
    "vivid_light",
    "vivid_sand",
    "vivid_garden_dark",
    "vivid_garden_light",
    "vivid_high_contrast_dark",
    "vivid_high_contrast_light",
    "lineup_dawn",
    "lineup_dusk",
    "lineup_garden_dark",
    "lineup_garden_light",
    "lineup_high_contrast_dark",
    "lineup_high_contrast_light",
    "lineup_sand",
    "diff_high_contrast_light",
    "lineup_welcome_dawn",
    "lineup_welcome_dusk",
    "mica_dark",
    "mica_light",
    "title_bar_windows_light",
    "welcome_start_light",
    "welcome_start_high_contrast_dark",
    "graph_bands_dusk",
    "branch_colours_graph_light",
    "branch_colours_colour_blind_dark",
    "branch_colours_colour_blind_light",
    "person_colours_graph_light",
    "graph_wip_row_light",
    "graph_wip_row_high_contrast",
    "repo_menu_light",
    "diff_lines_gutter_selected_light",
    "diff_image_before_after_light",
    "blame_view_light",
    "controls_light",
    "controls_high_contrast_dark",
    "controls_high_contrast_light",
    "controls_sand",
    "design_buttons_light",
    "design_buttons_high_contrast_dark",
    "design_buttons_sand",
    "design_surfaces_light",
    "button_styles_light",
    "fonts_light",
    "type_scale_specimen_light",
    "terminal_ansi_light",
    "terminal_ansi_high_contrast_dark",
    "modal_backdrop_light",
    "dialog_look_high_contrast_light",
    "dialog_look_sand",
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
