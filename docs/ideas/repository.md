# Ideas: Repositories

Clone and init, submodules, worktrees, LFS, sparse checkout, trust and the git engine. Untriaged; see [README.md](README.md).

### Repository trust

- Rescan submodules (their own settings and hooks) between a job's commands, as `scan_now` does for the repository itself; it skips them now (#277).
- Tell apart two items that couldn't be read for the same reason, so Trust and run on one doesn't pass the other too.
- Send a job's `on_fail_check` command through the trust gate like its own commands.
- Make the merges of a rebase-merges todo, an octopus merge and an `am -3` fallback in memory, so they are compared instead of held.
- A Preferences list to review and revoke trust decisions (they are already remembered per
  repository)
- Bring back the dialog a held job came from (New tag, Remote, worktree…) after Cancel, with its
  fields intact
- Don't count hooks Glowfly installed itself (git lfs install) as a reason to ask about a
  repository that was clean
- Offer Don't trust on the prompt that holds a job, alongside Trust and run and Cancel
- Queue several held jobs instead of refusing a second one while the prompt is open
- Pass trust on to a worktree or submodule made by a job that waited for the prompt and ran on
  Trust and run
- Trust nested submodules a recursive initialize checks out, not only the listed ones
- Offer to trust a trusted repository's existing worktrees and checked-out submodules when one is
  opened in a tab
- Read a repository that isn't open in any tab and ask about it, rather than refusing a job for it
- Find a favorite's trust decision when it is listed under another spelling (a link) and isn't
  open in a tab
- A preference to stop passing trust to new worktrees and submodules
- Show the unchanged, already trusted settings and hooks under a fold in the Trust this
  repository again? prompt
- Take a newly checked-out submodule's own settings into its parent's trust decision, so the
  parent isn't asked again about what the submodule brought
- Say in the terminal when a repository with running shells stops being trusted or gains
  settings or hooks (shells already running are never stopped)
- Offer Don't trust from the terminal's held panel prompt, alongside Trust and run and Cancel
- Show the LFS setup while a repository is untrusted by reading it without git (libgit2 for the
  hooks folder and local filters, git outside the repository for the rest)
- List the sparse paths of an untrusted repository by reading its sparse-checkout file instead
  of git sparse-checkout list
- Re-check checked-out submodules' settings and hooks at every refresh, not only at a full load
  (a refresh rescans only the repository's own, since opening every submodule each time costs
  too much where there are many)
- Stamp a submodule's included config files and a hooks folder its core.hooksPath points
  outside its git dir in the refresh key (only its own config files and hooks folder are now)
- Re-check checked-out submodules' settings and hooks in the runner just before a job, as the
  repository's own are (the runner's re-check skips submodules)
- Compare the incoming hooks of `reset --hard`, stash apply, submodule update and custom
  commands too (checkout, switch, merge, rebase, pull, cherry-pick, revert and the interactive
  rebase, undo and stack jobs are compared)
- Compare `checkout -- path` restores from the index (only those naming a tree-ish are compared)
- Notice hooks that run tracked scripts outside the hooks folder by a plain path (only links
  into the working tree are followed)
- Re-check the partial-clone, forge and status-tip reads in the runner, as the jobs and the
  other git reads are
- Offer a fetch with hooks off from the "couldn't compare" prompt of a partial clone
- Compare what a rebase leaves where hooks follow the working tree (each replayed commit and
  the base are compared, not the merges the replay makes), and merge with git's own `ort`
  options (`-X`, `merge.renames`) rather than libgit2's defaults
- Mark a status read that fell back to libgit2 (the repository changed since it was trusted)
  as reduced in the status bar
- Stamp reused read scans by file id on Windows too (only creation time there; same-size
  rewrites within the timestamp granularity reuse a stale read scan)
- Don't ask again after a checkout when a hook's checked-out bytes differ from the trusted
  commit's only by line endings (core.autocrlf)

### Submodules

- Nested (recursive) submodules shown as a tree under their parent
- Commit the current checkout as the new recorded commit (stage the gitlink) from the row menu,
  and discard a submodule's uncommitted changes
- Show a gitlink change in the WIP and commit diffs as the commit range it moves over, and
  explain that a submodule's own changes are committed inside it
- Fetch and pull inside a submodule; recurse-submodules options for fetch, pull and push; a
  clear message when checkout or reset fails because of submodule changes
- Add recursively, cloning the new submodule's own submodules too
- Shallow add (`--depth`) and a single-branch clone for large submodules
- Choose a submodule name other than its path (`--name`), which also gets past a leftover stored
  module
- Add a repository that is already inside the working tree as a submodule
- Browse… for a local repository URL, and a picker of the hosting service's repositories, in the
  Add submodule dialog
