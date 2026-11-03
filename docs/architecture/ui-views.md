# UI: views, panels and menus (glowfly-ui, `crates/glowfly-ui/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. Paths are relative to `ui/` (glowfly-ui's `src/`) unless they
start with `state/`, `update/` or `ui/`.

## Dialogs with their own files

- `clone_dialog.rs`, `clone_service.rs`: the Clone dialog.
- `init_dialog.rs`: the Init dialog.
- `branch_delete_dialog.rs`: a local branch's Delete dialog (`Modal::DeleteBranch`): the question, a counting line, then the commits only the branch has and Delete anyway; Cancel stays the default.
- `remote_branch_delete_dialog.rs`: Delete remote branch (`Modal::DeleteRemoteBranch`): the question, the local branches tracking it with Unset upstream, and the Git Flow warning.
- `conflict_check.rs`: the toolbar's conflict-check chip, its popover and the conflict report dialog; `conflict_check_parts.rs`: the names block, file list and divider they share.
- `op_preview_dialog.rs`: the operation preview (`Modal::OpPreview`): the dialog with its headline, conflicted paths, lost commits and Apply buttons.
- `op_preview_rows.rs`: the operation preview's history-afterwards lane and the lost commits' rows.
- `trust_dialog.rs`: the "Trust this repository?" dialog (`Modal::Trust`): the path, what runs, the advice, and Don't trust / Trust (or Cancel / Trust and run), neither filled.
- `trust_dialog_rows.rs`: its path line cut to one line and its setting and hook rows: a warning triangle or Git LFS tick, the program word emphasised, and the "Git LFS" tag.
- `welcome.rs`, `welcome_steps.rs`: the first-run welcome.
- `welcome_header.rs`: the welcome's header band and progress bar.
- `welcome_art.rs`: the band's lattice drawing per theme.
- `welcome_cards.rs`: the action cards: the Start step's three choices and the start page's row of four.
- `tour.rs`: the tour of the main window.
- `getting_started.rs`: Help > Getting started: the feature cards by group, each with Show me.
- `askpass.rs`: the credential prompt: a modal on top of everything (Preferences and other dialogs included) for what git or ssh asks through the askpass helper.
- `branch_dialogs.rs`: the new branch, rename and upstream dialogs of a local branch.
- `deep_link_dialog.rs`: the dialogs a glowfly:// link may need (`Modal::DeepLink`): a link Glowfly can't open (or won't, when its URL is a file or network path), a repository not found here (locate it on disk or clone it), and several copies to choose from.
- `file_dialogs.rs`: the New file and Rename file dialogs.
- `forge_filter_dialog.rs`: the filter dialog (`Modal::ForgeFilter`): a saved pull request or issue filter's name and conditions.
- `patch_apply_dialog.rs`: the Apply patch dialog (`Modal::ApplyPatch`): what the chosen file holds (its commits and the files it touches), how to apply it (its changes with the working tree, index, fallback and reverse options, or as commits with the three-way, carriage return, Signed-off-by and date options), why it can't be applied now, then Apply.
- `patch_save_dialog.rs`: the Create patch dialog for uncommitted changes (`Modal::SavePatch`): which changes go in (all, only staged, only unstaged), untracked files, and Only selected files with a checklist of the scope's files.
- `publish.rs`: the "Publish to GitHub" dialog: where to create the repository, its name, visibility and description.
- `repo_finder.rs`: the Find repositories dialog (`Modal::FindRepos`): the project folders Glowfly looks in, a filter, and the repositories found under them, grouped by project folder.
- `ssh_keygen_dialog.rs`: the Generate SSH key dialog (`Modal::SshKeygen`): where to save the key, its comment and an optional passphrase, checked as they are typed.
- `stash_dialog.rs`: the Rename stash dialog (`Modal::RenameStash`): one field for the stash list text after "On <branch>: ".
- `stash_push_dialog.rs`: the Stash dialog (`Modal::StashPush`): a message, what goes into the stash (every change, only the staged changes, or files ticked in a list of the working tree) and whether the staged changes also stay staged (not offered with only the staged changes; unticked each time the dialog opens).
- `submodule_dialogs.rs`: the Add submodule dialog (a URL, the folder it goes in, an optional branch), the Edit submodule dialog (its URL and branch) and the Remove submodule confirmation (what it deletes and what would be lost), each with a side pane that explains the step.
- `tab_dialog.rs`: the Rename tab dialog (`Modal::RenameTab`): the name the tab shows instead of its folder's.
- `tag_dialog.rs`: the New tag dialog: a plain tag, or an annotated one whose message is required.
- `trash_offer_dialog.rs`: the No Trash on this drive dialog (`Modal::TrashOffer`): the trash on a file's drive can't be used, so the file can go to the Trash in the home folder (copied there) or be deleted permanently instead.
- `undo_dialog.rs`: the dialog before undoing a checkout that created a branch (`Modal::UndoCreated`): it switches back, and also deletes the branch while the box is ticked.
- `worktree_dialog.rs`: the Create worktree dialog: a folder (typed, or a parent picked with Browse…), an optional new branch (required for a remote branch, which it then tracks) and whether to open the worktree in a tab, with a side pane that explains the step.
- `worktree_manage_dialog.rs`: the Remove worktree dialog (what goes, why force is needed, whether the branch and its copy on the remote are deleted too, and that another tab showing it closes) and the Lock worktree dialog (an optional reason), each with a side pane that explains the step.
- `worktree_move_dialog.rs`: the Move worktree dialog: the new folder (typed, or a parent picked with Browse…), checked as the user types with the pure rules in `state::worktree_move`; the runner checks the disk and reports a taken folder under the field.
- `remotes/fork_create.rs`: the Fork dialog: looks for the user's own fork of a remote's repository first (offering to add it), says when the repository is the user's own, and otherwise is the confirmation to fork it into the user's account, add the fork as a remote and fetch it.
- `remotes/fork_pick.rs`: the Add a fork dialog: the forks of a remote's repository on its hosting service, filtered as the user types, with the ones already added as remotes marked; the picked fork is added as a remote under a name Glowfly suggests.
- `menu_bar/about.rs`: Help > About Glowfly (`Modal::About`): the version, licence and git in use and the project's website.

