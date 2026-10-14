# Trellis roadmap

What Trellis does today and what is planned next, roughly in priority order. Each item is a
target to design, not a finished spec. Ticked items are done; open sub-items under a ticked
item are known follow-ups.

Want to help? Pick an unticked item, open an issue to discuss the approach, then see
[CONTRIBUTING.md](CONTRIBUTING.md).

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
| SSH | ✘ | Key generation and selection, use local agent (P1 SSH key management) |
| Integrations | ✔ 9 services | Trello; PR/issue depth (P2) |
| AI | ✘ | Possible later: opt-in features on the Claude API |
| External Tools | ✘ | Editor, diff tool, merge tool, terminal (P1 External tools) |
| Notifications | ◐ location under UI Customization | Desktop notifications (P1 Notifications) |
| UI Customization | ✔ UI Customization | Repo group colors and worktree tab grouping (wait on those features) |
| Commit Signing | ✔ Commit Signing | Signature badge on commits (P1) |
| Editor | ◐ Editor (font size, tab size, line numbers, syntax and word highlights) | Font family, word wrap, EOL character |
| In-App Terminal | ✘ | Needs the built-in terminal (P1) |
| Experimental | ✘ | Git executable selection and version (P3) |
| *Repo:* Encoding | ✘ | Per-repo file encoding for diffs |
| *Repo:* Gitflow | ✘ | P1 Git Flow |
| *Repo:* Git Hooks | ✔ hooks path | — |
| *Repo:* Commit | ✘ | Commit template, push after commit, skip hooks per repo |
| *Repo:* Agents | — | Out of scope |
| *Repo:* Conflict Prevention | ✘ | P1 Conflict prediction, with per-repo enable and target branch |
| *Repo:* LFS | ✘ | P1 Git LFS |
| *Repo:* Sparse Checkout | ✘ | P1 Sparse checkout |
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
    - [ ] Highlight from the whole old and new files instead of per-hunk fragments, so hunks
      that start inside a block comment or multi-line string color correctly (with the
      whole-file view)
    - [ ] More languages than syntect's defaults (TOML, TypeScript/TSX, PowerShell,
      Dockerfile, Kotlin, Swift) by bundling extra syntax definitions
    - [ ] Language overrides from `.gitattributes` (`linguist-language`) or a per-file picker
    - [ ] Color the function context after `@@` in hunk headers
    - [ ] A command or shortcut for the toggle (now on the Editor preferences page)
    - [ ] User-selectable syntax color themes beyond the dark and light palettes
  - [x] Word-level intraline highlights
    - [x] A preference to turn them off (Preferences > Editor)
    - [ ] Pair removed and added lines by similarity instead of by position
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
      - [ ] Collapse an opened run again (a fold-back control on its first line), and Expand all
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
  - [ ] Cache the computed diff rows instead of rebuilding them every frame
- [x] **Line-level staging**
  Select lines in a hunk to stage, unstage or discard (click, Shift-click for a range,
  Ctrl/Cmd-click to toggle; Esc clears). Working-tree diffs only, one hunk at a time.
  - [ ] Per-line hover buttons (stage / unstage / discard this line)
  - [ ] Drag to select a range of lines
  - [ ] Selections spanning several hunks
  - [ ] Keyboard shortcuts (see P1 Keyboard shortcuts); reverting lines of a commit diff is
    under the diff viewer
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
  - [ ] Rename a branch from a command palette or shortcut
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
  - [ ] Move deleted files to the OS trash or recycle bin, or make Delete undoable with Ctrl+Z
  - [x] Rename tracked files with git mv so the rename is staged (option in the rename dialog, remembered)
  - [ ] Unstage and delete a staged new file in one step
  - [ ] Open the newly created file in the editor or default app after creating it
  - [ ] Create a file from a template, or with initial contents
  - [ ] Rename or delete folders and submodules
  - [ ] Explain when a created path is inside a nested repository or submodule, so the parent's
    status won't list it
  - [ ] Edit a file in place
  - [ ] Restore several selected files from a commit at once (needs multi-select in file lists)
  - [ ] Restore from comparison rows (two commits selected): the newer commit's version
  - [ ] Make Restore undoable (keep the overwritten working copy, as Discard does)
  - [ ] Honour smudge filters (Git LFS) when restoring, by writing through `git restore --source`
  - [ ] Explain when a restored or opened path is outside a sparse checkout
  - [ ] Open with default app and show in folder from the file history view and the diff header
  - [ ] Restore button in the file history's selected-commit header and the commit diff header

## P1: power features

