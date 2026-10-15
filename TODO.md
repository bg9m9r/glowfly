# Trellis roadmap

What Trellis does today and what is planned next, roughly in priority order. Each item is a
target to design, not a finished spec. Ticked items are done; open sub-items under a ticked
item are known follow-ups.

Want to help? Pick an unticked item, open an issue to discuss the approach, then see
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Recent changes

The last 20 completed TODO items, newest first: when (local time), which item, and what was
done. Add a line at the top when ticking an item, and drop the oldest past 20.

- 2026-10-15 16:54 · **Signing beyond config**: Signature badge for the selected commit in the details panel: verified, unverified, unknown key or bad signature, checked with git and cached per commit (`07ab360`)
- 2026-10-15 16:43 · **Repository management**: Favorite repositories above Recent on the welcome screen and in the repository menu, remove one recent entry or clear the list, and missing folders marked (`59fc992`)
- 2026-10-15 16:02 · **Sparse checkout**: Sparse Checkout page in Preferences: turn on with folders or patterns, save, reapply and turn off; palette entry; libgit2 status hides left-out files (`0969dcf`)
- 2026-10-15 15:59 · **Graph columns and loading**: resizable graph columns: drag the header dividers to resize, double-click to reset (`3bc171e`)
- 2026-10-15 14:14 · **Clone and init options**: Fetch full history or N more commits of a shallow clone from the palette and the graph header (`94affbb`)
- 2026-10-15 14:01 · **Stash improvements**: Rename (edit the message of) a stash from the STASHES row menu and the palette, keeping its commit and position (`a6ee8cb`)
- 2026-10-15 13:39 · **Tag management**: Annotate an existing lightweight tag from the TAGS row menu: one job replaces it with an annotated tag on the same commit, signed when tag signing is on (`082c49c`)
- 2026-10-15 13:31 · **Repository trust**: Untrusted chip in the toolbar and a Review repository trust… palette command bring the trust prompt back (`6764632`)
- 2026-10-15 12:24 · **File actions**: Undo and redo a restore from a commit: the overwritten working copy is kept and put back byte for byte (`977d157`)
- 2026-10-15 12:14 · **Repository trust**: ask before running git in repositories whose config or hooks run commands; hold auto-fetch, hosting-service sign-in and git status until trusted (`8c0fb03`)
- 2026-10-15 12:14 · **Diff viewer upgrade**: Pair removed and added lines by similarity in the split layout and for word highlights (`9e4288f`)
- 2026-10-15 12:14 · **Diff viewer upgrade**: Cache the computed diff rows (done by the rows cache; the Previous/Next button state is left as a follow-up) (`bdc1893`)
- 2026-10-15 11:55 · **Command palette (fuzzy finder)**: Commands button in the toolbar, and recently run commands listed first (`f3b5231`)
- 2026-10-14 16:36 · **Tag management**: Delete a tag from one or all remotes and push it to all remotes, from the TAGS row menu (`4f5c0a5`)
- 2026-10-14 16:33 · **Clone and init options**: Shallow clone in the Clone dialog by commit count or since a date, with an optional branch (`ea485b4`)
- 2026-10-14 16:05 · **Remote management**: Add, edit and remove remotes and fetch one remote from the REMOTE section (`fc64d34`)
- 2026-10-14 15:43 · **Git Flow**: Finish feature, release and hotfix from a dialog, the palette and the Git Flow page: --no-ff merges, version tag, delete or keep the branch (`d0f1c9b`)
- 2026-10-14 15:14 · **Worktrees**: Remove, lock, unlock and prune worktrees from the WORKTREES section (`714c150`)
- 2026-10-14 15:09 · **Git LFS**: Track in Git LFS submenu on file rows for the file, its extension or its folder (`6b98a2b`)
- 2026-10-14 14:40 · **Worktrees**: WORKTREES section with open in a new tab and Create worktree from a branch or commit (`a661ce2`)
- 2026-10-14 14:34 · **Git Flow**: Git Flow preferences page with initialize, branch and prefix settings, and start feature/release/hotfix (`f6dd571`)

---

## Done

- Commit graph with lanes and merge curves, a BRANCH/TAG column, avatar nodes, a WIP row, and
  ↑/↓ navigation
- Left panel: Local, Remote, Pull Requests, Issues, Tags and Stashes, with filtering
- Staging by file and by hunk, discard (file, hunk, or all through a recoverable stash),
  commit, and amend
- Checkout, branch/tag here, cherry-pick, revert, reset soft/mixed/hard, merge, rebase, branch
  delete and force delete, and remote branch delete; branch rename, set/unset upstream,
  fast-forward without checkout, push any branch or to a chosen remote, and delete local +
  remote
- Fetch, Pull (ff / ff-only / rebase), Push (sets upstream automatically, force-with-lease),
  Stash and Pop, Clone, Init, and search
- Merge/rebase/cherry-pick in-progress banner with Continue/Skip/Abort
- Preferences: General, Profile (git identity), Integrations, UI, Commit Signing, plus
  repo-specific Identity, Hooks and Issue Tracker
- Integrations: GitHub, GitHub Enterprise Server, GitLab, GitLab Self-Managed, Bitbucket,
  Bitbucket Data Center, Azure DevOps, Jira Cloud, and Jira Data Center. Each supports listing
  and creating pull requests where applicable, listing issues and mentions, creating a branch
  from an issue, and connecting through Git Credential Manager or a token.

---

## Preferences parity (page by page)

The pages a full-featured Git client offers, and where Trellis stands:

| Page | Trellis | Gap |
|---|---|---|
| General | ✔ General | Missing: delete `.orig` files after merge, keep submodules updated, show all commits, lazy loading, path to `sh.exe`, extended logging, conflict detection |
| Profiles | ◐ Profile (git identity only) | Several named profiles with switching (P1 Profiles) |
| SSH | ◐ SSH page (list key pairs, copy a public key, generate an Ed25519 key) | Choose the key git uses, use local agent, add a key to a hosting service (P1 SSH key management) |
| Integrations | ✔ 9 services | Trello; PR/issue depth (P2) |
| AI | ✘ | Possible later: opt-in features on the Claude API |
| External Tools | ✘ | Editor, diff tool, merge tool, terminal (P1 External tools) |
| Notifications | ◐ location under UI Customization | Desktop notifications (P1 Notifications) |
| UI Customization | ✔ UI Customization | Repo group colors and worktree tab grouping (wait on those features) |
| Commit Signing | ✔ Commit Signing | Signature badges on graph rows and tags (P1 follow-ups) |
| Editor | ◐ Editor (font size, tab size, line numbers, syntax and word highlights) | Font family, word wrap, EOL character |
| In-App Terminal | ✘ | Shell, font, cursor style and dimming for the built-in terminal (P1 Built-in terminal) |
| Experimental | ✘ | Git executable selection and version (P3) |
| *Repo:* Encoding | ✘ | Per-repo file encoding for diffs |
| *Repo:* Gitflow | ◐ Git Flow page, Start and Finish | Git Flow panel, branch from issue, finish options (P1 Git Flow) |
| *Repo:* Git Hooks | ✔ hooks path | — |
| *Repo:* Commit | ✘ | Commit template, push after commit, skip hooks per repo |
| *Repo:* Agents | — | Out of scope |
| *Repo:* Conflict Prevention | ✘ | P1 Conflict prediction, with per-repo enable and target branch |
| *Repo:* LFS | ◐ Git LFS page, Track in Git LFS on file rows | LFS-aware auto-fetch (P1 Git LFS) |
| *Repo:* Sparse Checkout | ◐ Sparse Checkout page | Sparse clone, indicators (P1 Sparse checkout) |
| *Repo:* Issue Tracker | ✔ | — |
| *Repo:* Team | — | Out of scope (cloud teams) |

### UI Customization

Done except the two items that wait on other features (repo groups, tabs and worktrees).

