# glowfly-app: features across the layers

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. Each section lists one feature's files in every layer
(`msg/`, `effect/`, `state/`, `update/`, `runner/`, `ui/`), all under `crates/glowfly-app/src/`.

## Glowfly's own updates (`UpdateMsg`, `UpdateEffect`)

- Files: `msg/updates.rs`, `effect/updates.rs`, `runner/updates.rs`, `runner/update_restart.rs`, `state/updates.rs`, `state/release_notes.rs`, `update/updates.rs`, `update/updates_restart.rs`, `update/release_notes.rs`.
- The check's and download's status, opening the release notes, and the runner's check, download, Install and restart, and tidying of old staging folders.
- Drawn by `ui/release_notes.rs` (the dialog), `ui/update_notice.rs` (the status bar chip) and `ui/prefs/updates_row.rs` (Preferences > General > Updates).

## Operation journal (`JournalMsg`, `LostMsg`, `JournalEffect`)

- Files: `journal.rs`, `state/journal.rs`, `state/journal_restore.rs`, `state/journal_lost.rs`, `update/journal.rs`, `update/journal_lost.rs`, `update/git_job.rs`, `runner/journal.rs`, `ui/journal/`.
- Shown in place of the graph.
- `state/journal.rs`: the loaded timeline, the selection and its preview, and the row labels.
- `state/journal_restore.rs`: what Restore does for a state and why it can't.
- `state/journal_lost.rs`: the States / Lost commits tab, the lost list and what each lost commit or dropped stash offers (Apply stash waits for a running job or an operation in progress).
- `update/journal.rs`: opening, reloading and restoring.
- `update/journal_lost.rs`: the Lost commits tab's loads and actions.
- `update/git_job.rs`: which git jobs carry a note.
- `runner/journal.rs`: writing the notes around a run.
- `ui/journal/`: the header, the rows and the preview.
- `ui/journal/lost.rs`, `ui/journal/lost_preview.rs`: the lost list and its preview.

## Custom commands (`CustomCommandMsg`, `CustomCommandEffect`)

- Files: `custom_commands.rs`, `state/custom_commands.rs`, `state/custom_menu_items.rs`, `state/palette_custom.rs`, `update/custom_commands.rs`, `update/custom_commands_run.rs`, `runner/custom_commands.rs`, `ui/custom_menu.rs`, `ui/shortcuts/custom_keys.rs`.
- `state/`: the loaded commands, their keys and placeholder values, and the menus' Commands items.
- `palette_custom.rs`: the palette's entries.
- `update/custom_commands.rs`: loading and announcing problems.
- `update/custom_commands_run.rs`: running, confirming and holding for trust.
- `runner/`: reading the files and running a shell command.
- `ui/custom_menu.rs`: the Commands submenu; `ui/shortcuts/custom_keys.rs`: their keys.

## Partial clones and stopping jobs

- Files: `msg/partial.rs`, `effect/partial.rs`, `state/partial.rs`, `state/partial_confirm.rs`, `state/job_stop.rs`, `update/partial.rs`, `update/partial_size.rs`, `update/partial_content.rs`, `update/job_stop.rs`, `runner/partial.rs`, `runner/stops.rs`, `ui/content_fetch.rs`.
- Fetch full history's size check and its confirmation for a large history (`PartialMsg::HistorySized`).
- A partial clone's missing contents: the diff, File view or blame read that waits, its background download (trusted) or download job (through the trust gate), Cancel, and the read again (`PartialMsg::ContentMissing`, `PartialEffect::FetchContent`).
- The status bar's Cancel for a job that only fetches (`RunGit::stoppable`, `JobMsg::Cancel`, `Effect::StopGit`), with the stops the runner registers when it is handed a stoppable effect (`runner/stops.rs`), and the job ending as cancelled.

## Whole-history search index (`IndexEffect`, `SearchMsg::Index*`)

