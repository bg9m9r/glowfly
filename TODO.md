# Trellis roadmap

What Trellis does today and what is planned next, roughly in priority order. Each item is a
target to design, not a finished spec. A top-level item is the unit of work: it is ticked when
its core behaviour works and every sub-item is done or moved to [Later](#later). A substantial
new feature becomes its own top-level item; smaller ideas, polish and follow-ups go to Later as
plain bullets, and are built once the open items are done.

Want to help? Pick an unticked item, open an issue to discuss the approach, then see
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Recent changes

The last 20 completed TODO items, newest first: when (local time), which item, and what was
done. Add a line at the top when ticking an item, and drop the oldest past 20.

- 2026-10-16 07:15 · **Stash improvements**: Stash dialog with a message and a choice of all changes, only staged changes or chosen files, from the palette, the Staged Files header menu and Stash this file… on WIP rows (`25ab29a`)
- 2026-10-16 06:51 · **Remote management**: View a remote on its hosting service, add a teammate's fork from the service's list of forks, and fork a repository into your account (GitHub, GitLab) then add and fetch it, from the REMOTE row menu and the palette (`dc17a10`)
- 2026-10-16 03:57 · **Tag management**: Hide and solo tags in the graph from the TAGS row menu, with markers on hidden and soloed rows (`152cd40`)
- 2026-10-16 03:00 · **Git Flow**: Git Flow panel in place of the graph, a GIT FLOW section in the left panel grouping flow branches by kind, and Start Git Flow feature/release/hotfix from an issue in the issue menu and palette (`8f751ea`)
- 2026-10-16 00:41 · **Diff viewer upgrade**: Word wrap toggle in the diff header and Preferences > Editor, with variable-height rows (`bb11923`)
- 2026-10-16 00:37 · **Repository trust**: git jobs started in an untrusted repository (pull, push, commit and its hooks, undo, …) wait for the trust prompt, which names the operation and offers Trust and run or Cancel (`0c57fb6`)
- 2026-10-15 22:15 · **Notifications**: "N commits behind" toast with a Pull button when a background fetch leaves the checked-out branch further behind its upstream; toasts can carry an action button; Behind alerts setting under UI Customization (`90241d7`)
- 2026-10-15 19:55 · **Conflict prediction**: Dry-run merge of the checked-out branch against its upstream or default branch with git merge-tree after loads and fetches: toolbar warning with the conflicting files, Merge/Rebase from its popover, and on-demand checks from the branch menus (`62f321b`)
- 2026-10-15 19:44 · **External tools**: Preferences > External Tools: choose an editor (open a file from file rows, or the repository from the palette) with argument templates, and a custom terminal for Open in terminal (`e7ea38a`)
- 2026-10-15 19:30 · **Profiles**: Named identity profiles in Preferences > Profile: add (copying the current identity), rename, delete (asking first) and switch from the page or the palette, writing user.name/email/signingkey to the global config; the first is seeded from the global identity (`764d3c6`)
- 2026-10-15 17:54 · **Local patches**: Create patch… from one commit or a multi-selection in the graph menus (one mbox-style file, oldest first, binary-safe) and Apply patch… from the palette, checked first so nothing changes when it fails (`56ae4b4`)
- 2026-10-15 17:46 · **Search and filter**: Filter the graph to search matches (Filter toggle, Ctrl+Shift+F) with a live match count, and narrow by author with author: terms and an author picker (`b922cb6`)
- 2026-10-15 17:37 · **SSH key management**: SSH page in Preferences: list the key pairs in ~/.ssh with fingerprints, copy a public key, and generate an Ed25519 key with ssh-keygen, asking before replacing one (`cac1207`)
- 2026-10-15 16:54 · **Signing beyond config**: Signature badge for the selected commit in the details panel: verified, unverified, unknown key or bad signature, checked with git and cached per commit (`07ab360`)
- 2026-10-15 16:43 · **Repository management**: Favorite repositories above Recent on the welcome screen and in the repository menu, remove one recent entry or clear the list, and missing folders marked (`59fc992`)
- 2026-10-15 16:02 · **Sparse checkout**: Sparse Checkout page in Preferences: turn on with folders or patterns, save, reapply and turn off; palette entry; libgit2 status hides left-out files (`0969dcf`)
- 2026-10-15 15:59 · **Graph columns and loading**: resizable graph columns: drag the header dividers to resize, double-click to reset (`3bc171e`)
- 2026-10-15 14:14 · **Clone and init options**: Fetch full history or N more commits of a shallow clone from the palette and the graph header (`94affbb`)
- 2026-10-15 14:01 · **Stash improvements**: Rename (edit the message of) a stash from the STASHES row menu and the palette, keeping its commit and position (`a6ee8cb`)
- 2026-10-15 13:39 · **Tag management**: Annotate an existing lightweight tag from the TAGS row menu: one job replaces it with an annotated tag on the same commit, signed when tag signing is on (`082c49c`)

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
- Preferences: General, Profile (named identity profiles), Integrations, UI, Commit Signing, plus
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
| General | ✔ General | Missing: delete `.orig` files after merge, keep submodules updated, show all commits, path to `sh.exe`, extended logging, conflict detection |
| Profiles | ◐ Profile (named identity profiles, switching) | Per-profile integrations and preferences (P1 Profiles) |
| SSH | ◐ SSH page (list key pairs, copy a public key, generate an Ed25519 key) | Choose the key git uses, use local agent, add a key to a hosting service (P1 SSH key management) |
| Integrations | ✔ 9 services | Trello; PR/issue depth (P2) |
| AI | ✘ | Possible later: opt-in features on the Claude API |
| External Tools | ◐ Editor and terminal | Diff tool, merge tool (P1 External tools) |
| Notifications | ◐ location and behind alerts under UI Customization | Desktop notifications (P1 Notifications) |
| UI Customization | ✔ UI Customization | Repo group colors and worktree tab grouping (wait on those features) |
| Commit Signing | ✔ Commit Signing | Signature badges on graph rows and tags (P1 follow-ups) |
| Editor | ◐ Editor (font size, tab size, line numbers, syntax and word highlights, word wrap) | Font family, EOL character |
| In-App Terminal | ✘ | Shell, font, cursor style and dimming for the built-in terminal (P1 Built-in terminal) |
| Experimental | ✘ | Git executable selection and version (P3) |
| *Repo:* Encoding | ✘ | Per-repo file encoding for diffs |
| *Repo:* Gitflow | ✔ Git Flow page, Start and Finish, Git Flow panel, branch from issue | Finish options (Later > Git Flow) |
| *Repo:* Git Hooks | ✔ hooks path | — |
| *Repo:* Commit | ✘ | Commit template, push after commit, skip hooks per repo |
| *Repo:* Agents | — | Out of scope |
| *Repo:* Conflict Prevention | ✘ (on/off is under General) | Per-repo enable and target branch rules (P1 Conflict prediction) |
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
- [x] **Undo / Redo**
  Toolbar Undo/Redo and Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) for checkout, commit/amend, reset,
  branch delete and Discard all; per-tab, in-memory history of 20, refuses (and says why) when
  the repository no longer matches or local changes would be overwritten.
