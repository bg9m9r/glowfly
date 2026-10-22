# Glowfly backlog

The items behind [ROADMAP.md](ROADMAP.md), which holds the goal, the milestones and their status.
What Glowfly does today and what is planned next, roughly in priority order: the current
milestone's section first. Each item is a target to design, not a finished spec. A top-level
item is the unit of work: it is ticked when its core behaviour works and every sub-item is done
or moved to [docs/ideas](docs/ideas/README.md). A substantial new feature becomes its own top-level item; smaller
ideas, polish and follow-ups go to [docs/ideas](docs/ideas/README.md) as plain bullets. [Later](#later) holds only triaged
Milestone 2 work, built once the open items are done; nothing in docs/ideas is built until a triage
moves it.

Want to help? Pick an unticked item, open an issue to discuss the approach, then see
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Recent changes

Not kept here: `git log --oneline main` lists what landed, in the Conventional Commit subjects.

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
have landed too (see `git log`).

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

- [x] **Glowfly's own wording**: rewrite these labels and help texts in fresh words of our
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
      Glowfly draws one title bar with the tabs, a ☰ menu (Alt still opens File, Edit, View…)
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
- [x] **Terminal colour themes**: the terminal's background, text, cursor, selection and 16
      basic colours are theme-file keys (`terminal_*`), and Preferences > Terminal > Colors picks
      Match app theme, a built-in theme's terminal colours or a scheme imported from Windows
      Terminal (a scheme or settings.json) or iTerm2 (.itermcolors), warning quietly when its
      colours are hard to read on its background.
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
      (760 px) welcome with a tinted header holding a lattice drawing in Glowfly's own style,
      a four-segment progress bar, the user's name from the first step, and the three ways to
      start as large cards side by side; the drawing needs light and high-contrast versions.

What [Milestone 1](ROADMAP.md#milestone-1-first-release-in-progress) needs that tests and CI can
check. Its hands-on criteria (daily use, merging the release pull request) are in ROADMAP.md.

- [x] **Working-changes panel, step by step** (direction B, chosen 2026-10-21): while a merge,
      rebase, cherry-pick, revert or patch series is in progress the WIP panel shows numbered steps
      (Resolve conflicts, Check what goes in, Commit or Continue pinned at the bottom) under a heading
      with a quiet Abort; done steps fold to a line and reopen; the plain panel shares the look:
      plain headers with counts, row actions at the row end, no pill before the name
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
      start Glowfly from them, uninstall cleanly, and update an older build to the new one
      against a local release feed with checksums

---

## Preferences parity (page by page)

The pages a full-featured Git client offers, and where Glowfly stands:

| Page | Glowfly | Gap |
|---|---|---|
| General | ✔ General | Missing: delete `.orig` files after merge, show all commits, path to `sh.exe`, extended logging, conflict detection |
| Profiles | ✔ Profile (identity, connected accounts and app preferences per profile, switching) | Follow-ups in Ideas > Profiles |
| SSH | ✔ SSH page (key cards with their marks, a Generate SSH key dialog, Use existing for keys outside ~/.ssh, Which key dropdowns for all repositories and the open one, the local agent with a status pill, add a key to a hosting service) | Follow-ups in Ideas > SSH key management |
| Integrations | ✔ 10 services | PR/issue depth (P2) |
| AI | ✘ | Possible later: opt-in features on the Claude API |
| External Tools | ✔ Editor, diff tool and merge tool (the external terminal is on the Terminal page) | Follow-ups in Later > External tools and Ideas > External diff and merge tools |
| Notifications | ✔ location, behind alerts and desktop notifications under UI Customization | Per-kind choices (Ideas > Notifications) |
| UI Customization | ✔ UI Customization | Repo group colors (wait on repository groups) |
| Commit Signing | ✔ Commit Signing | Signature badges on graph rows and tags (P1 follow-ups) |
| Diff viewer | ◐ Diff viewer (font size, tab size, line numbers, syntax and word highlights, word wrap) | Font family, EOL character |
| Terminal | ✔ Terminal page (built-in: shell, font size, colors (app theme, a built-in's or an imported scheme), cursor style, dim when unfocused, copy on select, middle-click paste (Linux), Open in terminal in the panel; external: program and arguments) | Font family, line height (Ideas > Built-in terminal) |
| Experimental | ✘ | Git executable selection and version (P3) |
| *Repo:* Encoding | ✘ | Per-repo file encoding for diffs |
| *Repo:* Gitflow | ✔ Git Flow page, Start and Finish, Git Flow panel, branch from issue | Finish options (Ideas > Git Flow) |
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
      (`YYYY-MM-DD HH:mm`) for a locale Glowfly has no row for. Date shows in file history and
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
  `git rebase -i` with `GIT_SEQUENCE_EDITOR` set to Glowfly itself (`--write-todo`), which
  installs the prepared todo after checking it lists the same commits as git's; rewords are
  `exec git commit --amend -F` lines. Edit steps and conflicts stop in the "Interactive rebase
  in progress" banner with Continue / Skip commit / Abort rebase. The panel is kept per tab.
  The list also takes exec and break steps, a typed message for the commit a squash group
  combines into, and keyboard control (arrows, Alt+arrows, action letters, X / B, Enter); "Edit
  remaining steps…" opens it on a running rebase's todo (`rebase --edit-todo`, also for one
  started outside Glowfly), rebases and graph edits undo in one step back to where the branch
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
  Glowfly is git's and ssh's askpass helper: HTTPS sign-in (username and password or token in
  one dialog), SSH key passphrases, SSH passwords and host-key confirmations open an in-app
  prompt, with retry and rejected-credentials notices, cancel and a 5-minute timeout.
  Background jobs and hosting-service sign-in never prompt. HTTPS sign-ins can be remembered
  with git's credential helper (setting up the system's store when git has none), OpenSSH
  before 8.4 asks through Glowfly on Linux and macOS, and auto-fetch skips a remote that needs a
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
  hooks can run arbitrary commands as soon as git touches it. Before Glowfly runs git in a
  repository it didn't clone or create (auto-fetch, hooks), ask whether to trust it.
  - [x] Ask on first open of an untrusted repository, naming the config keys and hooks that
    would run commands; repositories Glowfly cloned or initialised are trusted automatically
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
  Ask for GPG passphrases for commit and tag signing in Glowfly (a Glowfly pinentry, or loopback
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
  - [x] Paginate PR and issue lists. Glowfly stops at 100 (50 on Bitbucket Cloud); `morluto/rea`
        already exceeds that.
  - [x] Fetch review states for every PR. Only the first 25 are fetched today, to save API calls.
  - [x] Jira key detection treats strings like `UTF-8` as keys unless a project filter is set.

## P3: app polish and platform

- [x] **Custom themes**: High Contrast Dark, High Contrast Light and Glowfly Sand built in, plus
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
- [x] **Command-line companion**: `glowfly <path>` and `glowfly .` work (subcommands are under
      Later)
- [x] **Deep links**: `glowfly://` links to a repository, branch, commit or tag (named by root
      commit and remote URL), copied from the branch, tag, remote and commit menus and the
      palette, opened in the running window with Locate, Clone and Fetch when they don't match,
      and registered per user in Preferences > General (Windows and XDG desktops)
- [x] **Packaging and updates**: an app icon; MSI, DMG and DEB installers next to the zip and
      tar.gz downloads, with "Open in Glowfly" on folders and `glowfly://` links; code signing in
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
follow-ups go to docs/ideas, not here.

### New directions

Chosen on 2026-10-19 as the features that make Glowfly its own, in this order.

### File history and blame

- Open folder history from a folder row once file lists can show a tree

### Limits found while building integrations

- Live-test GitLab, Bitbucket Cloud and Data Center, Azure DevOps and Jira against real accounts, including the Azure DevOps sign-in through Git Credential Manager (Microsoft Entra Bearer token)

### In-app pull request view

- GitLab merge request notes, approve / unapprove and merge (merge, squash, fast-forward) in the
  panel
- Bitbucket Cloud and Data Center comments, approve / request changes and merge strategies in the
  panel

### Packaging and updates

- More formats: Flatpak