- [x] **Theme** dropdown (dark, light, more)
- [x] **Notification location** (bottom left, top right, …), once toasts exist
- [x] **Date/Time section**: date/time locale (system or a chosen locale) and four custom format
      strings with live previews: *Date* (`MM/DD/YYYY`), *Date word* (`MMMM D, YYYY`),
      *Date/Time* (`MM/DD/YYYY @ h:mm A`, the graph's date column) and *Date verbose*
      (`MMMM D, YYYY h:mm A`, commit details).
  - [ ] Use the *Date* and *Date word* formats somewhere (they can be edited but nothing shows
    them yet), and add date tooltips
- [ ] **Default group color / reset all to default**,
      for repo groups in the repository manager. (Workspace color belongs to cloud Workspaces,
      which are out of scope.)
- [ ] **Automatically group worktree tabs together**, after tabs and worktrees
- [x] **Show toolbar icon labels**
- [x] **Enable spell checking** in commit messages
- [x] **Author initials vs avatars**: real avatars (Gravatar or the hosting service's
      avatar, cached on disk), with initials as the opt-in.
- [x] **Show ghost branch on the current commit**: a faded "+ branch" after the checked-out
      commit's labels when it is hovered or selected; click it to branch from the current branch
- [x] **Highlight associated rows when hovering a branch**
- [x] **Branch visibility in commit graph**: All / Smart.
      Smart shows only the current branch, its upstream and target, and branches with PRs.
- [x] **Graph column toggles**, one per column:
  - [x] Show branches and tags (BRANCH/TAG column)
  - [x] Show commit author (exists)
  - [x] Show commit date/time (exists)
  - [x] Show commit message
  - [x] Show commit description: **Always / On hover / Never** dropdown
  - [x] Show commit SHA column
  - [x] Show commit tree (the graph column itself)
  - [x] Show commit changes (+/- counts)
- [x] **Reset graph columns** to the default or compact layout
- Out of scope from this page: "Show Agents view in left panel"

---

## P0: daily-driver gaps

- [x] **Tabs (multiple open repos)**
  Tab bar; Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+1–9 and Ctrl+Shift+T to reopen; open tabs restored
  on start (only the active one loads until selected).
  - [x] Drag to reorder tabs; tab context menu (close others, close to the right)
    - [ ] Keyboard reordering, and Escape to cancel a drag in progress
    - [ ] Rename a tab; a tab list with recently closed tabs to reopen any of them
    - [ ] Ask before closing tabs that hold a commit draft or conflict picks
- [x] **Undo / Redo**
  Toolbar Undo/Redo and Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) for checkout, commit/amend, reset,
  branch delete and Discard all; per-tab, in-memory history of 20, refuses (and says why) when
  the repository no longer matches or local changes would be overwritten.
  - [ ] Undo per-file, per-hunk and per-line discard (needs a safety stash or blob backup first)
  - [ ] Undo across restarts by rebuilding actions from the HEAD, branch and stash reflogs
  - [ ] Undo merge, rebase, pull, cherry-pick, revert, create branch/tag, stash, pop and apply
  - [ ] Safety stash before `reset --hard`, so undoing the reset restores discarded changes;
    offer to stash local changes when undoing or redoing a hard reset or Discard all
  - [ ] When undoing `checkout --track` / `checkout -b`, offer to delete the created branch
  - [ ] Edit menu entries for Undo/Redo, once Trellis has a menu bar
