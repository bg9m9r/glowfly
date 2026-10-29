# Ideas: Diffs, blame and conflicts

The diff viewer, blame and file history, range-diff, the conflict check and the merge tool. Untriaged; see [README.md](README.md).

### Range-diff

- Choose the base by hand: a picker for the mainline branch or any commit, and the creation
  factor (--creation-factor)
- Pick the two versions from any refs or reflog entries in a dialog, including older rewrites
  than the last one, and a Swap sides button
- Show an added or dropped commit's own diff in the pane instead of a sentence
- Up and Down move through the pairs, and a key jumps to the next changed pair
- A palette command and a View menu item for comparing the checked-out branch with its version
  before the last rebase
- A Copy submenu (old and new SHA) and Show in graph on pair rows
- Wrap long interdiff lines with the diff viewer's wrap setting, and a side-by-side layout
- Syntax colours for the code lines inside the interdiff
- Include merge commits (--diff-merges=remerge, git 2.39+) and say when merges were left out
- Reload the comparison when the branch moves again instead of keeping a fixed pair of tips
- Notes in the comparison as an option (--notes / --no-notes)
- Only the left or right side (--left-only / --right-only, git 2.31+)
- Range-diff a pull request's previous and current head from the hosting service's push history
- Compare more than 300 commits a side in the background with progress and Stop
- Exercise the pull request and issue panels closing when the view opens in an update test
- Measure the header's three lines instead of a fixed 72 px height

### Diff viewer upgrade

- Whole-file view: derive the hunk spans from the whole-file highlight (or load the old text
  too), so switching back to Hunks without a reload keeps in-context colors
- Count the lines before a hunk twice when an earlier hunk leaves the two sides in different
  states (the highlight cost now assumes they are parsed once)
- With whitespace ignored, take the new side's unchanged lines from the new file instead of the
  old one (they can differ in whitespace only, so such diffs now fall back to per-hunk colors)
- PowerShell highlighting: `$(...)` subexpressions inside strings as code, splatting
  (`@params`), method and static member calls (`[Math]::Round`, `.Trim()`), and here-strings
  that don't start at the end of a line
- Color Kotlin annotations (`@Suppress(...)`), which the bundled grammar leaves unscoped
- Color Dockerfile `RUN` commands as shell (the hidden "Dockerfile (with bash)" grammar)
- Hunk view: show the lines above the first hunk and below the last hunk too
- Hunk view: Expand all / Collapse all for the gaps between hunks in the diff header
- Fill in gaps with whitespace hidden, taking the new side's lines
- Colour filled-in gap lines from the in-context parse (new side's states) instead of the old
  file on its own
- Code comment marks on filled-in gap lines of pull request diffs
- Show a loading state on a gap control while its lines are read
- Trim the bundled syntax set to the languages Glowfly wants, if the extra ~0.6 MB of binary
  size matters
- A per-file language picker (e.g. in the diff header), remembered per path
- Take the hunk-header function context from `.gitattributes` diff drivers (`diff=<lang>`,
  `xfuncname`) rather than only libgit2's default (lines starting with a letter, `_` or `$`)
- Parse the hunk-header context in the file's context (the state at the function line in the
  old text) for contexts that start inside a multi-line construct
- Color the function context in the conflict view's chunk headers
- A command or shortcut for the syntax highlighting toggle (now on the Editor preferences page)
- User-selectable syntax color themes beyond the dark and light palettes
- Pair moved lines across change runs and hunks for word highlights
- Pair the conflict view's side-by-side rows by similarity too
- Also ignore whitespace inserted where there was none (indenting from column 0, which git's
  `-b` still shows): ignore-all-whitespace or a leading/trailing line filter
- An ignore-whitespace choice on the Editor preferences page, and a command or shortcut for the
  toggle
- Apply the whitespace filter to the commit details file list and the graph's CHANGES counts
  (hide files whose only changes are whitespace)
- Ignore blank-line-only changes as a separate option
- Editor preferences: monospace font family choice and end-of-line character (LF/CRLF)
- Use the editor font size, tab size and line numbers in the conflict view
- Line selection and single-line stage/unstage/discard in the whole-file view (map whole-file
  line indexes back to the normal hunk and line)
- Open part of a fold (N more lines above or below) instead of all of it at once
- Keep folds open when a reload shifts their lines (follow them through the edit)
- Editor preferences for the context kept around changes and the minimum fold size, or turning
  folding off
