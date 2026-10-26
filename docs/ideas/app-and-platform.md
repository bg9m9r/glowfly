# Ideas: App, preferences and performance

Preferences, notifications, zoom, logs, onboarding, the welcome screen, performance and new directions. Untriaged; see [README.md](README.md).

### New directions

- Absorb and split: drag hunks from the working tree onto existing commits (fixup and
  autosquash), and split a commit by dragging its hunks into new commits; offline and exact
- Line history: select lines and follow their history across renames and moves (`git log -L`,
  move detection), with a slider through the versions of that block
- Bisect assistant: good and bad shading on the graph, Test this commit buttons, an optional
  script that runs each step, and a final report with the culprit's diff
- Repository health page: size, largest blobs, LFS candidates, stale branches, unpruned remotes,
  commit-graph and multi-pack-index state, fsmonitor and untracked cache, each with an
  explained one-click fix and `git maintenance start`
- Compare two refs: merge base, commits unique to each side, the combined diff and predicted
  conflicts, with a merge that uses Preview before you act
- Patch email: format-patch with a cover letter, send-email set-up, and applying a mailbox
  series as a series
- Trailer composer: pickers for Co-authored-by, Signed-off-by, Fixes and Reviewed-by from the
  repository's history, a Conventional Commits helper with scopes from the changed paths, and
  per-repository message rules
- Offline pull request review: fetch a pull request's refs and threads, review and comment
  offline, and send the queued comments and approvals when back online
- Accessibility as a headline: screen-reader support through AccessKit everywhere, a keyboard
  path for every operation, reduced motion, and colour-blind-safe lanes with patterns
- Monorepo tools: history and graph filtered to a folder, a CODEOWNERS overlay in file lists
  and diffs, and editing the sparse-checkout cone from the file tree
- Daily work log: a private, local summary of the day's commits, branches and pull requests
  across repositories, to copy into a stand-up
- Timeline graph: an optional left-to-right graph on a true time axis, branches as labelled
  tracks, zoomable from years to minutes
- Native platform feel: native title bar and menus on macOS, the system accent colour as an
  option, Windows jump lists, the Linux portal file picker, and a portable mode
- An identity from the name: the lattice as a quiet motif in empty states, the icon's green and
  sand in the palette, a vine lane style, and plain descriptive feature names
- Explain mode: a toggle that shows, in tooltips and confirmations, the git command each action
  runs and what it does in one plain sentence

### Preferences structure

- Ask before quitting the app (window close) while a Preferences page has unsaved edits
- Ctrl/Cmd+S saves the page and Esc on the Save / Revert bar reverts
- Commit signing text fields (program, allowed signers file) on the Save / Revert bar instead
  of saving on leaving the field
- "Saved" waits for the git config write to succeed and shows the error inline when it fails
- Integrations: rename or reorder accounts, and a per-account Test connection button
- Rename the prefs.ui.date_* and prefs.tools.terminal.* translation keys to match their new
  pages
- A search field over all Preferences pages
- Remember the last Preferences page between openings
- Highlight the unsaved fields on a page and in its nav row

### Notifications

- Behind alerts for other local branches with upstreams and for inactive tabs, batched into one
  alert when several have new commits
- Pull request activity alerts (new pull requests, review requests, finished checks)
- Choose which events go to the desktop (job successes, job failures, behind alerts) instead of
  one switch
- A Pull button on the desktop behind alert, like the toast's
- Desktop notifications for results outside git jobs (interactive rebase, pull request creation
  and other hosting-service calls)
- Turn desktop notifications off automatically, or offer to, when the OS refuses the confirming
  notification
- Click a Windows notification from the notification centre after its popup closed (needs a
  registered COM activator, ToastActivatorCLSID on the shortcut)
- Portable Windows copies: register Glowfly's app id (an HKCU AppUserModelId key with name and
  icon) so their notifications stop showing PowerShell
- A taskbar pin made from an install before the app id doesn't group with the running window
  until re-pinned; offer to fix the pin
- On Wayland, raise the window with the notification's xdg-activation token instead of only
  flashing it
- Wait for clicks on more than 8 desktop notifications still on screen (later ones aren't
  waited on)