- [ ] **Diff viewer upgrade**
  - [x] Split (side-by-side) vs inline view
  - [x] Next/previous change (F7 / Shift+F7, Alt+Down / Alt+Up)
  - [x] Syntax highlighting (`syntect`)
    - [x] Highlight in a second background pass, so diffs over the size limit (2,000 lines,
      100 KB, or a line over 1,000 bytes) show plain at once and gain colors later, with no
      plain flash on reload
    - [x] Highlight from the whole old and new files instead of per-hunk fragments, so hunks
      that start inside a block comment or multi-line string color correctly (with the
      whole-file view)
      - [ ] In the whole-file view, derive the hunk spans from the whole-file highlight (or
        load the old text too), so switching back to Hunks without a reload keeps in-context
        colors
      - [ ] Keep parse states per hunk across reloads (or checkpoint states by line), so
        staging one hunk of a large file doesn't re-parse the whole prefix in the background
      - [ ] Count the lines before a hunk twice when an earlier hunk leaves the two sides in
        different states (the cost now assumes they are parsed once)
      - [ ] Take the new side's unchanged lines from the new file instead of the old one when
        whitespace is ignored (they can differ in whitespace only; context text inside hunks
        then comes from the new side and doesn't match the old text, so such diffs fall back
        to per-hunk colors)
    - [x] More languages than syntect's defaults (TOML, TypeScript/TSX, PowerShell,
      Dockerfile, Kotlin, Swift) by bundling extra syntax definitions
      - [x] Highlight `.jsx` files with the bundled TSX grammar (the JavaScript (Babel) grammar
        is left out of the fancy-regex set)
      - [ ] PowerShell: color `$(...)` subexpressions inside strings as code, splatting
        (`@params`), method and static member calls (`[Math]::Round`, `.Trim()`), and
        here-strings that don't start at the end of a line
      - [ ] Color Kotlin annotations (`@Suppress(...)`), which the bundled grammar leaves
        unscoped
      - [ ] Color Dockerfile `RUN` commands as shell (the hidden "Dockerfile (with bash)"
        grammar)
      - [ ] Trim the bundled syntax set to the languages Trellis wants, if the extra ~0.6 MB
        of binary size matters
      - [ ] Show the third-party notices inside the app (an About dialog)
    - [x] Language overrides from `.gitattributes` (`linguist-language=<name>`: GitHub language
      names and common aliases, any case; `Text` turns highlighting off; unknown names fall back
      to detection)
      - [ ] A per-file language picker (e.g. in the diff header), remembered per path
      - [ ] For commit diffs, read the attributes stored in the commit instead of the working
        tree's
    - [x] Color the function context after `@@` in hunk headers
      - [ ] Take the context from `.gitattributes` diff drivers (`diff=<lang>`, `xfuncname`)
        rather than only libgit2's default (lines starting with a letter, `_` or `$`)
      - [ ] Parse the context in the file's context (the state at the function line in the old
        text) for contexts that start inside a multi-line construct
      - [ ] Color the context in the conflict view's chunk headers
    - [ ] A command or shortcut for the toggle (now on the Editor preferences page)
    - [ ] User-selectable syntax color themes beyond the dark and light palettes
  - [x] Word-level intraline highlights
    - [x] A preference to turn them off (Preferences > Editor)
    - [x] Pair removed and added lines by similarity instead of by position
      - [ ] Pair moved lines across change runs and hunks
      - [ ] Pair the conflict view's side-by-side rows by similarity too
  - [x] Ignore leading/trailing whitespace (diff header toggle, saved in settings; hunk and line
    operations are off while it is on, since `apply_hunk` counts the unfiltered hunks)
    - [ ] Map hunk and line stage/unstage/discard, and Revert Hunk on commit diffs, from the
      filtered diff back to the unfiltered one, instead of disabling them while whitespace is
      hidden
    - [ ] Also ignore whitespace inserted where there was none (indenting from column 0, which
      git's `-b` still shows): ignore-all-whitespace or a leading/trailing line filter
    - [ ] An ignore-whitespace choice on the Editor preferences page, and a command or shortcut
      for the toggle
    - [ ] Apply the filter to the commit details file list and the graph's CHANGES counts (hide
      files whose only changes are whitespace)
    - [ ] Ignore blank-line-only changes as a separate option
  - [ ] Word wrap toggle (header and editor preference); needs rows of varying height
  - [x] Editor preferences page: font size, tab size (tab stops, was a fixed 4 spaces), line
    numbers on/off, syntax highlighting (moved from UI Customization) and word-level highlights
    - [ ] Monospace font family choice
    - [ ] End-of-line character (LF/CRLF)
    - [ ] Word wrap preference (with the header toggle)
    - [ ] Use the editor font size, tab size and line numbers in the conflict view
  - [x] Whole-file view (all lines, changes marked in place), keeping hunk actions on change blocks
    (Hunks | Whole file toggle in the diff header, saved in settings; works with Inline and
    Split, and in the File history view)
    - [ ] Line selection and single-line stage/unstage/discard in the whole-file view (map
      whole-file line indexes back to the normal hunk and line)
    - [x] Collapse long unchanged runs into expandable "N unchanged lines" rows
      - [x] Collapse an opened run again (a fold-back control on its first line), and Expand all
        / Collapse all in the diff header
      - [ ] Open part of a fold (N more lines above or below) instead of all of it at once
      - [ ] Keep folds open when a reload shifts their lines (follow them through the edit)
        instead of closing them again
      - [ ] Editor preferences for the context kept around changes and the minimum fold size,
        or turning folding off
      - [ ] Keyboard access to fold rows (open the fold at or after the top row)
      - [ ] Fill in the unchanged lines between hunks in the hunk view
    - [ ] Hunks / Whole file and Inline / Split toggles in the File history header, and a command
      or shortcut for the view mode toggle
    - [x] Syntax colors for whole files over the highlight size limit (see the background
      highlighting item above)
  - [ ] Diff view vs file view toggle (plain file, or a rendered preview for Markdown/images)
  - [x] Revert a hunk of a commit diff into the working tree (Revert Hunk on commit, compare and
    File history diffs, Hunks and Whole-file views; off while whitespace is hidden)
    - [ ] Revert selected lines of a commit diff (line selection on commit diffs, then the
      partial patch with only those lines, reversed)
    - [ ] Revert single lines in the whole-file view (needs whole-file line selection first)
    - [ ] Follow renames when reverting from File history: map a commit's older path to the
      file's current name instead of refusing with "not in the working tree"
    - [ ] Remove the file, rather than leaving it empty, when reverting the hunk of a commit
      that added it
    - [ ] Fall back to a three-way merge (conflict markers) when the working file has moved on,
      instead of refusing
    - [ ] Record a hunk revert in Trellis's undo history
    - [ ] Revert a whole file of a commit into the working tree (file-list context menu), and a
      keyboard shortcut for Revert Hunk
  - [x] Next/previous change continues to the next/previous changed file at either end
    - [ ] Scroll the Unstaged/Staged, commit or comparison file list so the file opened by
      next/previous change is in view
    - [ ] Next/previous change across entries in the File history view (step into the next or
      previous commit's diff of the file)
    - [ ] An optional wrap-around from the last changed file back to the first (with a notice)
    - [ ] Keep a step from being superseded when a status refresh reloads the old file while
      the next one is loading
    - [ ] Show which file of how many (e.g. 3 of 12) in the diff header
  - [x] Cache the computed diff rows instead of rebuilding them every frame
    - [ ] Cache the change rows behind the Previous/Next buttons too (their state rebuilds the
      change rows twice per frame)
- [x] **Line-level staging**
  Select lines in a hunk to stage, unstage or discard (click, Shift-click for a range,
  Ctrl/Cmd-click to toggle; Esc clears). Working-tree diffs only, one hunk at a time.
  - [ ] Per-line hover buttons (stage / unstage / discard this line)
  - [ ] Drag to select a range of lines
  - [ ] Selections spanning several hunks
  - [x] Keyboard shortcuts: S/U stage or unstage the selected lines (see P1 Keyboard
    shortcuts); reverting lines of a commit diff is under the diff viewer
  - [ ] Keep the line selection across tab switches
  - [ ] Remove the index entry, rather than leaving an empty file, when every line of an added
    or deleted file is unstaged or staged line by line
- [x] **Merge conflict tool**
  Three-pane view (ours / theirs / output), take a whole side per conflict, save and mark
  resolved. Opens from a conflicted file in the WIP panel or "View conflicts" in the banner;
  binary, deleted and special files take a whole version.
  - [ ] Take or untake individual lines from either side of a conflict
  - [ ] Auto-resolve trivial conflicts (whitespace-only, or one side equal to the base), with a
    note that they were resolved automatically
  - [ ] Edit the output pane directly before saving
  - [ ] Resolve in an external merge tool (needs P1 External tools: a configured merge tool
    command, launched with base/ours/theirs/output files)
  - [ ] Save the output while keeping conflict markers for chunks not picked yet
  - [ ] Mark several selected files resolved, Mark all resolved, and Mark conflicted
    (un-resolve, e.g. `git checkout -m <path>`)
  - [ ] Split the WIP file list into Conflicted Files (N) and Resolved Files (N), with
    per-file conflict counts
  - [ ] Step to the next conflicted file after saving one
  - [ ] Undo and redo of a saved resolution through the undo history
  - [ ] Collapse long unchanged runs in the panes; a resizable split between panes and output
  - [ ] Syntax highlighting in the conflict panes
  - [ ] Warn before Mark resolved when the working file still contains conflict markers
- [x] **File history and blame**
  File history: commits that changed a file, newest first, following renames, opened from a
  file's context menu in the commit details or WIP panel; each shows that commit's diff of the
  file and links to the graph. Searches back in batches of the commit limit.
  - [ ] Blame gutter with author, date and commit, opened from file history or the file context
    menu
  - [x] Open file history or blame for any file from the command palette (a file finder), not
    only from a changed-file list. History done; blame waits for the blame gutter
  - [ ] Start from HEAD when the file was renamed after the chosen commit (track renames
    forwards)
  - [ ] Refresh an open file history when HEAD moves (commit, checkout, pull, fetch)
  - [ ] Show the whole file at the selected commit (file view) as an alternative to its diff
  - [ ] History of a folder
  - [ ] Let `diff::show` delegate to `diff::body` once the diff header changes in flight have
    landed
- [x] **Interactive rebase**
  "Interactive rebase from here…" on a commit (that commit and every later one on the current
  branch) and "Interactive rebase {current} onto {branch}…" on a local branch open a panel in
  place of the graph. It lists the commits oldest first; each row picks, rewords, squashes,
  fixes up, drops or edits its commit, and rows move by dragging or with the up/down buttons.
  Rewording opens an inline message editor. Reset restores the list (or reloads it once HEAD
  has moved), Cancel or Esc closes it, and Start explains on hover why it is unavailable
  (another job, uncommitted changes, an operation in progress, a stale list, a squash with
  nothing before it, an empty message, nothing changed). Dropping asks first. Start runs
  `git rebase -i` with `GIT_SEQUENCE_EDITOR` set to Trellis itself (`--write-todo`), which
  installs the prepared todo after checking it lists the same commits as git's; rewords are
  `exec git commit --amend -F` lines. Edit steps and conflicts stop in the "Interactive rebase
  in progress" banner with Continue / Skip commit / Abort rebase. The panel is kept per tab.
  - [ ] Rebase onto a commit, and branch context menus on the graph's ref labels
  - [ ] History editing from the graph: reword, squash with parent, or drop commits from the
    context menu (several at once needs multi-select)
  - [ ] Start with uncommitted changes by stashing them first (`--autostash`), after asking
  - [ ] Rebase ranges that contain merge commits (`--rebase-merges`)
  - [ ] Keep `update-ref` steps (`rebase.updateRefs`) so branches stacked inside the range move
    with it
  - [ ] Edit the combined message of a squash group in the panel instead of git's concatenation
  - [ ] Keyboard control in the panel: select a row, Alt+Up/Down to move it, a letter key per
    action
  - [ ] Edit the remaining steps of a rebase already in progress (`rebase --edit-todo`),
    including one started outside Trellis
  - [ ] Show the step (N of M) and the stopped commit in the rebase banner, and report edit or
    conflict stops after Continue too (Continue currently says "succeeded" even when it stops
    again)
  - [ ] Undo an interactive rebase (back to `ORIG_HEAD`) from the undo history
  - [ ] `exec` and `break` steps in the panel
- [x] **Multi-select commits**
  Ctrl-click toggles a commit and Shift-click selects a range of graph rows; Esc returns to one
  commit. The details panel lists the selection; with exactly two commits it lists the files that
  differ between them and opens their diff. Cherry-pick the selection (oldest first) and copy
  their SHAs from the panel or the context menu.
  - [ ] Compare a selected commit with the working directory (including Ctrl-clicking the WIP
    row into a selection)
  - [ ] Combined diff of a contiguous range of three or more commits (the oldest's parent to the
    newest), explaining when the selection isn't contiguous
  - [ ] Extend the selection from the keyboard: Shift+Up/Down and Ctrl+A
  - [ ] Swap the compare direction (newer to older) in the two-commit view
  - [ ] Revert several commits; squash, drop, move or reword several (interactive rebase);
    create a patch from several commits
  - [ ] When a multi-commit cherry-pick stops on a conflict, select the WIP row, and add Skip
    commit (`cherry-pick --skip`) to the cherry-pick banner
  - [ ] Cherry-pick merge commits (choose the mainline parent with `-m`)
  - [ ] Undo and redo for cherry-picks (one commit or several)
  - [ ] Highlight the sidebar branches whose tips are in a multi-selection
- [x] **Live working-tree updates**
  The active tab's repository is watched with the `notify` crate (Preferences > General, on by
  default): outside edits re-read the status and ref or HEAD changes reload everything, within
  about 0.3 s. Window focus and F5 still refresh as before.
  - [ ] Notice a watcher that started but receives no events (a probe after starting) and warn
    once, suggesting a restart
  - [ ] Watch the common git dir's refs for linked worktrees (their refs live outside the
    worktree's own git dir)
  - [ ] Watch inactive tabs so their state updates in the background
  - [ ] Treat `.git/config` changes (remotes added or removed outside Trellis) as a full refresh
  - [ ] Status-bar indicator showing whether live updates are active for the repository
  - [ ] Reflect submodule HEAD changes live in the parent's status (paths under a nested `.git`
    are ignored for now)