- [x] **Diff viewer upgrade**
  - [x] Split (side-by-side) vs inline view
  - [x] Next/previous change (F7 / Shift+F7, Alt+Down / Alt+Up)
  - [x] Syntax highlighting (`syntect`)
    - [x] Highlight in a second background pass, so diffs over the size limit (2,000 lines,
      100 KB, or a line over 1,000 bytes) show plain at once and gain colors later, with no
      plain flash on reload
    - [x] Highlight from the whole old and new files instead of per-hunk fragments, so hunks
      that start inside a block comment or multi-line string color correctly (with the
      whole-file view)
    - [x] More languages than syntect's defaults (TOML, TypeScript/TSX, PowerShell,
      Dockerfile, Kotlin, Swift) by bundling extra syntax definitions
      - [x] Highlight `.jsx` files with the bundled TSX grammar (the JavaScript (Babel) grammar
        is left out of the fancy-regex set)
    - [x] Language overrides from `.gitattributes` (`linguist-language=<name>`: GitHub language
      names and common aliases, any case; `Text` turns highlighting off; unknown names fall back
      to detection)
    - [x] Color the function context after `@@` in hunk headers
  - [x] Word-level intraline highlights
    - [x] A preference to turn them off (Preferences > Editor)
    - [x] Pair removed and added lines by similarity instead of by position
  - [x] Ignore leading/trailing whitespace (diff header toggle, saved in settings; hunk and line
    operations are off while it is on, since `apply_hunk` counts the unfiltered hunks)
  - [x] Word wrap toggle (header and editor preference); needs rows of varying height
  - [x] Editor preferences page: font size, tab size (tab stops, was a fixed 4 spaces), line
    numbers on/off, syntax highlighting (moved from UI Customization) and word-level highlights
  - [x] Whole-file view (all lines, changes marked in place), keeping hunk actions on change blocks
    (Hunks | Whole file toggle in the diff header, saved in settings; works with Inline and
    Split, and in the File history view)
    - [x] Collapse long unchanged runs into expandable "N unchanged lines" rows
      - [x] Collapse an opened run again (a fold-back control on its first line), and Expand all
        / Collapse all in the diff header
    - [x] Syntax colors for whole files over the highlight size limit (see the background
      highlighting item above)
  - [x] Revert a hunk of a commit diff into the working tree (Revert Hunk on commit, compare and
    File history diffs, Hunks and Whole-file views; off while whitespace is hidden)
  - [x] Next/previous change continues to the next/previous changed file at either end
  - [x] Cache the computed diff rows instead of rebuilding them every frame
- [x] **Line-level staging**
  Select lines in a hunk to stage, unstage or discard (click, Shift-click for a range,
  Ctrl/Cmd-click to toggle; Esc clears). Working-tree diffs only, one hunk at a time.
  - [x] Keyboard shortcuts: S/U stage or unstage the selected lines (see P1 Keyboard
    shortcuts); reverting lines of a commit diff is under the diff viewer
- [x] **Merge conflict tool**
  Three-pane view (ours / theirs / output), take a whole side per conflict, save and mark
  resolved. Opens from a conflicted file in the WIP panel or "View conflicts" in the banner;
  binary, deleted and special files take a whole version.
- [x] **File history and blame**
  File history: commits that changed a file, newest first, following renames, opened from a
  file's context menu in the commit details or WIP panel; each shows that commit's diff of the
  file and links to the graph. Searches back in batches of the commit limit. (The blame gutter
  itself is under Later.)
  - [x] Open file history or blame for any file from the command palette (a file finder), not
    only from a changed-file list. History done; blame waits for the blame gutter
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
- [x] **Multi-select commits**
  Ctrl-click toggles a commit and Shift-click selects a range of graph rows; Esc returns to one
  commit. The details panel lists the selection; with exactly two commits it lists the files that
  differ between them and opens their diff. Cherry-pick the selection (oldest first) and copy
  their SHAs from the panel or the context menu.
- [x] **Live working-tree updates**
  The active tab's repository is watched with the `notify` crate (Preferences > General, on by
  default): outside edits re-read the status and ref or HEAD changes reload everything, within
  about 0.3 s. Window focus and F5 still refresh as before.
- [x] **Credential prompts**
  Trellis is git's and ssh's askpass helper: HTTPS sign-in (username and password or token in
  one dialog), SSH key passphrases, SSH passwords and host-key confirmations open an in-app
  prompt, with retry and rejected-credentials notices, cancel and a 5-minute timeout.
  Background jobs and hosting-service sign-in never prompt.
- [x] **Branch operations**
  From the local branch menu: rename, set and unset the upstream (also to a branch not on the
  remote yet), fast-forward without checkout, delete local + remote in one step when the
  checked-out branch contains it (undo restores the local branch), push any branch to its
  upstream, push to a chosen remote.
  - [x] Rename the checked-out branch from the command palette
- [x] **Hide / solo branches in the graph**
  Hide or show local and remote branches and solo one or more of them from the left panel's
  branch menus. Hidden rows stay listed, dimmed, with a marker that shows them again; soloed rows
  get a marker that stops soloing; a banner under the filter box says what is filtered, with
  Stop soloing and Show all. Saved per repository; refs that no longer exist are dropped.