- Remove a gitlink that has no .gitmodules entry (`git rm --cached`, then clear its folder)
- Send a removed submodule's stored module to the OS trash instead of deleting it
- Undo for adding or removing a submodule
- A context-menu entry to show or hide the SUBMODULES section
- Remember "don't offer to initialize submodules" per repository instead of once per session
- A live "resolves to" preview of a relative URL in the Edit submodule dialog
- Rename or move a submodule (name and path) from the Edit submodule dialog
- Update nested submodules that are out of sync inside an in-sync parent submodule on Update all
- Per-submodule progress and a cancel button for long recursive updates
- Translate the SUBMODULES section, its menus and dialogs

### Worktrees

- An Open folder button in the "folder couldn't be fully deleted" report, and a retry of the
  folder delete
- A context-menu entry to show or hide the WORKTREES section
- A Refresh worktree changes item in the WORKTREES header menu that counts again without walking
  the history
- Watch other worktrees' folders so their WIP rows update live
- A WIP row for a worktree whose HEAD is outside the loaded history (hidden or shown-only branch,
  truncated history), e.g. pinned at the top
- Select a worktree WIP row to preview its changed files in the details panel without opening
  its tab
- Create a worktree from a fork's pull request: add the fork as a remote (asking first) or fetch
  the pull request's head ref
- In the Create worktree dialog, offer to use or reset an existing local branch that already
  tracks the remote branch, instead of only asking for another name
- Warn when the branch tip a pull request worktree would check out differs from the pull
  request's head (fetch first)
- Move a worktree to another spelling of its own folder (case- or Unicode-form-only rename)
  through a temporary name
- Force-move a locked worktree (`git worktree move -f -f`) from the Move dialog
- Carry a moved worktree's per-repository settings (graph filter, forge filters) and trust
  decision to its new folder
- Close or move terminal sessions running in a worktree before moving it
- Offer to close the other tab before Move as well, or confirm before closing a tab that has an
  unsent commit message
- A Count now action on a worktree row whose changes were too large to count
- Count split-index and sparse-index worktrees by their real entry count (a split index's header
  counts only its own part)

### Git LFS

- Offer Track in Git LFS from file history rows, the diff header's file buttons and the palette
- Disable "Only this file" when a broader pattern already covers the file (attribute lookup in
  the background)
- Warn when a folder pattern would also catch a nested .gitattributes in that folder
- Say on the Git LFS page when the file or byte cap left some nested .gitattributes unread
- Read LFS patterns from untracked nested .gitattributes and from core.attributesFile too
- Open a nested .gitattributes or info/attributes from its pattern's row on the Git LFS page
- Untrack a nested file's pattern by editing that file (git-lfs only edits the root one)
- Untrack lines that `--filename` wrote with backslash-escaped glob characters (`git lfs
  untrack` leaves them): remove them directly
- Offer to convert files already committed (`git lfs migrate`) after tracking their pattern
- When initializing fails because a non-LFS hook already exists, offer to merge or overwrite it
  (`git lfs update --manual` / `--force`)
- Initialize with LFS when creating or cloning a repository
- File locking: lockable patterns (`track --lockable`), locking and unlocking files, and showing
  who holds locks
- Check for a minimum git and git-lfs version and say clearly when either is too old
- After auto-fetch, also download the LFS files of the checked-out branch's upstream, so a later
  pull doesn't wait on them
- A per-repository switch to leave LFS downloads out of auto-fetch
- Warn in the main-window banner when git-lfs is installed but too old for the repository (no
  filter-process support), next to the not-installed case
- Check for git-lfs again when the window regains focus, so the missing-git-lfs banner clears
  after installing it without opening Preferences
- LFS badges in file history rows, stash file lists and the palette's file search
- Say in the diff's LFS summary whether the object is downloaded locally, and offer to fetch it
- Preview LFS-stored images or text (the smudged content) in the diff instead of only sizes
- Mark lockable LFS files, and who holds a lock, in file rows
- Mark files that match a filter=lfs pattern but were committed as regular files, in file lists
  and commit details
- Show a failed background LFS download outside the log (for example a status-bar note with Retry)
- Say on the Git LFS page that auto-fetch downloads the checked-out commit's LFS files
- Restore from a commit: compare skip-worktree or assume-unchanged converted (LFS) files by their
  cleaned contents instead of raw bytes
- Restore from a commit: explain a failed LFS smudge (object not on the remote, or offline) and
  offer LFS fetch

### Clone and init options

- Create the hosting-service repository and push on init (Publish to GitHub already covers
  GitHub for a repository with no remote)
- Deepen a shallow clone by date (`--shallow-since`) or exclude what a ref reaches
  (`--shallow-exclude`)
