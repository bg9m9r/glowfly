//! A ratchet for public items nobody uses. A library crate's `pub` items never get a dead-code
//! warning, so in glowfly-model, glowfly-runner and glowfly-ui a `pub fn`, `const`, `static`,
//! `type`, `struct` or `enum` of non-test code must be named in the non-test code of some other
//! file (any crate's `src/`, outside comments, literals and test code: an item only tests use is
//! dead to the app). The ones that aren't today are in `DEAD_PUB`, which only shrinks: delete the
//! item, make it private or move it into test code, then drop its entry. A type also counts as used when another
//! public item of its file names it (a `pub fn` returning it, a `pub` field holding it), since it
//! has to stay public for that.

use super::views::{self, code_lines};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// The library crates whose public items are checked.
const CHECKED: &[&str] = &["glowfly-model", "glowfly-runner", "glowfly-ui"];

/// Public items named in no other file's non-test code today, as `<path>: <name>`. Most are
/// used only by tests, found when tests stopped counting as uses (#365). Only shrinks.
const DEAD_PUB: &[&str] = &[
    "crates/glowfly-model/src/keymap/chord.rs: parts_in",
    "crates/glowfly-model/src/state/activity_copy.rs: activity_text",
    "crates/glowfly-model/src/state/activity_copy.rs: entry_text",
    "crates/glowfly-model/src/state/activity_rows.rs: ROUTINE_KEEP",
    "crates/glowfly-model/src/state/clone.rs: parent_is_dir",
    "crates/glowfly-model/src/state/conflict.rs: set_file",
    "crates/glowfly-model/src/state/dialog_guard.rs: edited_form",
    "crates/glowfly-model/src/state/diff_header.rs: split_path",
    "crates/glowfly-model/src/state/git_env.rs: stops_monitor_for",
    "crates/glowfly-model/src/state/journal.rs: note_label",
    "crates/glowfly-model/src/state/journal_lost.rs: as_row",
    "crates/glowfly-model/src/state/load_more.rs: AUTO_RETRY_DELAYS",
    "crates/glowfly-model/src/state/load_more.rs: FILL_ROWS_FALLBACK",
    "crates/glowfly-model/src/state/load_more.rs: more_row",
    "crates/glowfly-model/src/state/load_more.rs: searching_older",
    "crates/glowfly-model/src/state/menu_native.rs: key_equivalent",
    "crates/glowfly-model/src/state/menu_native.rs: untag",
    "crates/glowfly-model/src/state/palette.rs: MAX_PICK_ROWS",
    "crates/glowfly-model/src/state/palette_commands.rs: SHOW_REPO_FOLDER",
    "crates/glowfly-model/src/state/patch_apply.rs: STAGED_FIRST",
    "crates/glowfly-model/src/state/patch_apply.rs: am_blocked",
    "crates/glowfly-model/src/state/patch_apply.rs: index_blocked",
    "crates/glowfly-model/src/state/patch_changes.rs: NOTHING_STAGED",
    "crates/glowfly-model/src/state/patch_changes.rs: NO_PICK",
    "crates/glowfly-model/src/state/prefs_save.rs: FADE_FOR",
    "crates/glowfly-model/src/state/prefs_save.rs: SHOW_FOR",
    "crates/glowfly-model/src/state/pull_request.rs: in_remotes",
    "crates/glowfly-model/src/state/pull_request.rs: initial_page",
    "crates/glowfly-model/src/state/pull_request_actions.rs: NO_METHODS",
    "crates/glowfly-model/src/state/pull_request_checks.rs: run_label",
    "crates/glowfly-model/src/state/pull_request_source.rs: fork_known",
    "crates/glowfly-model/src/state/pull_request_source.rs: head_fetchable",
    "crates/glowfly-model/src/state/range_diff.rs: exclude_for_remote",
    "crates/glowfly-model/src/state/rebase.rs: CONFLICTED_FILES",
    "crates/glowfly-model/src/state/rebase_quick.rs: REWORD_LABEL",
    "crates/glowfly-model/src/state/rebase_steps.rs: insert_at",
    "crates/glowfly-model/src/state/rebase_update_refs.rs: moved_refs",
    "crates/glowfly-model/src/state/refresh.rs: LARGE_HISTORY",
    "crates/glowfly-model/src/state/refresh/recount.rs: RECOUNT_EVERY",
    "crates/glowfly-model/src/state/section_actions.rs: NO_COMMIT_TAG",
    "crates/glowfly-model/src/state/section_actions.rs: NO_COMMIT_WORKTREE",
    "crates/glowfly-model/src/state/section_actions.rs: add_submodule",
    "crates/glowfly-model/src/state/section_actions.rs: add_worktree",
    "crates/glowfly-model/src/state/section_actions.rs: worktree_base",
    "crates/glowfly-model/src/state/selection.rs: stack_slice",
    "crates/glowfly-model/src/state/selection_wip.rs: WORKDIR_NEEDS_ONE",
    "crates/glowfly-model/src/state/stack_prs.rs: base_for",
    "crates/glowfly-model/src/state/stash_confirm.rs: dirty_body",
    "crates/glowfly-model/src/state/stash_confirm.rs: overlap",
    "crates/glowfly-model/src/state/status_counts.rs: open_requests_text",
    "crates/glowfly-model/src/state/terminal.rs: for_workdir",
    "crates/glowfly-model/src/state/terminal_names.rs: program_hint",
    "crates/glowfly-model/src/state/toasts.rs: ACTION_TOAST_LIFETIME",
    "crates/glowfly-model/src/state/toasts.rs: lifetime",
    "crates/glowfly-model/src/state/trust.rs: record_as_found",
    "crates/glowfly-model/src/state/undo.rs: MAX_UNDO",
    "crates/glowfly-model/src/state/watch.rs: POLL_GAP_CAP",
    "crates/glowfly-model/src/state/watch.rs: WINDOWS_POLL",
    "crates/glowfly-model/src/state/watch.rs: poll_every",
    "crates/glowfly-model/src/state/worktrees.rs: checked_out_in",
    "crates/glowfly-model/src/state/worktrees.rs: counts_text",
    "crates/glowfly-model/src/update/pick.rs: MAINLINE_MISFIT",
    "crates/glowfly-model/src/update/revert.rs: DIFF_CHANGED",
    "crates/glowfly-model/src/update/search_loaded.rs: TYPING_PAUSE",
    "crates/glowfly-model/src/update/status_tip.rs: note_text",
    "crates/glowfly-model/src/update/submodule_add.rs: ADD_CHECK_DELAY",
    "crates/glowfly-runner/src/highlight.rs: decorate",
    "crates/glowfly-runner/src/highlight.rs: decorate_in_context",
    "crates/glowfly-runner/src/watcher.rs: MAX_WAIT",
    "crates/glowfly-ui/src/backdrop.rs: MICA_DARK",
    "crates/glowfly-ui/src/backdrop.rs: MICA_LIGHT",
    "crates/glowfly-ui/src/backdrop.rs: OPAQUE",
    "crates/glowfly-ui/src/list_row.rs: FILE_ROW_BLEED",
    "crates/glowfly-ui/src/stack_rows.rs: STEP",
    "crates/glowfly-ui/src/theme/ansi.rs: body_indices",
    "crates/glowfly-ui/src/theme/branch_colours.rs: chip_in",
    "crates/glowfly-ui/src/theme/branch_colours.rs: initials_fill",
    "crates/glowfly-ui/src/theme/branch_colours.rs: line_in",
    "crates/glowfly-ui/src/theme/branch_palette.rs: LIGHT_STRONG_LINES",
    "crates/glowfly-ui/src/theme/branch_palette.rs: LIGHT_WINE_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette.rs: ORANGE_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: DAWN_FOREST_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: DAWN_LIGHT_LINES",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: DAWN_LIGHT_TEXT",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: GOLD_CB_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: GOLD_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_gold.rs: JADE_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_CB_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_CB_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_JADE_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_LIGHT_LINES",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: COBALT_LIGHT_TEXT",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: FLAME_CB_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: FLAME_CB_SLOT",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: FLAME_DARK_TONES",
    "crates/glowfly-ui/src/theme/branch_palette_more.rs: FLAME_JADE_SLOT",
    "crates/glowfly-ui/src/theme/contrast.rs: brownish_text",
    "crates/glowfly-ui/src/theme/contrast.rs: dimmed",
    "crates/glowfly-ui/src/theme/contrast.rs: lightness",
    "crates/glowfly-ui/src/theme/contrast.rs: luminance",
    "crates/glowfly-ui/src/theme/contrast.rs: max_chroma",
    "crates/glowfly-ui/src/theme/fonts.rs: SEMIBOLD",
    "crates/glowfly-ui/src/theme/fonts.rs: use_test_cjk",
    "crates/glowfly-ui/src/theme/mod.rs: apply_style",
    "crates/glowfly-ui/src/theme/mod.rs: apply_theme",
    "crates/glowfly-ui/src/theme/mod.rs: egui_theme",
    "crates/glowfly-ui/src/theme/surfaces.rs: wash_on",
    "crates/glowfly-ui/src/theme/tokens.rs: FOCUS_GAP",
    "crates/glowfly-ui/src/theme/tokens.rs: reduced",
    "crates/glowfly-ui/src/toasts.rs: TOAST_W",
    "crates/glowfly-ui/src/widgets/buttons.rs: tonal",
    "crates/glowfly-ui/src/widgets/menu_submenu.rs: CLOSE_GRACE",
    "crates/glowfly-ui/src/widgets/menu_submenu.rs: OPEN_DELAY",
    "crates/glowfly-ui/src/widgets/mod.rs: chip_with_edge",
    "crates/glowfly-ui/src/widgets/row_fill.rs: ACCENT_BAR_W",
    "crates/glowfly-ui/src/widgets/tooltip_motion.rs: rise",
];