## Blame and the file history's File view

- `blame/`: the blame view: its header, gutter and line menu, and its body.
- `blame/body.rs`: wrapped rows, or sideways scrolling under a fixed gutter, with line widths and wrapped heights measured once per blame in `rows.rs`.
- `find_bar.rs`: find and go to line, shared by the blame header and the file history's File view.
- `history_text_cache.rs`: the File view's measured line widths and wrapped heights.
- `history_preview.rs`: the File view's previews: the Text | Preview switch and a Markdown file drawn by the shared `markdown.rs` renderer through `MarkdownCache`, or an image.
- `preview_cache.rs`: frees an image preview's texture at the end of a frame that drew none.
- `image_paint.rs`: a decoded image's texture and checkerboard, shared with the binary diff.
- `file_view_switch.rs`: the Diff | File | Blame switch the file history and blame headers share.
- `blame/gutter.rs`: a run's gutter: its author, date, commit and summary, the age strip and tint, and the hover text with the commit's whole message.
- `blame/header.rs`: the blame header.
- `blame/menu.rs`: a blame line's menu: blame before its commit or skip it, the file history and the graph, and copying the line or the commit's SHA.
- `file_buttons.rs`: file actions as buttons in the file history's header (its selected commit): restore one commit's version of a file, open the working-tree file in its default program, show it in the file manager.
- `history_file.rs`: the history's File view: the whole file as of the selected commit (for a commit that deletes it, the version just before), in the editor font with optional line numbers and syntax colors.
- `history_folder.rs`: a folder history entry's changed files: the files under the folder that the selected commit changed (against its first parent), one selected, whose diff or text shows below.

## Markdown bodies

- `markdown.rs`: draws Markdown bodies (pull request and issue descriptions and comments, and the File view preview).
- `markdown_inline.rs` (tests in `markdown_inline_tests.rs`): text runs, with room after an italic run's slant and beside a code run's background; links' tooltips with the host in punycode, and clicks sent as `Msg::OpenLink` with the link's whole text. The mismatched-link prompt's snapshots are in `snapshot_tests/snapshot_tests_link_prompt.rs`.
- `markdown_code.rs`: code blocks.
- `markdown_cache.rs`: parses and highlights each body once (bounded by body count and bytes).

## The graph (history table)