- Widen a single-branch shallow clone to fetch every branch
- Mark the shallow boundary commits in the graph
- Unshallow shallow submodules when an update or reset needs missing commits
- List your repositories on Azure DevOps and Bitbucket Data Center in the Clone dialog
- List repositories for a service that isn't connected yet, signing in through Git Credential
  Manager from the Clone dialog
- Remember the SSH/HTTPS choice and the last account between Clone dialogs
- Search a service's repositories on the server beyond the first 1000
- Mark archived repositories and forks in the repository list, and double-click a row to clone
- Sparse clone in patterns (non-cone) mode, and picking folders from the remote's tree before
  cloning
- Merge a .gitignore template into an existing .gitignore instead of keeping the old one
- More .gitignore templates and licenses (AGPL, LGPL, EPL, ISC, CC0), and a README on init
- A custom initial commit message in the Init dialog
- Size Fetch more commits (`--deepen`) too, and Bitbucket Data Center and the other services
  that report no size (count the remote's objects some other way)
- Cancel a sparse clone too (it runs as its own effect, not through the job's stoppable steps)
- Stop a pull that updates submodules during its fetch (it runs as one `git pull`, which can't be
  split safely)
- Show a clone's progress in the Clone dialog and the taskbar button, not only the status bar
- Prefetch a commit's missing contents in the background when it is selected in a partial
  clone, so its diff opens at once, and download a whole folder's contents for a folder diff
- Download missing contents for the commit diff's line counts, the binary preview and the
  conflict view, which still read only what is there
- Offer to turn a partial clone into a full one (`git fetch --refetch` without the filter, or
  dropping the promisor settings after a full fetch)
- Reload a partial clone's commit file list after a download, so a rename it listed as a delete
  and an add (its contents missing) shows as the rename

### Sparse checkout

- Show in the toolbar or status bar that the worktree is sparse, and notice sparse changes made
  outside Glowfly on a plain refresh
- Explain when an opened or restored path is outside the sparse checkout, and keep left-out
  files out of the palette's file finder
- Add or remove a folder from the paths from file rows and the commit file tree
- Warn when a folders-mode path names a file, or a folder that doesn't exist at HEAD
- Refill the form from the kept list after Turn off, so turning it on again starts there
- Convert the list between folders and patterns when switching the mode
- Sparse index support (needs libgit2 to read index.sparse directory entries)
- Palette entries that reapply or turn off sparse checkout directly

### Repository management

- Open all for the Recent list and for the Find repositories results
- Fetch or pull across repositories from the welcome screen and for the Find repositories results
- Ask for credentials during a fetch or pull across repositories (one prompt at a time) instead of failing
- Retry only the failed repositories of a fetch or pull across repositories
- Remember scan results between sessions, and notice new repositories in project folders
  without looking again
- Scan options in Preferences: how deep to look and which folder names to skip
- List bare repositories found by the scan
- Drag to reorder favorites in the repository menu and File > Open recent

### Repository dropdown

- A filter field in File > Open recent too, once the list is long (only the toolbar menu has one)
- Ctrl+O shown beside Open repository… once menus have a shortcut column
- Remove missing entries… in the macOS native File > Open recent menu as well

### Git engine choice

- Warn when the chosen git is a minimal install with no shell for hook scripts (on Windows, no sh.exe next to it)
- Show the OpenSSH version the chosen git will use
- Find gits bundled inside other applications and IDEs (versioned app folders) during discovery
- Type or paste a git path in a text field as well as Browse…
- Choose a git per repository (e.g. a WSL git for repositories under `\\wsl$`)
- Re-check the chosen git's version automatically when it is upgraded while Glowfly runs (Look again covers it by hand)
- Run every command of a multi-command job with the git it started with, even when another git is chosen meanwhile
- Offer other discovered gits in the banner, not only the PATH one, when the saved git is gone
- Offer the PATH git when the saved git runs but is older than the PATH one
- Translate the git banner's own text (GitInfo::banner in glowfly-core is English)
- Notice the saved git disappearing during a session without Look again (e.g. when a job fails to start it)
- Return to the saved git automatically when it comes back (e.g. a removable drive is reconnected)

### Worktree tabs side by side

- A tab group context menu: close the group, close the other groups, move the group to the start
  or end
- Collapse a tab group to one tab showing a count
- A modifier-drag that takes one tab out of its group or drops it into another group
- A repository name chip at the start of each group in the tab strip
- Strip colors chosen per repository, shared with the repository manager's group colors once
  those exist (#239)
- Save each tab's group in the tabs file so the startup read isn't needed
- Keyboard shortcuts to jump to the next or previous tab group
