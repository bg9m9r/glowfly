# Trellis backlog

The items behind [ROADMAP.md](ROADMAP.md), which holds the goal, the milestones and their status.
What Trellis does today and what is planned next, roughly in priority order: the current
milestone's section first. Each item is a target to design, not a finished spec. A top-level
item is the unit of work: it is ticked when its core behaviour works and every sub-item is done
or moved to [Ideas](#ideas). A substantial new feature becomes its own top-level item; smaller
ideas, polish and follow-ups go to Ideas as plain bullets. [Later](#later) holds only triaged
Milestone 2 work, built once the open items are done; nothing in Ideas is built until a triage
moves it.

Want to help? Pick an unticked item, open an issue to discuss the approach, then see
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Recent changes

The last 20 completed TODO items, newest first: when (local time), which item, and what was
done. Add a line at the top when ticking an item, and drop the oldest past 20.

- 2026-10-21 13:46 · **Search and filter**: background commit index of large repositories' whole history, so search counts, Enter, Filter and Keep searching cover every commit and load straight to the next match, with review fixes applied (`b733326f`)
- 2026-10-21 13:41 · **SSH key management**: Keys named in ~/.ssh/config or by Use existing are matched against the listed keys on disk so a link, short name or other case lists once, and the Windows OpenSSH switch lands on main (`d817ddba`)
- 2026-10-21 12:06 · **SSH key management**: Preferences > SSH lists the keys ~/.ssh/config and its Includes name (with an In ssh config chip, and the entries it can't list with the reason), shows keys only the agent holds with Copy public key, and on Windows offers Use Windows' OpenSSH (global core.sshCommand) when git runs its bundled ssh, with Use Git's own ssh to undo (`d817ddba`)
- 2026-10-21 11:49 · **Rebind keyboard shortcuts**: Preferences > Keyboard gets a Repository commands group where fetch, pull, push, create branch or tag, stash, pop, file history, blame, the operation journal, open in terminal and show folder can each be given a key that runs the palette command while no text field has focus, with blocked commands logging the toolbar's reason (`f39befe2`)
- 2026-10-21 11:09 · **Stacked branches**: Push stack and Create pull requests for the stack land on main, with each layer's push outcome logged to the pushed repository's tab rather than the active one (`989e237b`)
- 2026-10-21 09:48 · **UI review polish**: Dialog bodies, Preferences and the File view get an always-visible scroll cue, About folds its third-party notices behind their own dialog, tool view headers close with a ghost × with an Esc tooltip, the rebase editors and Keyboard preferences use the standard small buttons and weak ×, the commit summary counter sits beside the field turning amber past 50 and red past 72, and the status bar joins only the parts present with counts matching the filtered lists (`5360df2b`)
- 2026-10-21 08:29 · **Stacked branches**: Push stack sends every layer of a stack in one job, with upstreams for new layers and a ref-specific --force-with-lease only for layers a restack rewrote, leaving layers behind or diverged from the remote alone and logging each layer's outcome; Create pull requests for the stack pushes first, then opens one create dialog per layer bottom first, each based on the layer below, with Skip and a step line, skipping layers that already have one (`989e237b`)
- 2026-10-21 06:34 · **Stacked branches**: Trellis recognises which local branches are built on which, draws them as an indented tree in LOCAL with a restack hint on layers that are behind, and restacks them with `git rebase --update-refs` from the branch menu or after an amend, undoable as one entry (`afec3859`)
- 2026-10-21 06:13 · **Zoom**: Full screen toggles with F11 (Ctrl+Cmd+F on macOS), a palette entry and a View > Full screen item, with the drawn title bar adapting while full screen, and a hidden side panel comes back from a thin edge strip with the panel widths kept across restarts (`65d505fa`)
- 2026-10-21 05:58 · **Profiles**: Folder rules give every repository under a folder a profile's identity through includeIf.gitdir entries in the global git config pointing at include files Trellis writes, and Preferences > Profile, the commit box and Repo > Identity show where the open repository's identity comes from (`9053f2a0`)
- 2026-10-21 05:05 · **Clone and init options**: Fetch full history sizes the download and asks before 500 MiB or more, a running fetch gets Cancel in the status bar that stops git's whole process tree and cleans up its locks and temporary packs, and diffs, the File view and blame in a partial clone report missing contents and download exactly those objects (`a38b630b`)
- 2026-10-21 05:04 · **External diff and merge tools**: With no program set in Preferences, Open in external diff tool and Resolve in external merge tool run the tool git config names (diff.tool, merge.tool, their cmd or path, or a built-in one through git difftool and git mergetool), gated by repository trust, and Preferences > External Tools shows which tool that is (`492a939a`)
- 2026-10-21 03:49 · **Keyboard shortcuts**: The Unstaged and Staged lists take the keyboard focus as one Tab stop with a ringed current row, Up/Down open the file above or below, S and U work there and on a file still opening, and after S or U stages or unstages the open file the view moves on to the next file in its list (`7cf3838e`)
- 2026-10-21 02:56 · **Multi-select commits**: A selection that isn't one line of history lists each commit's own changes newest first with the gap named in the details panel, picks and reverts that include merges ask for one mainline parent for all, a commit that changes nothing is skipped and named, and Cmd/Ctrl+Shift+A goes back from a multi-selection to the focused commit (`4d0edecd`)
- 2026-10-21 01:55 · **Graph columns and loading**: A failed load for more history stays in the row below the last commit with its reason and Try again, and runs again by itself after 3 s and 10 s; while a failure is held, Load more tries the same load again. A filtering search whose matches don't fill the graph loads older history by itself in doubling batches (five per query, only with Load more while scrolling on), and the no-matches pane says how far it looked, with Keep searching (`e172a79c`)
- 2026-10-21 00:49 · **Design system**: One set of tokens for every theme, custom ones included: a corner scale (2 to 12 px, pills, nested radii), Float and Dialog shadows, an outside accent focus ring (inset in packed rows, a halo on text fields), hover, pressed and latched layers, an overlay surface for menus, tooltips and toasts, five semantic tones and motion timings; the button family (primary, tonal, secondary, quiet, danger, icon-only, split) is built on them, Commit and its menu become one split button whose caret turns amber while hooks are skipped, Stage all turns tonal, every view's radii, focus rings and icon sizes move onto the tokens with a lint keeping them there, and docs/COLOUR.md lists the derived colours and their contrast floors (`a201c080`)
- 2026-10-21 00:19 · **Range-diff**: A local branch's menu compares it with its version before the last rebase (a remote branch's, before the last force-push), found in its reflog and cut at the mainline; the multi-commit menu range-diffs two commits and the journal preview a state with its branch now; the Range-diff view lists each commit pair as Same, Changed, Dropped or New with a changed pair's interdiff beside it, stacks in a narrow window, and refuses with reasons (no earlier version, no shared history, over 300 commits a side, git before 2.19) (`90e9ba12`)
- 2026-10-21 00:09 · **Custom commands**: Git and shell actions defined in a `commands.toml` file (yours in the config folder, or a repository's committed `.trellis/commands.toml`) appear in the command palette with a Reload entry, in a Commands submenu of the commit, local branch and file menus, and on their own keys; `{branch}`, `{sha}`, `{files}` and `{repo}` are filled from the row (one git argument each, single-quoted for sh), shell commands run under POSIX sh (Git for Windows' `sh.exe` on Windows), a command can ask first, a repository's own commands wait for the trust prompt until it is trusted, and file mistakes are logged with their line and toasted once (`668d84fc`)
- 2026-10-20 22:57 · **Repository trust**: A trusted repository is asked about again when its hooks, filters or other run-on-open config change, worktrees and submodules Trellis creates inherit their parent's trust, background jobs are gated by the repository they run in, and built-in terminal shells and the LFS and Sparse Checkout pages' git reads wait until the repository is trusted (`810dc550`)
- 2026-10-20 22:55 · **Preview before you act**: Preview… items in the commit, multi-selection, branch, remote branch and edit menus open a dialog that works out, without changing the files, index or branches, the history a merge, rebase, reset, cherry-pick or drop would leave: where it stops and on which files, the commits that move, are copied or arrive, and the commits no branch or tag will reach any more; Apply runs what the direct item runs (soft, mixed or hard for a reset), refused once the branch moved, with Preview again (`00ea49c3`)

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
  Bitbucket Data Center, Azure DevOps, Jira Cloud, Jira Data Center, and Trello. Each supports listing
  and creating pull requests where applicable, listing issues and mentions, creating a branch
  from an issue, and connecting through Git Credential Manager or a token.

---

## Milestone 1: first release

The [UI review of 2026-10-18](docs/reviews/ui-2026-10-18.md) found these before a first
release; each finding there names the snapshots and code it was seen in. Its minor findings
are in Later > UI review polish.

The look comes first: the open items below are in the order to build them, the visual
foundation (spacing, fonts, palette, type) and the views built on it before the rest.

- [x] **In-progress operations stay visible and safe**: a rebase, merge, cherry-pick, revert,
      am or bisect in progress shows in a strip under the toolbar (or a status-bar chip) whatever
      is selected and whether or not the details panel is shown; clicking it opens the WIP view
  - Abort (rebase, merge, cherry-pick, revert, am), Skip and Reset bisect ask first, naming
    what is lost ("The 2 files you resolved go back…"), with a danger-styled button set apart
    from Continue; stash the resolved state first where that makes Undo work
  - The merge banner gets a primary "Commit merge" (disabled with the reason until every
    conflict is resolved); the summary is prefilled from `MERGE_MSG` and the commit button says
    "Commit merge"
  - The conflict view's header has one primary "Save and mark resolved" with progress ("1 of
    2 conflicts resolved"); Save with markers, Open in merge tool and Reset picks… go in a ⋯
    menu, Reset picks asks when picks exist, and the duplicate buttons in the binary view go
- [x] **Dialogs keep what was typed**: a click on the backdrop never closes a form dialog;
      Esc on an edited form asks "Discard your changes?" (or the draft is kept and restored
      when the dialog reopens); only information and confirm dialogs close on a backdrop click
      (`ui/modals.rs`)
- [x] **No missing glyphs**: the sidebar's ahead/behind arrows (`2□`) are painted as shapes,
      with a snapshot of a branch that is ahead and behind; a test walks the UI's string
      literals and label templates and fails on a character the bundled fonts can't draw
- [x] **Readable controls in every theme**: text fields get a 1 px border (at least 3:1) and a
      focus ring; checkboxes and radios are always-visible boxes and circles with the mark
      inside; the modal backdrop dims about the same in light and dark and dialogs get a
      border; muted text, chips (the HEAD pill, tag pills, selected toggles) and selection
      colours reach 4.5:1 for text in light, dark, sand and high contrast
- [x] **Truncation with an ellipsis and the full text on hover**: one helper for every
      single-line label that can be cut (the graph's message and body preview, resized columns,
      dates, sidebar issue titles with their state chip reserved first, the search placeholder,
      the update pill, repository names in repo sync); column headers keep a minimum width so
      headers can't run together ("GRAPHCOMMIT MESSAGE"); actionable notices (fetch this remote,
      trust for LFS) wrap instead of being cut; translated header buttons move into a ⋯ menu
      instead of being cut to "Neu…"
- [x] **Menus name their object once**: the tag, remote, remote-group and multi-commit menus
      get the branch menu's header-and-short-verbs treatment; object menus are grouped by intent
      (branch and tag, integrate, rewrite, copy, delete) with a Copy ▸ submenu; every item that
      opens a dialog or confirmation ends in "…"; commits are named by summary, not a bare SHA;
      the reset submenu says what each mode keeps ("Hard: reset files too; changes are stashed
      and Undo restores them")
- [x] **Dialog frame and button styles**: one shared dialog frame (16–20 px padding, title,
      body, footer with buttons in a fixed order, two standard widths with content-driven
      limits, long-content dialogs resizable) and three button styles (primary filled with the
      accent, secondary with a border, danger filled red; disabled is a neutral fill with weak
      text, never an outline) used by every modal, panel and preference page
  - [x] Enter runs the primary action and Esc cancels in every dialog; the primary button (the
        safe one for a danger confirm) has focus when the dialog opens
  - [x] `Msg::Confirm` gets a danger kind; Delete, Discard, Force delete, closing tabs with unsaved
        work and the like use it
  - [x] Losing unpushed commits, stashes or changed files for good (remove a submodule, force
        remove a worktree) needs an acknowledgement checkbox before the button enables
  - [x] Cancel and Close come from the translation tables, not literals
- [x] **Comfortable look**: the UI aims for comfortable and pleasing over dense. List rows
      (graph, sidebar, file lists) about 36 px; 16–20 px padding in panels and dialogs on an 8 px
      spacing grid; Noto Sans for the interface and JetBrains Mono for code, SHAs and paths
      (bundled, OFL, with egui's fonts as fallback); a refreshed palette with one teal accent
      for accent, primary and focus, lifted panels, and amber kept for "in progress"; the light
      theme tuned on its own rather than inverted. An opt-in compact density in Preferences at
      most, never the default
- [x] **Type scale and text styles**: a five-step scale with 14 px body text and an 11 px
      floor (nothing that carries information smaller: the 0/72 counter, "Esc returns to one
      commit", hunk buttons, welcome-screen links, LFS Untrack); section headers set by
      weight; monospace only for SHAs, paths, code and fingerprints (not the compare header or
      activity log prose);
      sentence case for every label and button; one section-header style; one link colour
- [x] **Restyled toolbar and rich tooltips**: the toolbar keeps a label under each icon
      (outline icons, even spacing, a rounded hover fill) and every button gets a rich tooltip:
      a header with the action's title and a weak line under it (the shortcut as keycaps, or
      the state: "main is 2 ahead, 0 behind", "4 files changed", "Last fetched 2 minutes ago"),
      then an inset section with what it will touch: the commits a push sends or a pull brings
      in, the stash Pop applies, the commit Undo reverts. Buttons with nothing to preview show
      the header and one sentence on what they do
  - One shared tooltip widget, used across the app, not only the toolbar: raised, rounded
    corners, soft shadow, no pointer or caret
  - Shows after 400 ms, then switches instantly while moving along the bar; 120 ms fade and a
    4 px rise, none with reduced motion; stays on screen at the window edge; keyboard focus
    shows it too
  - Disabled buttons keep their tooltip and say why ("No stashes to pop")
- [x] **History table that carries more information**: 40 px rows with columns for branches
      and tags, graph, message (the widest, with a weak first body line after the summary),
      changes (`+N −M`), commit (author with the short SHA under it) and a right-aligned date
  - Each branch keeps one colour through its lane, its label, a line joining label to node and
    a faint band across the row, so a commit's branch reads without tracing lines
  - Nodes show the committer's initials on a muted fill; merge commits are small solid dots;
    the WIP node is a dashed ring
  - One label per branch with small local and remote marks instead of separate local and
    remote labels; tags get a tag mark; ref labels are soft tinted fills with no outline; a
    "+N" chip when a commit has more labels than fit, and hovering lists them all
  - Selection is a light accent tint with a 3 px accent bar at the left edge; hover is a
    slightly lifted row
- [x] **A calm file header**: the bar above a diff becomes a real header, 48 px on the raised
      surface with a 1 px edge below, and one row at every width (nothing wraps to a second row)
  - Left: a file-type icon, the folder in a quiet colour then the file name in semibold (the
    folder shortens from the left when space runs out, never the name), a status chip with
    line counts (M +24 −9, or the sizes for a binary file), and a chip for the context: the
    short commit with its summary on hover, "Unstaged" or "Staged"
  - Right, always in this order with thin dividers: previous/next change with "2 / 5" (for a
    file with no lines they step between files); a View ▾ button that names the current view
    ("Inline · Hunks") and opens Layout (Inline | Split) and Show (Hunks | Whole file) as
    segmented controls, Ignore whitespace and Wrap long lines as checkboxes, and Expand all /
    Collapse all in Whole file; the file actions with icon and label (Restore…, Open, Show in
    folder) plus the external diff tool as an icon; Stage file or Unstage file as the one
    filled primary button; Close as a × with "Esc" in its tooltip. Groups that don't apply
    drop out and the rest keep their places
  - Every control 28 px tall, 6 px radius, transparent until hovered, 13 px text and 16 px
    outline icons; every icon-only control has a tooltip in the shared style
  - Working-tree files get Open as well as Stage file
  - In a narrow panel the labelled file actions fold into a ⋯ menu
  - A changed image shows before and after with dimensions and sizes instead of "Binary file
    not shown"; other binary files show both sizes
- [x] **Line counts in red and green**: `+N −M` for each changed file (with a small bar sized to
      the change), each commit row and the commit details header, and the WIP row (one line:
      "Uncommitted changes", then M 3 · A 1 · D 1 chips with tinted fills and a right-aligned
      `+48 −17`); added green, removed red, modified amber, matching the status letters;
      computed off the UI thread and cached per commit
- [x] **Toggles look like toggles**: Diff/File/Blame, the dashboard tabs and the search mode
      are segmented controls (the diff's own view switches move into the file header's View
      menu); hunk buttons at body size with Discard set apart
- [x] **Full-row selection in file lists**: the commit panel's and file history's file lists
      select the whole row with the same fill and accent bar as the graph and sidebar
- [x] **One style for empty, loading, error and banner states**: an `empty_state(icon, title,
      hint, action)` widget for empty panes (the dashboard with Open, Clone and Connect; search
      with no matches with Clear search and a syntax hint; a whitespace-only diff with "Show
      whitespace changes"), loading and errors (warning icon, plain title, Try again and Copy
      details, no "not found:" prefixes); a full-width `banner(kind)` with an icon and
      normal-contrast buttons for operations and the git-too-old notice; error, warning and
      info message styles for every form, with field errors under the field and none before
      the user has typed
- [x] **Layout that gives tool views room**: the commit panel collapses to a narrow rail while
      file history, folder history, blame, conflicts, issues or Git Flow are open, and comes
      back on Close; a hidden sidebar or details panel leaves an edge rail or status-bar link
      to bring it back, and the View menu shows them with check marks; toasts sit below the
      toolbar and line up; the toolbar's buttons don't move with the branch name's length or
      the PR button
- [x] **Disabled actions say why**: a disabled primary button has a one-line reason next to it
      (the rebase panel's pattern) and a disabled menu item a short weak suffix; setup-gated
      items ("Open in external diff tool" with no tool set) stay enabled and open the setting
- [x] **Sidebar create actions**: every section header shows a hover + (and a ⋯ for its menu)
      like ISSUES; SUBMODULES and WORKTREES are always listed, with "Add submodule…" and "Add
      worktree…" in their empty state; REMOTE counts remotes, not remote branches
- [x] **Preferences structure**: Editor is renamed Diff Viewer; one Terminal page for the
      built-in and external terminal; Language and date locale together; General split into
      titled sections; one save model (instant, with a short "Saved"; pages that write repo
      config get a sticky Save / Revert bar and warn on leaving with unsaved edits); the
      Integrations page shows each connected account once and hides the token form behind "Add
      another account"; checkbox rows always have an inline label; rows stay inside the content
      column
- [x] **Complete translations**: the graph column headers, sidebar sections, filter
      placeholder, WIP row, dates, the commit, branch and tag menus, hunk buttons and every
      dialog's Cancel come from the tables; "Discard all" and "Stage all" keep "all"; a test
      renders each panel in German and fails on strings from an English allow-list
- [x] **Clearer prompts and safer controls**: the SSH host-key prompt shows host, key type and
      fingerprint as fields with one sentence of explanation and "Trust and connect" / Cancel
      (Cancel the default), with the raw text under Details; the sign-in dialog labels Username
      and "Password or personal access token"; Skip hooks moves to a ⋯ menu by the commit
      button with a warning tint while on; Kill in the terminal asks while a process runs; repo
      sync gets Stop and per-row Retry and Open, with the full error on hover; the rebase
      panel's Reset becomes "Restore original order"; graph search gets a clear × and a
      fixed-width count

From hands-on testing on 2026-10-19. The behaviour fixes come first; the design items after them
wait for a choice on the design board, so don't plan an item marked "Waiting on a design choice"
until this list says which option was chosen.

- [x] **Trellis's own wording**: rewrite these labels and help texts in fresh words of our
      own, in every language table, the docs and the snapshots: the help for the default branch
      name, the initial-commits warning, core.longpaths and the auto-fetch interval; the "How do
      I renew an expired key?" link; "Automatically group worktree tabs together"; "Exit
      preferences" (say "Back to repository"); the commit button's "Commit changes to N files" /
      "Stage files to commit" (e.g. "Commit 2 files" / "Stage something to commit"); the
      worktree WIP row's placeholder text (use "Uncommitted changes · folder", as in the main row);
      the date settings' "Date word" and verbose names (say "Long date" and "Full date and
      time"), with the default format taken from the system's locale or ISO 8601 instead of
      `MM/DD/YYYY @ h:mm A`; and the option names Ghost branch (say "Quick-branch chip"), Smart
      branch visibility ("Relevant branches"), Solo / Soloing ("Show only" / "Showing only
      name") and Highlight associated rows ("Trace a branch on hover")
- [x] **Stash and carry on**: checkout, pull, merge, rebase, cherry-pick, revert and reset first
      check for staged or unstaged changes; if there are any, a prompt names the operation and
      the file count and offers "Stash and <operation>" or Cancel, plus "Always stash" (a
      preference); one click stashes, runs the operation and puts the changes back when it
      finishes (after Continue or Abort for a rebase or merge that stops on conflicts)
  - The stash is named for what it was for ("Auto-stash before rebasing main onto
    rebase/stepwise"); the toasts name it when it is made and when it is put back or kept,
    with a button that selects it, and its Stashes row has an "auto" marker
  - When putting it back conflicts, say so and leave it in the list; nothing is lost
- [x] **Double-click a branch to check it out**: in the sidebar and on graph labels, through
      Stash and carry on when there are changes; a remote branch checks out its local tracking
      branch, creating it when there is none
- [x] **Errors say what to do**: failed git operations show a plain toast that names the cause
      and the fix ("Your changes to 3 files would be overwritten: Stash and pull") instead of
      raw git text such as "Merge with strategy ort failed"; find where the `<stdin>` whitespace
      warnings in a failed pull's log come from and stop showing them
- [x] **Activity panel opens only when asked**: nothing opens it but the toolbar's Activity
      button, the View menu or a toast's Details; each repository tab has its own log
- [x] **Terminal keeps the prompt's space**: pwsh's "PS C:\path> " loses its trailing space on
      longer paths (the cursor sits right after ">"); reproduce from the raw byte stream in a
      test covering short, long and wrapping prompts and fix the emulator; the space always shows
- [x] **One label for a branch and its copies on every remote**: a same-commit copy on any
      remote (`fork/x` as well as the upstream) folds into the label with the cloud instead of
      counting as "+1"
- [x] **Small fixes from testing**: the terminal tab bar's + is centred and drawn like the ×
      (the repository tab bar's too); the ⚠ badge by the branch picker says on hover what it
      warns about and opens it on click
- [x] **Brand colour and tinted neutrals** (from the colour review, 2026-10-19): the accent moves
      from teal to the icon's lattice green, with the icon's sand as a second brand colour.
      Accent: dark #4dc588 (OKLCH 0.74 0.14 158) with dark ink #0f1a14 on primary buttons;
      light #14764a (0.50 0.11 158) with white. Sand carries tags (replacing today's tag
      colours): dark text #e4d6b6 on its tint, light #6f562a on #e3dac2. Neutrals keep chroma
      at or below 0.015. Dark, tinted green (hue 160): chrome #0f1612, base #151b18, panel
      #1b221e, raised #262e29, hairline #2d3530, selection #244534, text #e0e6e3, weak #a4aea8,
      faint #7f8983. Light, warm paper (hue 90): chrome #ece9e1, base #f9f7f1, panel #fdfcf9,
      hairline #dcd9d0, selection #cfeddb, text #161c19, weak #4c5550, faint #68716c. Status
      colours move off the accent: added #7bc85f / #357518, removed #ea6a64 / #b63132,
      modified #edb345 / #a06604, info #60aaf3 / #1f68bc, operation in progress #a689f1 /
      #6d47b8 (dark / light; in light, chip tints at about 10% so chip text reaches 4.5:1).
      Separate token families in the theme code (status, branch, repository, attention), and
      contrast tests that also assert the reserved hue bands below. Write the colour rules in
      docs/COLOUR.md.
- [x] **Branch-led colour** (the base of colour direction C, Vivid, chosen 2026-10-19): each
      branch keeps one colour wherever it shows (graph lane and label, a sidebar dot, the
      Branch picker, a line under the active tab, the details header), picked from its name so
      it stays the same between sessions; only main uses the accent; two lanes side by side
      never share a hue; right-click a branch for Colour… to pin one; a colour per person for
      avatars; a colour-blind-safe preset in Preferences, with names always beside the colour.
      Palette: P1 tuned after the colour review: the accent band (about 145–175°) and the
      attention band (about 35–60°) are kept out, leaving 14 hues plus main, and neighbouring
      hues alternate between two lightness tiers (dark L 0.70 / 0.80, C 0.145), in this order:
      Rose, Lime, Violet, Orange, Cyan, Magenta, Olive, Azure, Red, Teal (192°), Orchid, Gold,
      Sky, Indigo. Dark: #e77492 #a3cf64 #c5aeff #d78c1a #04d4f1 #fb97db #9ea827 #93c1fe #fea195
      #0ab5b1 #c37fd8 #dfba3b #0aabea #8794f7. Light text and chip tones: #861f43 #4d7002
      #6d4ea8 #6c4302 #057080 #953d7c #4d5201 #2462b1 #a43c33 #005957 #6b2d7d #786101 #005475
      #3b4097. Light graph lines and dots use brighter tones (3:1 is enough for non-text):
      #c04d6e #719c27 #9776db #a86b06 #0b9db3 #c664a8 #7a8201 #498de5 #d86357 #0a8c89 #9e59b3
      #a98904 #0685b6 #646fd1.
- [x] **Vivid colour** (colour direction C, after Branch-led colour), with one job per colour
      kind: status colours appear only as letter badges or +/− signs in diffs and file lists;
      branch colours only as lines, dots and name pills; repository colours only on window
      chrome (its tab, a thin line along the top of the window, the repository list), from a
      quieter set of about 8 hues at chroma 0.08, picked by you, with none as the default;
      a coral used only for "needs you" moments such as a conflict, the one solid-filled
      status (dark #fb794a with ink #22110b, light #bf4306 with white, always with the warning
      icon); toolbar icons on a neutral backing (the accent at 8–10%), never a colour per icon.
      At most three hue families in the chrome at once (accent, repository, coral when needed).
- [x] **Design system**: one set of tokens (surface steps, borders, hover and pressed tints,
      radii, shadows, focus ring, accent and semantic colours, motion) for every theme, and the
      components built on them: the button family (primary, tonal, quiet, danger, icon-only,
      split), toolbar groups, text fields, tabs, menus, tooltips, chips, dialogs, toasts and one
      icon set; what has landed (dialog frame, button styles, file header, toggles, chips) moves
      onto them unchanged in intent. Chosen 2026-10-19: the foundations and components boards
      as drawn, open to change once they're seen in the app.
- [x] **Window chrome and toolbar**: the title bar, menu, repository tabs (each with a shape,
      hover state, even widths and an ellipsis), the toolbar's grouped buttons, a new Branch
      icon, and Repository › Branch pickers sized to their names. Chosen 2026-10-19 (option C):
      Trellis draws one title bar with the tabs, a ☰ menu (Alt still opens File, Edit, View…)
      and the window buttons, giving about 60 px back to content. On Windows 11 the title bar,
      toolbar and sidebar sit on the Mica material while the graph and diff stay solid; Windows
      10, Linux and "transparency effects off" use the solid colour. It must keep everything the
      system frame gives: drag by empty title-bar space, double-click to maximise, resize from
      every edge and corner, Snap Layouts on hovering maximise, and the system menu on
      Alt+Space. macOS keeps its traffic lights inset in our bar; Linux keeps the system frame.
      Check text contrast over light and busy wallpapers. Branch icon: A, trunk and branch.
- [x] **History table edges and commit details**: 12 px cell padding and a 16 px inset at the
      table's edges with the headers aligned; the commit details panel gets one clear type
      hierarchy, SHA and parent chips that copy, and file rows with status chips.
      Chosen 2026-10-19: the board as drawn, with commit details option A (roomy, title
      first): an 18 px title, one person row with a relative date (full date and email on
      hover), quiet chips for the SHA, parent and committer, a readable body block, and files
      with status chips and counts beside the name. The lane bands and selection still run edge
      to edge.
- [x] **Dialogs and text fields**: 36 px fields, label-field-help groups, roomier dialog frames
      with a × and size classes, and a fresh look at colour, type size and layout (tinted fields,
      a header band or icon, a side explanation pane); every dialog moves onto them.
      Chosen 2026-10-19 (option 3): dialogs for less familiar operations (submodules, worktrees,
      sparse checkout, LFS, signing set-up and the like) get a tinted side pane that explains
      the operation in a few plain steps, with the form beside it. Everyday dialogs such as
      Create branch use option 1: roomy and neutral, one pane.
- [x] **Terminal panel**: an always-present header, tabs named from the shell and folder,
      + with a shell picker, quiet icon buttons (Clear, Kill, ⋯, Settings to Preferences ›
      Terminal, maximise, hide) and a readable 16-colour palette per theme.
      Chosen 2026-10-19 (option A): one 40 px header on the panel surface; each tab is a pill
      with a shell icon and an automatic name ("pwsh · folder" or the running command), × on
      the active tab and on hover, an amber dot for a shell that has exited, double-click to
      rename; + opens a terminal and its ▾ picks the shell. On the right, icon buttons with
      tooltips: Clear, Kill terminal (asks only while a program runs), ⋯ (Kill all, Rename),
      Settings, maximise and hide.
- [x] **SSH preferences**: key cards with Copy public key, key generation in a dialog, a key
      picker for all repositories and this one, and an agent status pill. Chosen 2026-10-19
      (option A): one column of three titled sections; each key is a card with its
      fingerprint, label and Copy public key, the default key marked, Generate and Use existing
      under the list; "Which key" is two rows with a dropdown each (the keys plus "Let ssh
      decide").
- [x] **Activity panel design**: a list of runs (title, time, result) with their output folded
      underneath and routine fetches kept quiet. Chosen 2026-10-19 (option A): one row per
      operation with a result icon, a plain title, a short summary, how long it took and when;
      a failed run opens in place to show what went wrong, the fix as a button and the command
      output in a quiet block; routine auto-fetches fold into one grey row.
- [x] **Disabled menu items explain on hover**: a greyed label with a small ⓘ and the reason in a
      tooltip on hover or keyboard focus, in every menu. Chosen 2026-10-19: the menus board as
      drawn.
- [x] **Welcome screen**: choice cards with an icon and a line of description, a clearer
      stepper and the tour as a quiet link. Chosen 2026-10-19 (option B, to try): a wider
      (760 px) welcome with a tinted header holding a lattice drawing in Trellis's own style,
      a four-segment progress bar, the user's name from the first step, and the three ways to
      start as large cards side by side; the drawing needs light and high-contrast versions.

What [Milestone 1](ROADMAP.md#milestone-1-first-release-in-progress) needs that tests and CI can
check. Its hands-on criteria (daily use, merging the release pull request) are in ROADMAP.md.

- [x] **Whole-app journey tests**: headless tests that start the whole app (state, update,
      views and the real effect runner with git) on a fixture repository and drive it through
      the UI by visible labels, checking the screen and the repository on disk at each step;
      run on Windows, macOS and Linux CI
  - [x] Clone from a local bare remote, stage a hunk and a file, commit, push, pull
  - [x] Create a branch, merge it, delete it; checkout with local changes
  - [x] Rebase onto a diverged branch, hit a conflict, resolve it in the merge tool, continue
  - [x] Stash and pop; undo and redo a commit and a merge
- [x] **Real-input smoke tests (Linux)**: a CI job that runs the release build under Xvfb and
      sends real X11 mouse and keyboard events with xdotool
  - The first click after the window gains focus, typing in the commit box, a keyboard
    shortcut, a context menu, a drag, a resize, and a 150% scale factor
  - Fails with a screenshot of the window
- [x] **Real-input smoke tests (Windows)**: the same checks on the Windows CI runner's desktop,
      through real input events or UI Automation over the AccessKit tree
- [ ] **Input reliability**: the first click after the window gains focus was dropped while
      testing with synthetic input; fix it if the real-input tests reproduce it, else close it
      with their result
  - Windows: `smoke-windows` check first-click covers click to focus and a click after
    SetForegroundWindow; its ::notice lines and first-click.txt give the result (not run
    locally: no desktop)
- [x] **Installer and update checks in CI**: install the MSI and DEB the release workflow built,
      start Trellis from them, uninstall cleanly, and update an older build to the new one
      against a local release feed with checksums

---

## Preferences parity (page by page)

The pages a full-featured Git client offers, and where Trellis stands:

| Page | Trellis | Gap |
|---|---|---|
| General | ✔ General | Missing: delete `.orig` files after merge, show all commits, path to `sh.exe`, extended logging, conflict detection |
| Profiles | ✔ Profile (identity, connected accounts and app preferences per profile, switching) | Follow-ups in Ideas > Profiles |
| SSH | ✔ SSH page (key cards with their marks, a Generate SSH key dialog, Use existing for keys outside ~/.ssh, Which key dropdowns for all repositories and the open one, the local agent with a status pill, add a key to a hosting service) | Follow-ups in Ideas > SSH key management |
| Integrations | ✔ 10 services | PR/issue depth (P2) |
| AI | ✘ | Possible later: opt-in features on the Claude API |
| External Tools | ✔ Editor, diff tool and merge tool (the external terminal is on the Terminal page) | Follow-ups in Later > External tools and Ideas > External diff and merge tools |
| Notifications | ✔ location, behind alerts and desktop notifications under UI Customization | Per-kind choices, click to focus (Later > Notifications) |
| UI Customization | ✔ UI Customization | Repo group colors (wait on repository groups) |
| Commit Signing | ✔ Commit Signing | Signature badges on graph rows and tags (P1 follow-ups) |
| Diff viewer | ◐ Diff viewer (font size, tab size, line numbers, syntax and word highlights, word wrap) | Font family, EOL character |
| Terminal | ✔ Terminal page (built-in: shell, font size, cursor style, dim when unfocused, copy on select, middle-click paste (Linux), Open in terminal in the panel; external: program and arguments) | Font family, line height (Ideas > Built-in terminal) |
| Experimental | ✘ | Git executable selection and version (P3) |
| *Repo:* Encoding | ✘ | Per-repo file encoding for diffs |
| *Repo:* Gitflow | ✔ Git Flow page, Start and Finish, Git Flow panel, branch from issue | Finish options (Later > Git Flow) |
| *Repo:* Git Hooks | ✔ hooks path | — |
| *Repo:* Commit | ✘ | Commit template, push after commit, skip hooks per repo |
| *Repo:* Agents | — | Out of scope |
| *Repo:* Conflict Prevention | ✘ (on/off is under General) | Per-repo enable and target branch rules (P1 Conflict prediction) |
| *Repo:* LFS | ✔ Git LFS page, Track in Git LFS on file rows, LFS-aware auto-fetch | — |
| *Repo:* Sparse Checkout | ◐ Sparse Checkout page | Sparse clone, indicators (P1 Sparse checkout) |
| *Repo:* Issue Tracker | ✔ | — |
| *Repo:* Team | — | Out of scope (cloud teams) |

### UI Customization

Done except the group colors, which wait on repository groups in the repository manager.

- [x] **Theme** dropdown (dark, light, more)
- [x] **Notification location** (bottom left, top right, …), once toasts exist
- [x] **Date/Time section**: date/time locale (system or a chosen locale) and four format
      patterns with live previews: *Date*, *Long date*, *Date and time* (the graph's date
      column) and *Full date and time* (commit details). A blank pattern follows the locale's
      usual one (`MM/DD/YYYY h:mm A` in US English, `DD.MM.YYYY HH:mm` in German), or ISO 8601
      (`YYYY-MM-DD HH:mm`) for a locale Trellis has no row for. Date shows in file history and
      blame rows, Long date in the issue panel and the file history footer, and hovering a
      graph, history or comment date shows its full date and age.
- [ ] **Default group color / reset all to default**,
      for repo groups in the repository manager. (Workspace color belongs to cloud Workspaces,
      which are out of scope.)
- [x] **Keep a repository's worktree tabs side by side**: tabs of one repository's worktrees open
      and stay side by side with a colored strip; a drop outside the group moves the whole group
- [x] **Show toolbar icon labels**
- [x] **Enable spell checking** in commit messages
- [x] **Author initials vs avatars**: real avatars (Gravatar or the hosting service's
      avatar, cached on disk), with initials as the opt-in.
- [x] **Quick-branch chip on the checked-out commit**: a faint "+ branch" chip after the
      checked-out commit's labels when it is pointed at or selected; click it to branch from the
      current branch
- [x] **Trace a branch on hover**: a branch's commits light up while the pointer is on its label
- [x] **Branch visibility in commit graph**: All branches / Relevant branches.
      Relevant branches keeps only the current branch, its upstream, the default branch, and
      branches with PRs.
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
  on start (only the active one loads until selected); rename tabs; tab list with recently
  closed tabs; Ctrl+Shift+PgUp/PgDn move the tab; asks before closing unsaved drafts or
  conflict choices.
  - [x] Drag to reorder tabs; tab context menu (close others, close to the right)
- [x] **Undo / Redo**
  Toolbar Undo/Redo and Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) for checkout, commit/amend, reset,
  merge, pull, rebase, cherry-pick, revert, creating or deleting a branch or tag, stash push,
  pop and apply, Discard all and file discards and restores; per-tab, in-memory history of 20,
  refuses (and says why) when the repository no longer matches or local changes would be
  overwritten. A hard reset or Discard all blocked by local changes offers "Stash changes and
  undo"; undoing a checkout that created a branch asks whether to delete the branch too.
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
    operations then apply only the changes shown, leaving the hidden whitespace changes as they
    are)
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
    File history diffs, Hunks and Whole-file views; also with whitespace hidden, and undoable)
    - [x] Revert selected lines (line selection on commit, comparison and File history diffs)
      and whole files (Revert changes to this file in commit, comparison and File history file
      menus), R for the current hunk or the selected lines, the file's current name from File
      history after a rename, an added file removed rather than left empty, and a three-way
      merge with later edits (conflict markers when they overlap) instead of refusing
  - [x] Next/previous change continues to the next/previous changed file at either end
  - [x] Cache the computed diff rows instead of rebuilding them every frame
- [x] **Line-level staging**
  Select lines in a hunk to stage, unstage or discard (click, Shift-click for a range,
  Ctrl/Cmd-click or a click in the line's gutter to toggle, drag across lines; Esc clears).
  Selected lines show a check box in the sign column and a stronger tint. Working-tree diffs
  only, one hunk at a time.
  - [x] Keyboard shortcuts: S/U stage or unstage the selected lines (see P1 Keyboard
    shortcuts); reverting lines of a commit diff is under the diff viewer
- [x] **Merge conflict tool**
  Three-pane view (ours / theirs / output), take a whole side per conflict, save and mark
  resolved. Opens from a conflicted file in the WIP panel or "View conflicts" in the banner;
  binary, deleted and special files take a whole version.
- [x] **File history and blame**
  File history: commits that changed a file, newest first, following renames, opened from a
  file's context menu in the commit details or WIP panel; each shows that commit's diff of the
  file and links to the graph. Searches back in batches of the commit limit. Blame opens from
  the file history header and the file menus (see Blame gutter). Also the history of a folder
  (palette finder, "History of this folder"), a File view of the whole file as of a commit,
  renames followed forwards to HEAD, and a re-run when HEAD moves.
  - [x] Open file history or blame for any file from the command palette (a file finder), not
    only from a changed-file list. Both history and blame open from the finder
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
  The list also takes exec and break steps, a typed message for the commit a squash group
  combines into, and keyboard control (arrows, Alt+arrows, action letters, X / B, Enter); "Edit
  remaining steps…" opens it on a running rebase's todo (`rebase --edit-todo`, also for one
  started outside Trellis), rebases and graph edits undo in one step back to where the branch
  was, and Reword / Squash / Drop / Continue / Skip have keys and palette entries.
- [x] **Multi-select commits**
  Ctrl-click toggles a commit and Shift-click selects a range of graph rows; Esc returns to one
  commit. The details panel lists the selection; with exactly two commits it lists the files that
  differ between them and opens their diff. Cherry-pick the selection (oldest first) and copy
  their SHAs from the panel or the context menu. A commit can be compared with the working
  directory, an unbroken line of commits shows one combined diff (either way round), Shift+Up/Down
  and Ctrl+A extend the selection, several commits revert in one step, a merge is picked or
  reverted against a chosen parent, and stopped picks and reverts offer Skip commit.
- [x] **Live working-tree updates**
  The active tab's repository is watched with the `notify` crate (Preferences > General, on by
  default): outside edits re-read the status and ref or HEAD changes reload everything, within
  about 0.3 s. Window focus and F5 still refresh as before.
- [x] **Credential prompts**
  Trellis is git's and ssh's askpass helper: HTTPS sign-in (username and password or token in
  one dialog), SSH key passphrases, SSH passwords and host-key confirmations open an in-app
  prompt, with retry and rejected-credentials notices, cancel and a 5-minute timeout.
  Background jobs and hosting-service sign-in never prompt. HTTPS sign-ins can be remembered
  with git's credential helper (setting up the system's store when git has none), OpenSSH
  before 8.4 asks through Trellis on Linux and macOS, and auto-fetch skips a remote that needs a
  sign-in, with a banner to sign in.
- [x] **Branch operations**
  From the local branch menu: rename, set and unset the upstream (also to a branch not on the
  remote yet), fast-forward without checkout, delete local + remote in one step when the
  checked-out branch contains it (undo restores the local branch), push any branch to its
  upstream, push to a chosen remote. From the remote branch menu (left panel and graph labels):
  check out its tracking branch or a new one, fast-forward the tracking branch, and interactive
  rebase, conflict check, compare and set upstream against the checked-out branch.
  - [x] Rename the checked-out branch from the command palette
- [x] **Hide / show only branches in the graph**
  Hide or show local and remote branches, or show only one or more of them ("Show only this"),
  from the left panel's branch menus. Hidden rows stay listed, dimmed, with a marker that shows
  them again; shown-only rows get a marker that stops showing only them ("Stop showing only
  this"); a banner under the filter box says what is filtered, with Show the rest and Show all.
  Saved per repository; refs that no longer exist are dropped.
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
  (detached), a file finder for file history and blame, apply, pop or delete a chosen stash, and stash
  with a message. Entries whose command also has a global key
  show it dimmed at the right (Ctrl+F, Ctrl+W, F5, Ctrl+Z/Y, Ctrl+T, Ctrl+Shift+T and Ctrl+,).
  The list shows at most 200 rows; typing more narrows the rest. A Commands button in the
  toolbar opens the palette. The last 10 plain commands run from it (per session) are listed
  first for an empty query, and break ties between equally good matches for a typed one.
  PgUp/PgDn move a page, Ctrl+Home/End (and Home/End with an empty query) go to the ends, and
  the row under a moving mouse is selected. Applying or popping a stash on top of uncommitted
  changes asks first.
  - [x] File finder: pick any tracked file to open its history or blame it
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
  Restart, a starting message, the exit code and start failures are shown. Each session keeps
  5,000 rows of history (mouse wheel), text is selected by dragging and copied with Ctrl+C or the
  right-click menu (Copy, Paste, Select all, Clear), a repository can have several terminal tabs
  (Kill all asks first), Preferences > In-App Terminal picks the shell, font size, cursor style
  and dimming and can make Open in terminal use the panel, and the panel's state and height are
  remembered.
- [x] **Submodules**
  A SUBMODULES section at the bottom of the left panel (shown when the repository has any) lists
  the index's submodules with a painted marker and a word for each state: in sync, out of sync,
  changes, changes and out of sync, not initialized. The tooltip explains the state and shows
  the URL, the recorded commit and the checked-out commit. Double-click or Open in new tab opens
  an initialized submodule as a repository; Initialize / Update to recorded commit runs
  `git submodule update --init` for it as a job (asking first when it is out of sync, and naming
  any commits inside that no branch or tag contains, which the update would leave behind); Copy
  URL copies the .gitmodules URL. The left-panel filter also filters the section.
- [x] **Worktrees**
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
  - [x] Show worktree WIP nodes in the graph: a WIP row per worktree with uncommitted changes, at
    its HEAD commit (dashed node, "Uncommitted changes · <folder>" with counts, a tooltip, click to open its
    tab; counted with libgit2 on full loads; UI Customization > Show other worktrees' changes)
  - [x] Branches checked out in another worktree marked `worktree` in LOCAL (tooltip with the
    folder), with Checkout and Delete refused everywhere and Open worktree in their menu; create a
    worktree from a remote branch or a pull request (a new tracking branch); palette commands
    (open, create, remove, lock, unlock, move, prune); Remove also deleting the branch on its
    remote and closing another tab that shows the worktree; Move (`git worktree move`, tabs and
    lists follow); the WORKTREES row menu on a worktree's WIP row in the graph
- [x] **Git LFS**
  Initialize, track patterns (file, extension, folder), fetch/pull/push/prune, view
  `.gitattributes`, and LFS-aware auto-fetch.
  - [x] Preferences > Git LFS (repo-specific): status, Initialize, tracked patterns with Untrack
    and a new-pattern field, Open .gitattributes, Fetch / Pull / Push / Prune / Checkout; palette
    'Initialize Git LFS' and 'Configure Git LFS…'
  - [x] Track from a file's context menu: that one file (`git lfs track --filename`), all files
    with its extension, or all files in its folder
  - [x] LFS-aware auto-fetch: fetch LFS objects for the checked-out commit after a background
    fetch, and say so on the auto-fetch preference
  - [x] Honour smudge filters when restoring a file from a commit, by writing through
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
  - [x] Hide tags in the graph (see Hide / show only)
- [x] **Stash improvements**
  Stash selected files only, a stash message field, rename a stash. (Stash with a message exists
  in the palette; hiding stashes is under Later.)
  - [x] Rename (edit the message of) an existing stash, from the sidebar and the palette
  - [x] Stash only staged changes or selected files, with a message, from the palette too
- [x] **Clone and init options**
  Shallow and sparse clone; init with a `.gitignore` template and a license; clone from a list
  of your repos on a connected service. (Creating the hosting-service repo on init is under
  Later; Publish to GitHub covers GitHub.)
  - [x] Shallow clone from the Clone dialog: by commit count or since a date, optionally one branch
  - [x] Sparse clone (choose folders to check out when cloning)
  - [x] Init with a .gitignore template and a license
  - [x] Clone from a list of your repositories on a connected hosting service
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
  - [x] Find repositories in project folders (a dialog that scans the folders you keep
    repositories in, with a filter); Ctrl+Alt+1 to 9 open the first nine favorites (rebindable,
    shown in the sheet, the palette and the welcome rows); drag or Move up/down to reorder
    favorites; "open" tags and Open all favorites; Remove missing entries, and an offer to forget a
    listed repository whose folder is gone; palette commands to favorite the open repository and
    favorites listed first; fetch or pull (fast-forward only) all favorites or every listed
    repository, four at a time, with a result per repository, a "nothing to fetch" case, and
    repositories that aren't trusted skipped
- [x] **Graph columns and loading**
  Column toggles, compact layout and row highlighting are done (UI Customization). Missing:
  - [x] Resize columns: drag the header dividers (BRANCH/TAG, SHA, changes, author, date), double-click to reset, saved in settings
  - [x] Lazy-load more history when scrolling past the commit limit (the General page's "lazy
    loading"); jumping to a pinned branch goes with pinning in Branch operations
- [x] **Search and filter**
  Filter the graph to matches (not just dim them), filter by author, search by file path.
  (A background index covers the whole history of large repos.)
  - [x] Filter the graph to matches (Filter toggle, Ctrl+Shift+F, live match count)
  - [x] Author filter (author: terms and an author picker)
  - [x] Search by file path (path: and file: terms: a folder, a file name or a glob)
- [x] **Signing beyond config**
  Done on the Commit Signing page: generate a GPG key, copy the public key, gpg/ssh program
  path, SSH allowed-signers, sign tags.
  - [x] Verified/unverified signature badge on commits (the selected commit's details panel)
- [x] **SSH key management**
  Generate a key, choose the key pair, use the local agent, copy the public key (and add it to
  a connected service).
  - [x] Preferences > SSH: list the key pairs in ~/.ssh with type, comment and fingerprint, copy a public key, generate an Ed25519 key (asking before replacing one)
  - [x] Choose the key pair git uses for SSH remotes (core.sshCommand with `-i <key> -o IdentitiesOnly=yes`, globally or per repository), show the current choice and stop using it
  - [x] Use local SSH agent: detect a running agent (ssh-add -l, the Windows OpenSSH agent service or a Pageant-style agent), add the selected key to it, and leave core.sshCommand unset while it is on
  - [x] Add the public key to a connected hosting service through its API (optional key title, check whether it is already registered, fallback: copy the key and open the service's SSH key settings page)
  - [x] Keys outside ~/.ssh: IdentityFile entries in ~/.ssh/config (with Include) and agent-only keys
  - [x] On Windows, point git at Windows' OpenSSH (global core.sshCommand) so it reads keys from the agent service, with undo
- [x] **Profiles**
  Several named profiles (identity, integrations, preferences) with quick switching.
  - [x] Named identity profiles: add, rename, delete (asking first), switch from Preferences >
    Profile or the palette (writes user.name/email/signingkey to the global config); the first
    is seeded from the global identity
  - [x] Per-profile integrations: each profile keeps its own connected hosting-service accounts
  - [x] Per-profile app preferences: theme, UI customization, editor and general settings
    switch with the profile
- [x] **Local patches**
  Export commits, files or the WIP to a `.patch` file; open and apply a patch.
  - [x] Create patch… from one commit or a multi-selection in the graph menus (one mbox-style
    file, oldest first, binary-safe); Apply patch… from the palette to the working tree, checked
    first so nothing changes when it doesn't apply
  - [x] Patches of uncommitted changes (all, staged or unstaged, chosen files, untracked files)
    and of one file of a commit; Copy patch to the clipboard; Show in folder from the save toast;
    an Apply patch dialog with a preview of the files, git apply options (index, three-way,
    `.rej` files, reverse) or Apply as commits with git am, and an Applying patches banner
    (Continue, Skip patch, Abort)
- [x] **Conflict prediction**
  Local-only version: dry-run merge the branch against its target/upstream (`git merge-tree`)
  and warn about overlapping files before you merge or open a PR.
- [x] **External tools**
  Choose an editor, diff tool, merge tool and terminal (with custom commands); open the repo in
  the editor. (Diff and merge tools are their own item below.)
  - [x] Editor (open a file or the repository) and a custom terminal command in Preferences > External Tools
- [x] **Notifications**
  Toasts for every operation exist. Missing: optional desktop notifications.
  - [x] "N commits behind" toast with Pull after a background fetch (UI Customization > Behind alerts)
  - [x] Desktop (OS) notifications while the window is unfocused, for the behind alert and job
    results: an opt-in toggle with help text, confirmed once it is turned on

- [x] **Blame gutter**
  Blame for a file: a gutter with author, date and commit per line (grouped by commit), opened
  from file history (header and row menu), the file context menu and the palette's file finder;
  clicking a commit selects it in the graph. Runs `git blame --porcelain` (honouring .mailmap and
  blame.ignoreRevsFile), with the editor font, tab size and syntax colors. A line's menu blames
  the version before its commit (following renames, with Back to return) or skips the commit;
  the header toggles ignore-whitespace (-w) and move/copy detection (-M / -M -C), opens the file
  history at the blamed commit, finds in the file and goes to a line. Gutter hovers show the full
  message, an accent strip and tint mark newer commits, and N / Shift+N step between commits.
- [x] **Add and remove submodules**
  Add submodule… from the SUBMODULES header menu and the palette (the palette is how the first
  one is added, since the section only shows with submodules); Remove submodule… in the row menu.
  - [x] Add a submodule: URL, path and optional branch, through `git submodule add`, with checks
    for a path that already exists or is already in the index
  - [x] Remove a submodule (deinit, `git rm`, clean its `.git/modules` entry) behind a
    destructive confirmation
- [x] **Submodule upkeep**
  Keep submodules updated after checkouts, pulls, merges, rebases and resets (Preferences >
  General, with a per-repository override in the SUBMODULES menu); Initialize all and Update all,
  offered on open; Edit submodule… for the URL and branch; missing-folder and new states; resolved
  relative URLs; live status from submodule HEAD and ref changes; palette commands.
- [x] **External diff and merge tools**
  - [x] External diff tool: program and arguments ({local}, {remote}, {path}) in Preferences >
    External Tools, opened from file rows and the diff header
  - [x] External merge tool: program and arguments ({base}, {ours}, {theirs}, {output}), and the
    conflict view's Resolve in an external merge tool
- [x] **Rebind keyboard shortcuts**
  Rebind keys in Preferences, saved in settings and matched through the shortcut table, with
  conflict detection and reset to default.
- [x] **GPG passphrase prompts**
  Ask for GPG passphrases for commit and tag signing in Trellis (a Trellis pinentry, or loopback
  pinentry), like the existing SSH passphrase prompt.

## P2: integrations depth

- [x] **In-app pull request view**
  - [x] Files changed (from the local remote-tracking branches: merge base to source tip, with fetch notes)
  - [x] The conversation (comments) from the service
  - [x] Inline code comments: reply, resolve, show outdated
  - [x] Approve, request changes, finish review
  - [x] Merge with merge, squash or rebase
  - [x] Delete the branch after merge
- [x] **CI / build status**
  Check status on PRs and on branch tips (GitHub checks, GitLab pipelines, Azure Pipelines,
  Bitbucket builds).
  - [x] GitHub and GitHub Enterprise pull requests: check runs and commit statuses read with the
    reviews (same batched GraphQL call), a passed/failed/pending marker on PULL REQUESTS rows
    and a "Checks: N of M passed" line in the row tooltip
  - [x] GitLab pipelines: the head pipeline status on merge requests
  - [x] Azure Pipelines: build and policy status on Azure DevOps pull requests
  - [x] Bitbucket builds: commit build statuses for Bitbucket Cloud and Data Center pull requests
  - [x] Status on branch tips: markers on LOCAL and REMOTE rows and on graph ref labels, with or
    without a pull request
  - [x] Open the failing (or latest) build's page in the browser from the PR row menu or tooltip
- [x] **"Push and start a PR"**
  One action to push a branch and open the create-PR dialog; start a PR from a remote branch.
- [x] **PR and issue filters**
  Saved filters: mine, needs my review, assigned, mentioned, labels, milestones, sprints.
- [x] **Create and view issues**
  Create an issue (type, labels, assignees, epic); an issue detail panel with description and
  comments; "create issue then branch". Done for GitHub, GitHub Enterprise Server, GitLab, Jira,
  Bitbucket Cloud issues, Azure DevOps work items and Trello cards; Bitbucket Data Center has no
  issue tracker of its own and pairs with Jira.
- [x] **Trello**
  Trello cards as an issue tracker.
- [x] **Cross-repo dashboard**
  PRs and issues across all connected repos (local, with no cloud account).
- [x] **Limits found while building integrations**
  - [x] Paginate PR and issue lists. Trellis stops at 100 (50 on Bitbucket Cloud); `morluto/rea`
        already exceeds that.
  - [x] Fetch review states for every PR. Only the first 25 are fetched today, to save API calls.
  - [x] Jira key detection treats strings like `UTF-8` as keys unless a project filter is set.

## P3: app polish and platform

- [x] **Custom themes**: High Contrast Dark, High Contrast Light and Trellis Sand built in, plus
      JSON theme files in the config folder's `themes` folder (a base theme and the colors it
      changes, as #rrggbb or another color's name), listed in Preferences and the palette
- [x] **Zoom** in/out/reset; show/hide the left, detail and toolbar panels
- [x] **Menu bar** (File / Edit / View / Window / Help), including Undo/Redo under Edit, an
      About dialog with the third-party notices, and a UI Customization toggle to hide it
- [x] **Language**: translate the UI (date locale and relative dates already exist)
  - [x] Catalog (English base, Deutsch, Español) with system-language detection, Language on
    Preferences > General, and translated Preferences navigation, General (with its Updates row),
    Editor, External Tools and UI Customization pages, commit box, status bar and its update chip,
    menu bar and About dialog, toolbar, tab bar, the release notes dialog, the first-run welcome,
    the welcome screen and its repository lists (also the toolbar's repository menu and File >
    Open recent), Find repositories, and the details panel (commit details, the WIP lists, row
    actions and in-progress banner buttons, several commits and the working-directory comparison),
    the conflict view, the conflicted and resolved file lists and the conflict check, the file row
    menus and path warnings, the diff file header, blame, the signature badge, the commit and
    multi-select menus' reasons, and the in-progress banner titles and stop lines
- [x] **Logs**: keep the activity log across sessions; error and performance logs; extended
      logging (the in-session activity log exists)
- [x] **Command-line companion**: `trellis <path>` and `trellis .` work (subcommands are under
      Later)
- [x] **Deep links**: `trellis://` links to a repository, branch, commit or tag (named by root
      commit and remote URL), copied from the branch, tag, remote and commit menus and the
      palette, opened in the running window with Locate, Clone and Fetch when they don't match,
      and registered per user in Preferences > General (Windows and XDG desktops)
- [x] **Packaging and updates**: an app icon; MSI, DMG and DEB installers next to the zip and
      tar.gz downloads, with "Open in Trellis" on folders and `trellis://` links; code signing in
      CI when the secrets are set; SHA256SUMS; a daily update check with a verified download; and
      the release notes bundled in the app
- [x] **Git engine choice**: choose
      which git executable to use and show its version
- [x] **Performance**: incremental status for very large working trees: the watcher's changed
      paths re-read only their unstaged rows, and a slow full read offers git's untracked cache
      once per repository (loading already runs off the UI thread)
- [x] **Onboarding**: first-run welcome and tutorial

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

Milestone 2 work (see ROADMAP.md): the follow-ups a daily user would miss, triaged from the
ideas that grew out of each item and grouped by that item. They are not tracked as
checkboxes; they are built after the open items above, roughly in this order. New
follow-ups go to Ideas, not here.

### New directions

Chosen on 2026-10-19 as the features that make Trellis its own, in this order.

### UI review polish

The minor findings of the [UI review of 2026-10-18](docs/reviews/ui-2026-10-18.md); its
blockers and major findings are Milestone 1 items.

- Row context menus open below the row so its status text stays visible
- The trust prompt's "{ } Hooks" heading becomes a drawn icon on the text margin, with space
  before the first message
- Toolbar split buttons: the caret 2–4 px from its button inside one hover background
- The "+ branch" ghost on the selected row is a small ghost button, only on the anchor row, on
  hover or focus, and hidden while a ref menu is open
- Load more, Fetch full history and the shallow count move to a full-width row at the end of the
  graph
- The multi-selection header: "3 commits selected · Esc to clear", the cherry-pick reason inline,
  and the compare direction shown as "7e24110 → 222c0ad ⇄"
- The dashboard caps its content width (about 900 px) or tints alternate rows
- Clone with no connected service shows only the connect prompt and a link back to the URL tab
- Git Flow's blocked Finish offers "Stash changes and finish"
- The git-too-old banner links to how to update and says whether Dismiss is for good
- Deep-link dialogs say why an entry is preselected, and an unknown link kind offers Check for
  updates
- The View menu separates its two Diff radio groups
- One verb per action across menus, the palette and the welcome screen ("New repository…",
  "Open repository…", "Delete stash")
- The command palette's empty list shows Recent, then Suggested, then the rest
- Plain-language errors instead of git internals (safe.directory values; hosting-service errors
  rewritten as a sentence saying what to do)
- Submodule status dots have tooltips, and clean rows say "up to date"
- The tab list button sits after the last tab, and tab groups get a named chip
- The rebase banner keeps Continue as the primary button with Skip, Abort and Edit steps in a
  split button, so it fits the panel

### File history and blame

- Open folder history from a folder row once file lists can show a tree

### Git Flow

- Continue a Finish that stopped at a merge conflict once the merge is committed
- Confirm before deleting the configured production or development branch

### Stash improvements

- Keep the index when stashing (`--keep-index`) as a Stash dialog option
- When a staged rename's new path is chosen, stash its old path too, so the rename moves as a whole

### Notifications

- Keep a toast up while the pointer is over it, and a close button on toasts
- Click a desktop notification to bring Trellis to the front and switch to that repository's tab
- Show desktop notifications under Trellis's own name and icon: register an app id on Windows
  (now attributed to PowerShell) and set the bundle id on macOS once Trellis ships as an app bundle

### GPG passphrase prompts

- Passphrase prompts for SSH-format signing keys (ssh-keygen -Y sign)
- Resolve MSYS-style gpg.program paths such as /usr/bin/gpg on Windows

### PR and issue filters

- Team and group review requests counting as Needs my review (GitHub teams, Azure groups)

### Create and view issues

- Post a comment and reply from the issue panel

### Limits found while building integrations

- Live-test GitLab, Bitbucket Cloud and Data Center, Azure DevOps and Jira against real accounts, including the Azure DevOps sign-in through Git Credential Manager (Microsoft Entra Bearer token)

### "Push and start a PR"

- Pull requests across forks: a source on a fork remote, or pushing to your own fork and opening the pull request against the upstream repository

### In-app pull request view

- GitLab merge request notes, approve / unapprove and merge (merge, squash, fast-forward) in the
  panel
- Bitbucket Cloud and Data Center comments, approve / request changes and merge strategies in the
  panel
- Fork pull requests: fetch the head ref (e.g. refs/pull/N/head) when the source branch isn't on
  the remote
- Use the head SHA directly when that commit is already here but the remote-tracking branch is
  missing or behind
- A Commits tab: the commits between the merge base and the source tip
- Ctrl+Enter to post a comment or submit a review
- Start a new code comment on a diff line (a single comment or one added to a pending review),
  with suggested changes
- Place comments on their lines when the local source branch differs from the head (map the line
  numbers across the newer commits)

### Git engine choice

- Offer to fall back to the PATH git when the saved one disappears, instead of only explaining it in the banner

### Logs

- Copy one log entry or the whole log to the clipboard, and export all logs as one archive for a bug report

### Performance

- Edits to tracked files that match an ignore pattern are dropped by the watcher; check the index before dropping
- Read the staged list incrementally on index changes (compare the old and new index) instead of a whole status read
- Turn on git's built-in fsmonitor from Trellis (suggest watchman on Linux), checking that the daemon runs
- Make the Windows 30 s safety poll cheaper for huge trees (read only folders changed since the last read)

### Deep links

- macOS: receive links and folders as Apple events (GURL and odoc) and declare the scheme and the folder document type in Trellis.app's Info.plist, so links, Finder's Open With and dropping a folder on the Dock icon open them

### Cross-repo dashboard

- Count review requests made to a team the user belongs to
- Show pull requests assigned to the user under Assigned

### Menu bar

- Alt-letter mnemonics and keyboard navigation of the menus (Alt+F, arrow keys between menus)
- The native macOS application menu (the global menu bar with the standard app menu) instead of the in-window bar
- Ask before quitting (from the menu or the window's close button) while a git operation is still running
- A Quit key in the keymap (Cmd+Q on macOS, Ctrl+Q elsewhere)

### Packaging and updates

- Install and restart: quit Trellis and run the downloaded installer, or swap the portable program in place
- More formats and targets: .rpm, AppImage or Flatpak, a Homebrew cask, a winget manifest, Linux aarch64 and Windows ARM64 builds

### Language

- Honour gettext's rule that LANGUAGE is ignored when the locale is C on Linux

## Ideas

Polish, edge cases and power-user extras that are not planned work. Nothing here is built
until a triage moves it to Later or a milestone. New follow-ups from finished items go
here, under the item they grew from.

### Design system
- Underline panel tabs (an underline, no boxes) with an Errors count badge; the activity panel
  keeps its segmented tabs
- Animate hover and press fills over `motion::QUICK` instead of switching at once
- Panel and dialog open fades at 220 ms (a `motion::GENTLE` token) as on the foundations board
- Toast leading result mark (success, error, info) and a close ×
- Quiet danger button (red text, no fill) for destructive actions in menus and rows
- Try pill-shaped ref labels in the history table (kept at 4 px for the chosen variant C)
- Two-layer shadows matching the board's elevations (egui's `Shadow` is one layer)
- Theme-file keys for the derived tokens (overlay, overlay edge, tonal) so custom themes can
  override them
- Spacing scale tokens (4, 8, 12, 16, 24, 32, 48) and the remaining spacing literals moved onto
  them
- Replace the font check marks (✔ before the current branch, worktree and votes) with a painted
  `Icon::Check`
- Fold the toolbar caret, `widgets::chevron` and the tab glyphs into `Icon` variants
- The WIP header's more-actions button as an icon-only button with a popup instead of an egui
  menu button
- Give `icon_button` an `.enabled()` path so callers stop wrapping it in `add_enabled_ui` (which
  fades it)
- A `.kind()` builder so an icon-only button can be secondary, once a view needs one
- Views for `buttons::quiet` and a leading `.icon()`: both exist with no caller yet
- The design sheet's hover row from real pointer hovers (one per kind, rendered apart) instead of
  painted looks; a focused quiet button with its inset ring beside the outside ring
- The scroll cue (always-visible thin bar and edge fades) for the sidebar, WIP file lists, graph,
  diff viewer, activity log and terminal
- Size the shortcut sheet and other resizable dialogs to the window's height instead of 480 px
- Conflict check popover: a Show all link that opens the report dialog with the whole list
- Conflict report and operation preview: lay out only the visible paths of a very long list
- The × close button with an Esc tooltip for the range diff, journal and Git Flow panels and the
  graph search bar, and the dashboard's × through `ghost::close_view`
- The 50/72 summary counter and its tones in the annotate-tag dialog's message field
- Keyboard preferences: Add with a leading + icon, and Reset as a quiet button
- Move `widgets::small_icon` callers (recent repositories, repository finder, tour) to the
  weak-then-strong `remove_icon` look
- Status bar: name the filter with its section ("2 issues in Current sprint") once tracker nouns
  have plural keys
- Update-failed chip: open Preferences scrolled to the Updates row

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

### Preview before you act

- Draw the preview on the real lane graph (both sides of a merge, branch lanes), with a
  before/after view side by side
- Replay rebase, cherry-pick and drop previews with git merge-tree --merge-base per step (git
  2.40+) so merge drivers, rerere and merge.renames apply as in the real run
- Preview a cherry-pick or revert of a merge commit with its mainline (from the mainline
  dialog), and preview Revert
- Preview the interactive rebase panel's list (reorder, squash, reword, exec) and the graph's
  reword and squash edits
- Preview a pull, a Git Flow finish and a stash pop
- Preview a rebase.rebaseMerges rebase and octopus merges in drop plans
- A Cancel button and a progress line for long previews; lift the 500-commit replay cap
- Command palette entries for the preview actions
- Offer Preview from the conflict check report and chip popover, and from the drop and hard
  reset confirmations
- Show what happens to uncommitted changes (the auto-stash and whether putting it back
  conflicts)
- Click a preview row to see its rewritten diff against the original commit
- A preference that makes the merge, rebase, reset, cherry-pick and drop items always preview
  first

### Custom commands

- Optional toolbar slot for chosen custom commands (after the Design system toolbar rework)
- Commands in the remote branch, tag, stash and multi-commit menus, with a {shas} placeholder
  for a selection of commits
- A Windows cmd or PowerShell kind, or per-OS template variants (template_windows /
  template_unix)
- Run a command in the built-in terminal instead of as a job, for interactive programs and live
  output
- Stop a running custom command, and stream its output into the Activity row while it runs
- Ask for text before running ({input:Prompt} placeholders filled from a palette step)
- More placeholders: {short} sha, {upstream}, {remote}, {tag}, the file's folder
- Include .trellis/commands.toml in the trust print so a changed repository commands file asks
  again
- Preferences page listing loaded commands, their files and problems, with Open file, Reload
  and Create user file (with a commented example)
- List custom command keys in the shortcut sheet and Preferences > Keyboard, and let them be
  rebound there
- Remember custom commands among the palette's recently run commands
- A setting to ignore repository commands files entirely
- Watch the user commands file for changes (today it reloads on each repository load, focus
  refresh or Reload)
- Literal pathspecs for {files} so glob characters in file names don't match other files

### Operation journal

- Show the remote-tracking branches' and refs/stash reflogs in the timeline
- Filter the timeline by branch or HEAD, and search the labels and commit summaries
- Day headers in the list (Today, Yesterday, then dates)
- Up and Down to move between rows, Enter for Restore..., and a bindable shortcut to open the
  journal
- Expand a finished rebase's row to show the steps the timeline folds away
- Restore every branch a rebase --update-refs moved from its one note
- Notes for operations run in the terminal panel or outside Trellis
- Notes for stash drops and Discard all, with stash states in the timeline
- Draw the preview's history with the real lane graph instead of one lane
- Load more than the newest 2,000 states
- Compare a state with now as a plain diff
- Say when git will expire a state (gc.reflogExpire, gc.reflogExpireUnreachable)
- Notes for jobs run in other repositories (repo sync pulls)
- An Undo or Redo note's operation name comes from core's English description, so it reads in
  English in other languages; key it to the language tables instead
- Stop a long git fsck from the Lost commits tab
- Throttle Lost commits reloads on watcher refreshes in very large repositories
- Show the files and diff a dropped stash holds in its preview
- Branch from a dropped stash (git stash branch <name> <commit>)
- Undo for Put back in stash list (drop it again)
- Count the commits under each lost tip and show the whole lost segment
- Say when git will prune a lost commit (gc.pruneExpire)
- A palette command and a View menu item that open the journal on Lost commits
- Show the lost count on the Lost commits tab label
- Filter or search the Lost commits list (stashes only, by author, by text)
- Tell Trellis's own auto-stashes and rebase autostashes apart from other dropped stashes
- Say "lost" in an unreachable row's accessible name
- Recover dangling blobs (files staged but never committed)
- Jump from a lost tip to its marked row in the States tab
- Disable Apply stash while a job runs or an operation is in progress, with the reason

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

### Terminal keeps the prompt's space

- BS, CUB, EL and ECH right after a write into the last column act one cell off (vt100 keeps
  the cursor past the edge instead of on the last cell)
- A rebuilt screen (Clear history, parser recovery) forgets which rows wrapped, so copying a
  wrapped line afterwards splits it
- Diff, blame, file history and conflict views measure columns with the unrounded 'M' advance;
  check long lines for the same drift against their highlights and wrap widths
- Log the raw PTY bytes at trace level (RUST_LOG) so a shell's output can be captured for
  regression tests

### Stash and carry on

- Interactive rebase and graph edits: use the named auto-stash instead of git's `--autostash`
- A Show stash button on the toast when git's own autostash is kept after an interactive rebase
- Restore the staged state on put-back (`stash pop --index`, falling back to a plain pop)
- Guard multi-command jobs too: pull request checkout, Git Flow start and finish
- Don't count submodule-only changes as dirty in the guard
- An option to include untracked files in the auto-stash
- Remember pending auto-stashes across restarts (offer to put them back on reopen)
- Pull and Checkout tooltips say when the changes will be stashed first
- Offer Stash and carry on from the "would be overwritten" error toast for operations started
  outside the guard

### Complete translations

- Disabled menu items' reasons still in English: the rebase edit-menu reasons ("This commit
  isn't on the current branch"), the branch menus' "has no upstream" / "isn't on a code host",
  and the remaining BUSY / JOB_RUNNING users
- Job and activity-log labels (the titles git jobs run under, such as "Reset (hard)")
- The worktree row menu (Prune missing worktrees), the Git Flow header menu, the pull request
  and issue row menus, and the filter popover's "Saved filters" and "None yet"
- Built-in filter names on the pull request and issue filter chips
- Names only screen readers hear: the column resize grips and the hidden and show-only markers
- The branch rows' ahead/behind tooltips and the graph header's shallow-clone tooltip

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

### Disabled actions say why

- Other setup-gated entries stay enabled and open their setting: pull request and issue items
  that need a connected account open Preferences > Integrations, and an issue tracker that isn't
  chosen opens Repo-Specific Preferences > Issue Tracker
- When Preferences opens from a setup-gated entry, focus and highlight the matching field (diff
  tool, merge tool or editor)
- Footer reasons for the form dialogs whose primary is disabled only by a field problem with no
  reason given (clone, init, tag, new and rename branch, worktree create and move, submodule add
  and edit, publish, create PR, Git Flow dialogs, credential prompt)
- Reasons for the remaining preference-page buttons with more than one blocker (the integrations
  token Connect: token or user missing)
- One shared BUSY constant: replace the copies of "Another operation is still running." in the
  patch, stash, mainline, submodule and worktree dialogs and `state/remotes.rs` with
  `state::blocked::BUSY`
- Draw the rebase panel's blocker with the shared reason helper so every disabled reason has one
  look (it is red today)
- The multi-commit selection panel's disabled Cherry-pick and Revert buttons say why only on
  hover; give them a reason line like the other panels

### Disabled menu items explain on hover

- Keep the toolbar dropdowns open when a blocked item is clicked (their popup closes on any click)
- Let a blocked menu-bar submenu (Open recent) still open with its entries greyed, instead of
  being a flat blocked item
- Show the rich preview tooltip (what Fetch all or a Pull variant would bring in) for blocked
  toolbar dropdown items as well as the reason
- Give the dialog lists that grey out rows without an info mark (the fork picker's "added as"
  rows, the issue picker when full) the same mark, focus and tooltip
- Check with NVDA and VoiceOver that a focused blocked item reads its label, "unavailable" and
  the description, and tune the role or description if not
- Up and Down arrow focus that stays inside an open context menu and wraps at its ends, so the
  keyboard walks the items, blocked ones included
- A preference to show the reason inline under a blocked item instead of in a tooltip, for
  people who don't hover
- The saved-filter rows' disabled Move up and Move down icon buttons in the filter popover say why
  only through egui's disabled look; give them the info tooltip on hover and focus

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

### Tabs (multiple open repos)

- Ask before quitting (window close button or File > Quit) when any tab holds a commit draft or
  unsaved conflict choices
- A keyboard shortcut that opens the tab list, and a search field to filter it
- Move tab left / right entries in the Window menu and the tab context menu
- A Clear entry for the recently closed list, and keeping that list across restarts
- Offer a tab's custom label again when its repository is opened later from Recent or the
  dashboard
- Ask before closing tabs with other unsaved text (a half-written pull request description,
  issue form or stash message)
- Show the tab list button always, not only when there are closed tabs or the tabs had to shrink

### Undo / Redo

- Undo across restarts by rebuilding actions from the HEAD, branch and stash reflogs
- Offer "Stash changes and undo" for merge, pull, rebase, cherry-pick, revert and stash undos
  refused for local changes (the runner already supports it)
- Undo of Create tag and push: also delete the tag from the remote it was pushed to
- Keep the undo history across `rebase --abort` (HEAD goes back to where the rebase started)
- Undo a rebase whose branch can't be known (a detached HEAD rebase continued from the banner)
- Undo of branch rename and of the new branch made by `worktree add -b`
- Undo of a Git Flow start or finish as one step
- Roll back the remote-tracking refs a pull fetched when undoing it
- Restore a popped or dropped stash at its original position in the list instead of at the top
- Undo of a pop or apply onto a working tree that already had changes
- Keep the undo history across merge, cherry-pick and revert `--abort` (HEAD goes back to where
  the operation started)
- Show the in-progress banner with Continue for a pick or revert sequence whose stopped commit
  was committed from the commit box (libgit2 reports that state as clean)
- Name the picked or reverted commits in the undo label of a sequence finished after conflicts
  (it now says N commits)
- Undo of a merge, cherry-pick or revert continued in the terminal panel or outside Trellis
- Undo of discarding an untracked file or a symbolic link (only regular files are saved for undo)
- Undo of a stash drop across restarts, by finding the dropped commit in the stash reflog or with
  fsck

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
- Read gitattributes from the commit for blame at a commit and the file history's File view,
  and from the index for staged diffs
- Show a loading state on a gap control while its lines are read
- Trim the bundled syntax set to the languages Trellis wants, if the extra ~0.6 MB of binary
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
- Keep a revert's undo entry when the revert fails after changing the file (as Restore does)
- Name the commit in a revert's undo label ("revert of a hunk of 1a2b3c4 in a.txt")

### Line-level staging

- Per-line hover buttons (stage / unstage / discard this line)
- Selections spanning several hunks
- Keep the line selection across tab switches
- Auto-scroll the diff while a line drag is held past the viewport's top or bottom edge
- Drag selection in the whole-file view, once line operations work there
- Discarding every line of an untracked file deletes it instead of leaving an empty file
- After unstaging every line of a new file, follow the diff to its untracked view instead of
  closing it
- Ctrl/Cmd-drag over already selected lines removes them (toggle the range) rather than adding
- Changed-word fills on a selected line are a step weaker than the stronger selected tint in
  Dark; derive a selected-word fill from it
- Space toggles the line under the keyboard cursor (with the check box as its focus mark)

### Merge conflict tool

- Syntax highlighting in the conflict panes
- Mark several selected files resolved or conflicted at once (needs multi-select in the WIP
  file lists)
- Mark all resolved for one folder, once the WIP list has a folder view
- A Preferences switch to turn off automatic picks for trivial conflicts
- Pick trivial conflicts in every conflicted file at once, without opening each one
- Mark conflicted for a submodule resolved in Trellis (no resolve-undo record is written for
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
- Undo of a whole-version resolve (Take ours/theirs for binary, deleted, link and submodule conflicts)
- Undo of Mark resolved and Mark conflicted from the WIP panel and palette
- Drop expired resolution entries from the undo history as soon as the merge or rebase step is committed or aborted, not when they are next tried
- Reload a parked tab's conflict view when an undo there changed its open file
- An Undo button on the Resolved / Saved-with-markers toasts
- Undo of a resolution whose resolve-undo record was removed outside the app (restore the stages from the recorded ids with libgit2)

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

### Live working-tree updates

- Notice a watcher that started but receives no events (a probe after starting) and warn once,
  suggesting a restart
- Watch inactive tabs so their state updates in the background
- Status-bar indicator showing whether live updates are active for the repository
- Watch config files pulled in by include.path / includeIf (outside the git dir) so their edits
  reload too
- Ask about trust again when a trusted repository gains new command-running settings outside
  Trellis

### Credential prompts

- Caps Lock warning on password fields (egui doesn't expose Caps Lock state yet)
- Choose between a password, personal access token or app password per hosting service, with a
  link to create a token
- A non-blocking notice for SSH security-key touch requests (`SSH_ASKPASS_PROMPT=none`)
- Hosting-service sign-in without Git Credential Manager through the prompt (Preferences >
  Integrations)
- Approve the sign-ins for the remotes that worked in a multi-remote fetch that failed overall
- End an auto-fetch pause after a pull or a dashboard fetch of the remote works, not only fetch
  jobs
- Keep paused auto-fetch remotes across restarts
- A desktop notification when a remote needs a sign-in
- A status-bar chip listing paused remotes once their banner lines are dismissed
- Read the OpenSSH version of a repository's `core.sshCommand`, not only `GIT_SSH_COMMAND`,
  `GIT_SSH` or `ssh` on PATH
- Start every git child in its own session on Unix, not only with OpenSSH before 8.4
- Fall back to git's in-memory `cache` helper on Linux when no keyring helper is installed
- Let the dialog's Remember choice update the default in Preferences > General
- Remember sign-ins typed for git LFS endpoints and for clones whose remote isn't known yet
- Log quietly instead of toasting when a sign-in isn't saved because `credential.helper` is
  turned off on purpose

### Branch operations

- Pin a branch to the left (graph lane order, per-repo persistence, jump to the pinned branch)
- Force push (with lease) of a non-HEAD branch, and push to all remotes
- Rename the remote branch too when renaming (push the new name, delete the old one, move the
  upstream)
- Choose a local branch as the upstream in the Set upstream dialog
- A shortcut for renaming the checked-out branch, and renaming any branch through a two-step
  palette entry
- Delete local + remote when the branch isn't merged into HEAD (force), in one step
- A "Commit 1a2b3c4" submenu in the graph's label menu, so a row with labels reaches the commit
  menu from the chips too
- Checkout and fast-forward in one step from a remote branch row
- Create a tracking branch under another name (New branch dialog with a remote start point that
  sets the upstream)
- Push the current branch to a chosen remote branch from its row
- Warn in Delete remote branch's confirmation when a local branch tracks it, and offer to unset
  that upstream
- Show the remote branch's name (not a commit) in the interactive rebase dialog started from a
  remote row
- Compare with the current branch from the local branch and tag menus too
- Route the palette's remote checkout through the remote branch checkout message
- An entry per local branch when several track the same remote branch (only one is offered now)

### Double-click a branch to check it out

- Double-click a tag (graph chip and TAGS row) to check it out detached, through the existing
  tag checkout and its detached-HEAD warning
- Double-click the "+N" chip to choose which hidden branch to check out
- Double-clicking a branch that another worktree has checked out opens that worktree's tab
  instead of logging the refusal
- Branch chip and row tooltips mention that a double-click checks the branch out
- Double-clicking the checked-out branch does nothing instead of logging that it is already
  checked out
- Enter on a selected graph row whose commit carries one branch checks that branch out

### Hide / show only branches in the graph

- Hide / show only from the graph's BRANCH/TAG ref-label menus
- Hide stashes in the graph
- Hide, show only or stop showing only several selected branches at once (multi-select in the left panel)
- Hide all / show all per section (LOCAL, REMOTE, TAGS) from the section header menu
- Hover toggles (hide / show only) on rows that are not hidden or shown-only
- A graph-header button listing hidden branches and tags, to unhide them from there
- Keep the hidden / shown-only state when a branch is renamed (rename the entry instead of dropping it)

### Commit panel extras

- Repo-specific Commit preferences page: edit and save the template, skip hooks by default,
  push after commit by default

### File actions

- Create, rename and delete files from the command palette
- New file in this folder… on folder rows (folder rows wait for a tree view in file lists)
- Delete several selected files at once (needs multi-select in file lists)
- A button in the trash toast that opens the Recycle Bin / Trash or restores the file
- Send several selected files to the Recycle Bin / Trash at once (needs multi-select in file
  lists)
- Name a staged rename's source path in its Unstage and delete confirmation and toast
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
- Explain when a restored or opened path is outside a sparse checkout
- Restore, Open and Show buttons in the diff header for two-commit comparisons (restoring the
  newer commit's version, as the comparison rows do)
- Make discarding an untracked file undoable the same way Delete is
- Make Unstage deleted file (a staged new file already gone from disk) undoable by keeping its
  staged blob
- Take the file back out of the Recycle Bin / Trash on undo instead of leaving its copy there
- Keep an undo entry when a delete fails after the file went (the trash reported an error, or
  Unstage and delete couldn't change the index)
- Keep a copy for undo of files over 64 MB (outside the object database, or by restoring from
  the trash)
- Read Recycle Bin settings for redirected known folders and subst drives, which have no volume
  GUID
- Find the mount's trash folder on unix systems without /proc/self/mounts or /etc/mtab (the
  BSDs) instead of leaving the choice to the trash crate
- Show progress for, and allow cancelling, the copy into the home Trash for large files
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
- Bring back the dialog a held job came from (New tag, Remote, worktree…) after Cancel, with its
  fields intact
- Don't count hooks Trellis installed itself (git lfs install) as a reason to ask about a
  repository that was clean
- Offer Don't trust on the prompt that holds a job, alongside Trust and run and Cancel
- Queue several held jobs instead of refusing a second one while the prompt is open
- Ask again when a hook's script changes, not only when a new hook or setting appears
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
- Notice a new hook at the next refresh rather than the next full load (the change check reads
  the config and refs, not the hooks folder)
- Take a newly checked-out submodule's own settings into its parent's trust decision, so the
  parent isn't asked again about what the submodule brought
- Say in the terminal when a repository with running shells stops being trusted or gains
  settings or hooks (shells already running are never stopped)
- Hold or warn before Open in external terminal starts a shell in an untrusted repository
- Offer Don't trust from the terminal's held panel prompt, alongside Trust and run and Cancel
- Show the LFS setup while a repository is untrusted by reading it without git (libgit2 for the
  hooks folder and local filters, git outside the repository for the rest)
- List the sparse paths of an untrusted repository by reading its sparse-checkout file instead
  of git sparse-checkout list
- Run git lfs version outside the repository always, and count a repository's alias.* settings
  for commands Trellis runs (a repository's alias.lfs runs when git-lfs isn't installed)

### Command palette (fuzzy finder)

- Remember the last remote picked for tag pushes per repository
- Warn (without blocking) when a new tag name matches a branch name
- Replace (force-move) an existing tag from the New tag dialog
- Catch existing tags that point at non-commit objects before running git
- A Preferences control for pushing new tags by default
- Two-step entries for branches, themes and repositories (done for files and stashes)
- Check out a remote branch whose name is taken locally under a new local name
- Offer to fast-forward the checked-out branch when it is behind the picked remote branch
- Group ref entries by remote, or move them into a two-step checkout sub-list
- Virtualised command rows so the 200-row cap can be lifted
- Show the Commands button when no repository is open (the palette already works there)
- Remember recently run commands across restarts (a short key list in settings)
- A dim "recently used" marker or divider for the recent block in the list
- Remember recent commands per repository instead of per session
- Rank recently used ref commands (Fetch <remote>, Finish <flow branch>, checkouts)
- Rank recently picked items first in the file and stash sub-lists
- An entry to clear the recent-commands list
- Commands: syntax highlighting on/off (re-highlighting the open diff when flipped), show the
  working directory changes (select the WIP row), diff mode (whole file / hunks) toggle
- Refresh an open stash sub-list's rows when the stash stack changes while it is shown
  (choosing a stash already finds it again by its commit)
- Apply or pop with `--index` to restore the staged state
- Create, delete and edit files from the palette
- The one-time detached-HEAD notice also for checking out a commit or a remote branch detached
  (graph and commit menus)
- A Preferences control to show the detached-HEAD notice again
- macOS modifier symbols painted as shapes in the key caps (the bundled fonts lack the glyphs)
- The toolbar buttons' keys also exposed as the accessibility keyboard-shortcut property
- Accessible names that say what the repository and branch pickers are ("Repository: <name>")
- The stash-apply question lists the files both the stash and the working tree change, and
  says nothing when they don't overlap
- A refusal with a clear reason, instead of a question, when the index has conflicts (git
  can't apply a stash then)
- The toolbar Pop and the palette's Pop stash name the stash by position and commit like the
  sidebar does (stash@{0} with the revision guard)
- Footer text listing the PgUp/PgDn and Home/End keys

### Keyboard shortcuts

- Topological next/previous (the next or previous commit on the selected commit's own branch
  line) and first/last row in the graph
- The rows' Stage/Unstage buttons and menu items also move on to the next file when they act on
  the open one
- The sheet and Preferences > Keyboard notes name the WIP lists for the Up/Down and S/U rows
- Home/End and PageUp/PageDown in a focused WIP list
- A key to move the focus between the Unstaged and Staged lists keeping the place
- Enter to open, Space to stage or unstage, and Delete to discard the current file of a focused
  WIP list
- Shift+Up/Down to select several WIP files and stage or unstage them together
- Keyboard focus and Up/Down in the commit, comparison and folder-history file lists, and in the
  Conflicted and Resolved lists
- Moving on to the next conflicted file after one is marked resolved from the list
- Arrow keys in the first frame after a WIP list takes the focus still move egui's focus (the
  lock starts on the second frame)

### Rebind keyboard shortcuts

- Multi-step key sequences (press one chord, then another)
- Export and import a keymap file, or keymap presets
- Warn when a key is taken by the OS on the current system (Cmd+Tab, Cmd+Q, Cmd+H on macOS;
  Alt+F4 on Windows), including a Mod+Tab synced from another OS
- Record keys the table can't name yet (F13–F24, curly brackets, media keys)
- Rebind the fixed context keys (Commit's Ctrl+Enter, list arrows, palette keys) with their own
  per-context conflict rules
- Per-repository keymaps
- Default keys for fetch, pull and push once a chord free on Windows, macOS, Linux and common
  shells is agreed
- Keys for more palette commands: the apply, pop, delete and rename stash pickers, Open
  repository, Open dashboard, Switch profile, Copy repository link, Rename branch, Create pull
  request, Git Flow start
- A key for Push stack, once that palette entry has landed
- Keys for each pull variant from the toolbar's pull menu (rebase, fast-forward only)
- Toolbar Fetch, Pull, Push, Branch, Stash and Pop tooltips name the assigned key
- Warn on the Keyboard page when a key is also a custom command's key
- The shortcut sheet points to Preferences > Keyboard for repository commands with no key yet
- Split palette_commands.rs's entry builders by area (the binding map moved to
  state/palette_bindings.rs)

### Built-in terminal

- Mouse reporting (xterm mouse modes) and the window or tab title from the shell (OSC 0/2)
- Tab strip overflow: scroll-left/right buttons and a list of all tabs when they don't fit
- Keys for new terminal tab, close tab and next/previous tab while the terminal has focus
- Rename a terminal tab
- Double-click selects a word and triple-click a line; dragging past the top or bottom edge
  scrolls the history
- Mouse wheel in full-screen programs (alternate screen) sends arrow keys
- A preference for the history length (fixed at 5,000 rows)
- Keep the history when the screen is rebuilt after a parser error
- Font family and line height preferences for the terminal; draw the dim (faint) text attribute
- Honour a program's cursor shape requests (DECSCUSR) and a blinking cursor
- Browse… for the custom shell program, and WSL distributions as shell choices
- Find in the terminal's history
- Clear for cmd in step with Windows' pseudoconsole (today it only clears locally)
- A palette entry for New terminal tab
- On Linux, a selection also sets the PRIMARY selection and middle-click pastes PRIMARY instead
  of the clipboard (X11 and Wayland)
- An Option-as-Meta setting on macOS, so Option+letter sends the ESC prefix there too
- Alt+digit and Alt+punctuation (Alt+., Alt+_) as ESC-prefixed input
- Confirm before closing a repository tab or quitting while one of its terminals runs a
  program, and before Kill
- Show the running program's name in the close confirmation on macOS
- Note in the shortcut sheet and Preferences > Keyboard which keys the focused terminal keeps
- An option to send Shift+Home/End to the shell (PSReadLine selects with them) instead of
  scrolling the history
- Count background jobs and a custom launcher shell's (wsl.exe) children correctly when
  detecting a running program

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
- Also update submodules after undo/redo, cherry-pick, revert and stash pop
- A "Clone submodules too" (`--recurse-submodules`) checkbox in the Clone dialog, defaulting to
  the keep-updated setting
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
- Refresh other worktrees' WIP counts on watcher and status-only reloads and on returning to the
  tab (today only full loads count them)
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
- Treat a branch being rebased or bisected in another worktree as checked out there (LOCAL mark
  and refusals)
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
- Untrack lines that `--filename` wrote with backslash-escaped glob characters (`git lfs
  untrack` leaves them): remove them directly
- Offer to convert files already committed (`git lfs migrate`) after tracking their pattern
- When initializing fails because a non-LFS hook already exists, offer to merge or overwrite it
  (`git lfs update --manual` / `--force`)
- Initialize with LFS when creating or cloning a repository
- File locking: lockable patterns (`track --lockable`), locking and unlocking files, and showing
  who holds locks
- Read patterns from nested `.gitattributes` files and `.git/info/attributes`, and show which
  file each pattern comes from
- Check for a minimum git and git-lfs version and say clearly when either is too old
- After auto-fetch, also download the LFS files of the checked-out branch's upstream, so a later
  pull doesn't wait on them
- Detect LFS use from nested .gitattributes files too when deciding whether auto-fetch downloads
  LFS files
- A per-repository switch to leave LFS downloads out of auto-fetch
- Warn in the main-window banner when git-lfs is installed but too old for the repository (no
  filter-process support), next to the not-installed case
- Check for git-lfs again when the window regains focus, so the missing-git-lfs banner clears
  after installing it without opening Preferences
- Count nested .gitattributes files and info/attributes patterns for the missing-git-lfs banner
  and the auto-fetch LFS download, not only the root file
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

### Git Flow

- Finish options: rebase on develop first, squash, and push after finishing
- Finish options: skip the tag, sign it with a chosen key, delete the remote branch,
  force-delete
- Branch-menu and graph-menu entries for flow branches (finish this feature, start a hotfix
  from here)
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
- Rewrite the saved graph filter's hidden or shown-only refs when a remote is renamed or removed
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
- Show annotated tags' tagger date on tag rows (needs the tag time in the snapshot)

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
- Stash ignored files too (`--all`) from the Stash dialog
- Stash chosen hunks or lines of a file, not only whole files
- Filter field above the Stash dialog's file checklist for large working trees
- Check the installed git version before offering "Only staged changes", instead of explaining after git refuses
- Right-click menu on the toolbar Stash button with the Stash dialog's choices
- "Stash…" entry in the STASHES section header menu that opens the Stash dialog
- Stash the files selected in the WIP lists once working-tree files can be multi-selected
- Show each stash's date in the sidebar and palette rows (needs the stash commit time in StashInfo)

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
- Cancel other network jobs (push, pull's fetch half, clone) from the status bar, and show a
  running fetch's progress (objects received, bytes) beside its label
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

- Open all for the Recent list and for the Find repositories results
- Fetch or pull across repositories from the welcome screen and for the Find repositories results
- Ask for credentials during a fetch or pull across repositories (one prompt at a time) instead of failing
- Retry only the failed repositories of a fetch or pull across repositories
- Remember scan results between sessions, and notice new repositories in project folders
  without looking again
- Scan options in Preferences: how deep to look and which folder names to skip
- List bare repositories found by the scan
- Drag to reorder favorites in the repository menu and File > Open recent

### Graph columns and loading

- Hide or disable the header's Load more link while a load for more is already running
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
- A Browse button for the Generate SSH key dialog's save location
- Show in folder and delete a key locally from its row
- Flag encrypted private keys and keys whose file permissions are too open for ssh on their row
- Palette entry for the SSH page (palette_commands.rs needs splitting first)
- Start the Windows OpenSSH Authentication Agent service (needs an administrator) or an ssh-agent on Unix from the SSH page
- Per-repository key choice in a repository with no trust decision (trust it and choose the key in one step)
- Remove a key from the agent (ssh-add -d), and add with a lifetime (-t) or confirmation (-c)
- Add keys to Bitbucket Data Center and Azure DevOps through their APIs
- List and delete the keys registered on a connected service, and show on each key row which services have it
- Per-host key choice through ~/.ssh/config Host entries instead of core.sshCommand
- Palette entries for Use for git, Add to agent and Add to service (after palette_commands.rs is split)
- Link from the add-key fallback to reconnect a service with a token that may write SSH keys (Integrations, preselected)
- Resolve host-dependent IdentityFile tokens (%h, %r, %p, %n, %C, %k, %j) per literal Host pattern, or ask `ssh -G <host>`
- Read IdentityFile entries from the system ssh_config (/etc/ssh/ssh_config, %ProgramData%\ssh\ssh_config)
- Include patterns with wildcards in folder names, and tokens or environment variables in Include
- List CertificateFile entries next to their keys
- Keep Windows' OpenSSH when choosing a key for all repositories (`<Windows ssh> -i <key> -o IdentitiesOnly=yes`)
- Offer Windows' OpenSSH for one repository's own core.sshCommand
- Warn on Windows when HOME and USERPROFILE differ (Windows' OpenSSH reads USERPROFILE\.ssh, Git's ssh HOME/.ssh)
- Set up prompts for Windows' OpenSSH older than 8.4 instead of only warning
- Fill in fingerprints from the .pub file (SHA-256 of the key blob) when ssh-keygen is missing
- Match agent keys when the user's FingerprintHash is md5
- Open ~/.ssh/config in the editor from the SSH page
- Show which ~/.ssh/config Host entry and key ssh would use for the open repository's remote

### SSH preferences

- Translate the page's remaining English: the toasts, log lines and confirmations in update/ssh.rs, ssh_use.rs and ssh_service.rs
- Cancel on the Replace SSH key confirmation goes back to the Generate SSH key dialog instead of closing
- Suggest a free file name (id_ed25519_2, …) as the save location when the default key already exists
- Show remembered keys whose files have gone, with Remove from list, instead of skipping them silently
- Show the key core.sshCommand names as a card when it lives outside .ssh and wasn't added with Use existing
- Cut a long repository name in the "{repo} only" row label with … and a hover tip
- Move the section header with a right-hand slot (title, pill, button) into prefs/form.rs once another page needs it
- Offer to recreate the .pub (ssh-keygen -y) when a picked private key has none
- Key type choice (RSA 4096, ECDSA, security keys) in the Generate SSH key dialog
- Click the agent pill to check the agent again or jump to its detail lines
- Remember a picked key whose path isn't valid Unicode (settings.json keeps paths as text; such a key is listed until the page reloads)

### Profiles

- Per-profile signing format and program (gpg.format, gpg.program, commit.gpgsign) and SSH key
  (core.sshCommand)
- Profile icon or avatar picker, and a profile switcher in the toolbar or status bar
- Import and export of profiles, and sharing them between machines
- A keyboard shortcut for Switch profile
- Separate stored sign-ins per profile for the same service account (e.g. two GitHub users on github.com)
- Warn when connecting an account another profile already uses with a different user
- Per-profile diff header toggles (inline/split layout, hunks/whole file, ignore whitespace, word wrap)
- Per-profile External Tools (editor and terminal programs)
- Per-profile repository-specific settings (issue tracker, hidden and shown-only refs)
- A "Saved with profile X" caption on the General, UI Customization and Editor pages
- Copy or reset one profile's preferences from another profile on the Profile page
- Refresh an open Clone dialog's account list when the profile switches
- Show a profile's folder-rule count on its row in the profile list (e.g. "Ada <a@b> · 2 folders")
- Offer to unset a repository's own user.signingkey too when using a profile there, and name it in the confirmation
- Clean up Trellis's includeIf entries and include files from the global config (a "Remove all folder rules" action, or on uninstall)
- Handle an includeIf section the user shares with a Trellis entry (extra keys in the same subsection) instead of leaving it in place
- Rules on other conditions git offers (hasconfig:remote.*.url for a hosting-service account, onbranch:)
- Show which profile applies in the tab bar or the repository list, not only in the commit box and the Profile page
- Warn when GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL or GIT_COMMITTER_* in the environment override the identity a rule gives
- Re-read the effective identity when the repository's .git/config changes on disk (watcher), not only on open and focus
- A "Pick a profile for this folder" prompt when cloning or initializing into a folder no rule covers
- Per-rule signing settings (gpg.format, commit.gpgsign) written into the profile's include file
- Remove a repository's own identity from config.worktree too (extensions.worktreeConfig) when using a profile there

### Local patches

- Drop a `.patch` file on the window to apply it
- Say when a patch is already applied (a reverse check after a failed apply) instead of
  reporting that it doesn't apply
- Stash uncommitted changes before applying a patch and restore them afterwards
- Choose several files of a commit for one patch (only one file from the row menu now)
- Create a patch of one file from the rows comparing two commits
- Keep a renamed file's rename in a one-file commit patch (include its old path)
- git am options: keep carriage returns (`--keep-cr`) for CRLF content, Signed-off-by, and the
  committer date
- Remember the Apply dialog's last options
- Undo for Apply as commits
- Show the patch git am stopped on (`git am --show-current-patch`) from the banner
- Palette entries for Continue / Skip patch / Abort while applying patches, and Apply patch /
  Create patch in the menu bar's File menu
- Include more than 1000 untracked files in a changes patch (one diff through a temporary index)
- Copy a patch of uncommitted changes to the clipboard

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
- Cancel a running merge tool from Trellis
- Option to mark the file resolved automatically when the merge tool saves it and exits with 0
- Name the temp folders after the sides' branch names instead of ours/theirs
- Remove the diff tool's temporary copies when Trellis quits, not only by the 24-hour sweep
- git config's GUI choice: diff.guitool / merge.guitool and difftool.guiDefault (git difftool --gui)
- Name git config's tool in the file menus' hover notes and the palette's detail (needs a cached read per repository)
- Run console merge tools (vimdiff, nvimdiff, emerge in a terminal) in a built-in terminal tab
- Honour mergetool.keepBackup / keepTemporaries / hideResolved for commands Trellis runs itself
- Open the diff once the trust prompt is accepted (hold the diff tool like a held job)
- Suggest git's built-in tool names (git difftool --tool-help) on the External tools page
- Use a built-in tool's own trusted exit code when the user overrides its cmd
- Copy git config's tool into the External tools fields with one click
- Report a git difftool failure that comes after its 10-second watch (git's tool scripts start slowly on Windows)
- Run a built-in merge tool on a file whose name has *, ? or [ (refused now: git mergetool's script globs the name)

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

### GPG passphrase prompts

- Passphrase prompts for X.509 (gpgsm) signing through gpg.x509.program
- A git banner reason saying git older than 2.31 keeps GnuPG's own passphrase window
- Use Trellis's prompt when Trellis was started with GIT_CONFIG_COUNT set (append at the next
  index)
- Wording for smartcard PIN and touch requests (loopback with a card is unverified)
- Show the key's expiry date in the passphrase dialog
- Signing passphrase prompts for git commands typed in the terminal panel

### CI / build status

- Name the permission build status needs in the GitHub token help, once it is confirmed which
  fine-grained permissions expose check runs and commit statuses
- Open the build page from the LOCAL and REMOTE branch menus and from graph ref labels
- A tooltip with the checks line on graph ref label markers
- GitLab: job counts for the pipeline (N of M jobs passed) and external commit statuses
- Azure Pipelines: partially succeeded builds as warnings, and optional build policies counted
  apart from required ones
- Bitbucket Data Center: one batched build-status call for many commits
- GitHub without GraphQL access: a REST fallback for branch-tip status
- Branch-tip status for branches on other remotes (forks) and their services
- Re-read pending pull request checks sooner than the 5-minute refresh
- A desktop notification when a build on the checked-out branch finishes
- A Preferences switch to turn off branch-tip status requests
- Re-run a failed check, job or pipeline from the pull request panel
- Show a check's log, annotations or failure output in the panel instead of only linking to it
- GitLab bridge (downstream pipeline) jobs in the checks list
- Bitbucket Data Center required builds (merge check) and Bitbucket Cloud required-builds merge
  checks as Required tags
- Group the checks list by workflow / stage with collapsible groups
- A "Show only failing" filter for long checks lists
- The same per-check list for a branch tip or a selected commit (commit details panel)
- Bitbucket Data Center build durations and build numbers from the newer builds API

### PR and issue filters

- Azure Boards iterations (the current or a named iteration) as a sprint condition for work items
- A typed filter query (author:, label:, base:, draft:, review:) with a syntax reference, as an
  alternative to the form
- Conditions on other people: author, assignee or reviewer by name, involves, reviewed by
- Pull request conditions on CI status, review outcome and created or updated dates
- Issue filters for closed or all states, and a choice of sort order
- Saved filters shared by every repository, and importing and exporting filters
- Palette commands and a shortcut for switching the shown pull request or issue filter
- Searching closed or merged pull requests on the service (filters only narrow the open list)
- Label suggestions on Jira Data Center; fix versions and sprints without a Jira project set
- Mentions on trackers that can't search by them (GitLab to-dos, Jira watchers or comments)
- A fuller search for Azure's Mentioning me, which covers only the last 30 days

### Create and view issues

- Change an issue's state (Jira: its status, through a transition), assignees and labels from the panel
- Threaded replies with show and collapse (GitLab discussions, Jira comment threads)
- Choose the base branch when creating a branch from an issue
- GitHub issue types and milestones when creating an issue
- GitLab milestones, epics and several assignees (paid tiers) when creating an issue
- Jira priority, components, fix versions, sprint, Epic Name and other required custom fields when creating an issue
- Open the tracker's own new-issue page in the browser when creating in Trellis fails on required fields
- GitHub issue templates in the Create issue dialog
- Remember the "Create a branch for it afterwards" choice
- Open the issue panel from issue references in commit details and branch names
- Bitbucket Cloud components, versions and milestones when creating an issue
- Bitbucket Cloud issue attachments and the change history (state changes) in the panel
- Azure DevOps area path, iteration path, priority, severity and other required custom fields when creating a work item
- Azure DevOps child links and related work items in the panel; choosing a parent from any work item by search
- Show Azure DevOps inline images from descriptions and comments (signed-in attachment downloads)
- Trello due dates, checklists, attachments and cover images in the card panel
- Trello card comments past the first 1000 actions
- Choose a Trello card's position in its list (top or bottom) when creating
- Check a Trello token's write access before Create instead of after a refused create
- Azure DevOps work item priority and Jira priority in the panel's details

### Limits found while building integrations

- Make the branch-name issue heuristic configurable (`2024-cleanup` is currently read as issue
  2024)
- Pace per-PR detail lookups by each service's rate-limit headers instead of a fixed max age
- Read per-PR details a few at a time in parallel once the test mock supports calls from other threads
- Let users add their own words to the list of Jira non-project prefixes
- Read the cached Jira project keys again when a key's project is missing (a project created after the first read)

### Trello

- Sign in through Trello's browser authorization flow (redirect back to Trellis) instead of
  pasting the API key and token
- Show card labels as coloured chips, and filter the ISSUES list by list or label
- Recognise board-local card numbers (#12) and bare short ids in commit text, once a board is
  chosen
- The ISSUES error's Connect… button opens Integrations on the Trello page
- Wait out a rate limit and retry automatically instead of waiting for a manual refresh
- A token lifetime choice (1 day / 30 days / never) on the Allow a token link
- Add the commit or pull request link to the card as an attachment (needs a write-scope token)

### Command-line companion

- History, diff and status subcommands

### "Push and start a PR"

- On a LOCAL row, create a pull request from the checked-out branch into that branch (the REMOTE row has it)
- Choose source and target by selecting two branch rows
- Remember the last target used per branch and preselect it next time
- For a diverged branch, offer a force push with lease and then the pull request (today it is refused with the reason)
- A Preferences setting to create pull requests as drafts by default
- Push-and-create entries in the graph's commit and ref-label menus

### In-app pull request view

- Azure DevOps threads, votes (approve / wait for author / reject) and completion with a merge
  type in the panel
- A note when the local target branch differs from the service's base (the base SHA is loaded now)
- Show the pull request from the graph's PR chip, a local branch's menu and the command palette
- Fetch more history from the no-common-commit note in a shallow clone
- Delete the local branch too after a merge
- Remember the last merge method per repository
- Edit or delete your own comments, and reactions
- Re-request a review, and a participants list
- Mark a draft ready for review, and enable auto-merge
- Choose the commit email for a merge
- Open a single comment or review in the browser
- Code comment threads for GitLab diff discussions, Bitbucket inline comments and Azure DevOps
  file threads in the panel
- Comment counts on the Files changed rows
- Highlight the whole range of a multi-line comment, and markers on folded rows of the whole-file
  view
- Remember Show resolved / Show outdated per repository
- Load threads with more than 100 comments in full (a link opens the rest on the service now)
- Previous / next code comment thread in the diff
- Markdown bodies: show remote images (fetched in the background, size-capped, opt-in per
  repository) instead of their alt text as a link
- Markdown bodies: emoji shortcodes (`:robot:`), `#123` / `!12` references and `@mentions` as
  links to the service, and relative links resolved against the repository's web address
- Markdown bodies: a few safe HTML tags (`<details>`, `<br>`, `<sub>`, `<kbd>`) instead of
  dropping them, and select text across styled runs (each run selects on its own now)
- A Preview tab on the comment and reply boxes that renders the Markdown being written
- Remember the Checks section's collapsed state per repository
- Load the checks (and page) by itself once the repository is trusted, instead of on the next open
  or tab switch

### Custom themes

- Reload theme files automatically when they change on disk (watch the themes folder)
- Follow system with a chosen dark and light theme pair (today it picks Trellis Dark or Trellis Light)
- A theme file based on another theme file (inheritance, with loops detected across files)
- Edit colors in Preferences with a color picker and live preview, saved to the theme file
- Suggest the closest color name for an unknown key ("acent" -> "accent")
- Warn when a theme file's text or syntax colors fall below the contrast rules
- Translucent colors (#rrggbbaa) in theme files
- Theme the graph lane colors, avatar initials colors and the terminal's ANSI palette from theme files
- Import or share a theme file from Preferences (copy it into the themes folder)
- A toast with an Open Preferences button when the selected theme is missing or broken at startup
- A per-repository theme

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

### Git engine choice

- Warn when the chosen git is a minimal install with no shell for hook scripts (on Windows, no sh.exe next to it)
- Show the OpenSSH version the chosen git will use
- Find gits bundled inside other applications and IDEs (versioned app folders) during discovery
- Type or paste a git path in a text field as well as Browse…
- Choose a git per repository (e.g. a WSL git for repositories under `\\wsl$`)
- Re-check the chosen git's version automatically when it is upgraded while Trellis runs (Look again covers it by hand)
- Run every command of a multi-command job with the git it started with, even when another git is chosen meanwhile

### Logs

- Search and filter inside the activity panel and the error and performance tabs
- Show hook runs (pre-commit, pre-push) as their own titled entries in the activity log
- Show each entry's time in the current session (on hover or in a column)
- Settings for the slow-operation threshold and the log size limits
- Record how long queued effects wait behind a repository's queue, and time the stats and path-search workers
- Leave time spent waiting on a credential or passphrase prompt out of git job timings
- Rotate the error and performance logs by day as well as by size
- Keep two Trellis instances' activity logs apart (one file per instance or a lock during trims)
- Open a log file in the system's text editor from the panel

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

### Deep links

- Offer to add the link's remote when the repository matched by root commit has no remote with that URL
- Copy link to a commit or tag on a chosen remote
- Forward `trellis <path>` to the running window too, not only links
- A palette step to paste and open a trellis:// link
- Links to a file or line at a commit, and to a stash or pull request
- A progress toast while a link is resolved against long recent lists
- Linux fallback that edits mimeapps.list when xdg-mime isn't installed
- Show machine-wide or packaged registrations in the Preferences status on Windows
- An https redirect page so links can be shared where custom schemes aren't clickable

### Cross-repo dashboard

- Keep the last results on disk so they show at startup before the first refresh
- Reuse the pull request lists open tabs already loaded instead of asking those repositories again
- Read checks and votes past the first pull requests on services read one pull request at a time
- Refresh in the background while the dashboard stays open, and notify about new review requests
- A keyboard shortcut for opening the dashboard
- Remember the Mine / Review requested / Assigned choice across sessions
- Add repositories found by scanning a folder, beyond tabs, favorites and recent
- Treat service repository names that differ only in case as one repository when grouping clones
- Open issue rows in the in-app issue panel once issues can be viewed in Trellis
- Apply saved PR and issue filters (labels, milestones) across repositories
- Check out a pull request's branch from the dashboard
- Created/updated dates on pull request and issue rows (the service models don't carry them yet)

### Automatically group worktree tabs together

- A tab group context menu: close the group, close the other groups, move the group to the start
  or end
- Collapse a tab group to one tab showing a count
- A modifier-drag that takes one tab out of its group or drops it into another group
- A repository name chip at the start of each group in the tab strip
- Strip colors chosen per repository, shared with the repository manager's group colors once
  those exist
- Save each tab's group in the tabs file so the startup read isn't needed
- Keyboard shortcuts to jump to the next or previous tab group

### Menu bar

- A key to show or hide the menu bar, and Alt showing it for one use while it is hidden
- About Trellis and Quit as command palette entries
- A Show menu bar button in the status bar while the menu bar is hidden
- Repository and Branch menus (fetch, pull, push, stash, new branch and tag)
- Recently closed tabs as a submenu, to reopen any of them
- The OS and architecture in the About dialog's copied version info

### Packaging and updates

- A download progress bar (bytes and percent) with Cancel, streaming the file to disk instead of holding it in memory
- Skip this version: don't offer a release again until a newer one is out
- "Open in Trellis" in Windows 11's modern context menu (an IExplorerCommand with a sparse package)
- "Open in Trellis" on drives as well as folders
- Register an AppUserModelID on the Start menu shortcut and the process, so notifications carry Trellis's name
- Sign SHA256SUMS (minisign or GPG) and check that signature in the app before trusting the checksums
- Azure Trusted Signing as an alternative to a PFX certificate for Windows signing
- A beta channel that offers pre-releases to users who opt in
- A per-machine (all users) MSI

### Installer and update checks in CI

- Start the installed MSI copy's window with the smoke probe on the Windows runner once it has OpenGL 2 (Mesa llvmpipe or the Windows smoke job's setup)
- Run the installer checks from the Release workflow (workflow_call) against the files it uploads, before SHA256SUMS is published
- A macOS DMG check: mount the image, copy Trellis.app to a temporary Applications folder, start it, and check that the update check picks the .dmg
- Attach to the parent console on Windows so `trellis --version` and `--check-update` print in a terminal without redirection
- Show in Preferences > Updates when a local update feed (TRELLIS_UPDATE_FEED) is in use
- Accept an https mirror URL in TRELLIS_UPDATE_FEED besides a local folder
- Validate the MSI with `wix msi validate` (ICE checks) in the Installers workflow
- Check the MSI's Options page choices (no desktop shortcut, no folder menu, no scheme) with silent installs that set DESKTOPSHORTCUT, FOLDERMENU and LINKSCHEME to empty, and that an upgrade keeps them off
- Install the .deb through apt (`apt-get install ./file.deb`) to check dependency resolution on a minimal image
- Test the `<file>.sha256` single-checksum fallback end to end in the feed script (it is unit-tested)

### Language

- Key names in shortcut hints follow the language (e.g. "Strg" on German keyboards)
- Plural rules beyond one/other (needed if a language with other forms is added)
- Wrap or shrink right-aligned Preferences labels that are too long in a translation
- Load user-supplied translation files from the settings folder
- A palette command to switch the language
- Right-to-left layout for right-to-left languages
- Translate undo action descriptions (Action::describe, e.g. "checkout of main") and the labels of the jobs they name, shown in the Edit menu, toolbar tips, status bar and activity log
- Translate error texts that come from trellis-core: update check and download errors, theme file problems and the themes folder error
- Translate the tour of the main window (ui/tour.rs, state/onboarding_tour.rs) and its closing toast
- Translate git's missing or too-old banner text built in trellis-core (welcome Git step and window banner)
- Translate the folder picker titles (Add a project folder, Open a Git repository and the other Preferences pickers)
- Translate the palette's repository commands (e.g. Remove missing repositories from the lists)
- Translate the pull request panel's file list texts (only its rows' Copy path follows the language)
- Translate the rebase edit-menu reasons from trellis_core::rebase (This commit isn't on the current branch, merge selected, side line) and core's in_progress_text / DETACHED used by the rebase panel
- Translate the file history views' own texts (history.rs, history_folder.rs, history_file.rs, history_preview.rs, file_buttons.rs, file_view_switch.rs, diff/preview.rs) and drop the English constants kept for them (file_menu::BLAME, OPEN_DEFAULT, files::NOT_IN_WORKDIR, SHOW_IN_FOLDER, history NOT_ON_GRAPH, WRAP_LINES)
- Translate the find and go-to-line fields shared by blame and the file views (ui/find_bar.rs)
- Translate core's case-mismatch line in the File names banner (Snapshot::case_mismatch)
- Translate the external tool failure toasts (NO_DIFF_TOOL, NO_MERGE_TOOL, merge_end_text, unmatched quote) and the palette's tool details
- Translate the remaining JOB_RUNNING / blocked::BUSY users (tags, preference pages, fork and worktree dialogs, rebase quick edits)
- Translate the conflict view's undo descriptions and the merge tool job label shown in the status bar
- Translate the Stash dialog (its blocked reasons stay English with it until then)
- The conflict Output strip says "1 of 1 conflicts resolved": give it the header's singular and plural forms (its English snapshots change with it)
- A narrow conflict view cuts the panes' "picked automatically · whitespace only" tag in German; wrap it or drop the reason part when it doesn't fit
- The automatic-pick notes ("The conflict in this file was picked automatically…") are separate one / all texts; fold them into one plural text
- Translate the palette's rebase entries (Restore original order, Reload rebase steps)

### Whole-app journey tests

- Journey through an external merge tool, using a scripted fake tool that works on every OS
- Credential-prompt journey: the askpass server answering a password-protected local remote
- Journeys with the file watcher on, so outside edits show up without Refresh
- Interactive rebase journey, with the built executable as the sequence editor
- Multi-tab journeys: switching tabs while a job runs and checking each result reaches its tab
- Keyboard-driven journeys that press each OS's own shortcut keys
- Hosting-service journeys against a local fake API (pull requests, issues, build status)
- On a journey failure, save a rendered screenshot where wgpu is available, to make CI failures easier to read
- Accessible names for the remaining painted widgets (section headers, graph ref chips, sidebar ahead/behind counts), plus a test that every clickable widget has one
- Offer stash-and-retry when a checkout is refused because of local changes

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
- Rendered Markdown: an italic run followed by a space draws without the gap ("*try* the" reads "trythe")

### Dialogs keep what was typed

- Keep a discarded or closed form's draft per dialog kind and offer it again when the same
  dialog reopens
- Restore the keyboard focus to the field that had it after Keep editing, for dialogs that don't
  request focus every frame
- Count the Create issue form's chosen type and priority as edits (left out because loading the
  options fills their defaults)
- Create fork dialog: treat a typed remote name as an edit (it is a confirm today because the
  name is filled after the lookup)
- Ask before a trust prompt or another dialog replaces an edited form
- Ask before closing the window or the tab while an edited form is open

### In-progress operations stay visible and safe

- Keep resolved files recoverable when aborting with conflicts still unresolved (save the resolved files' blobs first, since git can't stash unmerged entries)
- Undo for aborting a rebase, a patch series or a multi-commit cherry-pick or revert
- Undo for Skip commit and Skip patch (the resolved files of the skipped step)
- Undo of a kept abort also brings back the merge state (MERGE_HEAD), not only the changes
- Keep the resolved changes in a stash also when unrelated edits are present (stash only the operation's paths, renames' old paths too)
- A dedicated undo label for the abort stash ("Undo abort of merge") instead of the stash's
- Prefill the commit draft from MERGE_MSG when a cherry-pick or revert stops for conflicts too
- Disable the Amend checkbox during a merge, with the reason on hover
- Bisect in the strip: the good and bad counts, and Mark good / Mark bad
- Keyboard shortcuts for Continue and Commit merge
- A status-bar chip variant of the strip, and a preference to hide the strip
- Hover tips on the conflict header menu's items (what Save with markers and Mark resolved do)
- Translate the remaining English text in the conflict view body and the rebase and patch banner titles

### No missing glyphs

- Put the ahead/behind counts into the LOCAL row's tooltip and accessible description ("2 ahead,
  3 behind origin/search"), so the painted arrows have words for screen readers
- A fallback font (system or bundled) for CJK, Hangul and other scripts in commit messages,
  author names, branch names and paths, which draw as boxes today
- Dates in the ja_JP, zh_CN and ko_KR locales use CJK month and weekday words the fonts lack, so
  the date preview in Preferences and every date show boxes
- A debug-build warning when a frame lays out a character with no glyph, for runtime text the
  static check can't see (server messages, theme names, hook output)
- Use the painted ahead/behind arrows wherever else counts appear (remote and Git Flow rows, tab
  tooltips, the dashboard)

### Readable controls in every theme

- Combo boxes get the field border and focus ring (they share egui's button visuals today)
- Menus, tooltips and popups get the stronger dialog_border edge (window_stroke still uses border)
- Dialogs in dark themes get a surface one step lighter than panel and a stronger shadow
- DragValue editors and other egui-internal text edits get the field border
- The selected row stays clearly different from a hovered row in the light themes
- Other chips reach 4.5:1: issue and PR state chips, PR thread chips, SSH key chips, the keyboard
  pill, the update and trust chips, the LFS badge, the graph's worktree and stash label chips,
  release-note chips
- The text cursor takes its colour from the palette
- Graph lane colours checked for contrast against bg in the light themes

### Truncation with an ellipsis and the full text on hover

- Narrow windows (below about 1100 px): the graph's right-aligned columns run over BRANCH / TAG
  and GRAPH, and the toolbar's right-hand buttons overlap the left ones; give columns way in turn
- The untrusted LFS note could carry a Trust… link that opens the trust dialog
- Fade out the last few pixels instead of drawing an ellipsis, as an appearance option
- Middle ellipsis for paths (keep the file name, cut folders) in file lists, repo sync paths and
  the stash dialogs
- Trim trailing spaces before the ellipsis ("Project …" becomes "Project…")
- Cut a very long issue or PR state when it would leave the title less than ~40% of the row
- Show the full 40-character SHA and the author's email in the graph cells' hover
- Fold the Conflicted and Resolved Files headers' buttons into the same more-actions menu
- A more-actions overflow for the main toolbar and the issue and PR panel headers when translated
  labels don't fit
- Translate the graph column titles (the floors already size the columns for longer titles)
- Cut the file history row's date when the panel is narrower than the date
- Show the whole text of a cut row when it gets keyboard focus, not only on hover

### Menus name their object once

- Palette entries name commits by summary instead of a short SHA ("Rebase main onto 1a2b3c4",
  "Reword commit 1a2b3c4…", "Squash 1a2b3c4 into parent")
- The disabled-rebase hover ("main already contains 1a2b3c4.") says "this commit" in menus and
  uses the summary elsewhere
- Copy > Message offers the full message (summary and body), not only the summary
- The commit, branch, remote-branch, tag and remote menus' English labels move onto the Lang
  tables (only the multi-commit menu is translated)
- The blame row, file history entry, stash, worktree and submodule menus get the same header,
  intent groups and Copy submenu
- Tag Push to submenu marks (or hides) the remotes that already have the tag
- Tag and branch Copy submenus add a link to the object on a chosen remote's hosting page
- The Hard reset confirmation says how many files will be stashed
- The remote group menu's header shows the repository path (owner/repo) weak next to the name

### Restyled toolbar and rich tooltips

- The other hover texts (sidebar rows, graph ref chips, file rows, status bar chips) move onto
  the rich layout: a title, a state line and a preview where one helps
- The pull menu's items preview what each does: the commits a rebase replays, or "can't
  fast-forward" for fast-forward only when the branches have diverged
- Push on a branch with no upstream previews the commits no remote has yet
- Last fetched is per repository (FETCH_HEAD); a per-remote time would tell a fetch of backup
  from one of origin
- Menus share the tooltip's 8 px corners through egui's popup frame; give menus their own
  radius if they should stay tighter
- A narrow window drops toolbar buttons off the right edge; move the ones that don't fit into a
  More menu
- Keycaps show a binding's first chord only; list a second chord when one is set
- Undo's preview of a rebase or history edit lists the rewritten commits side by side with the
  ones it brings back

### Dialog frame and button styles

- Draw the menu items, the issue picker's list and the remaining small inline buttons (diff hunk
  actions, file-row Stage) with the shared styles, or give them a documented style of their own
- Keep the trust prompt's footer the same between held and not-held prompts (Don't trust vs
  Cancel as the dismiss button)
- Show the primary button's text in a strong weight once the bundled font has one
- Remember a resizable dialog's size between openings

### Comfortable look

- An opt-in compact density in Preferences > UI Customization (Comfortable | Compact: about
  28 px rows on a 4 px grid), never the default
- 16 px padding and grid spacing for the bottom panels (activity log, terminal, conflict
  output), the banners and the Git Flow panel
- Preferences navigation and integrations side panels on the 16 px padding with 36 px rows
- Menu and popup margins on the 8 px grid, once dialogs no longer share egui's popup frame
  (menu_margin)
- A soft shadow or edge under the lifted side panels in light themes, beyond the palette step
- Retune the graph lane colours against the refreshed canvases (with the History table item)
- The graph filter banner (Showing only / hidden from the graph) on the sidebar's 16 px inset, its
  icon and text lined up with the rows' markers and names, without cutting its text

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

### A calm file header

- Compare a changed image as an overlay or with a swipe handle, beside Before and After
- Preview ICO, TIFF, AVIF and SVG images, drawn rather than shown as text
- Zoom and pan the image preview, with a pixel difference view
- Preview images stored in Git LFS from their local objects
- The image preview in the conflict view
- Keyboard keys for the View menu's choices (layout, section, wrap, ignore whitespace)
- Remember the View menu's last section per repository
- A renamed file's header names the old path too ("old → new", the arrow painted)

### One style for empty, loading, error and banner states

- Show a warning for a prefilled value (a remote branch's name in Create worktree) only once it is
  edited, like errors
- A banner with a determinate progress bar for operations that report progress
- Reach banner buttons with Tab and run them with Enter
- Empty states for the stash, worktree, submodule and activity lists
- Translate the pane titles and hints of the empty, loading and failed states
- Split `ui/mod.rs` and `state/mod.rs`, which are past 400 lines

### Line counts in red and green

- Line counts on the multi-commit, compare-with-working-tree and pull request file lists
- A tooltip on a file row's counts and bar ("12 lines added, 3 removed")
- Compact large counts (12.3k) in narrow panels
- Other worktrees' WIP rows drawn with the same chips and total
- Counts and totals for commits and working trees past the 2,000-file cap
- Fill the graph's CHANGES total from a selected commit's per-file counts when they arrive first
- Counts for Git LFS files from their real content
- Sort a file list by the size of its change
- A preference to hide the per-file bars

### Toggles look like toggles

- Blame header switch: after "Blame before this commit" or Back, Diff and File could select the blamed commit in the history instead of its original selection
- Segmented control: an accessible radio-group node with a group name, and Left and Right arrow keys to move between segments
- Blame header row 2: show Ignore whitespace and Wrap lines as checkboxes, like the diff's View menu
- Blame header: hide or rename the row-2 File history button while the Diff | File | Blame switch shows, since both lead back to the history
- Hunk header: fold the hunk buttons into one menu when the visible width is too narrow for body-size labels
- Search bar: measure the Highlight | Filter width instead of the fixed CONTROLS_W

### Full-row selection in file lists

- Up and Down arrow keys move the open file in the commit, WIP and pull request file lists, as the file history list already does
- Clicks in the panel padding beside a file row (where its selection now reaches) select the row too
- Right-click on a row's Stage / Mark resolved / Mark conflicted button opens the row's menu as well
- Double-click a file row to open it in the external diff tool
- Stable per-file row ids in scrolled file lists, so an open row menu stays with its file while the list scrolls
- A pointing-hand cursor over file rows, matching the file history list

### Layout that gives tool views room

- Toolbar name limits that grow with the window (repository and branch names are cut at 96 and 128 px at most; wide windows could show more)
- Toolbar groups that fold into an overflow menu, or captions that drop, when even the narrowest picker slot leaves them overlapping (with captions: under about 1250 px in English, 1330 px in German)
- Commit rail: a collapse control on the expanded commit panel, to go back to the rail without closing the tool view
- Commit rail: a preference for which tool views collapse the commit panel, or to never collapse it
- Commit rail: entries for an operation in progress (conflicted count, Continue / Abort) and separate staged and unstaged counts
- Collapse the left panel to a rail as well while a tool view is open
- Remember the expanded commit panel per tool view kind or per tab
- Status-bar links for a hidden left or details panel alongside the edge strips
- A command palette entry to expand the collapsed commit panel
- Toasts narrower than 360 px in windows too small for one plus its margins

### Type scale and text styles

- WIP and conflict list headers: show the count as a muted trailing badge instead of "(n)" in the title
- Host-key prompt: explain it in words, with the host and the fingerprint as separate monospace rows
- Activity log entries: a time column, an error icon and a Details expander
- Check the Spanish labels for sentence case too (German capitalises its nouns)
- A text-size preference that scales the five steps, apart from zoom
- A semibold monospace face for status letters and emphasised code
- Check that long context menus still fit small windows at 14 px items; scroll or regroup where they don't
- Tune the Preferences label column for the longest translated labels at 14 px
- The commit details' "ISSUES" / "CARDS" heading and its German and Spanish values are still all caps
- The graph's columns overlap (lanes under the Commit column) when the centre gets narrower than about 560 px

### Sidebar create actions

- Fetch all remotes from the REMOTE header menu
- Push all tags and a "Push new tags" toggle in the TAGS header menu
- Pop or apply the latest stash from the STASHES header menu
- Translate the left panel's section titles, menus and empty-state lines
- Open a focused header's menu with the menu key or Shift+F10
- A preference to hide empty sections (WORKTREES, SUBMODULES, TAGS, STASHES)
- Count only linked worktrees in the WORKTREES header, so the count doesn't jump from 0 to 2
  when the first one is added
- A PR header + that pushes the branch first (Push and create) when the host lacks it
- Let the PULL REQUESTS and ISSUES filter chip use the + and ⋯ room while they are hidden; the
  reserved room cuts the filter name short at the default panel width

### Clearer prompts and safer controls

- Stop in repo sync also kills the git processes already running (a cancellable gitcli run)
- Repo sync: Retry all failed rows at once, and Copy error from a failed row
- Host-key prompt: list the other names or addresses the key is known by as their own field
- Host-key prompt: a Copy fingerprint button and a hint linking to the service's published
  fingerprints
- Explain ssh's "host key has changed" refusal in the failed job's error, with the
  known_hosts line to check
- Sign-in dialog: a caps-lock warning on the secret field
- Terminal: Kill all names the programs running in each terminal before stopping them
- Commit more-actions menu: further per-commit options (sign-off trailer, sign this commit)
- Graph search: Esc in a non-empty field clears it before a second Esc closes the bar
- The commit button's label is cut with "…" in a narrow panel; show the whole label on hover
- The search bar's fixed count slot leaves the query field at its 160 px minimum in a
  740 px graph pane; shrink the slot or move the count under the field when narrow

### Trellis's own wording

- Translate the rest of the Commit signing page (labels, help, expiry warning, key generation)
- Translate the worktree WIP row's tooltip lines and change counts ("2 modified, 1 added")
- Read the OS's own short and long date patterns instead of the per-language table
- Regional date defaults beyond the language (en_CA and fr_CA in ISO order, pt_PT, de_CH)
- Genitive month names in Polish long dates ("1 lutego", not "1 luty")
- A "Use the locale's format" reset button beside each date pattern field
- Rename internal identifiers to the new names (ghost_refs, highlight_ref_rows,
  BranchVisibility::Smart, FilterOp::Solo, date_word_format, date_verbose_format), with serde
  aliases so old settings files still load
- The "Showing only …" banner line is cut short beside the wider "Show the rest" button in the
  default sidebar width; give the line more room or move the button under it

### Activity panel opens only when asked

- Name the tab in a failure toast raised for a tab that isn't active ("In web-app: ...")
- Reopen closed tab brings back that tab's activity lines from this session (today they return
  only next session, from activity.jsonl)
- Keep more earlier-session lines per repository in activity.jsonl (its 500-entry cap is shared
  by every repository)
- A marker on the toolbar's Activity button while the active tab has failures not yet seen
- An "All tabs" view in the activity panel that merges every tab's lines
- Clear in a repository tab also forgets the legacy untagged lines shown there (they age out)
- Say in the activity panel's header which tab's log it shows

### One label for a branch and its copies on every remote

- Honour `checkout.defaultRemote` (and `remote.pushDefault`) when picking the lead copy of a
  remote-only chip
- Also fold copies named like the upstream's branch when it differs from the local name (main
  tracking origin/trunk, with fork/trunk on the commit)
- A small count or distinct mark on a chip that folds copies on more than one remote
- Group a folded chip's refs under one submenu in the right-click menu, with actions such as
  delete on every remote
- Map remote-tracking refs to remotes through each remote's fetch refspec instead of the name
  prefix
- Say in the tooltip when a local chip's own upstream is at another commit while a copy on
  another remote is level with it

### Errors say what to do

- A Try again button on lock-file and HTTPS sign-in failures (needs the retry Msg for every
  job, not just guarded ones)
- A Delete lock file button when no git process is running in the repository
- Network causes in plain words (Could not resolve host, repository not found, 'origin' does
  not appear to be a git repository, timeouts)
- Count conflicted files from the refreshed status instead of git's CONFLICT lines (rename and
  directory conflicts are undercounted)
- Force push with lease as a second fix on the rejected-push toast
- Use the classifier for the auto-stash put-back failure toast (auto_stash::told still shows the
  last line) and for other flows' "<x> failed: <reason>" headlines (clone, submodule, gitflow,
  worktree)
- Keep git's "LF will be replaced by CRLF" warnings out of job logs, or fold them into one line
- Translate job labels so the fallback "<label> failed: ..." text is fully in the UI language
- A Fetch-then-fast-forward fix for a sidebar Fast-forward that was rejected as non-fast-forward
  on a non-HEAD branch

### Small fixes from testing

- Show the conflict chip for a failed automatic check too (with the error in its tooltip),
  instead of only logging the error once
- Let the conflict chip take keyboard focus, with Enter/Space opening its popover and Esc
  returning focus to it
- Give the repository tab bar's close × an accessible name (Close tab) without breaking queries
  for the tab menu's item of the same name
- Use `tab_glyph` for the other small painted × buttons (repository list, repository finder,
  welcome, tour) so every close cross in the app looks the same
- A middle-click or double-click on the empty tab-bar area opens a new tab, like the + does
- The chip's hover and click failure seen in the running app didn't reproduce headlessly (pointer
  tests, the whole app, 60 fps mouse moves); if it recurs, check egui's hit-test there with
  `debug.show_interactive_widgets`

### Brand colour and tinted neutrals

- Retint the diff, hunk and banner backgrounds onto the tinted neutrals (the blue-grey hunk_bg
  sits on green-tinted panels)
- Status-coloured chips on the raised fill (issue state, a thread's Outdated, rebase step chips)
  use the status chip tint, so their text reaches 4.5:1 in light
- Give errors and failures their own attention colour instead of removed's red, and success its
  own token instead of added's green
- Theme files: note when a file uses an old colour name (green, red, amber, blue, purple) and
  offer the new one
- Theme files set the repository group colours
- Move the high-contrast tag chips onto sand too (they keep their amber-tinted tags)
- Retune the favourite star and other non-warning uses of warn (favourites, CI pending) once
  Vivid colour gives each job its own colour
- A slightly stronger modal backdrop in Dark: its darker panel now drops only about 8 L* under
  the backdrop

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

### Window chrome and toolbar

- A Preferences option to keep the system title bar on Windows
- The DWM shadow and rounded corners around the drawn Windows frame
- Centre the macOS traffic lights on the taller title bar
- Follow macOS's title-bar double-click setting (zoom or minimize) instead of always maximizing
- F10 opens the title bar's menu, like an Alt tap
- Dragging the only tab moves the window instead of the tab
- Dim the window buttons under a dialog's backdrop
- Toolbar groups that don't fit fold into a ⋯ menu instead of crowding the pickers
- Cut long repository names in the middle on tabs, like the branch picker
- Vibrancy behind the macOS title bar, toolbar and sidebar
- Mica on Windows 11 21H2 (build 22000), which lacks the documented backdrop attribute
- A dark system menu under a dark theme
- The system menu opened by Alt+Space highlights its first item
- Mica behind Preferences' navigation and the status bar
- The build-status mark's cut-out ring matches a Mica sidebar
- The Maximize button's tooltip while Windows tracks it for Snap Layouts
- A Preferences switch for solid surfaces where Mica is available
- A slightly lighter Dark `faint`, so Dark's sidebar can sit on Mica too (it misses 4.5:1 over
  the lightest dark Mica stand-ins at every tint step and stays opaque)
- Resize from the top edge above the Maximize button (the hit test gives that strip to the
  button, for Snap Layouts)
- The stroked dropdown caret on the other dropdowns too (the diff header's View menu, the
  pull request filter chips, the terminal's shell caret, combo-style menu buttons)
- A quiet tab's faint repository icon on a Mica title bar: the Chrome surface's contrast checks
  leave out `faint`, so check it at 3:1 as an icon
- In a window too narrow for every group, a chip appearing still narrows the pickers (it counts
  in the right-hand block's width); fold the chips into the right-hand group's room instead
- Animate the dropped tab settling into its slot, like the others slide
- The system frame's tab bar lifts the active tab within its own 30 px (the tabs start 2 px
  lower); give it the drawn title bar's 34 px tabs

### Terminal panel

- Grey out shells in the caret's menu that aren't installed (probe PATH on a runner thread when the menu opens)
- Follow the shell's working folder from OSC 7 for the "program · folder" name
- Strip the elevated console's localised "Administrator: " prefix from window titles
- Remember custom tab names and the maximised state across restarts
- Keyboard shortcuts for maximise, rename and a new terminal with a chosen shell
- Drag pills to reorder terminal tabs
- Distinct painted icons per shell (PowerShell, cmd, bash, custom)
- Theme-file keys for the 16 terminal colours (custom themes take their base theme's table)
- Right-click menu on a pill: Rename, Restart, Close, Close others
- Bell or new-output indicator on inactive terminal tabs
- Restart terminal in the More menu when the active shell has ended

### Branch-led colour

- A tooltip naming the branch when hovering a lane line in the graph
- A pinned colour follows a branch rename, and pins of deleted branches are pruned at load
- A local branch whose upstream has a different name shares the upstream's colour
- A branch dot on WORKTREES rows (the worktree's checked-out branch) and in the command
  palette's checkout entries
- Remember resolved colours per repository so a branch keeps its colour when new neighbours
  appear
- Branch tones as theme-file keys
- Separate pins per palette preset (a pin keeps its slot number; 8 hues show it modulo 8)
- Share pins across a repository's worktrees (they are per working folder, like graph filters)
- Show on the Color submenu's Automatic row which hue it would give
- A preview strip of the chosen preset's hues on the Branch colors preference row
- Person colours in the blame gutter, the file history and the author picker
- Person colours that follow .mailmap (they follow the raw author email today)
- Pin a branch colour from the command palette
- Warn when a pin matches a side-by-side branch's pinned hue

### Activity panel design

- Attach extended logging's git command lines under the job that ran them instead of as notes
  (they arrive from the log worker and can race the job's end), so they no longer break an
  auto-fetch streak
- A live row for the job that is running, with its elapsed time and a Stop button
- When the activity file trims, drop routine auto-fetch lines first so they don't push older runs out
- Fix buttons for earlier sessions' failures, rebuilt from the saved cause when it still applies
- Filter the activity log to failures only, or search it
- Keyboard navigation of activity rows (arrows move, Enter or Space folds)
- Runs with durations for operations that are not jobs (stage, hunk and line operations, local
  file operations)
- Copy or export the whole activity log
- Selectable text inside the output block
- Hide a failed row's summary while it is open, since the cause sentence under it repeats it

### Dialogs and text fields

- Guide panes for sparse checkout, LFS and signing set-up once those flows get their own dialogs
  (today they are Preferences pages); the SSH key generation dialog from SSH preferences should
  use .guide too
- A tinted header band or a painted icon in each dialog's header (only the credential prompt
  has an icon today)
- Dropdowns (ComboBox) in dialogs at the 36 px field height, with the same tint and edge as text
  fields
- A way to hide the guide pane ("Hide how it works"), remembered for each dialog
- A field-fill colour key in theme files, with the contrast check run on custom themes
- A guide pane in resizable dialogs (Dialog rejects resizable + guide today)
- Preferences pages' text fields on TextField and FieldGroup (they keep their right-aligned
  label form)
- A short animation when the guide pane switches between beside and stacked as the window is
  resized
- The create issue dialog's label and people pickers at the 36 px field height inside field
  groups, like its text fields

### Real-input smoke tests (Linux)

- A Wayland pass (weston --backend=headless with wtype or ydotool) beside the X11 one
- More scale factors: 1.25 (fractional rounding) and 2.0
- More real-input gestures: a double-click on a branch chip, mouse-wheel scrolling of the graph,
  hover tooltips, dragging a repository tab and a graph column's resize handle
- Typing non-ASCII text and dead keys (a remapped keycode, a German layout through setxkbmap)
  and an IME pass
- A clipboard round trip with real Ctrl+C / Ctrl+V between Trellis and another X client
- Keep each step's screenshots as an artifact on success too, for visual review of release
  builds
- Share the probe's region_of with journey/find.rs so the journeys and the smoke test name
  regions the same way
- Run the smoke job against the packaged .deb (installed binary and desktop entry) instead of
  target/release

### Real-input smoke tests (Windows)

- A real per-monitor DPI pass at 150% (the runner's one virtual monitor is fixed at 96 DPI; it
  needs a second virtual display or a sign-out), including dragging the window across monitors
  with different DPI (WM_DPICHANGED)
- A UI Automation pass over the AccessKit tree beside SendInput (Invoke and Value patterns, and
  checking the names a screen reader gets)
- Alt+Tab back into Trellis with real keys (releasing Alt must not open the title bar menu), and
  Alt+Space opening the system menu
- The drawn title bar with real input: dragging the window by it, a double-click to maximize,
  Snap Layouts over Maximize, and the Minimize, Maximize and Close buttons
- Typing through another keyboard layout (German via LoadKeyboardLayout), dead keys,
  KEYEVENTF_UNICODE text and an IME pass
- A clipboard round trip with real Ctrl+C and Ctrl+V between Trellis and another window
- Run the Windows smoke job against the installed MSI instead of target/release
- Keep each step's screenshots as an artifact on success too
- One shared list of the labels and checks both smoke scripts read, so Linux and Windows can't
  drift apart
- A Windows ARM64 runner pass
- A fallback in the app when a transparent (Mica) OpenGL window can't be created, if the
  software renderer ever refuses it

### Welcome screen

- Grow the header's vine by one commit node per completed step
- Animate the progress bar's fill and the step change
- Arrow keys move the focus between the Start step's cards
- Give each start card its description as the accessible description, not only its title
- A Recent repositories card on the Start step when Trellis already knows repositories (Show
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

### Vivid colour

- Theme files set the repository hues and the coral (`repo_*`, `needs_you`, `on_needs_you`
  keys, with an ink fallback like `on_primary`'s)
- A Color row for the open repository on a repository page of Preferences
- A worktree group's strip takes its worktrees' colour when they share one, and an option to
  share one colour across a repository's worktrees
- Repository swatches in the Find repositories dialog, the tab list dropdown, the Window menu's
  tab list, and a "Set repository color…" command palette entry
- Prune repository colours of folders removed with Forget or Remove missing
- Failed operations whose cause is conflicts (error panes, toasts, the activity log) use the
  coral needs-you mark
- A coral conflict chip on the graph's WIP row while files are conflicted (conflicted paths are
  counted under M today)
- The conflict view's ours/theirs line tints move off info/progress at 18% to their own
  diff-family pair
- Retune the favourite star (warn) in the repository list and finder with its own token now
  that warn has left the chrome
- The "Merge in progress" strip under the toolbar takes the coral NeedsYou kind while files are
  conflicted, like the WIP panel's banner
- Name the conflicted rows' coral badge for screen readers and on hover ("Conflicted"), now that
  the "!" letter is gone