- [x] **Commit panel extras**
  - [x] Commit template (`commit.template` prefills an empty draft; the template's comment lines are left out)
  - [x] "Push after committing" checkbox
  - [x] Skip hooks (`--no-verify`)
  - [x] Stage-all-and-commit shortcut (Ctrl+Shift+Enter; Ctrl+Enter commits)
- [x] **File actions**
  - [x] Restore a file from a commit (commit details and file history menus; a deleted file
    comes back from the parent)
  - [x] Open a file in the default program and show it in Explorer / Finder (WIP and commit
    file rows)
  - [x] Open in an external editor (file rows' menus; Preferences > External Tools)
  - [x] Create, delete and rename a file (WIP panel: New file… in the Unstaged Files header,
    Rename… and Delete… in working-tree file rows' menus)
  - [x] New file in this folder… on file rows (WIP and commit file menus), prefilled with the folder
  - [x] Move deleted files to the Recycle Bin / Trash (Delete… and Unstage and delete…; asks
    before deleting permanently when the trash can't take a file; a General preference turns it off)
    - [x] Move untracked files that Discard deletes to the Recycle Bin / Trash too
  - [x] Rename tracked files with git mv so the rename is staged (option in the rename dialog, remembered)
  - [x] Unstage and delete a staged new file in one step (Delete… on a file staged as new asks
    once and also removes it from the index)
    - [x] The same for a staged rename (the old path's staged deletion would be left behind)
    - [x] Finish removing a staged new file whose working copy is already gone (Delete is
      disabled there)
    - [x] Label the item "Unstage and delete…" on rows whose path is staged as new
  - [x] Restore from comparison rows (two commits selected): the newer commit's version, or the
    older one's for a file the newer commit doesn't have; open and show there too
  - [x] Make Restore undoable (keep the overwritten working copy, as Discard does)
  - [x] Open with default app and show in folder from the file history view and the diff header
  - [x] Restore button in the file history's selected-commit header and the commit diff header
- [x] **Repository trust**
  A repository's `.git/config` (`core.sshCommand`, `credential.helper`, `core.fsmonitor`) and its
  hooks can run arbitrary commands as soon as git touches it. Before Trellis runs git in a
  repository it didn't clone or create (auto-fetch, hooks), ask whether to trust it.
  - [x] Ask on first open of an untrusted repository, naming the config keys and hooks that
    would run commands; repositories Trellis cloned or initialised are trusted automatically
  - [x] Hold background fetch (and anything else that runs git on its own) until the user decides
  - [x] While untrusted, browse read-only through libgit2, say in the toolbar why fetch, pull
    and commit hooks are paused, and ask before running git commands the user starts (Trust and
    run / Cancel)
  - [x] A way to bring the prompt back for the open repository (toolbar chip or palette
    command) after Don't trust or Esc

## P1: power features

- [x] **Command palette (fuzzy finder)**
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
  - [x] Shortcut hints next to entries (Ctrl+F, Ctrl+, Ctrl+W)
    - [x] A Refresh entry carrying the F5 hint
  - [x] Check out remote branches (creating a tracking branch) and tags
  - [x] A toolbar button that opens the palette, with the shortcut in its tooltip
  - [x] Rank recently run commands first
  - [x] More commands: stash and pop, rename branch, undo and redo, new tab and reopen closed
    tab, diff layout and whitespace toggles, toolbar labels, open in terminal and file manager
    - [x] Apply, pop or drop a chosen stash through a stash sub-list, and stash with a message
- [x] **Keyboard shortcuts**
  Exist: tabs, undo/redo, F5, Ctrl+F, Ctrl+, , diff navigation, commit from the message box
  (Ctrl+Enter, Ctrl+Shift+Enter), S/U to stage or unstage the open file or its selected lines,
  Ctrl+Shift+S/U to stage or unstage all, Ctrl+Shift+M for the commit message, and a Ctrl+/
  shortcut sheet (filterable; also in the palette). One binding table (keymap.rs) feeds the
  sheet, palette hints and tooltips.
- [x] **Built-in terminal**
  A resizable Terminal panel under the central area, shown or hidden with the toolbar's Terminal
  button, the palette or Ctrl+` (right-clicking the button still opens an external terminal).
  Each repository tab gets its own shell (pwsh, Windows PowerShell or cmd on Windows; $SHELL
  elsewhere) on a pseudo-terminal, started in its working directory the first time the panel
  shows it; hidden tabs' shells keep running, closing a tab stops its shell. The screen is drawn
  with 16, 256 and true colours, bold, underline, inverse and a cursor; typing, named keys,
  Ctrl+letters and pastes (bracketed when asked for) go to the shell while it has focus. Kill,
  Restart, a starting message, the exit code and start failures are shown.
- [x] **Submodules**
  A SUBMODULES section at the bottom of the left panel (shown when the repository has any) lists
  the index's submodules with a painted marker and a word for each state: in sync, out of sync,
  changes, changes and out of sync, not initialized. The tooltip explains the state and shows
  the URL, the recorded commit and the checked-out commit. Double-click or Open in new tab opens
  an initialized submodule as a repository; Initialize / Update to recorded commit runs
  `git submodule update --init` for it as a job (asking first when it is out of sync, and naming
  any commits inside that no branch or tag contains, which the update would leave behind); Copy
  URL copies the .gitmodules URL. The left-panel filter also filters the section.
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
- [ ] **Git LFS**
  Initialize, track patterns (file, extension, folder), fetch/pull/push/prune, view
  `.gitattributes`, and LFS-aware auto-fetch.
  - [x] Preferences > Git LFS (repo-specific): status, Initialize, tracked patterns with Untrack
    and a new-pattern field, Open .gitattributes, Fetch / Pull / Push / Prune / Checkout; palette
    'Initialize Git LFS' and 'Configure Git LFS…'
  - [x] Track from a file's context menu: that one file (`git lfs track --filename`), all files
    with its extension, or all files in its folder
  - [ ] LFS-aware auto-fetch: fetch LFS objects for the checked-out commit after a background
    fetch, and say so on the auto-fetch preference
  - [ ] Honour smudge filters when restoring a file from a commit, by writing through
    `git restore --source` so LFS files aren't left as pointers (the same item as under File
    actions > Restore)
- [x] **Git Flow**
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
  - [x] A Git Flow panel listing feature, release and hotfix branches by type with Start and
    Finish, a palette entry to open it, and a Git Flow group in the left panel: the panel
    replaces the graph (production and development tips, Start per kind, Finish and Checkout per
    branch, closed by Esc or jumping to a commit, parked with its tab); the GIT FLOW section
    groups the local flow branches by the longest matching prefix, with a Start '+' per group,
    Finish in each row's menu and the header menu opening the panel
  - [x] Create a feature, release or hotfix branch from an issue (issue menu and palette): three
    Start Git Flow items in the issue menu, and palette entries that list the assigned issues;
    the Start dialog opens with the issue's branch name typed
- [x] **Remote management**
  Add/edit/remove remotes (separate pull and push URLs), fetch one remote, view a remote in the
  browser, add a teammate's fork, fork on the service.
  - [x] Publish a repository with no remote to GitHub: Push (or Push ▾ > Publish to GitHub…)
    offers to create the repository (owner: you or an organization; private or public), adds it
    as `origin` and pushes the branch with upstream set
  - [x] Add, edit (name, fetch URL, separate push URL) and remove remotes, and fetch one remote,
    from the REMOTE section (header and remote row menus, palette)
  - [x] View a remote on its hosting service in the browser (row menu, from the remote URL)
  - [x] Add a teammate's fork as a remote (pick from the service's forks of this repository)
  - [x] Fork the repository on the hosting service and add the fork as a remote
- [x] **Tag management**
  Annotate an existing tag, delete from one or all remotes, push to all remotes, hide tags.
  (Push a tag and delete it locally exist; tag signing is on the Commit Signing page.)
  - [x] Delete a tag from one or all remotes and push it to all remotes, from the TAGS row menu
  - [x] Annotate an existing lightweight tag (replace it with an annotated tag at the same commit)
  - [x] Hide tags in the graph (see Hide/solo)
- [x] **Stash improvements**
  Stash selected files only, a stash message field, rename a stash. (Stash with a message exists
  in the palette; hiding stashes is under Later.)
  - [x] Rename (edit the message of) an existing stash, from the sidebar and the palette
  - [x] Stash only staged changes or selected files, with a message, from the palette too
- [ ] **Clone and init options**
  Shallow and sparse clone; init with a `.gitignore` template and a license; clone from a list
  of your repos on a connected service. (Creating the hosting-service repo on init is under
  Later; Publish to GitHub covers GitHub.)
  - [x] Shallow clone from the Clone dialog: by commit count or since a date, optionally one branch
  - [ ] Sparse clone (choose folders to check out when cloning)
  - [ ] Init with a .gitignore template and a license
  - [ ] Clone from a list of your repositories on a connected hosting service
  - [x] Fetch the full history of a shallow clone later (unshallow, or deepen by more commits)
- [x] **Sparse checkout**
  A repository page in Preferences (Sparse Checkout, also opened by the palette's Configure
  sparse checkout…) shows whether it is on, in folders (cone) or patterns mode, and how many
  tracked files are checked out. Turn on / Save paths runs `git sparse-checkout set` with the
  list typed one path per line (folders are checked and cleaned up first; a sparse index is never
  created); Reapply and Turn off run `reapply` and `disable`. Each is a job that reloads the tab,
  and changed files git keeps outside the new paths are named in the toast. The libgit2 status
  no longer lists the files sparse checkout leaves out as deleted. (Sparse clone is under Clone
  and init options.)
- [x] **Repository management**
  Favorites section above Recent on the welcome screen and in the repository menu; remove one
  recent entry or clear the list; missing folders marked.
- [x] **Graph columns and loading**
  Column toggles, compact layout and row highlighting are done (UI Customization). Missing:
  - [x] Resize columns: drag the header dividers (BRANCH/TAG, SHA, changes, author, date), double-click to reset, saved in settings
  - [x] Lazy-load more history when scrolling past the commit limit (the General page's "lazy
    loading"); jumping to a pinned branch goes with pinning in Branch operations
- [ ] **Search and filter**
  Filter the graph to matches (not just dim them), filter by author, search by file path.
  (Background indexing for large repos is under Later.)
  - [x] Filter the graph to matches (Filter toggle, Ctrl+Shift+F, live match count)
  - [x] Author filter (author: terms and an author picker)
  - [ ] Search by file path
- [x] **Signing beyond config**
  Done on the Commit Signing page: generate a GPG key, copy the public key, gpg/ssh program
  path, SSH allowed-signers, sign tags.
  - [x] Verified/unverified signature badge on commits (the selected commit's details panel)
- [ ] **SSH key management**
  Generate a key, choose the key pair, use the local agent, copy the public key (and add it to
  a connected service).
  - [x] Preferences > SSH: list the key pairs in ~/.ssh with type, comment and fingerprint, copy a public key, generate an Ed25519 key (asking before replacing one)
  - [ ] Choose the key pair git uses for SSH remotes (core.sshCommand with `-i <key> -o IdentitiesOnly=yes`, globally or per repository), show the current choice and stop using it
  - [ ] Use local SSH agent: detect a running agent (ssh-add -l, the Windows OpenSSH agent service or a Pageant-style agent), add the selected key to it, and leave core.sshCommand unset while it is on
  - [ ] Add the public key to a connected hosting service through its API (optional key title, check whether it is already registered, fallback: copy the key and open the service's SSH key settings page)
- [ ] **Profiles**
  Several named profiles (identity, integrations, preferences) with quick switching.
  - [x] Named identity profiles: add, rename, delete (asking first), switch from Preferences >
    Profile or the palette (writes user.name/email/signingkey to the global config); the first
    is seeded from the global identity
  - [ ] Per-profile integrations: each profile keeps its own connected hosting-service accounts
  - [ ] Per-profile app preferences: theme, UI customization, editor and general settings
    switch with the profile
- [x] **Local patches**
  Export commits, files or the WIP to a `.patch` file; open and apply a patch.
  - [x] Create patch… from one commit or a multi-selection in the graph menus (one mbox-style
    file, oldest first, binary-safe); Apply patch… from the palette to the working tree, checked
    first so nothing changes when it doesn't apply
- [x] **Conflict prediction**
  Local-only version: dry-run merge the branch against its target/upstream (`git merge-tree`)
  and warn about overlapping files before you merge or open a PR.
- [x] **External tools**
  Choose an editor, diff tool, merge tool and terminal (with custom commands); open the repo in
  the editor. (Diff and merge tools are their own item below.)
  - [x] Editor (open a file or the repository) and a custom terminal command in Preferences > External Tools
- [ ] **Notifications**
  Toasts for every operation exist. Missing: optional desktop notifications.
  - [x] "N commits behind" toast with Pull after a background fetch (UI Customization > Behind alerts)
  - [ ] Desktop (OS) notifications while the window is unfocused, for the behind alert and job
    results: an opt-in toggle with help text, confirmed once it is turned on

- [ ] **Blame gutter**
  Blame for a file: a gutter with author, date and commit per line (grouped by commit), opened
  from file history, the file context menu and the palette's file finder; clicking a commit
  selects it in the graph.
- [ ] **Add and remove submodules**
  - [ ] Add a submodule: URL, path and optional branch, through `git submodule add`, with checks
    for a path that already exists or is already in the index
  - [ ] Remove a submodule (deinit, `git rm`, clean its `.git/modules` entry) behind a
    destructive confirmation
- [ ] **External diff and merge tools**
  - [ ] External diff tool: program and arguments ({local}, {remote}, {path}) in Preferences >
    External Tools, opened from file rows and the diff header
  - [ ] External merge tool: program and arguments ({base}, {ours}, {theirs}, {output}), and the
    conflict view's Resolve in an external merge tool
- [ ] **Rebind keyboard shortcuts**
  Rebind keys in Preferences, saved in settings and matched through the shortcut table, with
  conflict detection and reset to default.
- [ ] **GPG passphrase prompts**
  Ask for GPG passphrases for commit and tag signing in Trellis (a Trellis pinentry, or loopback
  pinentry), like the existing SSH passphrase prompt.

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
  - [ ] Jira key detection treats strings like `UTF-8` as keys unless a project filter is set.

## P3: app polish and platform

- [ ] **Custom themes** (dark, light and follow-the-system exist)
- [ ] **Zoom** in/out/reset; show/hide the left, detail and toolbar panels
- [ ] **Menu bar** (File / Edit / View / Window / Help), including Undo/Redo under Edit
- [ ] **Language**: translate the UI (date locale and relative dates already exist)
- [ ] **Logs**: keep the activity log across sessions; error and performance logs; extended
      logging (the in-session activity log exists)
- [x] **Command-line companion**: `trellis <path>` and `trellis .` work (subcommands are under
      Later)
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

## Later

Ideas and polish beyond each item's core, grouped by the item they grew from. They are not
tracked as checkboxes; they are built after the open items above, roughly in this order.

### Date/Time section

- Use the *Date* and *Date word* formats somewhere (they can be edited but nothing shows them
  yet), and add date tooltips

### Tabs (multiple open repos)

- Keyboard reordering of tabs, and Escape to cancel a drag in progress
- Rename a tab; a tab list with recently closed tabs to reopen any of them
- Ask before closing tabs that hold a commit draft or conflict picks

### Undo / Redo

- Undo per-file, per-hunk and per-line discard (needs a safety stash or blob backup first)
- Undo across restarts by rebuilding actions from the HEAD, branch and stash reflogs
- Undo merge, rebase, pull, cherry-pick, revert, create branch/tag, stash, pop and apply
- Safety stash before `reset --hard`, so undoing the reset restores discarded changes; offer to
  stash local changes when undoing or redoing a hard reset or Discard all
- When undoing `checkout --track` / `checkout -b`, offer to delete the created branch
- Edit menu entries for Undo/Redo, once Trellis has a menu bar

### Diff viewer upgrade

- Whole-file view: derive the hunk spans from the whole-file highlight (or load the old text
  too), so switching back to Hunks without a reload keeps in-context colors
- Keep parse states per hunk across reloads (or checkpoint states by line), so staging one hunk
  of a large file doesn't re-parse the whole prefix in the background
- Count the lines before a hunk twice when an earlier hunk leaves the two sides in different
  states (the highlight cost now assumes they are parsed once)
- With whitespace ignored, take the new side's unchanged lines from the new file instead of the
  old one (they can differ in whitespace only, so such diffs now fall back to per-hunk colors)
- PowerShell highlighting: `$(...)` subexpressions inside strings as code, splatting
  (`@params`), method and static member calls (`[Math]::Round`, `.Trim()`), and here-strings
  that don't start at the end of a line
- Color Kotlin annotations (`@Suppress(...)`), which the bundled grammar leaves unscoped
- Color Dockerfile `RUN` commands as shell (the hidden "Dockerfile (with bash)" grammar)
- Trim the bundled syntax set to the languages Trellis wants, if the extra ~0.6 MB of binary
  size matters
- Show the third-party notices inside the app (an About dialog)
- A per-file language picker (e.g. in the diff header), remembered per path
- For commit diffs, read `.gitattributes` from the commit instead of the working tree
- Take the hunk-header function context from `.gitattributes` diff drivers (`diff=<lang>`,
  `xfuncname`) rather than only libgit2's default (lines starting with a letter, `_` or `$`)
- Parse the hunk-header context in the file's context (the state at the function line in the
  old text) for contexts that start inside a multi-line construct
- Color the function context in the conflict view's chunk headers
- A command or shortcut for the syntax highlighting toggle (now on the Editor preferences page)
- User-selectable syntax color themes beyond the dark and light palettes
- Pair moved lines across change runs and hunks for word highlights
- Pair the conflict view's side-by-side rows by similarity too
- Map hunk and line stage/unstage/discard, and Revert Hunk on commit diffs, from the
  whitespace-filtered diff back to the unfiltered one instead of disabling them
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
- Keyboard access to fold rows (open the fold at or after the top row)
- Fill in the unchanged lines between hunks in the hunk view
- Hunks / Whole file and Inline / Split toggles in the File history header, and a command or
  shortcut for the view mode toggle
- Diff view vs file view toggle (plain file, or a rendered preview for Markdown/images)
- Revert selected lines of a commit diff (line selection on commit diffs, then the partial
  patch with only those lines, reversed)
- Revert single lines in the whole-file view (needs whole-file line selection first)
- Follow renames when reverting from File history: map a commit's older path to the file's
  current name instead of refusing with "not in the working tree"
- Remove the file, rather than leaving it empty, when reverting the hunk of a commit that added
  it
- Fall back to a three-way merge (conflict markers) when the working file has moved on, instead
  of refusing a hunk revert
- Record a hunk revert in Trellis's undo history
- Revert a whole file of a commit into the working tree (file-list context menu), and a
  keyboard shortcut for Revert Hunk
- Scroll the Unstaged/Staged, commit or comparison file list so the file opened by
  next/previous change is in view
- Next/previous change across entries in the File history view (step into the next or previous
  commit's diff of the file)
- An optional wrap-around from the last changed file back to the first (with a notice)
- Keep a next/previous step from being superseded when a status refresh reloads the old file
  while the next one is loading
- Show which file of how many (e.g. 3 of 12) in the diff header
- Cache the change rows behind the Previous/Next buttons too (their state rebuilds the change
  rows twice per frame)
- Wrap long lines in the conflict view too (it keeps fixed-height rows)
- Keep a wrapped line's indentation on its continuation lines
- A marker in the gutter on continuation lines of a wrapped row
- Wrap by display width so wide (CJK, emoji) characters count as two columns
- A keyboard shortcut and a command palette entry for the wrap toggle
- Wrap long hunk headers (their function context) instead of clipping them

### Line-level staging

- Per-line hover buttons (stage / unstage / discard this line)
- Drag to select a range of lines
- Selections spanning several hunks
- Keep the line selection across tab switches
- Remove the index entry, rather than leaving an empty file, when every line of an added or
  deleted file is unstaged or staged line by line

### Merge conflict tool

- Take or untake individual lines from either side of a conflict
- Auto-resolve trivial conflicts (whitespace-only, or one side equal to the base), with a note
  that they were resolved automatically
- Edit the output pane directly before saving
- Save the output while keeping conflict markers for chunks not picked yet
- Mark several selected files resolved, Mark all resolved, and Mark conflicted (un-resolve,
  e.g. `git checkout -m <path>`)
- Split the WIP file list into Conflicted Files (N) and Resolved Files (N), with per-file
  conflict counts
- Step to the next conflicted file after saving one
- Undo and redo of a saved resolution through the undo history
- Collapse long unchanged runs in the panes; a resizable split between panes and output
- Syntax highlighting in the conflict panes
- Warn before Mark resolved when the working file still contains conflict markers

### File history and blame

- Start from HEAD when the file was renamed after the chosen commit (track renames forwards)
- Refresh an open file history when HEAD moves (commit, checkout, pull, fetch)
- Show the whole file at the selected commit (file view) as an alternative to its diff
- History of a folder
- Let `diff::show` delegate to `diff::body` once the diff header changes in flight have landed

### Interactive rebase

- Rebase onto a commit, and branch context menus on the graph's ref labels
- History editing from the graph: reword, squash with parent, or drop commits from the context
  menu (several at once needs multi-select)
- Start with uncommitted changes by stashing them first (`--autostash`), after asking
- Rebase ranges that contain merge commits (`--rebase-merges`)
- Keep `update-ref` steps (`rebase.updateRefs`) so branches stacked inside the range move with
  it
- Edit the combined message of a squash group in the panel instead of git's concatenation
- Keyboard control in the panel: select a row, Alt+Up/Down to move it, a letter key per action
- Edit the remaining steps of a rebase already in progress (`rebase --edit-todo`), including one
  started outside Trellis
- Show the step (N of M) and the stopped commit in the rebase banner, and report edit or
  conflict stops after Continue too (Continue currently says "succeeded" even when it stops
  again)
- Undo an interactive rebase (back to `ORIG_HEAD`) from the undo history
- `exec` and `break` steps in the panel

### Multi-select commits

- Compare a selected commit with the working directory (including Ctrl-clicking the WIP row
  into a selection)
- Combined diff of a contiguous range of three or more commits (the oldest's parent to the
  newest), explaining when the selection isn't contiguous
- Extend the selection from the keyboard: Shift+Up/Down and Ctrl+A
- Swap the compare direction (newer to older) in the two-commit view
- Revert several commits; squash, drop, move or reword several (interactive rebase)
- When a multi-commit cherry-pick stops on a conflict, select the WIP row, and add Skip commit
  (`cherry-pick --skip`) to the cherry-pick banner
- Cherry-pick merge commits (choose the mainline parent with `-m`)
- Undo and redo for cherry-picks (one commit or several)
- Highlight the sidebar branches whose tips are in a multi-selection

### Live working-tree updates

- Notice a watcher that started but receives no events (a probe after starting) and warn once,
  suggesting a restart
- Watch the common git dir's refs for linked worktrees (their refs live outside the worktree's
  own git dir)
- Watch inactive tabs so their state updates in the background
- Treat `.git/config` changes (remotes added or removed outside Trellis) as a full refresh
- Status-bar indicator showing whether live updates are active for the repository
- Reflect submodule HEAD changes live in the parent's status (paths under a nested `.git` are
  ignored for now)

### Credential prompts

- Remember entered HTTPS credentials through `git credential approve` / the OS credential
  store, and reject them when they fail
- Caps Lock warning on password fields (egui doesn't expose Caps Lock state yet)
- Choose between a password, personal access token or app password per hosting service, with a
  link to create a token
- A non-blocking notice for SSH security-key touch requests (`SSH_ASKPASS_PROMPT=none`)
- Support OpenSSH older than 8.4 on Linux/macOS (no `SSH_ASKPASS_REQUIRE`; needs `DISPLAY`)
- Auto-fetch that needs credentials: a "sign in needed" notice instead of failing quietly
- Hosting-service sign-in without Git Credential Manager through the prompt (Preferences >
  Integrations)

### Branch operations

- Pin a branch to the left (graph lane order, per-repo persistence, jump to the pinned branch)
- Force push (with lease) of a non-HEAD branch, and push to all remotes
- Rename the remote branch too when renaming (push the new name, delete the old one, move the
  upstream)
- Choose a local branch as the upstream in the Set upstream dialog
- The same operations from the graph's BRANCH/TAG labels and the remote branch menu (e.g.
  fast-forward a local branch from its remote row)
- A shortcut for renaming the checked-out branch, and renaming any branch through a two-step
  palette entry
- Delete local + remote when the branch isn't merged into HEAD (force), in one step

### Hide / solo branches in the graph

- Hide / solo from the graph's BRANCH/TAG ref-label menus
- Hide stashes in the graph
- Hide, solo or stop soloing several selected branches at once (multi-select in the left panel)
- Hide all / show all per section (LOCAL, REMOTE, TAGS) from the section header menu
- Hover toggles (hide / solo) on rows that are not hidden or soloed
- A graph-header button listing hidden branches and tags, to unhide them from there
- Keep the hidden/solo state when a branch is renamed (rename the entry instead of dropping it)

### Commit panel extras

- Repo-specific Commit preferences page: edit and save the template, skip hooks by default,
  push after commit by default

### File actions

- Create, rename and delete files from the command palette
- New file in this folder… on folder rows (folder rows wait for a tree view in file lists)
- Delete several selected files at once (needs multi-select in file lists)
- A button in the trash toast that opens the Recycle Bin / Trash or restores the file
- Respect a drive's "remove files immediately" Recycle Bin setting on Windows
- On Linux, offer the home trash when a mount has no usable trash folder, instead of asking to
  delete permanently
- Send several selected files to the Recycle Bin / Trash at once (needs multi-select in file
  lists)
- Make Delete undoable with Ctrl+Z (restore the working copy from the Recycle Bin / Trash, or
  keep a copy as Discard does)
- Name a staged rename's source path in its Unstage and delete confirmation and toast
- Keep the staged version recoverable so Unstage and delete can be undone
- Open the newly created file in the editor or default app after creating it
- Create a file from a template, or with initial contents
- Rename or delete folders and submodules
- Explain when a created path is inside a nested repository or submodule, so the parent's
  status won't list it
- Edit a file in place
- Restore several selected files from a commit at once (needs multi-select in file lists)
- Choose which side to restore from comparison rows for a file the newer commit modified
- Re-check which comparison and commit-detail paths are on disk after a status refresh (Open /
  Show can go stale until the commits are reselected)
- Honour smudge filters when restoring (tracked as core under Git LFS)
- Explain when a restored or opened path is outside a sparse checkout
- Restore, Open and Show buttons in the diff header for two-commit comparisons (restoring the
  newer commit's version, as the comparison rows do)
- Open and Show for the file history's current file in its top header, independent of the
  selected commit
- Re-check whether the file history's file is on disk after status refreshes and file watcher
  events (an untracked file deleted outside Trellis keeps Open enabled)
- Open the commit's version of a file (a temporary copy) when it is no longer in the working
  tree
- Name the file in the show-in-folder tooltip and the open failure message
- Painted icons for Open and Show in the diff header to save room (split widgets.rs first)

### Repository trust

- A Preferences list to review and revoke trust decisions (they are already remembered per
  repository)
- Trust worktrees Trellis creates from a trusted repository (and optionally submodules of one);
  ask again when a trusted repository gains new command-running settings or hooks
- Hold shells the built-in terminal starts in an untrusted repository (prompts that run git)
- Bring back the dialog a held job came from (New tag, Remote, worktree…) after Cancel, with its
  fields intact
- Don't count hooks Trellis installed itself (git lfs install) as a reason to ask about a
  repository that was clean
- Hold the git CLI reads the LFS and Sparse Checkout Preferences pages make in an untrusted
  repository
- Hold Publish and other jobs that run in a different untrusted repository than the active one,
  using that repository's own decision
- Offer Don't trust on the prompt that holds a job, alongside Trust and run and Cancel
- Queue several held jobs instead of refusing a second one while the prompt is open

### Command palette (fuzzy finder)

- Push an existing tag to a remote from the sidebar TAGS menu and the graph's tag labels
- Remember the last remote picked for tag pushes per repository
- Warn (without blocking) when a new tag name matches a branch name
- Replace (force-move) an existing tag from the New tag dialog
- Catch existing tags that point at non-commit objects before running git
- A Preferences control for pushing new tags by default
- Cmd-style labels on macOS for the shortcut hints and tooltips
- Two-step entries for branches, themes and repositories (done for files and stashes)
- Check out a remote branch whose name is taken locally under a new local name
- Offer to fast-forward the checked-out branch when it is behind the picked remote branch
- Name the tag in the log after a tag checkout, and warn once that commits made while detached
  can be lost
- Group ref entries by remote, or move them into a two-step checkout sub-list
- Virtualised command rows so the 200-row cap can be lifted
- Show the Commands button when no repository is open (the palette already works there)
- Accessible labels for every toolbar button, once tests no longer look up palette rows by bare
  labels such as Refresh or Push
- Remember recently run commands across restarts (a short key list in settings)
- A dim "recently used" marker or divider for the recent block in the list
- Remember recent commands per repository instead of per session
- Rank recently used ref commands (Fetch <remote>, Finish <flow branch>, checkouts)
- Rank recently picked items first in the file and stash sub-lists
- An entry to clear the recent-commands list
- Commands: syntax highlighting on/off (re-highlighting the open diff when flipped), show the
  working directory changes (select the WIP row), diff mode (whole file / hunks) toggle
- Confirm before applying or popping a stash on top of uncommitted changes, in the palette and
  the sidebar alike
- Share one stash-action message builder between the sidebar and the palette
- Refresh an open stash sub-list's rows when the stash stack changes while it is shown
  (choosing a stash already finds it again by its commit)
- Apply or pop with `--index` to restore the staged state
- Create, delete and edit files from the palette
- PageUp/PageDown/Home/End in the list, and the selection following the mouse

### Keyboard shortcuts

- Topological next/previous (the next or previous commit on the selected commit's own branch
  line) and first/last row in the graph
- Match the older keys (tabs, undo/redo, F5, Ctrl+F, Ctrl+, , F7/Alt+arrows, the palette's
  keys) through the table too, so it is checked rather than kept in step by hand
- A visible way to the shortcut sheet (a toolbar, status bar or Help entry naming Ctrl+/)
- S in the WIP file lists (keyboard focus in the lists, Up/Down between files), and moving on
  to the next unstaged file after S stages the open one

### Built-in terminal

- Several terminals per repository as tabs in the panel ("Terminal N", new and close buttons),
  and kill all with a confirmation that says how many will stop
- Scrollback (history in the screen model, scrolled with the mouse wheel), with mouse selection
  and copying to the clipboard
- Right-click menu in the terminal: copy, paste, select all, clear
- In-App Terminal preferences page: default shell (auto, pwsh, Windows PowerShell, cmd, bash or
  a custom path with arguments), font size, cursor style, dim when unfocused
- A setting to use the built-in terminal for Open in terminal (palette entry and the toolbar
  button's right-click item)
- Let the focused terminal keep the keys the global shortcuts take today (Ctrl+W, Ctrl+T,
  Ctrl+Tab, Ctrl+1–9, Ctrl+P, Ctrl+F, F5; Ctrl+F and F5 also reach the shell), and send
  Alt+letter as an ESC prefix; coordinate with the keyboard shortcuts in ui/mod.rs
- Remember whether the panel is open, and its height, across restarts
- Mouse reporting (xterm mouse modes) and the window or tab title from the shell (OSC 0/2)

### Submodules

- Set a submodule's URL (`git submodule set-url`, then `git submodule sync`) and edit its branch
- "Keep submodules up to date": a General preference with a per-repository override (use global
  / on / off) that runs `git submodule update --init --recursive` after checkout, pull, merge,
  rebase and reset
- Initialize / update all submodules from the section header's menu, and offer to initialize
  them when a repository with uninitialized submodules is opened or cloned
- Refresh submodule status on status-only reloads and watcher events, not just full loads
- Nested (recursive) submodules shown as a tree under their parent
- Tell apart a deleted submodule folder, a newly added one (staged, not committed) and a
  never-initialized one in the marker and tooltip
- Resolve a relative .gitmodules URL against the parent's remote for Copy URL and the tooltip
- Commit the current checkout as the new recorded commit (stage the gitlink) from the row menu,
  and discard a submodule's uncommitted changes
- Show a gitlink change in the WIP and commit diffs as the commit range it moves over, and
  explain that a submodule's own changes are committed inside it
- Fetch and pull inside a submodule; recurse-submodules options for fetch, pull and push; a
  clear message when checkout or reset fails because of submodule changes
- Palette commands for opening, initializing and updating submodules
- A context-menu entry to show or hide the SUBMODULES section

### Worktrees

- Mark local branches checked out in another worktree in LOCAL (tooltip with the folder), and
  disable Checkout / Delete for them with the reason
- Create a worktree from a remote branch or a pull request (creating the tracking branch)
- Watch the common git dir (refs, packed-refs) when a linked worktree is the open tab, so ref
  changes made elsewhere reload it
- Palette commands: open a worktree, Create worktree…, Remove / Lock / Prune worktree
- Remove a worktree and also delete its branch on the remote
- Offer to close the other tab showing a worktree, instead of only refusing Remove
- Move a worktree (`git worktree move`)
- An Open folder button in the "folder couldn't be fully deleted" report, and a retry of the
  folder delete
- A context-menu entry to show or hide the WORKTREES section; optionally group worktree tabs
  next to their main repository's tab

### Git LFS

- Offer Track in Git LFS from file history rows, the diff header's file buttons and the palette
- Disable "Only this file" when a broader pattern already covers the file (attribute lookup in
  the background)
- Refresh the tracked patterns on status-only (watcher) refreshes, so hand edits to
  .gitattributes show without a full reload
- Warn when a folder pattern would also catch a nested .gitattributes in that folder
- Untrack lines that `--filename` wrote with backslash-escaped glob characters (`git lfs
  untrack` leaves them): remove them directly
- Offer to convert files already committed (`git lfs migrate`) after tracking their pattern
- When initializing fails because a non-LFS hook already exists, offer to merge or overwrite it
  (`git lfs update --manual` / `--force`)
- Warn in the main window, not only on the page, when a repository tracks files in LFS but
  git-lfs isn't installed
- Initialize with LFS when creating or cloning a repository
- File locking: lockable patterns (`track --lockable`), locking and unlocking files, and showing
  who holds locks
- Read patterns from nested `.gitattributes` files and `.git/info/attributes`, and show which
  file each pattern comes from
- Show LFS pointer files and LFS status in file lists and diffs (for example "stored in LFS"
  instead of the pointer text)
- Check for a minimum git and git-lfs version and say clearly when either is too old

### Git Flow

- Finish options: rebase on develop first, squash, and push after finishing
- Continue a Finish that stopped at a merge conflict once the merge is committed
- Finish options: skip the tag, sign it with a chosen key, delete the remote branch,
  force-delete
- Branch-menu and graph-menu entries for flow branches (finish this feature, start a hotfix
  from here)
- Confirm before deleting the configured production or development branch
- Bugfix and support branch types on the page (their default prefixes are already written)
- Fetch before Start, and publish or track flow branches on the remote
- Read the newer per-branch config layout (gitflow.branch.<name>.type / parent) and
  gitflow.origin
- Show how far each flow branch is ahead of or behind its base in the panel and the GIT FLOW
  section
- Bugfix and support branch groups in the panel and the GIT FLOW section
- List flow branches that exist only on a remote, with a track and checkout action
- Link a branch started from an issue back to the issue (mention it in the merge or tag
  message, move the issue to in progress)
- Publish, pull and rebase actions for flow branches in the panel and the section
- Order release and hotfix rows by version (1.10.0 after 1.9.0)
- Hide or show the GIT FLOW section from a left-panel context menu
- Show which issue a flow branch was started from in the Start dialog

### Remote management

- Publish to GitLab, Bitbucket and Azure DevOps
- Add an existing remote URL from the Publish dialog
- Undo removing or renaming a remote (re-add it with its URLs and fetch, or rename it back)
- Rewrite the saved graph filter's hidden or soloed refs when a remote is renamed or removed
- Edit remotes defined outside the repository's config, and every URL of a remote with several
  fetch or push URLs
- Per-remote options: fetch refspecs, tag fetching, prune on fetch, the default branch
- Offer Add remote… when fetch, pull or push finds no remote
- Fork on Bitbucket Cloud and Bitbucket Data Center (find the user's fork and create one)
- List forks and fork on Azure DevOps
- Fork into an organization or group, under another name, or with only the default branch
- Show the user's own fork first in the Add a fork list, with a one-click Add
- View on service for self-hosted servers whose web pages use another port than their SSH address
- After forking, offer to make the fork the push remote of the current branch, or to rename
  origin to upstream
- View a remote branch or commit on its hosting service from the branch and commit menus
- Search a service's forks on the server, beyond the newest 1000

### Tag management

- After deleting a tag locally, offer to delete it from the remotes too
- Offer Annotate… from the graph's tag labels and from the command palette (two steps: pick a
  lightweight tag, then the message)
- Edit the message of an already annotated tag (re-annotate with the old message prefilled)
- Push the annotated tag over a remote's lightweight copy (force-push one tag, after a
  confirmation)
- Honour tag.forceSignAnnotated too (git applies it only without -a), and say in the Annotate
  dialog when the tag will be signed

### Stash improvements

- Hide stashes in the graph
- Edit the stash message inline from the commit details panel when a stash is selected, and
  show the stash list message there (the stash commit's own message keeps its old text)
- Keep the original reflog dates of the stash entries above the renamed one
- Move apply, pop and drop (`JobMsg::Stash`) into `StashMsg`, so all stash actions share one
  message enum
- Share one stash-action message builder between the sidebar and the palette, Rename… included
- Undo a stash rename (record the old message so Ctrl+Z restores it)
- Rank recently picked stashes first in the rename sub-list
- Keep the index when stashing (`--keep-index`) as a Stash dialog option
- Stash ignored files too (`--all`) from the Stash dialog
- Stash chosen hunks or lines of a file, not only whole files
- Filter field above the Stash dialog's file checklist for large working trees
- When a staged rename's new path is chosen, stash its old path too, so the rename moves as a whole
- Check the installed git version before offering "Only staged changes", instead of explaining after git refuses
- Right-click menu on the toolbar Stash button with the Stash dialog's choices
- "Stash…" entry in the STASHES section header menu that opens the Stash dialog
- Stash the files selected in the WIP lists once working-tree files can be multi-selected

### Clone and init options

- Create the hosting-service repository and push on init (Publish to GitHub already covers
  GitHub for a repository with no remote)
- Deepen a shallow clone by date (`--shallow-since`) or exclude what a ref reaches
  (`--shallow-exclude`)
- Widen a single-branch shallow clone to fetch every branch
- Mark the shallow boundary commits in the graph
- Unshallow shallow submodules when an update or reset needs missing commits
- Confirm before unshallowing a very large repository, and cancel a running fetch

### Sparse checkout

- Sparse clone from the Clone dialog (tracked under Clone and init options)
- Show in the toolbar or status bar that the worktree is sparse, and notice sparse changes made
  outside Trellis on a plain refresh
- Explain when an opened or restored path is outside the sparse checkout, and keep left-out
  files out of the palette's file finder
- Add or remove a folder from the paths from file rows and the commit file tree
- Warn when a folders-mode path names a file, or a folder that doesn't exist at HEAD
- Refill the form from the kept list after Turn off, so turning it on again starts there
- Convert the list between folders and patterns when switching the mode
- Sparse index support (needs libgit2 to read index.sparse directory entries)
- Palette entries that reapply or turn off sparse checkout directly

### Repository management

- Scan project folders for repositories (choose one or more parent folders, list the Git
  repositories found under them, filter by name)
- Keyboard shortcuts to open favorites (e.g. the first nine), listed in the shortcut sheet and
  the palette
- Reorder favorites by dragging
- Fetch or pull across many repositories (all favorites or all listed), with per-repository
  results and a "nothing to fetch" case
- Mark repositories already open in a tab in the recent and favorite lists, and open all of
  them in new tabs at once
- Remove all missing entries in one go, and offer to remove an entry when opening it fails
  because its folder is gone
- Palette commands to favorite or unfavorite the active repository, with favorites listed first
  among the palette's repositories

### Graph columns and loading

- Retry a failed load-more by itself after a delay, or show the failure in the loading row
  instead of only in the log
- Keep loading further back while a filtering search finds too few matches (search the whole
  history without scrolling)
- Hide or disable the header's Load more link while a load for more is already running
- A per-repository commit limit, or remembering the extra loaded commits across tab switches
  and restarts
- Keep the top visible commit in place when a load-more refresh also brings new commits above it

### Search and filter

- Background indexing for large repos

### Signing beyond config

- Signature badges on graph rows (batch-check visible signed commits, cached by oid, throttled
  like Stats)
- Signature status in multi-commit views (details_multi, compare) and the file history list
- Flag a verified signature whose signer identity doesn't match the committer's email
- Verify signed tags in the TAGS section and tag tooltips
- Show signatures on merged tags (mergetag headers) of merge commits
- Show the SSH key type and GPG primary user ID / primary-key fingerprint in the badge tooltip
- Warn on the Commit Signing page when gpg.ssh.allowedSignersFile points to a missing file or a
  repository-local value overrides the global one

### SSH key management

- Generate RSA or ECDSA keys, and change an existing key's passphrase or comment (ssh-keygen -p
  / -c)
- Browse buttons for picking a private key file and the save location
- Show in folder and delete a key locally from its row
- Flag encrypted private keys and keys whose file permissions are too open for ssh on their row
- Palette entry for the SSH page (palette_commands.rs needs splitting first)
- Keys outside ~/.ssh (IdentityFile entries in ~/.ssh/config, agent-only keys)

### Profiles

- Per-profile signing format and program (gpg.format, gpg.program, commit.gpgsign) and SSH key
  (core.sshCommand)
- Profile icon or avatar picker, and a profile switcher in the toolbar or status bar
- Automatic per-repository or per-folder profiles (includeIf / local identity applied on open)
- Import and export of profiles, and sharing them between machines
- A keyboard shortcut for Switch profile

### Local patches

- Export the WIP (unstaged and/or staged changes) to a `.patch` file
- Export selected files of a commit or the WIP to a `.patch` file (from the file menu)
- Apply a patch as commits with `git am` (series, three-way fallback, continue, skip and abort,
  conflicts in the conflict view)
- Apply options: to the index as well, three-way fallback, partial apply with `.rej` files,
  reverse apply
- Create patch button in the multi-selection details panel, and a palette entry for the
  selected commits
- Copy a commit's patch to the clipboard instead of saving a file
- Show in folder from the toast after saving a patch
- Drop a `.patch` file on the window to apply it, and preview the files a patch touches before
  applying

### Conflict prediction

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

### External tools

- Open at the selected diff line: pass {line} from the diff or whole-file view
- Presets for well-known editors and terminals (fill in the arguments when one is picked), and
  detecting installed ones
- Name the editor in labels ("Open in <editor>"), and an Open in external editor button in the
  diff and file history headers
- A keyboard shortcut for opening the selected file in the external editor
- Report an editor that exits at once with an error
- Open several selected files or repositories at once, asking first when there are many
- Show / hide arguments toggle that collapses the argument fields

### Notifications

- Behind alerts for other local branches with upstreams and for inactive tabs, batched into one
  alert when several have new commits
- Pull request activity alerts (new pull requests, review requests, finished checks)
- Keep a toast up while the pointer is over it, and a close button on toasts

### Limits found while building integrations

- Make the branch-name issue heuristic configurable (`2024-cleanup` is currently read as issue
  2024)

### Command-line companion

- History, diff and status subcommands
