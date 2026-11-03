//! The Preferences search's index: (page, label key, help key) for every form row with a
//! translated label, and what a query matches in it. A test below scans `prefs/*.rs` for row
//! label keys and fails on any missing here.

use crate::state::PrefPage;
use glowfly_core::i18n::Lang;

/// Every searchable row: its page, its label's key and its help's key.
const INDEX: &[(PrefPage, &str, Option<&str>)] = &[
    (PrefPage::General, "prefs.general.auto_fetch.label", Some("prefs.general.auto_fetch.help")),
    (PrefPage::General, "prefs.general.auto_prune.label", None),
    (PrefPage::General, "prefs.general.auto_stash.label", Some("prefs.general.auto_stash.help")),
    (PrefPage::General, "prefs.general.autocrlf.label", Some("prefs.general.autocrlf.help")),
    (PrefPage::General, "prefs.general.conflict_check.label", Some("prefs.general.conflict_check.help")),
    (PrefPage::General, "prefs.general.default_branch.label", Some("prefs.general.default_branch.help")),
    (PrefPage::General, "prefs.general.delete_to_trash.label", Some("prefs.general.delete_to_trash.help")),
    (PrefPage::General, "prefs.general.extended_logging.label", Some("prefs.general.extended_logging.help")),
    (PrefPage::General, "prefs.general.forget.label", None),
    (PrefPage::General, "prefs.general.frame_times.label", Some("prefs.general.frame_times.help")),
    (PrefPage::General, "prefs.general.git.label", Some("prefs.general.git.help")),
    (PrefPage::General, "prefs.general.index_history.label", Some("prefs.general.index_history.help")),
    (PrefPage::General, "prefs.general.initial_commits.label", Some("prefs.general.initial_commits.help")),
    (PrefPage::General, "prefs.general.load_more.label", Some("prefs.general.load_more.help")),
    (PrefPage::General, "prefs.general.logs.label", Some("prefs.general.logs.help")),
    (PrefPage::General, "prefs.general.longpaths.label", Some("prefs.general.longpaths.help")),
    (PrefPage::General, "prefs.general.remember_sign_ins.label", Some("prefs.general.remember_sign_ins.help")),
    (PrefPage::General, "prefs.general.reopen.label", None),
    (PrefPage::General, "prefs.general.submodules.label", Some("prefs.general.submodules.help")),
    (PrefPage::General, "prefs.general.tips.label", Some("prefs.general.tips.help")),
    (PrefPage::General, "prefs.general.tips_again.label", Some("prefs.general.tips_again.help")),
    (PrefPage::General, "prefs.general.updates.label", Some("prefs.general.updates.help")),
    (PrefPage::General, "prefs.general.watch_files.label", None),
    (PrefPage::General, "prefs.links.label", Some("prefs.links.help")),
    (PrefPage::General, "prefs.ui.behind_alerts.label", Some("prefs.ui.behind_alerts.help")),
    (PrefPage::General, "prefs.ui.desktop_notifications.label", Some("prefs.ui.desktop_notifications.help")),
    (PrefPage::General, "prefs.ui.notifications.label", None),
    (PrefPage::General, "prefs.ui.spellcheck.label", Some("prefs.ui.spellcheck.help")),
    (PrefPage::Git, "prefs.general.git.label", Some("prefs.general.git.help")),
    (PrefPage::Git, "prefs.git_program.version.label", None),
    (PrefPage::Ui, "prefs.a11y.zoom.label", Some("prefs.a11y.zoom.help")),
    (PrefPage::Ui, "prefs.ui.avatars.label", Some("prefs.ui.avatars.help")),
    (PrefPage::Ui, "prefs.ui.branch_visibility.label", Some("prefs.ui.branch_visibility.help")),
    (PrefPage::Ui, "prefs.ui.columns.label", Some("prefs.ui.columns.help")),
    (PrefPage::Ui, "prefs.ui.details_panel.label", None),
    (PrefPage::Ui, "prefs.ui.file_lists.label", Some("prefs.ui.file_lists.help")),
    (PrefPage::Ui, "prefs.ui.ghost.label", Some("prefs.ui.ghost.help")),
    (PrefPage::Ui, "prefs.ui.graph.columns.label", None),
    (PrefPage::Ui, "prefs.ui.graph.density.label", None),
    (PrefPage::Ui, "prefs.ui.graph.description.label", None),
    (PrefPage::Ui, "prefs.ui.group_colour.label", Some("prefs.ui.group_colour.help")),
    (PrefPage::Ui, "prefs.ui.group_tabs.label", Some("prefs.ui.group_tabs.help")),
    (PrefPage::Ui, "prefs.ui.highlight_rows.label", None),
    (PrefPage::Ui, "prefs.ui.left_panel.label", None),
    (PrefPage::Ui, "prefs.ui.menu_bar.label", Some("prefs.ui.menu_bar.help")),
    (PrefPage::Ui, "prefs.ui.theme.label", Some("prefs.ui.theme.help")),
    (PrefPage::Ui, "prefs.ui.title_bar.label", Some("prefs.ui.title_bar.help")),
    (PrefPage::Ui, "prefs.ui.toolbar.label", Some("prefs.ui.toolbar.help")),
    (PrefPage::Ui, "prefs.ui.toolbar_labels.label", None),
    (PrefPage::Ui, "prefs.ui.worktree_wips.label", Some("prefs.ui.worktree_wips.help")),
    (PrefPage::Accessibility, "prefs.a11y.announce.label", Some("prefs.a11y.announce.help")),
    (PrefPage::Accessibility, "prefs.a11y.blink.label", Some("prefs.a11y.blink.help")),
    (PrefPage::Accessibility, "prefs.a11y.colors.label", Some("prefs.a11y.colors.help")),
    (PrefPage::Accessibility, "prefs.a11y.contrast.label", Some("prefs.a11y.contrast.help")),
    (PrefPage::Accessibility, "prefs.a11y.diff_size.label", None),
    (PrefPage::Accessibility, "prefs.a11y.focus.label", Some("prefs.a11y.focus.help")),
    (PrefPage::Accessibility, "prefs.a11y.reduce_motion.label", Some("prefs.a11y.reduce_motion.help")),
    (PrefPage::Accessibility, "prefs.a11y.shortcuts.label", Some("prefs.a11y.shortcuts.help")),
    (PrefPage::Accessibility, "prefs.a11y.terminal_size.label", None),
    (PrefPage::Accessibility, "prefs.a11y.toast_time.label", Some("prefs.a11y.toast_time.help")),
    (PrefPage::Accessibility, "prefs.a11y.tooltip_delay.label", None),
    (PrefPage::Accessibility, "prefs.a11y.zoom.label", Some("prefs.a11y.zoom.help")),
    (PrefPage::Language, "prefs.general.language.label", Some("prefs.general.language.help")),
    (PrefPage::Language, "prefs.ui.date_format.label", None),
    (PrefPage::Language, "prefs.ui.date_locale.label", None),
    (PrefPage::Language, "prefs.ui.date_time_format.label", None),
    (PrefPage::Language, "prefs.ui.full_date_format.label", None),
    (PrefPage::Language, "prefs.ui.long_date_format.label", None),
    (PrefPage::Language, "prefs.ui.relative_dates.label", None),
    (PrefPage::DiffViewer, "prefs.editor.font_size.label", Some("prefs.editor.font_size.help")),
    (PrefPage::DiffViewer, "prefs.editor.history_pane.label", Some("prefs.editor.history_pane.help")),
    (PrefPage::DiffViewer, "prefs.editor.history_preview.label", Some("prefs.editor.history_preview.help")),
    (PrefPage::DiffViewer, "prefs.editor.line_numbers.label", Some("prefs.editor.line_numbers.help")),
    (PrefPage::DiffViewer, "prefs.editor.reset.label", None),
    (PrefPage::DiffViewer, "prefs.editor.syntax.label", Some("prefs.editor.syntax.help")),
    (PrefPage::DiffViewer, "prefs.editor.tab_size.label", Some("prefs.editor.tab_size.help")),
    (PrefPage::DiffViewer, "prefs.editor.word_highlights.label", Some("prefs.editor.word_highlights.help")),
    (PrefPage::DiffViewer, "prefs.editor.wrap.label", Some("prefs.editor.wrap.help")),
    (PrefPage::Tools, "prefs.tools.arguments.label", None),
    (PrefPage::Tools, "prefs.tools.diff.label", Some("prefs.tools.diff.help")),
    (PrefPage::Tools, "prefs.tools.diff_args.label", None),
    (PrefPage::Tools, "prefs.tools.editor.label", Some("prefs.tools.editor.help")),
    (PrefPage::Tools, "prefs.tools.editor_file_args.label", None),
    (PrefPage::Tools, "prefs.tools.editor_repo_args.label", Some("prefs.tools.editor_repo_args.help")),
    (PrefPage::Tools, "prefs.tools.merge.label", Some("prefs.tools.merge.help")),
    (PrefPage::Tools, "prefs.tools.merge_args.label", None),
    (PrefPage::Terminal, "prefs.terminal.args.label", Some("prefs.terminal.args.help")),
    (PrefPage::Terminal, "prefs.terminal.colours.label", Some("prefs.terminal.colours.help")),
    (PrefPage::Terminal, "prefs.terminal.copy_on_select.label", Some("prefs.terminal.copy_on_select.help")),
    (PrefPage::Terminal, "prefs.terminal.cursor.label", Some("prefs.terminal.cursor.help")),
    (PrefPage::Terminal, "prefs.terminal.dim.label", Some("prefs.terminal.dim.help")),
    (PrefPage::Terminal, "prefs.terminal.font_size.label", Some("prefs.terminal.font_size.help")),
    (PrefPage::Terminal, "prefs.terminal.middle_click.label", Some("prefs.terminal.middle_click.help")),
    (PrefPage::Terminal, "prefs.terminal.open_in_panel.label", Some("prefs.terminal.open_in_panel.help")),
    (PrefPage::Terminal, "prefs.terminal.option_meta.label", Some("prefs.terminal.option_meta.help")),
    (PrefPage::Terminal, "prefs.terminal.program.label", None),
    (PrefPage::Terminal, "prefs.terminal.reset.label", None),
    (PrefPage::Terminal, "prefs.terminal.shell.label", None),
    (PrefPage::Terminal, "prefs.tools.terminal.label", Some("prefs.tools.terminal.help")),
    (PrefPage::Terminal, "prefs.tools.terminal_args.label", Some("prefs.tools.terminal_args.help")),
    (PrefPage::RepoIdentity, "prefs.identity.email.label", Some("prefs.identity.email.help")),
    (PrefPage::RepoIdentity, "prefs.identity.name.label", None),
    (PrefPage::RepoHooks, "prefs.hooks.monitor.label", None),
    (PrefPage::RepoTracker, "prefs.tracker.year_branches.label", Some("prefs.tracker.year_branches.help")),
];

