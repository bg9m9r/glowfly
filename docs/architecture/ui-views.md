# glowfly-app UI: views, panels and menus (`crates/glowfly-app/src/ui/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. Paths are relative to `ui/` unless they start with `state/`,
`update/` or `ui/`.

## Dialogs with their own files

- `clone_dialog.rs`, `clone_service.rs`: the Clone dialog.
- `init_dialog.rs`: the Init dialog.
- `branch_delete_dialog.rs`: a local branch's Delete dialog (`Modal::DeleteBranch`): the question, a counting line, then the commits only the branch has and Delete anyway; Cancel stays the default.
- `op_preview_dialog.rs`: the operation preview (`Modal::OpPreview`): the dialog with its headline, conflicted paths, lost commits and Apply buttons.
- `op_preview_rows.rs`: the operation preview's history-afterwards lane and the lost commits' rows.
- `welcome.rs`, `welcome_steps.rs`: the first-run welcome.
- `welcome_header.rs`: the welcome's header band and progress bar.
- `welcome_art.rs`: the band's lattice drawing per theme.
- `welcome_cards.rs`: the Start step's choice cards.
- `tour.rs`: the tour of the main window.

## Blame and the file history's File view

- `blame/`: the blame view: its header, gutter and line menu, and its body.
- `blame/body.rs`: wrapped rows, or sideways scrolling under a fixed gutter, with line widths and wrapped heights measured once per blame in `rows.rs`.
- `find_bar.rs`: find and go to line, shared by the blame header and the file history's File view.
- `history_text_cache.rs`: the File view's measured line widths and wrapped heights.
- `history_preview.rs`: the File view's previews: the Text | Preview switch and a Markdown file drawn by the shared `markdown.rs` renderer through `MarkdownCache`, or an image.
- `preview_cache.rs`: frees an image preview's texture at the end of a frame that drew none.
- `image_paint.rs`: a decoded image's texture and checkerboard, shared with the binary diff.
- `file_view_switch.rs`: the Diff | File | Blame switch the file history and blame headers share.

## Markdown bodies

- `markdown.rs`: draws Markdown bodies (pull request and issue descriptions and comments, and the File view preview).
- `markdown_inline.rs`: text runs.
- `markdown_code.rs`: code blocks.
- `markdown_cache.rs`: parses and highlights each body once (bounded by body count and bytes).

## The graph (history table)