- Post macOS notifications through UNUserNotificationCenter instead of the deprecated
  NSUserNotification API
- Select the commit or open the log entry a desktop notification was about, not just its
  repository's tab
- Close a toast with Esc or a shortcut while its x has keyboard focus or the pointer is over it
- Report macOS notification delivery errors (the synchronous send's error is only logged)

### Zoom

- Fine-grained zoom: Ctrl+mouse wheel, a slider in the status bar, custom percentages
- Offer to show a hidden details panel when the WIP row or a commit is selected
- Keep the trust chip and conflict-prediction indicator visible while the toolbar is hidden
- A palette entry and key for showing or hiding the activity log
- Per-repository panel layout
- A toast on entering full screen that names the key to leave it
- An opt-in setting to reopen in full screen when the app was closed in full screen
- A distraction-free full screen that also hides the toolbar and side panels, restoring them on leave
- A Leave full screen button in the status bar or title bar while full screen
- Show a hidden panel's edge-strip tooltip beside the pointer, not at the window's top-left corner over the toolbar

### Logs

- Search and filter inside the activity panel and the error and performance tabs
- Show hook runs (pre-commit, pre-push) as their own titled entries in the activity log
- Show each entry's time in the current session (on hover or in a column)
- Settings for the slow-operation threshold and the log size limits
- Record how long queued effects wait behind a repository's queue, and time the stats and path-search workers
- Leave time spent waiting on a credential or passphrase prompt out of git job timings
- Rotate the error and performance logs by day as well as by size
- Keep two Glowfly instances' activity logs apart (one file per instance or a lock during trims)
- Open a log file in the system's text editor from the panel
- Compress the exported logs bundle (zip) when the logs grow large
- Offer to leave repository paths and command output out of an exported bundle before sharing it publicly
- Name the OS release and the graphics backend in the export header, not only the platform and architecture
- Copy a single line of the error and performance tabs from a row menu
- Copy one run from an open routine group's sub-rows
- A palette entry to copy the current log tab
- Offer to attach the exported bundle when choosing Help > Report an issue

### Onboarding

- A hands-on tutorial in a practice repository (make a branch, commit and merge in a sandbox)
- A Check again button on the welcome's git step (git is looked for once, at startup)
- Choosing a theme in the welcome itself (it links to UI Customization for now)
- A welcome step that names the first profile or sets up signing and SSH keys
- Tour steps for tabs, the terminal panel, the activity log and the status bar
- Keep tour progress across restarts
- Highlight the toolbar's Commands button on the tour's palette step
- First-use tips in context (first diff, first conflict, first pull request panel)

### Performance

- Re-read only the submodules a path read touched instead of the whole submodule list
- A Preferences control to show the slow-status note again or turn it off
- Report status read timings in the activity log (debug)
- Refresh `Snapshot.collisions` on status reads so twins can be marked without scanning the index in the worker
- Read the open tab's own status after its graph for very large working trees, showing the WIP
  panel as loading meanwhile
- Word the slow-status note for another worktree's background count by that worktree's name
- Use the index comparison for Glowfly's own stage, unstage and discard refreshes instead of a whole status read
- Pack the index memo's paths into one buffer so indexes past `MAX_INDEX_MEMO` entries can be compared too
- Sweep submodules' and nested repositories' work trees in the Windows safety poll (it skips nested `.git` folders)
- Report the Windows sweep's duration and counts in the activity log (debug)
- Turn the file system monitor off again (`git fsmonitor--daemon stop` and unset core.fsmonitor), e.g. on a repository Preferences page
- Probe `git version --build-options` for the fsmonitor daemon when git is detected, so builds without it get no Turn on button
- Stop the repository's fsmonitor daemon before Glowfly removes or moves a worktree on Windows
- Check that Glowfly's libgit2 index writes (staging, discard) keep git's fsmonitor token and untracked cache extensions, and re-record them if not
- Say in the slow-status note that the monitor setting applies to every worktree of the repository, or set it per worktree under extensions.worktreeConfig
- Link to Watchman's install page in the Linux slow-status note
- Offer the monitor when a later status read is slow after the untracked cache was turned on (the note shows once per repository)
- Show whether the fsmonitor daemon is watching in the status bar or Preferences
- Lend pooled repositories to more reads (full load, blame, file history, compare, conflicts) once their costs are measured
- Notice changes to config files pulled in by `include.path` / `includeIf` before lending a pooled repository (only the main config files are stamped; the 5 s idle limit covers the rest)
- Hold the repository pool for git commands typed into the built-in terminal (`git gc` there within 5 s of a read leaves the old pack on Windows)

### Welcome screen

- Grow the header's vine by one commit node per completed step
- Animate the progress bar's fill and the step change
- Arrow keys move the focus between the Start step's cards
- Give each start card its description as the accessible description, not only its title
- A Recent repositories card on the Start step when Glowfly already knows repositories (Show
  the welcome again)
- Greet with a typed name the user skipped saving, or with the active profile's name
- Hide the lattice drawing under a reduced-motion or plain-look setting, if one is added
- A visible scroll hint when the Start step's body scrolls in a short or narrow window
- Focus the Name field when the welcome opens and git has no name yet
- A busy mark (spinner) in the shared dialog footer while an action runs, for the identity
  write and other dialogs
- Offer Dialog::header to other dialogs that want a tinted band (see the tinted header band
  idea under Dialogs and text fields)
- A dialog whose footer has only its dismiss button grows by the item spacing on the right
  (the empty space after it); the welcome leaves it out (set width), the others still have it

### Per-area effect metadata and generated dispatch

- Fold the diff effects (LoadDiff, HighlightDiff, LoadDiffGaps, LoadBinarySides) into a `DiffEffect` area, with HighlightDiff's cancel ticket from `Runner::run`
- Fold RunGit, Pick and UndoRedo into a job effect area, with the fetch-and-push queue rule and RunGit's stop key beside it
- Fold LoadRepo, LoadStatus, LoadIndexStatus and LoadDetail into a load effect area
- Fold LocalOp, Revert, RestoreFile, WorkFile and LoadTrackedPaths into a working-tree effect area
- Fold SaveSettings, SaveRecent, SaveTabs, CheckRepoFolders and ReadTabGroups into an app-files effect area beside runner/saves.rs, with the Save sandbox rule on it
- Fold the remaining flat effects (Watch, SweepWatch, AnswerPrompt, StopGit, Forge, Stats, SearchPaths, VerifySignature, FetchAvatars, LoadFileHistory) into areas, so the flat arms in names.rs, scope.rs, sandbox.rs and execute.rs go away
- Move panic recovery (`Recovery::of` in runner/guard.rs) into a per-area hook beside each `Execute` impl
- Move `Runner::run`'s special routing (terminal, index worker, stats, path search, log worker, stops, watcher, prompts) into an `Execute`-side route hook
- Give every update handler the same (st, msg, now, fx) signature, so the message table needs no argument tags
- The module lists in update/mod.rs, runner/mod.rs and effect/mod.rs are still one-line merge hotspots: folders per area or generated lists
- Move state/*_tests.rs, runner/*_tests.rs and ui/*_tests.rs (tabs, toolbar, list rows) into test folders the same way
- Move the test files declared from their handler or view with `#[path]` (app, blame, dashboard, logs, patch, status_tip; ui layout, sidebar, widgets) under the folders' `mod.rs` lists where their parent's private items allow

### Per-tab state in one RepoSession

- `Gen::close()` / `Gen::next()` helpers in place of the hand-written `view.take()` + `generation += 1` pairs in each area's close and request functions
- A `RepoSession` query for "a central view is open", so `graph_shown`, `layout::tool_view` and ui/central.rs read one list instead of three kept in step
- Closing a parked view in place (`rebase::panel_slot`, tool_compare's `take_if`, `gitflow_resume::store`) doesn't bump its counter: harmless because unpark bumps, but a `Gen::close` would make it uniform
- `RangeDiffView` keeps its own `generation` beside `RepoSession::range_diff`'s counter: fold the two into one
- `LoadKind::Open` closes the views one area at a time (range_diff, conflict_files and diff_preview aren't in that list): a `RepoSession` reset hook could close them all in one place
- Move the Create issue form out of `st.modal` into the session if dialogs ever become per-tab, so `issue_form` leaves `Parked`