- Hunks / Whole file and Inline / Split toggles in the File history header, and a command or
  shortcut for the view mode toggle
- Diff view vs file view toggle (plain file, or a rendered preview for Markdown/images)
- Revert single lines in the whole-file view (needs whole-file line selection first)
- Follow renames when reverting from commit and comparison diffs opened outside File history
  (find the file's current name from HEAD)
- A Revert changes to this file button in the commit diff header, next to Restore
- Revert several selected files of a commit at once from the file list
- Open the conflict view (or highlight the markers) after a revert left conflict markers
- Revert selected lines across more than one hunk at a time
- Next/previous change across entries in the File history view (step into the next or previous
  commit's diff of the file)
- An optional wrap-around from the last changed file back to the first (with a notice)
- Show which file of how many (e.g. 3 of 12) in the diff header
- Cache the change rows behind the Previous/Next buttons too (their state rebuilds the change
  rows twice per frame)
- Keep a wrapped line's indentation on its continuation lines
- A marker in the gutter on continuation lines of a wrapped row
- Wrap by display width so wide (CJK, emoji) characters count as two columns
- A keyboard shortcut and a command palette entry for the wrap toggle
- Wrap long hunk headers (their function context) instead of clipping them
- With whitespace hidden, an option to also apply the hidden whitespace changes inside a hunk
  (stage or revert the region as the full diff has it)
- Name the commit in a revert's undo label ("revert of a hunk of 1a2b3c4 in a.txt")

### Merge conflict tool

- Syntax highlighting in the conflict panes
- Mark several selected files resolved or conflicted at once (needs multi-select in the WIP
  file lists)
- Mark all resolved for one folder, once the WIP list has a folder view
- A Preferences switch to turn off automatic picks for trivial conflicts
- Pick trivial conflicts in every conflicted file at once, without opening each one
- Mark conflicted for a submodule resolved in Glowfly (no resolve-undo record is written for
  gitlinks)
- Mark conflicted for modify/delete conflicts (git can't recreate a conflict missing one side)
- Show the pick order next to ticked lines
- Shift-click to take or leave a range of lines
- Leave a side out of every conflict from its pane header (the opposite of Take all)
- Side colours and line numbers in the editable output
- Palette entry to mark only the open conflict's file resolved
- Expand all / fold all unchanged runs in the conflict view at once (header buttons or palette)
- Mark the clean lines edited in the output in the ours/theirs panes too
- Bring back picks for a chunk whose typed text still holds conflict markers (the restore falls back to the stages today)
- Bring back picks when git converts the file (filter, working-tree-encoding or ident attributes)
- Keep the editor's scroll-to-conflict exact when long lines wrap
- Save the panes/output split between sessions
- Cache the conflict view's wrapped row heights for very large files
- Allow one edit to span a conflict and the clean lines next to it in the output editor
- A separate wrap setting for the conflict view instead of sharing the diff viewer's
- A fold-back control in the painted output too (only the panes have one)
- Text typed into a clean run deleted between two conflicts joins the next conflict; let it bring the run back
- Drop expired resolution entries from the undo history as soon as the merge or rebase step is committed or aborted, not when they are next tried
- Reload a parked tab's conflict view when an undo there changed its open file
- Undo of a resolution whose resolve-undo record was removed outside the app (restore the stages from the recorded ids with libgit2, as Mark resolved's undo does)
- A toast with Undo after Mark resolved or Mark conflicted from the WIP panel or palette (only the conflict view's own toasts carry Undo)
- Keep a copy of a file over 64 MB, so Take ours/theirs or Mark conflicted can write over it (both refuse today)
- Take a side or mark conflicted over a symbolic link no index entry holds (refused today, as the link couldn't be put back)
- Ignore the executable bit in a conflict undo's file check when `core.fileMode` is false (an undo there can leave a file the next redo calls changed)

### Blame gutter

- Live re-blame of working-tree edits (uncommitted lines shown as not committed yet)
- Blame a file the commit deleted (its version just before the deletion)
- Show which lines blame.ignoreRevsFile re-attributed (ignored and unblamable marks)
- Re-highlight an open blame when the syntax highlighting preference changes
- Unskip a single skipped commit (Show all clears them all)
- Remember skipped commits per repository across sessions
- Case-sensitive, whole-word and regex find in blame
- Blame before a merge commit through a parent other than the one git reports as previous
- Select a history entry past the first batch when jumping from blame (needs Search further)
- Show history and blame of one file side by side, or switch between them in one panel
- Move/copy score thresholds, and -C -C (copies from any commit)
- A setting to turn age colouring off, or colour by absolute date instead of rank
- Keep the cursor on the same source line across a re-blame after HEAD moved, mapping lines
  through the change
- Say when a re-blame followed a rename (a note naming the old and new path)
- Back steps recorded at HEAD keep the file's old name after a re-blame followed a rename
- A keyboard shortcut and a View menu item for Wrap lines that act on the open blame view
- Keep the sideways scroll position across Back, Blame before this commit and a re-blame that
  followed a rename
- Let a narrow window shrink the blame gutter, or fold the summary into a hover, when wrapping
  leaves few columns

### Conflict check

- A per-repository page to turn the check on or off for one repository, overriding the General
  setting (with a note when it is off there)
- Rules for the branches to compare with: include and exclude patterns (`origin/main`,
  `origin/**`, `!origin/legacy`) and branches never checked
- Prefer an open pull request's target branch as the merge target
- Per file: the overlapping line ranges and the kind of conflict (content, modify/delete,
  rename) from merge-tree's messages; open a file's predicted result from the list
- A marker on the current branch's LOCAL row, a Check for conflicts entry on remote branch
  menus, and a palette command
- Check other local branches against their targets in the background and mark them in the
  sidebar
- Suggest fetching first when the target's remote-tracking branch is old, and pushing when the
  conflicting commits are only local
- Ignore warnings for one branch, or stop comparing with one remote, from the popover
- Calm states in the toolbar ("up to date with main", "no conflicts") instead of nothing

### External diff and merge tools

- Presets for well-known diff and merge tools (fill program and arguments in one click)
- Keyboard shortcut to open the selected file in the diff tool (or the merge tool for a
  conflicted file)
- Show/hide arguments toggle on the External Tools page
- Directory diff: a whole commit or comparison in the diff tool at once
- Open several selected files in the diff tool at once
- Compare any commit's version with the working file in the diff tool (from file history and
  blame)
- Console diff/merge tools started in a terminal window
- Cancel a running merge tool from Glowfly
- Option to mark the file resolved automatically when the merge tool saves it and exits with 0
- Name the temp folders after the sides' branch names instead of ours/theirs
- Name git config's tool in the file menus' hover notes and the palette's detail (needs a cached read per repository)
- Run console merge tools (vimdiff, nvimdiff, emerge in a terminal) in a built-in terminal tab
- Honour mergetool.keepBackup / keepTemporaries / hideResolved for commands Glowfly runs itself
- Open the diff once the trust prompt is accepted (hold the diff tool like a held job)
- Suggest git's built-in tool names (git difftool --tool-help) on the External tools page
- Use a built-in tool's own trusted exit code when the user overrides its cmd
- Copy git config's tool into the External tools fields with one click
- Run a built-in merge tool on a file whose name has *, ? or [ (refused now: git mergetool's script globs the name)

### File history and blame

- Follow a folder forwards to its later name when its history is opened at an older commit
- Detect a folder whose contents were moved from or to the repository root
- Keep the File view's line in view when moving to another commit of the same file
- Re-read an LFS pointer's content as soon as the repository becomes trusted, without reselecting
- A button to fetch a missing Git LFS object for the File view
- Blame the version before the deletion from a deleting commit's File view
- Restore, Open and Show items in a folder entry file's context menu
- Match case, whole word and regex options for find in the File view
- Select and copy text in the File view
- Find and go to line inside the Markdown preview
- Open relative links from the preview as that file at the same commit, and jump to `#anchors`
- Show images embedded in Markdown from repository paths at the same commit
- Table column alignment (`:---:`) in rendered Markdown (File view preview and pull request bodies)
- Virtualized layout so Markdown longer than 512 KB can be previewed
- Zoom, pan and actual-size toggle for the File view's image preview
- Play animated GIF and WebP previews (first frame only now)
- A keyboard shortcut for the File view's Text / Preview switch
- Preview other renderable text (reStructuredText, AsciiDoc)
- Use the editor font and size for code blocks in the File view preview (they use the body size now)