- `graph/layout.rs`: the history table's column layout: each column a cell, divider to divider, whose stored width is the whole cell, with its content 12 px in from a divider and 16 px in from the table's edges.
- `graph/cells.rs`: the graph row's text cells.
- `graph/wip_row.rs`: this tab's WIP row with its label, change chips and line total, spanning the columns it leaves empty and keeping the label whole longest.
- `graph/labels.rs`: the row's ref chips with their marks, fitting and "+N" chip.
- `graph/nodes.rs`: the row's nodes.
- `graph/marks.rs`: the painted local, remote and tag marks.
- `graph/row_look.rs`: the row's background: the lane band and the highlight; the selection and hover come from `widgets/row_fill.rs`.
- `graph/lane_colours.rs`: the graph's per-row colours.
- `graph/more.rs`: the row for a failed load for more history.
- `graph/ref_menu.rs`: the graph row's right-click menu: a ref chip's own menu or the commit menu.
- `graph/ref_checkout.rs`: what a double-click on a branch chip checks out (a remote label split into remote and branch by `state/remote_branch.rs`).
- `branch_dot.rs`: the branch dots (in menus, in the row's icon column).

## Commit details and the WIP panel

- `details_commit.rs`: the selected commit's details, title first: the title as git splits it, the author's row, the body block, the files.
- `details_chips.rs`: the details' row of quiet chips for the SHA, parents and committer, drawn by `widgets/quiet_chip.rs`.
- `file_status.rs`: the changed-file rows' status chip.
- `details_wip_header.rs`: the WIP lists' headers with their more-actions menu.
- `details_wip_list.rs`: the Unstaged and Staged Files lists, in List or Tree mode.
- `file_tree.rs`: the changed-file lists as a tree: the rows a list draws (cached between frames), a folder row, the header's List / Tree toggle and a file row's indent.
- `folder_menu.rs`: a folder row's menu: Folder history, Copy path, and Stage or Unstage folder in the WIP lists.
- `wip_keys.rs`: the Unstaged and Staged Files lists' keyboard focus: one stable focus target per list with its plain Up/Down.
- The WIP lists' rows are `keyed_file_row_with_end`s with no Tab stops of their own, and the focused list's current row gets `file_row_focus`'s ring.
- `op_strip.rs`: the strip under the toolbar while an operation is in progress.
- `op_steps.rs`: the WIP panel's numbered steps while an operation is in progress: the heading with a quiet Abort, Resolve conflicts, Check what goes in, and Commit or Continue pinned at the bottom.

## Diff

- `diff/header/`: the diff's file header, one 48 px row built from `state/diff_header.rs`'s model.
- `diff/header/identity.rs`: the file's icon, folder, name and chips.
- `diff/header/nav.rs`: hunk and file steps with the counter.
- `diff/header/view_menu.rs`: the View button and its menu.
- `diff/header/actions.rs`: the file actions, their More menu, the primary Stage or Unstage and Close.
- `diff/header/controls.rs`: the ghost buttons and dividers, re-exported from `widgets/ghost.rs`, which the terminal panel's header shares.
- `diff/header/fit.rs`: which groups fit, in narrowing order.
- `diff/preview.rs`: a binary diff's body: the image's Before and After over a checkerboard, Added and Deleted placeholders, a card for an image that can't be decoded, or both sizes for any other binary.
- `conflict_header.rs`: the conflict view's header with its more-actions menu.

## Object menus

- `menus.rs`, `branch_menu.rs`, `remote_branch_menu.rs`, `tags.rs`, `remotes/mod.rs`: the object menus (commit, multi-commit, local and remote branch, tag, remote).
- Each object menu is headed once by its object (`common::menu_header`, `common::commit_menu_header`), with its items in the review's intent groups (spec §10), previews as split rows (`common::menu_row_preview`), app keys in the shortcut column, and a Copy submenu only for three or more copy items; the commit menus' wording is in `state/commit_menu.rs`, what they leave out or fold together in `state/menu_trim.rs`.
- `remote_branch_menu.rs`: a remote branch's menu, shared by the REMOTE rows and the graph's remote labels, with its reasons in `state/remote_branch.rs`.
- `branch_colour_menu.rs`: the branch menus' Color submenu.
- `stack_menu.rs`: the local branch menu's stack header note and its restack, Push stack and Create pull requests for the stack items.
- `create_pr_dialog.rs`: the create dialog's step line and Skip during a stack's run.
- `stack_rows.rs`: the LOCAL rows' stack indentation and painted elbows.
- `repo_colour_menu.rs`: the tab menu's and the welcome rows' repository Color submenu, with the square swatches in `repo_swatch.rs`.
- `repo_menu_list.rs`: the repository menu's and File > Open recent's Favorites and Recent groups, the filter field of a long list, and Remove missing / Clear recent.
- `repo_menu_row.rs`: one repository row of those menus: name, parent path, status, and the star and × on hover or focus; Up and Down step between the rows, Right and Tab reach the star and ×, typeahead by name.

## Snapshot tests

- `snapshot_tests/`: the views' snapshot tests, one file per view group (`mod.rs` lists them), with the shared fixtures in `helpers.rs`.
- `snapshot_check/mod.rs`: `check_snapshot` (the text snapshot, plus the image for a golden, cropped as listed; skipped with `GLOWFLY_SKIP_SNAPSHOTS`), and `builder()`, which every test harness starts from so harnesses render on pooled wgpu devices.
- `snapshot_check/text.rs`: a frame's text snapshot: the accessibility tree's roles, labels, values and states, then the painted text no widget carries; only the dialogs when one is open.
- `snapshot_check/compare.rs`: comparing a text snapshot with `tests/snapshots/<name>.txt` (`.new.txt` always, `.diff.txt` on a mismatch, `UPDATE_SNAPSHOTS=1` accepts).
- `snapshot_check/goldens.rs`: the ~110 pixel goldens and their crop (window, dialog, top strip, top-left corner), checked against the PNGs on disk.
- `snapshot_check/tests.rs`: the tolerance, the skip variable, cropping and the text comparison's files.
- `menu_testing.rs`: opens a submenu (hovering and resting past its open delay) and finds its entries.