- [x] **Credential prompts**
  Trellis is git's and ssh's askpass helper: HTTPS sign-in (username and password or token in
  one dialog), SSH key passphrases, SSH passwords and host-key confirmations open an in-app
  prompt, with retry and rejected-credentials notices, cancel and a 5-minute timeout.
  Background jobs and hosting-service sign-in never prompt.
  - [ ] GPG passphrases for commit signing (a Trellis pinentry, or loopback pinentry)
  - [ ] Remember entered HTTPS credentials through `git credential approve` / the OS
    credential store, and reject them when they fail
  - [ ] Caps Lock warning on password fields (egui doesn't expose Caps Lock state yet)
  - [ ] Choose between a password, personal access token or app password per hosting
    service, with a link to create a token
  - [ ] A non-blocking notice for SSH security-key touch requests (`SSH_ASKPASS_PROMPT=none`)
  - [ ] OpenSSH older than 8.4 on Linux/macOS (no `SSH_ASKPASS_REQUIRE`; needs `DISPLAY`)
  - [ ] Auto-fetch that needs credentials: a "sign in needed" notice instead of failing quietly
  - [ ] Hosting-service sign-in without Git Credential Manager through the prompt
    (Preferences > Integrations)
- [x] **Branch operations**
  From the local branch menu: rename, set and unset the upstream (also to a branch not on the
  remote yet), fast-forward without checkout, delete local + remote in one step when the
  checked-out branch contains it (undo restores the local branch), push any branch to its
  upstream, push to a chosen remote.
  - [ ] Pin a branch to the left (graph lane order, per-repo persistence, jump to the pinned branch)
  - [ ] Force push (with lease) of a non-HEAD branch, and push to all remotes
  - [ ] Rename the remote branch too when renaming (push the new name, delete the old one,
    move the upstream)
  - [ ] Choose a local branch as the upstream in the Set upstream dialog
  - [ ] The same operations from the graph's BRANCH/TAG labels and the remote branch menu
    (e.g. fast-forward a local branch from its remote row)
  - [x] Rename the checked-out branch from the command palette
    - [ ] A shortcut, and any branch through a two-step palette entry
  - [ ] Delete local + remote when the branch isn't merged into HEAD (force), in one step
- [x] **Hide / solo branches in the graph**
  Hide or show local and remote branches and solo one or more of them from the left panel's
  branch menus. Hidden rows stay listed, dimmed, with a marker that shows them again; soloed rows
  get a marker that stops soloing; a banner under the filter box says what is filtered, with
  Stop soloing and Show all. Saved per repository; refs that no longer exist are dropped.
  - [ ] Hide / solo from the graph's BRANCH/TAG ref-label menus
  - [ ] Hide tags (and stashes) in the graph, with hidden markers on TAGS rows
  - [ ] Hide, solo or stop soloing several selected branches at once (multi-select in the left
    panel)
  - [ ] Hide all / show all per section (LOCAL, REMOTE, TAGS) from the section header menu
  - [ ] Hover toggles (hide / solo) on rows that are not hidden or soloed
  - [ ] A graph-header button listing hidden branches and tags, to unhide them from there
  - [ ] Keep the hidden/solo state when a branch is renamed (rename the entry instead of
    dropping it)
- [x] **Commit panel extras**
  - [x] Commit template (`commit.template` prefills an empty draft; the template's comment lines are left out)
  - [x] "Push after committing" checkbox
  - [x] Skip hooks (`--no-verify`)
  - [x] Stage-all-and-commit shortcut (Ctrl+Shift+Enter; Ctrl+Enter commits)
  - [ ] Repo-specific Commit preferences page: edit and save the template, skip hooks by default, push after commit by default
- [ ] **File actions**
  - [x] Restore a file from a commit (commit details and file history menus; a deleted file
    comes back from the parent)
  - [x] Open a file in the default program and show it in Explorer / Finder (WIP and commit
    file rows)
  - [ ] Open in an external editor (needs P1 External tools)
  - [x] Create, delete and rename a file (WIP panel: New file… in the Unstaged Files header,
    Rename… and Delete… in working-tree file rows' menus)
  - [ ] Create, rename and delete from the command palette
  - [x] New file in this folder… on file rows (WIP and commit file menus), prefilled with the folder
  - [ ] New file in this folder… on folder rows (folder rows wait for a tree view in file lists)
  - [ ] Delete several selected files at once (needs multi-select in file lists)
  - [x] Move deleted files to the Recycle Bin / Trash (Delete… and Unstage and delete…; asks
    before deleting permanently when the trash can't take a file; a General preference turns it off)
    - [x] Move untracked files that Discard deletes to the Recycle Bin / Trash too
    - [ ] Show a button in the toast that opens the Recycle Bin / Trash or restores the file
    - [ ] Respect a drive's "remove files immediately" Recycle Bin setting on Windows
    - [ ] On Linux, offer the home trash when a mount has no usable trash folder, instead of
      asking to delete permanently
    - [ ] Send several selected files to the Recycle Bin / Trash at once (needs multi-select in
      file lists)
  - [ ] Make Delete undoable with Ctrl+Z (restore the working copy from the Recycle Bin / Trash,
    or keep a copy as Discard does)
  - [x] Rename tracked files with git mv so the rename is staged (option in the rename dialog, remembered)
  - [x] Unstage and delete a staged new file in one step (Delete… on a file staged as new asks
    once and also removes it from the index)
    - [x] The same for a staged rename (the old path's staged deletion would be left behind)
    - [x] Finish removing a staged new file whose working copy is already gone (Delete is
      disabled there)
    - [x] Label the item "Unstage and delete…" on rows whose path is staged as new
    - [ ] Name a staged rename's source path in its confirmation and toast
    - [ ] Keep the staged version recoverable so Unstage and delete can be undone
  - [ ] Open the newly created file in the editor or default app after creating it
  - [ ] Create a file from a template, or with initial contents
  - [ ] Rename or delete folders and submodules
  - [ ] Explain when a created path is inside a nested repository or submodule, so the parent's
    status won't list it
  - [ ] Edit a file in place
  - [ ] Restore several selected files from a commit at once (needs multi-select in file lists)
  - [x] Restore from comparison rows (two commits selected): the newer commit's version, or the
    older one's for a file the newer commit doesn't have; open and show there too
    - [ ] Choose which side to restore for a file the newer commit modified
    - [ ] Re-check which comparison and commit-detail paths are on disk after a status refresh
      (Open / Show can go stale until the commits are reselected)
  - [x] Make Restore undoable (keep the overwritten working copy, as Discard does)
  - [ ] Honour smudge filters (Git LFS) when restoring, by writing through `git restore --source`
  - [ ] Explain when a restored or opened path is outside a sparse checkout
  - [x] Open with default app and show in folder from the file history view and the diff header
  - [x] Restore button in the file history's selected-commit header and the commit diff header
    - [ ] Restore, Open and Show buttons in the diff header for two-commit comparisons
      (restoring the newer commit's version, as the comparison rows do)
    - [ ] Open and Show for the file history's current file in its top header, independent of
      the selected commit
    - [ ] Re-check whether the file history's file is on disk after status refreshes and file
      watcher events (an untracked file deleted outside Trellis keeps Open enabled)
    - [ ] Open the commit's version of a file (a temporary copy) when it is no longer in the
      working tree
    - [ ] Name the file in the show-in-folder tooltip and the open failure message
    - [ ] Painted icons for Open and Show in the diff header to save room (split widgets.rs first)
- [ ] **Repository trust**
  A repository's `.git/config` (`core.sshCommand`, `credential.helper`, `core.fsmonitor`) and its
  hooks can run arbitrary commands as soon as git touches it. Before Trellis runs git in a
  repository it didn't clone or create (auto-fetch, hooks), ask whether to trust it.
  - [x] Ask on first open of an untrusted repository, naming the config keys and hooks that
    would run commands; repositories Trellis cloned or initialised are trusted automatically
  - [ ] The decision is already remembered per repository; what remains is a Preferences list to
    review and revoke it
  - [x] Hold background fetch (and anything else that runs git on its own) until the user decides
  - [ ] While untrusted, browse read-only through libgit2 and say in the toolbar why fetch,
    pull and commit hooks are paused (the toolbar's Untrusted chip already says what is paused
    and why; manual pull, push and commit hooks still run)
  - [x] A way to bring the prompt back for the open repository (toolbar chip or palette
    command) after Don't trust or Esc
  - [ ] Trust worktrees Trellis creates from a trusted repository (and optionally submodules of
    one); ask again when a trusted repository gains new command-running settings or hooks
  - [ ] Hold shells the built-in terminal starts in an untrusted repository (prompts that run git)

## P1: power features

- [ ] **Command palette (fuzzy finder)**
  Exists: Ctrl+P / Ctrl+Shift+P opens a fuzzy-searched list (hidden keywords match synonyms):
  fetch all, pull, push, stash and pop, stage/unstage all, undo/redo, create branch, tag or
  annotated tag (at HEAD or the selected commit), rename the checked-out branch, search commits,
  refresh, open in terminal and in the file manager, diff layout and whitespace toggles, open a
  repository or a recent one, new and reopened tabs, close tab, Preferences, toolbar labels,
  theme, and checkout of local branches, remote branches (creating a tracking branch) and tags
  (detached), a file finder for file history, apply, pop or delete a chosen stash, and stash
  with a message. Entries whose command also has a global key
  show it dimmed at the right (Ctrl+F, Ctrl+W, F5, Ctrl+Z/Y, Ctrl+T, Ctrl+Shift+T and Ctrl+,).
  The list shows at most 200 rows; typing more narrows the rest. A Commands button in the
  toolbar opens the palette. The last 10 plain commands run from it (per session) are listed
  first for an empty query, and break ties between equally good matches for a typed one.
  - [x] File finder: pick any tracked file to open its history (blame waits for the blame gutter)
  - [x] Create tag and annotated tag
    - [x] Check tag names live in the New tag dialog (ref-format rules, an existing tag)
    - [x] Push the new tag to a remote right after creating it
    - [ ] Push an existing tag to a remote from the sidebar TAGS menu and the graph's tag labels
    - [ ] Remember the last remote picked for tag pushes per repository
    - [ ] Warn (without blocking) when a new tag name matches a branch name
    - [ ] Replace (force-move) an existing tag from the New tag dialog
    - [ ] Catch existing tags that point at non-commit objects before running git
    - [ ] A Preferences control for pushing new tags by default
  - [x] Shortcut hints next to entries (Ctrl+F, Ctrl+, Ctrl+W)
    - [x] A Refresh entry carrying the F5 hint
    - [ ] Cmd-style labels on macOS for the hints and tooltips
  - [ ] Two-step entries: pick a command, then a sub-list with its own prompt (done for files
    and stashes; branches, themes and repositories remain)
  - [x] Check out remote branches (creating a tracking branch) and tags
    - [ ] Check out a remote branch whose name is taken locally under a new local name
    - [ ] Offer to fast-forward the checked-out branch when it is behind the picked remote branch
    - [ ] Name the tag in the log after a tag checkout, and warn once that commits made while
      detached can be lost
    - [ ] Group ref entries by remote, or move them into a two-step checkout sub-list
    - [ ] Virtualised command rows so the 200-row cap can be lifted
  - [x] A toolbar button that opens the palette, with the shortcut in its tooltip
    - [ ] Show the Commands button when no repository is open (the palette already works there)
    - [ ] Accessible labels for every toolbar button, once tests no longer look up palette rows
      by bare labels such as Refresh or Push
  - [x] Rank recently run commands first
    - [ ] Remember recently run commands across restarts (a short key list in settings)
    - [ ] A dim "recently used" marker or divider for the recent block in the list
    - [ ] Remember recent commands per repository instead of per session
    - [ ] Rank recently used ref commands (Fetch <remote>, Finish <flow branch>, checkouts)
    - [ ] Rank recently picked items first in the file and stash sub-lists
    - [ ] An entry to clear the recent-commands list
  - [x] More commands: stash and pop, rename branch, undo and redo, new tab and reopen closed
    tab, diff layout and whitespace toggles, toolbar labels, open in terminal and file manager
    - [ ] Syntax highlighting on/off (re-highlighting the open diff when flipped)
    - [ ] Show the working directory changes (select the WIP row)
    - [ ] Diff mode (whole file / hunks) toggle
    - [x] Apply, pop or drop a chosen stash through a stash sub-list, and stash with a message
      - [ ] Confirm before applying or popping a stash on top of uncommitted changes, in the
        palette and the sidebar alike
      - [ ] Share one stash-action message builder between the sidebar and the palette
      - [ ] Refresh an open stash sub-list's rows when the stash stack changes while it is
        shown (choosing a stash already finds it again by its commit)
      - [ ] Apply or pop with `--index` to restore the staged state
  - [ ] Create, delete and edit files
  - [ ] PageUp/PageDown/Home/End in the list, and the selection following the mouse
- [x] **Keyboard shortcuts**
  Exist: tabs, undo/redo, F5, Ctrl+F, Ctrl+, , diff navigation, commit from the message box
  (Ctrl+Enter, Ctrl+Shift+Enter), S/U to stage or unstage the open file or its selected lines,
  Ctrl+Shift+S/U to stage or unstage all, Ctrl+Shift+M for the commit message, and a Ctrl+/
  shortcut sheet (filterable; also in the palette). One binding table (keymap.rs) feeds the
  sheet, palette hints and tooltips.
  - [ ] Topological next/previous (the next or previous commit on the selected commit's own
    branch line) and first/last row in the graph
  - [ ] Customization: rebind keys in Preferences, saved in settings, matched through the table
  - [ ] Match the older keys (tabs, undo/redo, F5, Ctrl+F, Ctrl+, , F7/Alt+arrows, the palette's
    keys) through the table too, so it is checked rather than kept in step by hand
  - [ ] A visible way to the sheet (a toolbar, status bar or Help entry naming Ctrl+/)
  - [ ] S in the WIP file lists (keyboard focus in the lists, Up/Down between files), and moving
    on to the next unstaged file after S stages the open one
- [x] **Built-in terminal**
  A resizable Terminal panel under the central area, shown or hidden with the toolbar's Terminal
  button, the palette or Ctrl+` (right-clicking the button still opens an external terminal).
  Each repository tab gets its own shell (pwsh, Windows PowerShell or cmd on Windows; $SHELL
  elsewhere) on a pseudo-terminal, started in its working directory the first time the panel
  shows it; hidden tabs' shells keep running, closing a tab stops its shell. The screen is drawn
  with 16, 256 and true colours, bold, underline, inverse and a cursor; typing, named keys,
  Ctrl+letters and pastes (bracketed when asked for) go to the shell while it has focus. Kill,
  Restart, a starting message, the exit code and start failures are shown.
  - [ ] Several terminals per repository as tabs in the panel ("Terminal N", new and close
    buttons), and kill all with a confirmation that says how many will stop
  - [ ] Scrollback (history in the screen model, scrolled with the mouse wheel), with mouse
    selection and copying to the clipboard
  - [ ] Right-click menu in the terminal: copy, paste, select all, clear
  - [ ] In-App Terminal preferences page: default shell (auto, pwsh, Windows PowerShell, cmd,
    bash or a custom path with arguments), font size, cursor style, dim when unfocused
  - [ ] A setting to use the built-in terminal for Open in terminal (palette entry and the
    toolbar button's right-click item)
  - [ ] Let the focused terminal keep the keys the global shortcuts take today (Ctrl+W,
    Ctrl+T, Ctrl+Tab, Ctrl+1–9, Ctrl+P, Ctrl+F, F5; Ctrl+F and F5 also reach the shell), and
    send Alt+letter as an ESC prefix; coordinate with the keyboard shortcuts in ui/mod.rs
  - [ ] Remember whether the panel is open, and its height, across restarts
  - [ ] Mouse reporting (xterm mouse modes) and the window or tab title from the shell (OSC 0/2)
- [x] **Submodules**
  A SUBMODULES section at the bottom of the left panel (shown when the repository has any) lists
  the index's submodules with a painted marker and a word for each state: in sync, out of sync,
  changes, changes and out of sync, not initialized. The tooltip explains the state and shows
  the URL, the recorded commit and the checked-out commit. Double-click or Open in new tab opens
  an initialized submodule as a repository; Initialize / Update to recorded commit runs
  `git submodule update --init` for it as a job (asking first when it is out of sync, and naming
  any commits inside that no branch or tag contains, which the update would leave behind); Copy
  URL copies the .gitmodules URL. The left-panel filter also filters the section.
  - [ ] Add a submodule: URL, path and optional branch, through `git submodule add`, with checks
    for a path that already exists or is already in the index
  - [ ] Set a submodule's URL (`git submodule set-url`, then `git submodule sync`) and edit its
    branch
  - [ ] Remove a submodule (deinit, `git rm`, clean its `.git/modules` entry) behind a
    destructive confirmation
  - [ ] "Keep submodules up to date": a General preference with a per-repository override (use
    global / on / off) that runs `git submodule update --init --recursive` after checkout,
    pull, merge, rebase and reset
  - [ ] Initialize / update all submodules from the section header's menu, and offer to
    initialize them when a repository with uninitialized submodules is opened or cloned
  - [ ] Refresh submodule status on status-only reloads and watcher events, not just full loads
  - [ ] Nested (recursive) submodules shown as a tree under their parent
  - [ ] Tell apart a deleted submodule folder, a newly added one (staged, not committed) and a
    never-initialized one in the marker and tooltip
  - [ ] Resolve a relative .gitmodules URL against the parent's remote for Copy URL and the
    tooltip
  - [ ] Commit the current checkout as the new recorded commit (stage the gitlink) from the row
    menu, and discard a submodule's uncommitted changes
  - [ ] Show a gitlink change in the WIP and commit diffs as the commit range it moves over, and
    explain that a submodule's own changes are committed inside it
  - [ ] Fetch and pull inside a submodule; recurse-submodules options for fetch, pull and push;
    a clear message when checkout or reset fails because of submodule changes
  - [ ] Palette commands for opening, initializing and updating submodules
  - [ ] A context-menu entry to show or hide the SUBMODULES section
- [ ] **Worktrees**
  WORKTREES section: create from a branch or commit, open (in a new tab), lock/unlock, prune,
  remove (optionally deleting the branch); show worktree WIP nodes in the graph.
  - [x] WORKTREES section (shown when a linked worktree exists): folder name, branch or detached
    commit, locked and missing markers, a tooltip, the left-panel filter; click jumps to its HEAD,
    double-click or Open in new tab opens it, Open folder, Copy path. Create worktree from a local
    branch or a graph commit: folder (default `<repo>-worktrees/<branch>` next to the main
    worktree, or Browse…), optional new branch, open in a new tab; live checks (registered
    folder, folder inside a worktree, branch name, branch checked out elsewhere) and a disk check
    of the folder before `git worktree add` runs. Worktree changes made outside reload the tab.
  - [x] Remove a worktree from its row menu (`git worktree remove`, with a confirmation; offer
    `--force` when it has changes or is locked), optionally deleting its branch afterwards; report
    separately when the folder couldn't be fully deleted or the branch delete failed
  - [x] Lock / unlock a worktree with an optional reason (`git worktree lock --reason` / `unlock`)
  - [x] Prune worktree records whose folders are gone (`git worktree prune`), from the section
    header menu or a missing row
  - [ ] Show worktree WIP nodes in the graph: a WIP row per worktree with uncommitted changes, at
    its HEAD commit
  - [ ] Mark local branches checked out in another worktree in LOCAL (tooltip with the folder), and
    disable Checkout / Delete for them with the reason
  - [ ] Create a worktree from a remote branch or a pull request (creating the tracking branch)
  - [ ] Watch the common git dir (refs, packed-refs) when a linked worktree is the open tab, so ref
    changes made elsewhere reload it
  - [ ] Palette commands: open a worktree, Create worktree…, Remove / Lock / Prune worktree
  - [ ] Remove a worktree and also delete its branch on the remote
  - [ ] Offer to close the other tab showing a worktree, instead of only refusing Remove
  - [ ] Move a worktree (`git worktree move`)
  - [ ] An Open folder button in the "folder couldn't be fully deleted" report, and a retry of
    the folder delete
  - [ ] A context-menu entry to show or hide the WORKTREES section; optionally group worktree tabs
    next to their main repository's tab
- [ ] **Git LFS**
  Initialize, track patterns (file, extension, folder), fetch/pull/push/prune, view
  `.gitattributes`, and LFS-aware auto-fetch.
  - [x] Preferences > Git LFS (repo-specific): status, Initialize, tracked patterns with Untrack
    and a new-pattern field, Open .gitattributes, Fetch / Pull / Push / Prune / Checkout; palette
    'Initialize Git LFS' and 'Configure Git LFS…'
  - [x] Track from a file's context menu: that one file (`git lfs track --filename`), all files
    with its extension, or all files in its folder
    - [ ] Offer it from file history rows, the diff header's file buttons and the palette
    - [ ] Disable "Only this file" when a broader pattern already covers the file (attribute
      lookup in the background)
    - [ ] Refresh the tracked patterns on status-only (watcher) refreshes, so hand edits to
      .gitattributes show without a full reload
    - [ ] Warn when a folder pattern would also catch a nested .gitattributes in that folder
    - [ ] Untrack lines that `--filename` wrote with backslash-escaped glob characters
      (`git lfs untrack` leaves them): remove them directly
    - [ ] Offer to convert files already committed (`git lfs migrate`) after tracking their
      pattern
  - [ ] LFS-aware auto-fetch: fetch LFS objects for the checked-out commit after a background
    fetch, and say so on the auto-fetch preference
  - [ ] Honour smudge filters when restoring a file from a commit, by writing through
    `git restore --source` so LFS files aren't left as pointers (the same item as under File
    actions > Restore)
  - [ ] When initializing fails because a non-LFS hook already exists, offer to merge or
    overwrite it (`git lfs update --manual` / `--force`)
  - [ ] Warn in the main window, not only on the page, when a repository tracks files in LFS but
    git-lfs isn't installed
  - [ ] Initialize with LFS when creating or cloning a repository
  - [ ] File locking: lockable patterns (`track --lockable`), locking and unlocking files, and
    showing who holds locks
  - [ ] Read patterns from nested `.gitattributes` files and `.git/info/attributes`, and show
    which file each pattern comes from
  - [ ] Show LFS pointer files and LFS status in file lists and diffs (for example "stored in
    LFS" instead of the pointer text)
  - [ ] Check for a minimum git and git-lfs version and say clearly when either is too old
- [ ] **Git Flow**
  Initialize with prefixes; start/finish feature, release and hotfix; a Git Flow panel; and
  "create Git Flow branch from issue".
  - [x] Preferences > Git Flow (repo-specific), with plain git and the git-flow config keys:
    production and development branches (missing ones created from the remote or from
    production), feature/release/hotfix and version-tag prefixes with live checks, Initialize
    and Save; Start feature, release and hotfix (`checkout -b` from develop or production, with
    the next version suggested from the tags); palette 'Configure Git Flow…' and 'Start Git
    Flow feature/release/hotfix…'
  - [x] Finish feature, release and hotfix: one git job that merges back into develop (and
    production first for release and hotfix) with --no-ff, tags production with an annotated
    version tag and message, and deletes the branch unless Keep is ticked; a Finish dialog with
    the steps checked live (changes, operations in progress, other worktrees, tag name), palette
    'Finish Git Flow <kind> <name>…' entries and a Finish row on the Git Flow page; a stopped
    job names the failed step and what is still to do
    - [ ] Rebase on develop first, squash, and push after finishing
    - [ ] Continue a Finish that stopped at a merge conflict once the merge is committed
    - [ ] Skip the tag, sign it with a chosen key, delete the remote branch, force-delete
  - [ ] A Git Flow panel listing feature, release and hotfix branches by type with Start and
    Finish, a palette entry to open it, and a Git Flow group in the left panel
  - [ ] Create a feature, release or hotfix branch from an issue (issue menu and palette)
  - [ ] Branch-menu and graph-menu entries for flow branches (finish this feature, start a
    hotfix from here)
  - [ ] Confirm before deleting the configured production or development branch
  - [ ] Bugfix and support branch types on the page (their default prefixes are already
    written)
  - [ ] Fetch before Start, and publish or track flow branches on the remote
  - [ ] Read the newer per-branch config layout (gitflow.branch.<name>.type / parent) and
    gitflow.origin
- [ ] **Remote management**
  Add/edit/remove remotes (separate pull and push URLs), fetch one remote, view a remote in the
  browser, add a teammate's fork, fork on the service.
  - [x] Publish a repository with no remote to GitHub: Push (or Push ▾ > Publish to GitHub…)
    offers to create the repository (owner: you or an organization; private or public), adds it
    as `origin` and pushes the branch with upstream set
  - [x] Add, edit (name, fetch URL, separate push URL) and remove remotes, and fetch one remote,
    from the REMOTE section (header and remote row menus, palette)
  - [ ] Publish to GitLab, Bitbucket and Azure DevOps
  - [ ] Add an existing remote URL from the same dialog
  - [ ] View a remote on its hosting service in the browser (row menu, from the remote URL)
  - [ ] Add a teammate's fork as a remote (pick from the service's forks of this repository)
  - [ ] Fork the repository on the hosting service and add the fork as a remote
  - [ ] Undo removing or renaming a remote (re-add it with its URLs and fetch, or rename it back)
  - [ ] Rewrite the saved graph filter's hidden or soloed refs when a remote is renamed or removed
  - [ ] Edit remotes defined outside the repository's config, and every URL of a remote with
    several fetch or push URLs
  - [ ] Per-remote options: fetch refspecs, tag fetching, prune on fetch, the default branch
  - [ ] Offer Add remote… when fetch, pull or push finds no remote
- [ ] **Tag management**
  Annotate an existing tag, delete from one or all remotes, push to all remotes, hide tags.
  (Push a tag and delete it locally exist; tag signing is on the Commit Signing page.)
  - [x] Delete a tag from one or all remotes and push it to all remotes, from the TAGS row menu
  - [x] Annotate an existing lightweight tag (replace it with an annotated tag at the same commit)
  - [ ] After deleting a tag locally, offer to delete it from the remotes too
  - [ ] Hide tags in the graph (see Hide/solo)
  - [ ] Offer Annotate… from the graph's tag labels and from the command palette (two steps: pick a
    lightweight tag, then the message)
  - [ ] Edit the message of an already annotated tag (re-annotate with the old message prefilled)
  - [ ] Push the annotated tag over a remote's lightweight copy (force-push one tag, after a
    confirmation)
  - [ ] Honour tag.forceSignAnnotated too (git applies it only without -a), and say in the Annotate
    dialog when the tag will be signed
- [ ] **Stash improvements**
  Stash selected files only, a stash message field, rename a stash, hide stashes.
  - [x] Rename (edit the message of) an existing stash, from the sidebar and the palette
    - [ ] Edit the stash message inline from the commit details panel when a stash is selected,
      and show the stash list message there (the stash commit's own message keeps its old text)
    - [ ] Keep the original reflog dates of the stash entries above the renamed one
    - [ ] Move apply, pop and drop (`JobMsg::Stash`) into `StashMsg`, so all stash actions share
      one message enum
    - [ ] Share one stash-action message builder between the sidebar and the palette, Rename…
      included
    - [ ] Undo a stash rename (record the old message so Ctrl+Z restores it)
    - [ ] Rank recently picked stashes first in the rename sub-list
  - [ ] Stash only staged changes or selected files, with a message, from the palette too
- [ ] **Clone and init options**
  Shallow and sparse clone; init with a `.gitignore` template and a license; create the hosting-service repo and push on init; clone from a list of
  your repos on a connected service.
  - [x] Shallow clone from the Clone dialog: by commit count or since a date, optionally one branch
  - [ ] Sparse clone (choose folders to check out when cloning)
  - [ ] Init with a .gitignore template and a license
  - [ ] Create the hosting-service repository and push on init
  - [ ] Clone from a list of your repositories on a connected hosting service
  - [x] Fetch the full history of a shallow clone later (unshallow, or deepen by more commits)
    - [ ] Deepen by date (`--shallow-since`) or exclude what a ref reaches (`--shallow-exclude`)
    - [ ] Widen a single-branch shallow clone to fetch every branch
    - [ ] Mark the shallow boundary commits in the graph
    - [ ] Unshallow shallow submodules when an update or reset needs missing commits
    - [ ] Confirm before unshallowing a very large repository, and cancel a running fetch
- [x] **Sparse checkout**
  A repository page in Preferences (Sparse Checkout, also opened by the palette's Configure
  sparse checkout…) shows whether it is on, in folders (cone) or patterns mode, and how many
  tracked files are checked out. Turn on / Save paths runs `git sparse-checkout set` with the
  list typed one path per line (folders are checked and cleaned up first; a sparse index is never
  created); Reapply and Turn off run `reapply` and `disable`. Each is a job that reloads the tab,
  and changed files git keeps outside the new paths are named in the toast. The libgit2 status
  no longer lists the files sparse checkout leaves out as deleted.
  - [ ] Sparse clone from the Clone dialog (see the sub-item under Clone and init options)
  - [ ] Show in the toolbar or status bar that the worktree is sparse, and notice sparse changes
    made outside Trellis on a plain refresh
  - [ ] Explain when an opened or restored path is outside the sparse checkout (see File
    actions), and keep left-out files out of the palette's file finder
  - [ ] Add or remove a folder from the paths from file rows and the commit file tree
  - [ ] Warn when a folders-mode path names a file, or a folder that doesn't exist at HEAD
  - [ ] Refill the form from the kept list after Turn off, so turning it on again starts there
  - [ ] Convert the list between folders and patterns when switching the mode
  - [ ] Sparse index support (needs libgit2 to read index.sparse directory entries)
  - [ ] Palette entries that reapply or turn off sparse checkout directly
- [x] **Repository management**
  Favorites section above Recent on the welcome screen and in the repository menu; remove one
  recent entry or clear the list; missing folders marked.
  - [ ] Scan project folders for repositories (choose one or more parent folders, list the Git
    repositories found under them, filter by name)
  - [ ] Keyboard shortcuts to open favorites (e.g. the first nine), listed in the shortcut sheet
    and the palette
  - [ ] Reorder favorites by dragging
  - [ ] Fetch or pull across many repositories (all favorites or all listed), with
    per-repository results and a "nothing to fetch" case
  - [ ] Mark repositories already open in a tab in the recent and favorite lists, and open all
    of them in new tabs at once
  - [ ] Remove all missing entries in one go, and offer to remove an entry when opening it fails
    because its folder is gone
  - [ ] Palette commands to favorite or unfavorite the active repository, with favorites listed
    first among the palette's repositories
- [ ] **Graph columns and loading**
  Column toggles, compact layout and row highlighting are done (UI Customization). Missing:
  - [x] Resize columns: drag the header dividers (BRANCH/TAG, SHA, changes, author, date), double-click to reset, saved in settings
  - [ ] Lazy-load more history when scrolling past the commit limit (the General page's "lazy
    loading"); jumping to a pinned branch goes with pinning in Branch operations
- [ ] **Search and filter**
  Filter the graph to matches (not just dim them), filter by author, search by file path,
  background indexing for large repos.
- [x] **Signing beyond config**
  Done on the Commit Signing page: generate a GPG key, copy the public key, gpg/ssh program
  path, SSH allowed-signers, sign tags.
  - [x] Verified/unverified signature badge on commits (the selected commit's details panel)
    - [ ] Signature badges on graph rows (batch-check visible signed commits, cached by oid, throttled like Stats)
    - [ ] Signature status in multi-commit views (details_multi, compare) and the file history list
    - [ ] Flag a verified signature whose signer identity doesn't match the committer's email
    - [ ] Verify signed tags in the TAGS section and tag tooltips
    - [ ] Show signatures on merged tags (mergetag headers) of merge commits
    - [ ] Show the SSH key type and GPG primary user ID / primary-key fingerprint in the badge tooltip
    - [ ] Warn on the Commit Signing page when gpg.ssh.allowedSignersFile points to a missing file or a repository-local value overrides the global one
- [ ] **SSH key management**
  Generate a key, choose the key pair, use the local agent, copy the public key (and add it to
  a connected service).
  - [x] Preferences > SSH: list the key pairs in ~/.ssh with type, comment and fingerprint, copy a public key, generate an Ed25519 key (asking before replacing one)
  - [ ] Choose the key pair git uses for SSH remotes (core.sshCommand with `-i <key> -o IdentitiesOnly=yes`, globally or per repository), show the current choice and stop using it
  - [ ] Use local SSH agent: detect a running agent (ssh-add -l, the Windows OpenSSH agent service or a Pageant-style agent), add the selected key to it, and leave core.sshCommand unset while it is on
  - [ ] Add the public key to a connected hosting service through its API (optional key title, check whether it is already registered, fallback: copy the key and open the service's SSH key settings page)
  - [ ] Generate RSA or ECDSA keys, and change an existing key's passphrase or comment (ssh-keygen -p / -c)
  - [ ] Browse buttons for picking a private key file and the save location
  - [ ] Show in folder and delete a key locally from its row
  - [ ] Flag encrypted private keys and keys whose file permissions are too open for ssh on their row
  - [ ] Palette entry for the SSH page (palette_commands.rs needs splitting first)
  - [ ] Keys outside ~/.ssh (IdentityFile entries in ~/.ssh/config, agent-only keys)
- [ ] **Profiles**
  Several named profiles (identity, integrations, preferences) with quick switching.
- [ ] **Local patches**
  Export commits, files or the WIP to a `.patch` file; open and apply a patch.
- [ ] **Conflict prediction**
  Local-only version: dry-run merge the branch against its target/upstream (`git merge-tree`)
  and warn about overlapping files before you merge or open a PR.
- [ ] **External tools**
  Choose an editor, diff tool, merge tool and terminal (with custom commands); open the repo in
  the editor.
- [ ] **Notifications**
  Toasts for every operation exist. Missing: optional desktop notifications and "N commits
  behind" alerts after auto-fetch.

## P2: integrations depth

- [ ] **In-app pull request view**
  - Files changed and the conversation
  - Inline code comments: reply, resolve, show outdated
  - Approve, request changes, finish review
  - Merge with merge, squash or rebase
  - Delete the branch after merge
- [ ] **CI / build status**
  Check status on PRs and on branch tips (GitHub checks, GitLab pipelines, Azure Pipelines,
  Bitbucket builds).
- [ ] **"Push and start a PR"**
  One action to push a branch and open the create-PR dialog; start a PR from a remote branch.
- [ ] **PR and issue filters**
  Saved filters: mine, needs my review, assigned, mentioned, labels, milestones, sprints.
- [ ] **Create and view issues**
  Create an issue (type, labels, assignees, epic); an issue detail panel with description and
  comments; "create issue then branch".
- [ ] **Trello**
  Trello cards as an issue tracker.
- [ ] **Cross-repo dashboard**
  PRs and issues across all connected repos (local, with no cloud account).
- [ ] **Limits found while building integrations**
  - [ ] Paginate PR and issue lists. Trellis stops at 100 (50 on Bitbucket Cloud); `morluto/rea`
        already exceeds that.
  - [ ] Fetch review states for every PR. Only the first 25 are fetched today, to save API calls.
  - [ ] Live-test GitLab, Bitbucket Cloud and Data Center, Azure DevOps, and Jira. Only GitHub
        has been checked against a real account. This includes the Azure DevOps sign-in through
        Git Credential Manager (Microsoft Entra Bearer token).
  - [ ] Make the branch-name issue heuristic configurable. `2024-cleanup` is currently read as
        issue 2024.
  - [ ] Jira key detection treats strings like `UTF-8` as keys unless a project filter is set.

## P3: app polish and platform

- [ ] **Custom themes** (dark, light and follow-the-system exist)
- [ ] **Zoom** in/out/reset; show/hide the left, detail and toolbar panels
- [ ] **Menu bar** (File / Edit / View / Window / Help), including Undo/Redo under Edit
- [ ] **Language**: translate the UI (date locale and relative dates already exist)
- [ ] **Logs**: keep the activity log across sessions; error and performance logs; extended
      logging (the in-session activity log exists)
- [ ] **Command-line companion**: `trellis <path>` and `trellis .` work; consider
      history/diff/status subcommands
- [ ] **Deep links**: `trellis://` links to a repo, branch, commit or tag (with
      clone/locate when not found)
- [ ] **Packaging and updates**: the Release workflow publishes zip/tar.gz downloads. Missing:
      installers, an app icon, code signing, file association, auto-update and a release notes
      view
- [ ] **Git engine choice**: choose
      which git executable to use and show its version
- [ ] **Performance**: incremental status for very large working trees (loading already runs
      off the UI thread)
- [ ] **Input reliability**: the first click after the window gains focus was dropped while
      testing with synthetic input; check with real input
- [ ] **Onboarding**: first-run welcome and tutorial

## Out of scope

These need a vendor account or cloud service rather than git:

- Sharing patches and code suggestions through a cloud service (local `.patch` files are
  covered under P1)
- Cloud workspaces, teams and organizations
- Accounts, trials, licensing, surveys and analytics
- Status hooks for coding agents
- **Possible later:** AI features, such as generating commit messages and PR descriptions,
  explaining a commit or branch, and composing commits. These could be built on the Claude API
  as an opt-in.