/// The identifiers in `text`.
fn words(text: &str) -> BTreeSet<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect()
}

/// The `pub` items `src` declares outside comments and inline test modules, as (name, whether
/// it is a type that another `pub` line of the file names).
fn public_items(src: &str) -> Vec<(String, bool)> {
    let code = code_lines(src);
    let mut out = Vec::new();
    for (at, line) in code.iter().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("pub ") else { continue };
        let rest = ["const fn ", "unsafe fn ", "async fn "].iter().find_map(|p| rest.strip_prefix(p).map(|r| ("fn", r)));
        let (kind, after) = match rest {
            Some(found) => found,
            None => {
                let rest = line.trim_start().strip_prefix("pub ").unwrap_or("");
                let Some((kind, after)) = rest.split_once(' ') else { continue };
                (kind, after)
            }
        };
        if !["fn", "const", "static", "type", "struct", "enum"].contains(&kind) {
            continue;
        }
        let after = after.strip_prefix("mut ").unwrap_or(after);
        let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && name != "_" {
            let is_type = ["struct", "enum", "type"].contains(&kind);
            let exposed = is_type
                && code.iter().enumerate().any(|(i, l)| i != at && l.trim_start().starts_with("pub") && words(l).contains(name.as_str()));
            out.push((name, exposed));
        }
    }
    out
}