/// What a query finds on one page: how many of its rows match, and the first of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMatch {
    pub page: PrefPage,
    pub rows: usize,
    /// The label key of the page's first matching row.
    pub first: Option<&'static str>,
}

/// The pages of `pages` (in that order) that `query` matches: by their name, or by a row's
/// label or help, ignoring case. An empty query matches nothing.
pub fn search(t: Lang, query: &str, pages: &[PrefPage]) -> Vec<PageMatch> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let has = |text: &str| text.to_lowercase().contains(&q);
    pages
        .iter()
        .filter_map(|&page| {
            let mut hits = INDEX.iter().filter(|(p, label, help)| *p == page && (has(t.tr(label)) || help.is_some_and(|h| has(t.tr(h)))));
            let first = hits.next().map(|(_, label, _)| *label);
            let rows = first.map_or(0, |_| 1 + hits.count());
            (rows > 0 || has(page.nav_label(t))).then_some(PageMatch { page, rows, first })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[PrefPage] = &[PrefPage::General, PrefPage::Ui, PrefPage::Accessibility, PrefPage::Terminal];

    #[test]
    fn a_query_finds_pages_by_rows_and_by_name() {
        let found = search(Lang::En, "notification", ALL);
        assert_eq!(found[0].page, PrefPage::General);
        assert!(found[0].rows >= 2, "{found:?}");
        assert!(found.iter().all(|m| m.page != PrefPage::Ui), "notifications left Appearance: {found:?}");
        let by_name = search(Lang::En, "TERMINAL", &[PrefPage::Terminal]);
        assert_eq!(by_name.len(), 1);
        assert!(search(Lang::En, "  ", ALL).is_empty());
        assert!(search(Lang::En, "zzzz-no-such-setting", ALL).is_empty());
    }

    #[test]
    fn the_first_match_is_the_pages_first_matching_row() {
        let found = search(Lang::En, "spell", ALL);
        assert_eq!(found, [PageMatch { page: PrefPage::General, rows: 1, first: Some("prefs.ui.spellcheck.label") }]);
    }

    #[test]
    fn every_key_in_the_index_is_translated() {
        for (_, label, help) in INDEX {
            for key in std::iter::once(*label).chain(*help) {
                for t in [Lang::En, Lang::De, Lang::Es] {
                    assert_ne!(t.tr(key), key, "{key} has no {t:?} text");
                }
            }
        }
    }

    /// Label keys in `prefs/*.rs` that name something other than a form row.
    const NOT_ROWS: &[&str] = &[
        "prefs.ssh.keys.error.label",
        "prefs.ssh.service.title.label",
        "prefs.ssh.which.all.label",
        "prefs.ssh.which.repo.label",
        "prefs.ui.theme.files.label",
    ];

    #[test]
    fn every_row_label_is_in_the_index() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/prefs");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") || path.ends_with("index.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (i, _) in text.match_indices(".label\"") {
                let start = text[..i].rfind('"').unwrap() + 1;
                let key = &text[start..i + ".label".len()];
                if key.starts_with("prefs.") && !NOT_ROWS.contains(&key) && !INDEX.iter().any(|(_, l, _)| *l == key) {
                    missing.push(format!("{}: {key}", path.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        assert!(missing.is_empty(), "row labels missing from prefs/index.rs:\n{}", missing.join("\n"));
    }
}
