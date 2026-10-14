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
  delete and force delete, and remote branch delete
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
| Editor | ✘ | Font, font size, tab size, line numbers, word wrap, EOL character; syntax highlighting is a toggle under UI Customization > Interface until this page exists |
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
    - [ ] Highlight in a second background pass, so diffs over the size limit (2,000 lines,
      100 KB, or a line over 1,000 bytes) show plain at once and gain colors later, with no
      plain flash on reload
    - [ ] Highlight from the whole old and new files instead of per-hunk fragments, so hunks
      that start inside a block comment or multi-line string color correctly (with the
      whole-file view)
    - [ ] More languages than syntect's defaults (TOML, TypeScript/TSX, PowerShell,
      Dockerfile, Kotlin, Swift) by bundling extra syntax definitions
    - [ ] Language overrides from `.gitattributes` (`linguist-language`) or a per-file picker
    - [ ] Color the function context after `@@` in hunk headers
    - [ ] A command or shortcut for the toggle (it moves with the Editor preferences page)
    - [ ] User-selectable syntax color themes beyond the dark and light palettes
  - [x] Word-level intraline highlights
    - [ ] A preference to turn them off (with the Editor preferences page)
    - [ ] Pair removed and added lines by similarity instead of by position
  - [x] Ignore leading/trailing whitespace (diff header toggle, saved in settings; hunk and line
    operations are off while it is on, since `apply_hunk` counts the unfiltered hunks)
    - [ ] Map hunk and line stage/unstage/discard from the filtered diff back to the unfiltered
      one, instead of disabling them while whitespace is hidden
    - [ ] Also ignore whitespace inserted where there was none (indenting from column 0, which
      git's `-b` still shows): ignore-all-whitespace or a leading/trailing line filter
    - [ ] An ignore-whitespace choice on the Editor preferences page, and a command or shortcut
      for the toggle
    - [ ] Apply the filter to the commit details file list and the graph's CHANGES counts (hide
      files whose only changes are whitespace)
    - [ ] Ignore blank-line-only changes as a separate option
  - [ ] Word wrap toggle (header and editor preference); needs rows of varying height
  - [ ] Editor preferences page: monospace font, font size, tab size (a tab is 4 spaces today),
    line numbers on/off, end-of-line character, and the syntax highlighting toggle (now under
    UI Customization)
  - [ ] Whole-file view (all lines, changes marked in place), keeping hunk actions on change blocks
  - [ ] Diff view vs file view toggle (plain file, or a rendered preview for Markdown/images)
  - [ ] Revert a hunk or selected lines of a commit diff into the working tree
  - [ ] Next/previous change continues to the next/previous changed file at either end
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
- [ ] **File history and blame**
  File history (commits that touched a path, following renames) with links to the graph;
  blame gutter with author, date and commit.
- [ ] **Interactive rebase**
  Drag-to-reorder list with pick, reword, squash, drop and edit, driven through
  `GIT_SEQUENCE_EDITOR` (Trellis as the editor: the same helper-process plumbing as credential
  prompts). "Edit" stops reuse the Continue/Skip/Abort banner.
  - [ ] Rebase onto a commit, and branch context menus on the graph's ref labels
  - [ ] History editing from the graph: reword, squash with parent, or drop commits from the
    context menu (several at once needs multi-select)
- [ ] **Multi-select commits**
  Ctrl/Shift-click in the graph; diff between two commits or between a commit and the WIP;
  cherry-pick several at once.
- [ ] **Live working-tree updates**
  Watch the working directory with the `notify` crate and refresh the WIP row instantly. Trellis
  currently refreshes only on window focus, F5, and after its own operations.
- [ ] **Credential prompts**
  Trellis runs git with `GIT_TERMINAL_PROMPT=0`, so HTTPS without Git Credential Manager, SSH
  keys with passphrases, and GPG passphrases all fail. Add an askpass helper (`GIT_ASKPASS` /
  `SSH_ASKPASS`) that shows an in-app dialog.
- [ ] **Branch operations**
  Rename, set upstream, fast-forward without checkout, delete local + remote, push a non-HEAD
  branch, push to a chosen remote, pin a branch to the left.
- [ ] **Hide / solo branches in the graph**
  Hide or show refs and solo one branch. The core already supports it (`LoadOptions.hidden` and
  `solo`); this needs the menus, a way to unhide, and per-repo persistence.
- [ ] **Commit panel extras**
  - Commit template
  - "Push after committing" checkbox
  - Skip hooks (`--no-verify`)
  - Stage-all-and-commit shortcut
- [ ] **File actions**
  Restore a file from a commit, open in an external editor or the default program, show in
  folder, create/delete/rename a file, and edit in place.

## P1: power features

- [ ] **Command palette (fuzzy finder)**
  Ctrl+P: checkout, create branch/tag, open repo, stage/unstage all, blame/history for a file,
  open settings, switch theme, search commits.
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