- `graph/layout.rs`: the history table's column layout: each column a cell, divider to divider, whose stored width is the whole cell, with its content 12 px in from a divider and 16 px in from the table's edges.
- `graph/fit.rs`: how the columns give way in a narrow graph (CHANGES, COMMIT, AUTHOR, DATE hide, then BRANCH / TAG narrows and its chips move inline, MESSAGE keeping 240 px), never saved.
- `graph/cells.rs`: the graph row's single-line text cells, and the pull request chips by head commit.
- `graph/refs_fit.rs`: how wide BRANCH / TAG's chips want to be (each row folded to its first chip and "+N"), for the column's auto-fit and inline chips.
- `graph/wip_row.rs`: this tab's WIP row with its label, change chips and line total, spanning the columns it leaves empty; short of room the total hides, then the label is cut to 6 characters, then the chips hide.
- `graph/branch_chip.rs`: a branch's ref chip drawn outside the graph, as the Branch / tag column draws it (the rebase panel's tip row).
- `graph/labels.rs`: the row's ref chips with their marks, fitting and "+N" chip, the halo round the checked-out branch and HEAD (no layout room; a ref chip after it moves 3 px).
- `graph/labels_tests.rs`: the chips' fitting, widths (marks; the halo takes none) and hit testing.
- `graph/nodes.rs`: the row's nodes, sized by the row density.
- `graph/marks.rs`: the painted local, remote and tag marks.
- `graph/row_look.rs`: the row's background: the lane band and the highlight (`surfaces::band_color`, `highlight_color`) and a search match's accent bar; the selection and hover come from `widgets/row_fill.rs`. Also a search-dimmed row's inks and opacity.
- `graph/frame_asks.rs`: what the rows ask for while they draw (change counts, the next chip tip, the build tips seen), applied once they are done.
- `graph/find_marks.rs`: a search's matched text on a matching row (summary, body preview, author, short SHA), inked and on the find fill.
- `graph/lane_colours.rs`: the graph's per-row colours, and each lane line's width (HEAD's branch wider) and dash pattern (`graph/paint.rs`, colour-safe colours only).
- `graph/more.rs`: the row for a failed load for more history.
- `graph/ref_menu.rs`: the graph row's right-click menu: a ref chip's own menu or the commit menu.
- `graph/ref_checkout.rs`: what a double-click on a branch chip checks out (a remote label split into remote and branch by `state/remote_branch.rs`).
- `branch_dot.rs`: the branch dots (in menus, in the row's icon column).
- `graph/header.rs`: the graph's column header: the column titles with their draggable dividers (`resize.rs`), the least width each column gets so its title fits, the columns fitted to the graph's width (`fit.rs`), and a ⋯ mark naming any columns that didn't fit.
- `graph/paint.rs`: drawing helpers for the commit graph: lanes and the details panel's avatar.
- `graph/ref_group.rs`: which of a row's ref labels share a chip, and the local branches' upstreams it looks up (kept per snapshot).
- `graph/resize.rs`: dragging the graph header's dividers to resize columns, and double-clicking one to reset its column.
- `graph/search_bar.rs`: the graph's search bar: the query field (with a painted clear button while it has text), the Highlight | Filter mode, the author picker, the match count in a fixed-width slot (so Close doesn't move as it changes) and Close.
- `graph/worktree_wip.rs`: other worktrees' WIP rows in the graph (a dashed node in its lane's colour joined to the worktree's HEAD commit, an "Uncommitted changes · <folder>" box and the change counts).

## Commit details and the WIP panel

- `details_commit.rs`: the selected commit's details, title first: the title as git splits it, the author's row, the body block, the files.
- `details_chips.rs`: the details' row of quiet chips for the SHA, parents and committer, drawn by `widgets/chip.rs`.
- `file_status.rs`: the changed-file rows' status badge (an 18 px circle).
- `details_wip_header.rs` (tests in `details_wip_header_tests.rs`): the WIP lists' headers with their more-actions menu, and their fit order (icon-only buttons, then the title cut before its count, then the menu), shared with `conflict_files.rs`.
- `details_wip_list.rs`: the Unstaged and Staged Files lists, in List or Tree mode.
- `commit_box.rs`: the commit box, top to bottom: header with the identity, summary with its length inside, description, the Amend and Push options, the button and its caption; `commit_button.rs`: its split button and caret menu (Stage all and commit, Commit and push, Amend last commit, Skip hooks).
- `file_tree.rs`: the changed-file lists as a tree: the rows a list draws (cached between frames by which list it is), a folder row, the header's List / Tree toggle and a file row's indent.
- `folder_menu.rs`: a folder row's menu: Folder history, Copy path, and Stage or Unstage folder in the WIP lists.
- `list_focus.rs`: a file list that takes the keyboard focus as one Tab stop: its focus target, the keys it takes before any binding (`state/list_keys.rs`; each list names which beyond its moves, `KeySet`), and its arrows kept from egui's focus moves from its first frame on (`restore`). Reuse it for any other file list.
- `wip_keys.rs`: the Unstaged and Staged Files lists on `list_focus.rs`: one stable focus target per list, their keys as `KeyAction::WipList` messages, and the list `update` asked to focus.
- `file_list_keys.rs`: the lists that only open files (commit, comparison, stacked, working-directory comparison, pull request, folder history, Conflicted and Resolved Files) on `list_focus.rs`: one focus target per list taking only its plain moves (`KeySet::MOVES`) as `KeyAction::FileList` messages, a row's click focusing it, and the focus ring.
- `pane_focus.rs`: the panes the pane keys and F6 move between (`state/layout.rs::Pane`): the graph's, the left panel's and the other central views' focus targets, where each pane was drawn (which pane has the focus), the details panel's first file list taking a requested focus, and Esc on a focused control giving the graph the focus back. Tests in `snapshot_tests/snapshot_tests_pane_focus.rs`.
- `shortcuts/live.rs`: what the frame's state allows each global binding (`allowed`): covered by a dialog, typing, the graph or a diff shown, the focused pane.
- `graph/select.rs`: the graph's clicks and keys: its focus target, and with it or nothing focused its arrows, Page, Home, End, Enter and the H, [ and ] bindings; a row shows `selected` while the graph is active, else `tokens::selected_inactive`, as the file lists do (`list_focus::rows_active`).
- The keyed lists' rows are `keyed_file_row`s or `keyed_file_row_with_end`s with no Tab stops of their own, and the focused list's current row gets `file_row_focus`'s ring.
- `op_strip.rs`: the strip under the toolbar while an operation is in progress; in the conflict view, the operation as a sentence with Abort and Commit merge or Continue.
- `op_steps.rs`: the WIP panel's numbered steps while an operation is in progress: the heading with a quiet Abort, Resolve conflicts, Check what goes in, and Commit or Continue pinned at the bottom.
- `details_banner.rs`: the heading over the WIP panel for an operation that has no steps (`state::op_steps`): a bisect, or git's ambiguous "apply mailbox or rebase" leftover, with nothing conflicted.
- `details_wip.rs`: the WIP panel: the file lists in a scrolling area over the commit box, which stays at the bottom (its height measured the frame before).
- `details_workdir.rs`: right panel for one commit compared with the working directory: the commit, and the files that differ from it to the working tree (staged and unstaged changes and untracked files), or with Swap direction from the working tree to it, each opening that diff.
- `signature_badge.rs`: the details panel's signature badge: Verified / Signed / Expired / Revoked / Bad signature, a tooltip with signer, key and reason, and a menu (Check again, Copy key, settings) that a click opens below it and a right-click at the pointer.
- `am_banner.rs`: the Applying patches lines under the WIP panel's step heading while a `git am` series is stopped (`state::am_banner`).

## Diff

- `diff/header/`: the diff's file header, one 48 px row built from `state/diff_header.rs`'s model.
- `diff/header/identity.rs`: the file's icon, folder, name and chips.
- `diff/header/nav.rs`: hunk and file steps with the counter.
- `diff/header/view_menu.rs`: the View button and its menu.
- `diff/header/actions.rs`: the ⋯ menu of file actions and the diff tool, the small secondary Stage or Unstage, and Close.
- `diff/header/controls.rs`: the ghost buttons and dividers, re-exported from `widgets/ghost.rs`, which the terminal panel's header shares.
- `diff/header/fit.rs`: which parts fit, in narrowing order.
- `diff/text_sel.rs`: text selection in the diff's code: the press, double- and triple-click, Shift-click and drag with auto-scroll past the edge, the opaque `code_sel` (`code_sel_dim` without the keyboard) behind the glyphs with its nub, the diff's own keyboard focus, Ctrl+C through `state/diff_text.rs`'s `copy`, Ctrl+A (`DiffMsg::SelectAllText`) and the code's screen-reader document node.
- `diff/row_menu.rs`: the diff's right-click menu over a line's code (Copy, Copy with diff signs, Copy lines, Copy hunk as patch, then the hunk header's line actions on the selected or clicked line), kept or dropping the text selection by where the click lands, and Shift+F10 at the topmost selected line.
- `diff/preview.rs`: a binary diff's body: the image's Before and After over a checkerboard, Added and Deleted placeholders, a card for an image that can't be decoded, or both sizes for any other binary.
- `conflict_header.rs`: the conflict view's file bar (path and sides, the centred counter and progress meter, Mark resolved, the more-actions menu and ×).
- `conflict_band.rs`: a conflict's band in the panes: amber mark while unresolved, the side-coloured Take checkboxes, and the order with Swap when both sides are taken.
- `diff/fold.rs`: the unchanged runs' controls: the whole-file view's "N unchanged lines" row of a folded run, which opens in place when clicked, the hunk view's unfold control on a hunk header, which fills in the unchanged lines above it, and the fold-back control on the first line of an opened run (in either view), which folds it again.
- `diff/hunk_header.rs`: a hunk header row: the `@@` line with its function context in syntax colors, the hunk's action buttons, and in the hunk view the control that fills in the unchanged lines above it.
- `diff/inline.rs`: the inline layout: one column of hunks (or of the whole file), each line with its old and new number (unless line numbers are off), and the unchanged lines between hunks once filled in.
- `diff/line_check.rs`: how a changed line shows that it is selected, or can be: the bar on its left edge (the accent when selected, faint only while its gutter is hovered), and in the sign column a check box that stands in for the `+`/`-`: hollow while the line's gutter is hovered and on every changed line of a hunk that holds selected lines, filled with a check mark when the line is selected.
- `diff/rows_cache.rs`: the open diff's row list, widest line and gaps between hunks, built when the diff, the layout, the open folds or gaps or the tab width change instead of on every frame: they walk every line of the file.
- `diff/split.rs`: the split layout: the old side on the left, the new side on the right, with removed and added lines paired by similarity (see `glowfly_core::line_pairs`) and a divider between the columns.
- `diff/style.rs`: per-frame drawing metrics of the diff viewer, from Preferences > Diff viewer: the text font, the row height and the gutter positions, which scale with the font size so the default size draws exactly as the fixed layout did.
- `diff/syntax.rs`: painting diff line text with syntax colors.
- `diff/thread_marks.rs`: code comment markers on a pull request file's diff lines: a painted speech bubble (and the comment count when there are several) pinned to the visible right edge of the line, which opens the thread in the comment pane when clicked.
- `conflict_cells.rs`: cells of the conflict view's panes and output: a line number and text on a tint, clipped to the column, the text broken into visual lines when long lines wrap, and the row heights that makes.
- `conflict_fold.rs`: the conflict view's folds, drawn as the whole-file diff draws its own: the "N unchanged lines" row of a folded clean run, which opens it in the panes and the output alike, and the fold-back control on the first line of an opened run in the panes' checkbox gutter.
- `conflict_output.rs`: the conflict view's output, as it will be saved: painted rows tinted by where each line came from (a placeholder for each conflict still to pick, typed lines in green, long clean runs folded), or, with Edit on, the whole file as text to change by hand, conflicts not picked yet shown as markers.
- `conflict_panes.rs`: the conflict view's ours and theirs panes side by side: a Take checkbox per side in each chunk's band (ui/conflict_band.rs) and one per line of a chunk, in the side's colour, so a chunk can take whole sides or single lines.

## Object menus

- `menus.rs`, `branch_menu.rs`, `remote_branch_menu.rs`, `tags.rs`, `remotes/mod.rs`: the object menus (commit, multi-commit, local and remote branch, tag, remote).
- Each object menu is headed once by its object (`common::menu_header`, `common::commit_menu_header`), with its items in the review's intent groups (spec §10), previews as split rows (`common::menu_row_preview`), app keys in the shortcut column, and a Copy submenu only for three or more copy items; the commit menus' wording is in `state/commit_menu.rs`, what they leave out or fold together in `state/menu_trim.rs`.
- `remote_branch_menu.rs`: a remote branch's menu, shared by the REMOTE rows and the graph's remote labels, with its reasons in `state/remote_branch.rs`.
- `branch_colour_menu.rs`: the branch menus' Color submenu, main's included (its Automatic is the trunk tone).
- `stack_menu.rs`: the local branch menu's stack header note and its restack, Push stack and Create pull requests for the stack items.
- `create_pr_dialog.rs`: the create dialog's step line and Skip during a stack's run.
- `stack_rows.rs`: the LOCAL rows' stack indentation and painted elbows.
- `repo_colour_menu.rs`: the tab menu's, the welcome rows' and the group headers' Color submenu, with the square swatches in `repo_swatch.rs`.
- `repo_list.rs`: the start page's repository area: Favorites with + New group, the groups nested under it, the other favorites, and Recent.
- `repo_list_row.rs`: one start-page repository row: what its click, star and × send, and its menu (right-click or ⋯) with Move to group.
- `repo_list_groups.rs`: the start page's group headers (swatch, name, count, Open all, ⋯), the bar down a group's rows, an empty group's dashed placeholder, and the group menu (with Reset all group colors); `repo_list_drag.rs` drags favorites by their grip between groups.
- `repo_group_menu.rs`: a welcome favorite's Move to group submenu and a Recent row's Add to group submenu, ways to group without dragging.
- `repo_group_dialog.rs`: the New group and Rename group dialog.
- `repo_group_picker.rs`: a group's Add repositories dialog, a checklist of favorites and Recent repositories.
- `repo_menu_list.rs`: the repository menu's and File > Open recent's repository groups, Favorites and Recent, the filter field of a long list, and Remove missing / Clear recent.
- `repo_row.rs`: one repository row, shared by those menus and the start page: name, parent path, status, and the star and × on hover or focus (on the start page also the grip, a status pill, the key cap and ⋯); in a menu Up and Down step between the rows, Right and Tab reach the star and ×, typeahead by name.
- `file_menu.rs`: file actions in file rows' context menus: history and blame, restore from a commit or revert a change, open in the default program, the external editor or diff tool (or resolve in the merge tool), show in the file manager, new file in its folder, rename or delete (working-tree rows), then the custom commands (`custom_menu.rs`); the LFS, stash and patch items are in `file_menu_extra.rs`.
- `file_menu_extra.rs`: the file row menus' Track in Git LFS submenu, the Stash dialog's entries (the Staged Files header's menu, Stash this file… on working-tree rows) and the Create patch entries of working-tree rows (`file_menu.rs` holds the rest).
- `pr_menu_items.rs`: pull request items shared by the branch row menus and the toolbar: create one (into the branch selected before the menu opened, or from the checked-out branch into a remote one), and push the branch first when the code host lacks it or some of its commits.
- `repo_menu.rs`: the toolbar repository menu's workspace commands, after Dashboard: finding repositories in project folders, opening every favorite in a tab, and fetching or pulling the listed repositories.

## Left panel and dashboard

- `sidebar.rs`: left panel: local and remote branches, pull requests, issues, Git Flow branches, tags, stashes, worktrees and submodules.
- `stashes.rs`: the STASHES section of the left panel: one row per stash (a click selects its commit) with a menu to apply, pop, rename or delete it; an auto-stash shows a weak "auto" on the right.
- `submodules.rs`: the SUBMODULES section of the left panel: one row per submodule with a painted status marker (with its own short tooltip), a status word and an explanatory tooltip; double-click opens it, and the row menu opens, initializes, updates or restores it, copies its URL, edits or removes it.
- `ref_visibility.rs`: hiding branches and tags in the graph, or showing only some, from the left panel: the branch and tag menu items, the markers on hidden and shown-only rows, and the banner above the lists.
- `forge_filter_menu.rs`: the filter chip in the PULL REQUESTS and ISSUES headers, and its popover: the built-in filters, the repository's saved filters (edit, move, remove) and New filter…, sent as `Msg::ForgeFilter`; the chip sits left of the shared header's + and ⋯.
- `build_status.rs`: the build-status marker on pull request rows, branch rows and graph ref labels, each check's icon in the pull request panel's Checks section, and the pull request menu's build page item.
- `dashboard_rows.rs`: the dashboard's rows: a repository row (opens its preferred clone), pull request rows (two lines, with the build marker, votes and the shared tooltip) and issue rows, each with its context menu.

## Banners and chips

- `git_banner.rs`: the banner under the toolbar while git is missing (an error) or too old for some features (a warning; see `glowfly_core::git_version`), with a way to choose another git (Preferences > Git).
- `path_warnings.rs`: paths Glowfly won't act on: the working-tree banner about paths one file system can't keep apart (and a `core.ignorecase` that disagrees with the disk), and the note after a file row's path saying why its actions are off.
- `trust_chip.rs`: the toolbar's "Untrusted" chip: shown while the open repository's git is held; its tooltip says what is paused and why, and a click brings the trust prompt back (update/trust.rs).

## Git Flow

- `gitflow_dialog.rs`: the Git Flow Start dialog: the kind's prefix, a name field (prefilled with the next version for a release or hotfix), what Start will do, and any problem with the name.
- `gitflow_finish_dialog.rs`: the Git Flow Finish dialog: the branch, the tag name and message (release and hotfix), Keep the branch, and a live check against the snapshot: the numbered steps Finish will run, or the problem stopping it.
- `gitflow_panel.rs`: the Git Flow panel, shown in place of the graph: a stopped Finish's remaining steps (with Continue finish once its merge is committed), the production and development branches, then the feature, release and hotfix branches by kind with Start and Finish buttons; a set-up prompt when Git Flow isn't configured.
- `gitflow_resume_banner.rs`: the banner under the toolbar once the merge a stopped Finish was waiting for is committed: what is still to do, with Continue finish and Forget (also used by the Git Flow panel's row).
- `gitflow_section.rs`: the GIT FLOW section of the left panel: Features, Releases and Hotfixes groups with a Start '+' and menu each, and the local flow branches under them (prefix stripped).

## Command palette and shortcuts

- `palette.rs`: the command palette (Ctrl+P): a search field over the commands available now, ranked by a fuzzy match, or over a two-step command's items (tracked files for File history, stashes to apply, pop or delete), or a free-text step (the message for a new stash).
- `palette_label.rs`: a palette row's label: its matched characters in the accent colour, a file's folder part dimmed, and a ref or repository that doesn't fit cut in its middle with its matches moved along.
- `shortcuts.rs`: keyboard shortcuts: every changeable key, and Go to tab 1 to 8, matched through the effective keymap (`global.rs`, with presses turned into chords in `chords.rs`); recording a key for Preferences > Keyboard (`record.rs`); the shortcut sheet (`sheet.rs`) and the key caps it and Preferences draw (`caps.rs`).
- `shortcuts/caps.rs`: keys drawn as key caps: each chord's parts as caps joined by "+", alternatives joined by "or".
- `shortcuts/chords.rs`: key presses as chords, and taking a press that one of the keymap's bindings answers to out of egui's input before the views see it.
- `shortcuts/sheet.rs`: the keyboard shortcut sheet (Ctrl+/): a filter field over every binding in the table, grouped, each row with its keys drawn as key caps; Repository commands rows without a key are left out.

## Logs panel

- `activity_fold.rs`: the open part of an activity row, under its title: a failure's cause in a full sentence, its fix (once) with any second fix beside it (Try again beside Delete lock file) and Copy (the row menu's Copy entry), then the output in a quiet monospace block coloured by kind of line (the last `OUTPUT_LINES` of it); and the sub-rows of an open routine group.
- `activity_log.rs`: the logs panel at the bottom, on the panel surface: a 40 px header (the Activity, Errors and Performance tabs on the left, quiet icon buttons on the right with the More menu's Copy log and Export logs, a hairline under it), then the active tab's activity log (ui/activity_rows.rs) or the end of the error or performance log, read by the runner.
- `activity_rows.rs`: the activity tab of the logs panel: one 32 px row per run or note (result mark, title, weak summary, duration and relative time on the right, a chevron when it folds), routine auto-fetches folded into one grey row, and earlier sessions under their headers, drawn dimmer.

## Issue panel (`issue/`)

- `issue/comment_box.rs`: the issue panel's comment box, pinned under the conversation once the details have loaded: the last post's failure, the draft (read-only while it is being posted, its height capped so a long draft doesn't push the conversation away), a draft too long for the service, and the Comment button with why it is disabled.
- `issue/conversation.rs`: the issue's description and comments, oldest first, as Markdown (the same way the pull request conversation shows them; plain text from Azure Boards and Jira), each comment with a Reply that quotes it into the comment box (`comment_box`) pinned below once the details have loaded; or why they aren't shown: loading, held while the repository isn't trusted, or the failure with Try again.
- `issue/create_dialog.rs`: the Create issue dialog (`Modal::CreateIssue`): the Jira project or Trello board, the type (Bitbucket: kind; Trello: list), the priority (Bitbucket), the title (Jira: summary; Trello: name), the description, labels and assignees (`picker`), the Jira epic or Azure parent, and whether to create a branch afterwards, all captioned in the tracker's words (`issue_words`).
- `issue/info.rs`: the issue panel's side: the details (state or status as a chip, type, assignees, labels as chips, milestone or fix version, epic, who opened it and when), the local branches named after it with a Checkout each (Open worktree for one another worktree has), and the actions (Create branch…, the Git Flow starts, Copy).
- `issue/picker.rs`: a multi-picker for the Create issue dialog's labels and assignees: the chosen items as chips with a painted remove cross, a search field, and up to `MAX_ROWS` matching items to add (shown while searching).

## Pull request panel (`pull_request/`)

- `pull_request/commits.rs`: the pull request panel's Commits tab, on the left: the commits the source brings (the target's tip..the source tip, oldest first), painted like file history's rows.
- `pull_request/conversation.rs`: the pull request's conversation: who opened it and its description, then every comment, submitted review and code comment thread (`threads`), oldest first, with the comment box pinned below.
- `pull_request/files.rs`: the pull request panel's changed files: the heading, notes on local branches that are missing or behind, or on the head commit shown instead (with Fetch buttons; the Commits tab shows the same notes), and the list, whose rows open their diff.
- `pull_request/info.rs`: the pull request panel's details: status lines and reviewers.
- `pull_request/line_comment.rs`: the new code comment's box, the first card of the Files changed pane under its file's diff: where it goes, the lines it is on, the text, Suggest a change, and Cancel, Start a review and Add single comment (Add review comment while a review is pending), each disabled with its reason.
- `pull_request/merge.rs`: the "Merge" section: whether the service would merge now, the merge methods the service offers (none when the repository allows none Glowfly can send), the commit title and message, deleting the source branch afterwards (on the remote, and the local branch), and the Merge button (with a confirmation, and why it is disabled under it).
- `pull_request/review.rs`: the "Your review" section: the verdict (comment, approve, request changes; on GitLab, comment, approve or remove approval; on Bitbucket, comment, approve or request changes, either taken back once given), a summary, and the Submit review button, or Finish review for a review already started on the service, with why it is disabled under it.
- `pull_request/threads.rs`: code comment threads in the pull request panel: the cards (in the conversation and in the Files changed tab's pane under a file's diff), the Show resolved / Show outdated bar, and the pane itself.

## Range-diff (`range_diff/`)

- `range_diff/interdiff.rs`: the selected pair's pane.
- `range_diff/pairs.rs`: the commit pairs: 56 px painted rows in git's order, laid out only where visible.

## Interactive rebase panel (`rebase/`)

- `rebase/banner.rs`: the rebase's lines under the WIP panel's step heading (`op_steps`): the stopped commit (its short SHA a link to it in the graph, the summary, and why it stopped), a failed command, and the autostash note.
- `rebase/commit_row.rs`: a commit row of the rebase panel: grip, action, squash connector, SHA, summary, author and date, and its controls (the action combo box, Edit message, Move up and Move down).
- `rebase/drag.rs`: dragging rebase panel rows to reorder them: the grab, the slot a row would drop into, and the `RebaseMove` sent on release.
- `rebase/edit_menu.rs`: the graph menus' history edits: reword, squash into the parent and drop one commit, or squash and drop a multi-selection.
- `rebase/editor.rs`: the rebase panel's inline editors, drawn under their row: a commit's new message, the message of the commit a squash group combines into (under the group's last row), and an exec step's command.
- `rebase/keys.rs`: the rebase panel's keys, while it can be edited, no editor is open, nothing has the keyboard focus and nothing covers the window: the arrows select a row (with Alt they move it), the action letters set a commit's action, X and B add an exec or break step, Delete or Backspace removes one, and Enter opens the selected row's editor.
- `rebase/merge_row.rs`: the fixed rows of a list that recreates merges: a label row (a "label" chip and the commit it marks), a reset row (a "reset" chip and where the next line starts), and the merge dot a merge row shows in place of the grip.
- `rebase/reword_dialog.rs`: the Reword dialog (`Modal::RewordCommit`): a new message for a commit chosen in the graph, or, as the Squash dialog, the message of the commit a squash makes (pre-filled with the joined messages).
- `snapshot_tests/snapshot_tests_rebase_header.rs`: the rebase panel at 1100 px: the subtitle wrapping, never cut, the order line and the tip row after the last step (none for an empty list).
- `rebase/step_row.rs`: an exec or break row of the rebase panel: a grip, a chip naming the step, the command (or a note for a break), and Edit command, Remove step, Move up and Move down.

## Preferences (`prefs/`)

- `prefs/nav.rs`: the navigation column: Back, "Search preferences" (Ctrl+F), the pages filtered by the query with their match counts, and the "This repository · name" line.
- `prefs/index.rs`: the search index, (page, label key, help key) for every row, and the query match; a test fails on a row label missing from it.
- `prefs/reveal.rs`: the row Enter in the search opened: scrolled to and tinted for 1.2 s (solid under reduce motion).
- `prefs/form_help.rs`: help lines: 13 px, at most 64 characters a line, long help behind "Learn more".
- `prefs/general_notify.rs`: General's Notifications and Commit messages groups.
- `prefs/appearance.rs`: Preferences > Appearance, and its Interface size row, which Accessibility shows too (`zoom_row`).
- `prefs/appearance_graph.rs`: Preferences > Appearance > Commit graph: which columns the graph shows (one labelled checkbox each, in one row), the row density, how commit descriptions show, and the two layout resets.
- `prefs/diff_viewer.rs`: Preferences > Diff viewer: how the diff viewer draws text.
- `prefs/form.rs`: the pieces every Preferences form is built from: a row (a right-aligned label, then the controls with their help underneath, stacked when the content column is narrow), titled sections, labelled checkbox rows, text fields and combos that fit the column, help and warning lines, and the page's primary button.
- `prefs/general.rs`: Preferences > General, in titled sections: Updates, Git (general_git.rs), Fetching, Graph, Repositories, Sign-ins, Help and tips, and Diagnostics.
- `prefs/general_git.rs`: Preferences > General > Git: the git program Glowfly runs (chosen on the Git page), the default branch name for new repositories, and two global git config values, long paths and line endings.
- `prefs/git.rs`: Repo > Identity and Git hooks.
- `prefs/integrations.rs`: Preferences > Integrations (one page per hosting service: a card for each connected account, then Add another account or the sign-in form, integrations_form.rs) and Repo > Issue Tracker (with whether year-like branch names name issues).
- `prefs/integrations_form.rs`: Preferences > Integrations: one card per connected account, and the sign-in form (Git Credential Manager where the service supports it, or a host, user and token), both aligned with the field column.
- `prefs/keyboard.rs`: Preferences > Keyboard: every binding by group with its keys, a filter, and Reset all (asked for first).
- `prefs/keyboard_row.rs`: one row of Preferences > Keyboard: the action and where it works on the left; on the right its keys as caps (click one to record a replacement), a weak × after each that strengthens on hover, Add and Reset.
- `prefs/language.rs`: Preferences > Language & region: the interface language, and how dates and times are written (the locale, four patterns that follow the locale while blank, each with a live preview of the current time, and relative dates).
- `prefs/profile_setup.rs`: Preferences > Profile: the selected profile's Accounts and Preferences rows, which say what switches with it.
- `prefs/profiles.rs`: Preferences > Profile: the open repository's identity (profile_rules.rs), the profile list (current marker, switch, inline rename, delete), the New profile form, and the selected profile's identity fields (saved by the Save / Revert bar) and folder rules (profile_rules.rs), then what else switches with it (profile_setup.rs).
- `prefs/save_bar.rs`: the Save / Revert bar fixed under a Preferences page with unsaved edits (shown only while `state::prefs_save::unsaved` holds), and the "Saved" tick beside the page title.
- `prefs/ssh_agent.rs`: Preferences > SSH: the section header the page's three sections share (the title, an optional status chip, a Refresh icon button at the right end, and the rule under them), and the Agent section: its status chip (`state::ssh_marks::agent_pill`), the Use local SSH agent setting, what the agent holds and why it may not be reachable, and on Windows the Windows' OpenSSH row (`ssh_windows`).
- `prefs/ssh_card.rs`: Preferences > SSH: one key pair's card.
- `prefs/ssh_git.rs`: Preferences > SSH: the Which key section.
- `prefs/ssh_service.rs`: Preferences > SSH: the inline Add to service form under a key's card.
- `prefs/ssh_sources.rs`: Preferences > SSH, under the key cards: the `~/.ssh/config` IdentityFile entries that can't be listed as cards (each with where it is and why), and the keys only the agent holds, as compact cards that can only copy their public key.
- `prefs/ssh_windows.rs`: Preferences > SSH, in the Agent section: the Windows' OpenSSH row.
- `prefs/trello.rs`: Trello's parts of Preferences: the API key and token links on its Integrations page, and the Board picker under This repository > Issue tracker.
- `snapshot_tests/snapshot_tests_prefs_search.rs`: the search, the 760 px page in a wide window, Learn more, and D18's light goldens.

## Snapshot tests

- `snapshot_tests/`: the views' snapshot tests, one file per view group (`mod.rs` lists them), with the shared fixtures in `snapshot_fixtures.rs`; `snapshot_tests_painted.rs` reads what a frame painted inside a widget (fills, shapes, underlined text) for the hover tests.
- `snapshot_fixtures.rs` (tests, and the app's tests through glowfly-ui's `test-support` feature): the fixture repository, harnesses and `render`; `snapshot_fixtures/scenes.rs`: the merge stopped on conflicts and the two-author history the tests share.
- The app's `runner_snapshots/` holds the view snapshot tests that run effects through the runner (the whole-file and wrapped diffs, the conflict markers warning, the path search), on these fixtures and snapshots; glowfly-ui never depends on the runner.
- `readme_screenshots/` (tests): the README's screenshots in `docs/screenshots/`, rendered at 2x from a made-up demo repository (`demo.rs`, its files in `demo_files.rs`): the main window, a split diff and the conflict resolver (`shots.rs`) and a pull request review (`pull_request.rs`), each in Graphite and Porcelain; `UPDATE_SCREENSHOTS=1` writes them, otherwise the stored sizes are checked.
- `snapshot_check/mod.rs`: `check_snapshot` (the text snapshot, plus the image for a golden, cropped as listed; skipped with `GLOWFLY_SKIP_SNAPSHOTS`), and `builder()`, which every test harness starts from so harnesses render on pooled wgpu devices; under `test-support` for the app's tests too, always against glowfly-ui's `tests/snapshots/`.
- `snapshot_check/text.rs`: a frame's text snapshot: the accessibility tree's roles, labels, values and states, then the painted text no widget carries; only the dialogs when one is open.
- `snapshot_check/compare.rs`: comparing a text snapshot with `tests/snapshots/<name>.txt` (`.new.txt` always, `.diff.txt` on a mismatch, `UPDATE_SNAPSHOTS=1` accepts).
- `snapshot_check/goldens.rs`: the ~125 pixel goldens and their crop (window, dialog, top strip, top-left corner), checked against the PNGs on disk; `IMAGE_ONLY`, the theme variants kept as images only.
- `snapshot_check/identical.rs`: no two text snapshots alike, with a shrink-only allowlist of the groups still alike.
- `snapshot_check/tests.rs`: the tolerance, the skip variable, cropping and the text comparison's files.
- `menu_testing.rs`: opens a submenu (hovering and resting past its open delay) and finds its entries.
