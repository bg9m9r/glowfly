# Ideas: History and the commit graph

The graph and history table, search, multi-select, interactive rebase and stacked branches. Untriaged; see [README.md](README.md).

### Stacked branches

- Palette entries for "Restack branches above <current>" and "Restack onto <lower>"
- Draw stacks in the graph too: a bracket or a stacked-on label beside the branch labels
- Use a branch whose upstream is a local branch (`branch.<name>.remote = .`) as an explicit
  stack hint
- Choose which of two branches at the same commit is the lower layer (name order for now)
- After the lower layer is squash-merged on the host, offer to restack the next layer onto
  the base
- Make the finished part of a restack aborted midway undoable as one entry (only the
  operation journal has it now)
- Stash uncommitted changes once for the whole restack instead of per step, so they always
  land back on the original branch
- Preference to restack automatically after amending or rewriting a lower branch, without the
  toast
- Preview a restack (commits per layer, predicted conflicts) in the operation preview dialog
- Restack a stack whose layer is checked out in another worktree by running that step in that
  worktree
- Retarget a layer's open pull request when the layer below it is merged or renamed, and after
  a restack, instead of leaving its base on the old branch
- Add a table of the stack's pull requests, linked and in order, to each layer's description
  and keep it current as layers are created or merged
- After a stale lease, offer Fetch and push again for the layers that were left alone
- Push stack to a remote other than each layer's upstream (or origin), chosen per run
- Keep a stack's create dialogs going across a tab switch instead of stopping the run
- Toolbar Push dropdown item for Push stack when the current branch is in a stack
- Handle rebase.updateRefs=true in the interactive rebase panel, whose todo then holds
  update-ref lines
- After aborting a stopped restack, check the branch that was checked out before out again
  (HEAD stays on the step's top, where git's abort leaves it)
- Show the git version reason (2.38 for `--update-refs`) on the branch menu's restack items
  too; today only the refusal toast says it

### Multi-select commits