- Files: `effect/index.rs`, `runner/index.rs`, `runner/index_disk.rs`, `runner/index_query.rs`, `state/history_index.rs`, `update/history_index.rs`, `update/search_index.rs`.
- `update/history_index.rs`: the index of a large repository: one build at a time across tabs, newest request first, kept with its tab while parked, updated from the previous index after a reload, the first build's delay and the freeing of long-parked tabs' indexes (`history_index::tick`).
- The first build waits `FIRST_BUILD_DELAY` after the history loads with no index in memory (driven by `Msg::Tick`); a tab parked `PARKED_FREE_AFTER` frees an index of `FREE_MIN_COMMITS` or more and reads it back from the store when shown.
- `runner/index_disk.rs`: the runner reads the saved index (`commit_index::Store`) before a build without a base, and saves the finished build after reporting it.
- `update/search_index.rs`: the index's answer for the query, the search bar's whole-history count, and loads that go straight to the next matches for Enter, Filter and Keep searching.
- `update/search_loaded.rs`: a query typed into a large loaded history (`refresh::LARGE_HISTORY`) waits for a pause in typing, then is matched on the query worker's second slot (`IndexEffect::MatchLoaded`, `SearchMsg::LoadedMatched`); Enter, a new history and smaller histories match on the UI thread.

## Large histories off the UI thread (`BackgroundEffect`, `GraphMsg::LaidOut`, `GraphMsg::AncestryFound`)

- Files: `effect/background.rs`, `runner/background.rs`, `update/large_graph.rs`, `state/ref_rows.rs`.
- `update/large_graph.rs`: a large history's relayout when its WIP rows change (`LayoutJob` on a worker, swapped in when it still fits), the default branch's ancestry for hover highlighting (each hovered tip's rows kept per snapshot in `RefRows`), and freeing replaced snapshots, layouts and closed tabs on a worker.

## Range-diff view (`RangeDiffMsg`, `RangeDiffJob`)

- Files: `range_diff.rs`, `state/range_diff.rs`, `update/range_diff.rs`, `runner/range_diff.rs`, `ui/range_diff/`.
- Shown in place of the graph.
- `state/range_diff.rs`: what was asked (`Origin`: a ref's last rewrite, or two tips from the journal or a two-commit selection), the phase, the selected pair, the words (subtitle, cut, counts, refusals, why an entry point is off) and the mainline to cut at.
- `update/range_diff.rs`: opening, holding until trusted, results by generation, Retry and Close.
- `runner/range_diff.rs`: the run.
- `ui/range_diff/`: the header, the pair list and the interdiff pane.
- Entry points: the local and remote branch menus, the multi-commit menu and the journal preview.

## Profile folder rules (`ProfileMsg` and `ProfileEffect` folder variants)

- Files: `state/repo_identity.rs`, `state/profile_rules.rs`, `update/profile_rules.rs`, `ui/prefs/profile_rules.rs`.
- `AppState::identities`: the identities git reported by git dir.
- `AppState::includes`: the one-at-a-time include sync and the git dirs repaired this session.
- `state/repo_identity.rs`: the words of the Open repository row, the commit box's identity line and the Repo > Identity hints.
- `state/profile_rules.rs`: the Folders row's texts.
- `update/profile_rules.rs`: adding after the runner's check, removing, syncing (never during a switch), reading the identity on open, focus, closing Preferences and after a sync or switch, the one placing repair per git dir, and Use <profile> here behind a danger confirmation.
- `ui/prefs/profile_rules.rs`: the Folders and Open repository rows.

## Several selected commits

- Files: `state/selection.rs`, `state/selection_span.rs`, `state/pick.rs`, `update/selection.rs`, `update/selection_focus.rs`, `update/pick.rs`, `update/pick_done.rs`, `ui/details_multi.rs`, `ui/details_stacked.rs`, `ui/mainline_dialog.rs`.
- `state/selection*.rs`: what they span (one line, a line from the first commit or a shallow boundary, or gaps, with the first commit whose parent isn't selected) and what they can do.
- `update/selection.rs`, `update/selection_focus.rs`: loading the combined or stacked changes and going back to the focused commit with a toast.
- `MainlineForm` in `state/pick.rs`, drawn by `ui/mainline_dialog.rs`: one mainline for every merge among them.
- `update/pick.rs`, `update/pick_done.rs`: the cherry-pick or revert job and the toast naming commits that changed nothing.
- `ui/details_multi.rs`: the details panel's list, gap note and combined list.
- `ui/details_stacked.rs`: each commit's own changes stacked.