#[test]
fn public_items_are_used_elsewhere() {
    // The non-test files of every crate's `src/`: what tests name doesn't keep an item alive.
    let crates = views::app_src().parent().and_then(|app| app.parent()).map(PathBuf::from).expect("the crates folder");
    let mut texts: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("the crates folder").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().join("src").is_dir() {
            let sources = views::crate_sources(&entry.path().join("src"));
            texts.extend(sources.into_iter().filter(|s| !s.2).map(|(rel, text, _)| (format!("crates/{name}/src/{rel}"), text)));
        }
    }
    assert!(texts.len() > 500, "found the workspace's sources");
    // Uses are counted in code: not in comments, literals or inline test modules.
    let codes: Vec<String> = texts.iter().map(|(_, text)| crate::lexer::bare_lines(text).join("\n")).collect();
    let mut seen_in: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (i, code) in codes.iter().enumerate() {
        for w in words(code) {
            seen_in.entry(w).or_default().insert(i);
        }
    }
    let mut dead = BTreeSet::new();
    for (i, (path, text)) in texts.iter().enumerate() {
        if !CHECKED.iter().any(|c| path.starts_with(&format!("crates/{c}/src/"))) {
            continue;
        }
        for (name, exposed) in public_items(text) {
            if !exposed && seen_in.get(name.as_str()).is_none_or(|files| files.iter().all(|f| *f == i)) {
                dead.insert(format!("{path}: {name}"));
            }
        }
    }
    let allowed: BTreeSet<String> = DEAD_PUB.iter().map(|s| s.to_string()).collect();
    let new: Vec<&String> = dead.difference(&allowed).collect();
    let gone: Vec<&String> = allowed.difference(&dead).collect();
    assert!(
        new.is_empty(),
        "public items no other file names; make them private (pub(crate) or none) or delete them:\n{}",
        new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
    assert!(gone.is_empty(), "no longer unused; drop from DEAD_PUB:\n{}", gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}

#[test]
fn public_items_finds_the_declarations() {
    let src = "pub fn a() {}\npub const fn b() {}\npub(crate) fn c() {}\n  pub struct D;\npub enum E {}\npub type F = u8;\n\
               pub static mut G: u8 = 0;\npub const H: u8 = 1;\npub mod m;\npub use x::y;\n// pub fn hidden() {}\n";
    let names: Vec<String> = public_items(src).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["a", "b", "D", "E", "F", "G", "H"]);
    let exposed = public_items(
        "pub struct Shown;
pub struct Hidden;
pub fn f() -> Shown { Shown }
fn g(_: Hidden) {}
",
    );
    assert_eq!(exposed, [("Shown".into(), true), ("Hidden".into(), false), ("f".into(), false)]);
}