- Revert changes to this file on a commit-to-working-tree row (Restore the version from the commit covers it today)
- A hunk action on the working-tree-to-commit direction (take the commit's version of a hunk from the swapped diff)
- Keep the open file's diff when swapping a working-directory comparison (reopen it the other way instead of closing)
- Syntax highlighting in context for the swapped comparison's working-tree side (it has no stored blob)
- Git LFS pointer note inside the swapped diff for its working-tree side (the note reads stored blobs only)
- Hide line check boxes on the working-tree-to-commit diff, where no line action applies
- Palette entries to swap or stop an active working-directory comparison
- A keyboard shortcut for Swap direction
- Compare a stash with the working directory
- List a renamed file under its working-tree name in the swapped comparison (it is listed under the commit's name)
- Show the whole heading of a combined range (hover or wrap) when the panel cuts it short
- Load more than the newest 200 commits' changes in a stacked list (a Show more row)
- Line counts and the change bar on stacked file rows
- One diff of a file across the stacked commits, each commit's hunks one after another
- Fold a commit's section in the stacked list (a painted disclosure icon)
- Click the commit named in the gap line to select it or scroll the graph to it
- A different mainline for each merge in one cherry-pick or revert (separate git runs per merge)
- Option to keep commits that change nothing as empty commits instead of skipping them
- Skip a commit that changes nothing inside a sequence on git older than 2.23
- Go back to the focused commit from the menu bar and the command palette
- An action on the Back to the focused commit toast that restores the previous multi-selection
- Preview cherry-picking several commits that include merges against the chosen mainline

### Interactive rebase

- Keep `update-ref` steps (`rebase.updateRefs`) so branches stacked inside the range move with
  it
- Move commits up/down or reword several commits from a multi-selection in the graph
- A preference to always stash automatically, without asking, before a rebase
- Spellcheck in the Reword dialog (needs `UiCache` in modals)
- Date tooltip on the painted author/date of rebase rows
- Edit the steps that finish a reworded commit while its rebase is stopped (they are shown locked
  now)
- Run a command after every commit (`--exec`) from the panel in one click
- Reschedule a failed exec step instead of skipping it on Continue (`--reschedule-failed-exec`)
- Show the shortcut keys on the graph menu's Reword / Squash / Drop entries and on the banner's
  Continue / Skip buttons
- Select several panel rows and move or change them together
- Tell a plain merge-backend rebase from an interactive one: recent git marks both interactive,
  so the banner names both "Interactive rebase" and offers "Edit remaining steps…" for both
- Edit the remaining steps of a running rebase that recreates merges (label, reset and merge
  steps in `rebase --edit-todo`)
- Reword or drop a merge commit from the graph's menu and keys (only the panel does it now)
- Squash, drop or reword commits that came in through a merge (side lines) from the graph
- Squash across a branch point from the graph (moving the later commit before the side line's
  start)
- Option to move merged side lines onto the new base too (`--rebase-cousins`)
- Readable label names taken from merge messages, and editing a merge's parents or moving whole
  lines in the panel
- Turn a merge row into an edit stop (a break after the merge) and squash or fix up into a merge
- A "Use the joined messages" reset and spellcheck in the squash dialog
- Rebase onto a commit by dragging a branch label onto it in the graph

### Hide / show only branches in the graph

- Hide / show only from the graph's BRANCH/TAG ref-label menus
- Hide stashes in the graph
- Hide, show only or stop showing only several selected branches at once (multi-select in the left panel)
- Hide all / show all per section (LOCAL, REMOTE, TAGS) from the section header menu
- Hover toggles (hide / show only) on rows that are not hidden or shown-only
- A graph-header button listing hidden branches and tags, to unhide them from there
- Keep the hidden / shown-only state when a branch is renamed (rename the entry instead of dropping it)

### Graph columns and loading

- A per-repository commit limit, or remembering the extra loaded commits across tab switches
  and restarts
- Keep the top visible commit in place when a load-more refresh also brings new commits above it
- Count down to the failed load's automatic retry ("Trying again in 8 s"); needs a clock value
  passed to the view
- Show the failed load's reason when the retry note leaves it no room in a narrow message column
- Skip the automatic retries for a load failure that won't pass (corrupt or missing objects)
- Ask a filtering search for another batch when the window grows taller and more rows fit
- Load more while scrolling to the end of a long filtered list
- Offer Keep searching under a short filtered list whose batch budget is spent
- Keep a deeper search's loaded history and budget across a tab switch
- Search older history for matches only, without laying out every commit (past the 200,000 cap)
- Lay out a large history with HEAD's lane kept for the WIP row, so a clean/dirty flip changes one row instead of a worker relayout
- Show a large history's WIP row at once (before the worker's relayout lands), drawn as a stub above HEAD
- Walk a hovered branch's rows on the worker too (still up to 5,000 first-parent steps on the UI thread, once per tip)
- Keep each parent commit the walk reads (for its time) in the queue, so popping it doesn't look the object up again; the walk is now mostly libgit2 object reads (about 1 s per 200,000 commits)
- Measure the walk with libgit2's strict hash verification off on read-only paths (not benchmarked with the slimmer commits)
- Share one `Authors` table between a tab's reloads and its search index, so an author is stored once per tab rather than once per load
- Graph edge lanes as `u32` (or `u16`) pairs would take a row from 104 to about 72 bytes

### Search and filter

- Path picker beside the author picker (tracked files and folders) that writes a path: term
- Follow renames for path: terms (match a file's earlier names, like file history does)
- Search paths across all merge parents, not only the first
- Show commits touching a file from the details panel or file list menu (fills path: in)
- Remember path answers across query changes (recent term sets) so going back doesn't search again
- Match path: terms against stashes and the WIP row's changed files
- Answer path: and file: terms from the history index (per-commit changed paths or a path-to-commits map built in the background)
- Keep the history index on disk between runs, keyed by repository and starts, so reopening a large repository needs no rebuild
- Share one history index between the worktree tabs of one repository
- Compact an index's text blocks after many updates leave unreachable commits' text behind
- Search the whole commit message, not only the summary and the body's first ~240 characters
- List the whole history's authors (with commit counts) in the author picker from the index
- Date terms (before:, after:) answered from the index's author and committer dates
- Apply .mailmap to author names and emails in search and the author picker
- Show the whole history's commit count in the graph header once indexed ("Newest 500 of 812345 commits")
- Hide the graph's Load more row in Filter mode when the index says no older commit matches
- Show a match older than the graph's 200,000-commit cap in the details panel without loading it
- Translate the search bar's count and hover texts
- Limit index memory by a byte budget as well as the commit cap, and say so in the hover
- Match path: queries in a large loaded history on the query worker too (they still match on the UI thread once files are searched)
- Work out the author picker's list on a worker for a large history (it groups every loaded commit on the UI thread)
- Match a large history from the index's answer when it is current (the loaded commits are its first positions) instead of a second scan

### History table that carries more information

- Give successive branches that reuse a lane slot different colours (colour per lane occupancy
  rather than per lane index), so neighbouring unrelated branches don't share a colour
- Option to show the committer rather than the author on nodes and in the commit column
  (CommitInfo carries only the author today)
- Show only the body's real first line in the message preview instead of the flattened body
  (needs the line break kept in CommitInfo)
- Show the changes column by default for new installs (stats are computed in the background per
  visible row)
- Right-clicking a specific chip opens that ref's menu directly instead of the per-label submenus
- Draw the quick-branch "+ branch" chip as a soft fill without an outline to match the ref labels
- Show which remote(s) a folded branch is in sync with as small remote initials beside the cloud
  mark when there are several remotes
- Click the "+N" chip to open a popup of all labels with their menus
- Cut a date that doesn't fit at its start ("…10:00 AM") so the time stays visible
- Fill the WIP node's dashed ring so the lane line doesn't show through it

### History table edges and commit details

- An 18 px display step in the type scale (a sixth step and its tests); the commit title uses
  SUBHEAD 16 semibold until then
- A taller graph column header (28-32 px) to match the roomier cells (shifts every row_y in
  tests)
- A brief "Copied" confirmation after clicking a copy chip
- Status names on hover for the file rows' status chips (Modified, Added, Renamed, Type changed)
- WIP Unstaged/Staged file rows use the status chips and the counts beside the name
- The compare-with-working-tree header uses the SHA chip instead of its Copy SHA button
- Rename rows show the old path (R from -> to)
- Commit trailers (Co-authored-by, Signed-off-by) shown as person chips
- Apply .mailmap to author and committer names in the details panel
- Note when the author and committer are the same but the commit date is later than the author
  date (rebased or amended)
- Read non-UTF-8 commit messages and names lossily instead of showing them empty
- A monospace option for the commit body block
- Mark the first parent of a merge (the branch merged into) on its chip
- Graph table: a keyboard focus ring on the column header dividers, to resize without a mouse
- The search bar's controls past the query field don't shrink: in a narrow graph the field's
  floor (160 px) is reached and Close can be cut at the right edge