- [ ] **Command palette (fuzzy finder)**
  Exists: Ctrl+P / Ctrl+Shift+P opens a fuzzy-searched list (hidden keywords match synonyms):
  fetch all, pull, push, stage/unstage all, create branch, tag or annotated tag (at HEAD or the
  selected commit), search commits, open a repository or a recent one, close tab, Preferences,
  theme, and checkout of local branches, and a file finder for file history. Entries whose
  command also has a global key show it dimmed at the right (Ctrl+F, Ctrl+W, Ctrl+,).
  - [x] File finder: pick any tracked file to open its history (blame waits for the blame gutter)
  - [x] Create tag and annotated tag
    - [ ] Check tag names live in the New tag dialog (ref-format rules, an existing tag)
    - [ ] Push the new tag to a remote right after creating it
  - [x] Shortcut hints next to entries (Ctrl+F, Ctrl+, Ctrl+W)
    - [ ] A Refresh entry carrying the F5 hint
    - [ ] Cmd-style labels on macOS for the hints and tooltips
  - [ ] Two-step entries: pick a command, then a sub-list with its own prompt (branches,
    themes, repositories)
  - [ ] Check out remote branches (creating a tracking branch) and tags
  - [ ] A toolbar button that opens the palette, with the shortcut in its tooltip
  - [ ] Rank recently run commands first
  - [ ] More commands: stash and pop, rename branch, undo and redo, new tab and reopen closed
    tab, diff layout and whitespace toggles, toolbar labels, open in terminal and file manager
  - [ ] Create, delete and edit files
  - [ ] PageUp/PageDown/Home/End in the list, and the selection following the mouse
- [ ] **Keyboard shortcuts**
  Exist: tabs, undo/redo, F5, Ctrl+F, Ctrl+, and diff navigation. Missing: stage/unstage the
  file or selected lines (S/U), stage all, commit (Ctrl+Enter), focus the message, topological
  next/previous, a shortcut reference sheet, and customization.
- [ ] **Built-in terminal**
  Bottom terminal panel with tabs (`portable-pty` plus a terminal widget). Trellis currently only
  launches an external terminal.
- [ ] **Submodules**
  SUBMODULES section with status (in sync, dirty, uninitialized); add, init, update, open as a
  repo, set URL, remove; "keep submodules up to date".
- [ ] **Worktrees**
  WORKTREES section: create from a branch or commit, open (in a new tab), lock/unlock, prune,
  remove (optionally deleting the branch); show worktree WIP nodes in the graph.
- [ ] **Git LFS**
  Initialize, track patterns (file, extension, folder), fetch/pull/push/prune, view
  `.gitattributes`, and LFS-aware auto-fetch.
- [ ] **Git Flow**
  Initialize with prefixes; start/finish feature, release and hotfix; a Git Flow panel; and
  "create Git Flow branch from issue".
- [ ] **Remote management**
  Add/edit/remove remotes (separate pull and push URLs), fetch one remote, view a remote in the
  browser, add a teammate's fork, fork on the service.
  - [x] Publish a repository with no remote to GitHub: Push (or Push ▾ > Publish to GitHub…)
    offers to create the repository (owner: you or an organization; private or public), adds it
    as `origin` and pushes the branch with upstream set
  - [ ] Publish to GitLab, Bitbucket and Azure DevOps
  - [ ] Add an existing remote URL from the same dialog
- [ ] **Tag management**
  Annotate an existing tag, delete from one or all remotes, push to all remotes, hide tags.
  (Push a tag and delete it locally exist; tag signing is on the Commit Signing page.)
- [ ] **Stash improvements**
  Stash selected files only, a stash message field, rename a stash, hide stashes.
- [ ] **Clone and init options**
  Shallow and sparse clone; init with a `.gitignore` template and a license; create the hosting-service repo and push on init; clone from a list of
  your repos on a connected service.
- [ ] **Sparse checkout**
  Enable/disable, set/edit patterns, reapply.
- [ ] **Repository management**
  Scan project folders for repos, favorites with shortcuts, a recent list with removal, and
  fetch or pull across many repos.
- [ ] **Graph columns and loading**
  Column toggles, compact layout and row highlighting are done (UI Customization). Missing:
  - Resize columns (widths are fixed)
  - Lazy-load more history when scrolling past the commit limit (the General page's "lazy
    loading"); jumping to a pinned branch goes with pinning in Branch operations
- [ ] **Search and filter**
  Filter the graph to matches (not just dim them), filter by author, search by file path,
  background indexing for large repos.
- [x] **Signing beyond config**
  Done on the Commit Signing page: generate a GPG key, copy the public key, gpg/ssh program
  path, SSH allowed-signers, sign tags.
  - [ ] Verified/unverified signature badge on commits
- [ ] **SSH key management**
  Generate a key, choose the key pair, use the local agent, copy the public key (and add it to
  a connected service).
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
