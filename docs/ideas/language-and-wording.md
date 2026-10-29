# Ideas: Language and wording

Translations and Glowfly's own wording. Untriaged; see [README.md](README.md).

### Complete translations

- Disabled menu items' reasons still in English: the rebase edit-menu reasons ("This commit
  isn't on the current branch"), the branch menus' "has no upstream" / "isn't on a code host",
  and the remaining BUSY / JOB_RUNNING users
- Job and activity-log labels (the titles git jobs run under, such as "Reset (hard)")
- The worktree row menu (Prune missing worktrees), the Git Flow header menu, the pull request
  and issue row menus, and the filter popover's "Saved filters" and "None yet"
- Built-in filter names on the pull request and issue filter chips
- Names only screen readers hear: the column resize grips and the hidden and show-only markers
- The branch rows' ahead/behind tooltips and the graph header's shallow-clone tooltip

### Language

- Say in Preferences > Language which OS setting or variable the System choice came from
- Key names in shortcut hints follow the language (e.g. "Strg" on German keyboards)
- Plural rules beyond one/other (needed if a language with other forms is added)
- Wrap or shrink right-aligned Preferences labels that are too long in a translation
- Load user-supplied translation files from the settings folder
- A palette command to switch the language
- Right-to-left layout for right-to-left languages
- Translate undo action descriptions (Action::describe, e.g. "checkout of main") and the labels of the jobs they name, shown in the Edit menu, toolbar tips, status bar and activity log
- Translate error texts that come from glowfly-core: update check and download errors, theme file problems and the themes folder error
- Translate the tour of the main window (ui/tour.rs, state/onboarding_tour.rs) and its closing toast
- Translate git's missing or too-old banner text built in glowfly-core (welcome Git step and window banner)
- Translate the folder picker titles (Add a project folder, Open a Git repository and the other Preferences pickers)
- Translate the palette's repository commands (e.g. Remove missing repositories from the lists)
- Translate the pull request panel's file list texts (only its rows' Copy path follows the language)
- Translate the rebase edit-menu reasons from glowfly_core::rebase (This commit isn't on the current branch, merge selected, side line) and core's in_progress_text / DETACHED used by the rebase panel
- Translate the file history views' own texts (history.rs, history_folder.rs, history_file.rs, history_preview.rs, file_buttons.rs, file_view_switch.rs, diff/preview.rs) and drop the English constants kept for them (file_menu::BLAME, OPEN_DEFAULT, files::NOT_IN_WORKDIR, SHOW_IN_FOLDER, history NOT_ON_GRAPH, WRAP_LINES)
- Translate the find and go-to-line fields shared by blame and the file views (ui/find_bar.rs)
- Translate core's case-mismatch line in the File names banner (Snapshot::case_mismatch)
- Translate the external tool failure toasts (NO_DIFF_TOOL, NO_MERGE_TOOL, merge_end_text, unmatched quote) and the palette's tool details
- Translate the remaining JOB_RUNNING / blocked::BUSY users (tags, preference pages, fork and worktree dialogs, rebase quick edits)
- Translate the conflict view's undo descriptions and the merge tool job label shown in the status bar
- Translate the Stash dialog (its blocked reasons stay English with it until then)
- The conflict Output strip says "1 of 1 conflicts resolved": give it the header's singular and plural forms (its English snapshots change with it)
- A narrow conflict view cuts the panes' "picked automatically · whitespace only" tag in German; wrap it or drop the reason part when it doesn't fit
- The automatic-pick notes ("The conflict in this file was picked automatically…") are separate one / all texts; fold them into one plural text
- Translate the palette's rebase entries (Restore original order, Reload rebase steps)

### Glowfly's own wording

- Translate the rest of the Commit signing page (labels, help, expiry warning, key generation)
- Translate the worktree WIP row's tooltip lines and change counts ("2 modified, 1 added")
- Read the OS's own short and long date patterns instead of the per-language table
- Regional date defaults beyond the language (en_CA and fr_CA in ISO order, pt_PT, de_CH)
- Genitive month names in Polish long dates ("1 lutego", not "1 luty")
- A "Use the locale's format" reset button beside each date pattern field
- Rename internal identifiers to the new names (ghost_refs, highlight_ref_rows,
  BranchVisibility::Smart, FilterOp::Solo, date_word_format, date_verbose_format), with serde
  aliases so old settings files still load
- The "Showing only …" banner line is cut short beside the wider "Show the rest" button in the
  default sidebar width; give the line more room or move the button under it
- German unsigned-builds help as one sentence ("Da die Builds von Glowfly noch nicht codesigniert sind, bestätigt dieser Abgleich nur, …")
- Spanish unsigned-builds help: end with "…, pero no quién la publicó"
