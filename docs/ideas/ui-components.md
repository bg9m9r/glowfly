# Ideas: Layout and components

The design system, dialogs, toolbar, window chrome, lists, panels, truncation and review polish. Untriaged; see [README.md](README.md).

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
- Glowfly themes (Dusk and Dawn): redraw the view snapshots in Dusk and Dawn once the pair is
  settled (they draw in the Garden pair for now); theme-file keys for the colour-blind branch set; a gold distance test
  for custom theme files whose accent lands near Orange or Gold; Dusk's warm ref chips
  (main's 40% gold #735c3c, Orange's #503825, Lime's #414b3a) are dark tints of warm hues and
  still read tan or olive: try an indigo chip with a lane-coloured edge and text for warm lanes
  (row bands, toolbar groups and the selected row came out of brown in fix/dusk-no-brown)
- Glowfly themes (six more built-ins): a light twin for Ember and a dark twin for Linen if
  owners ask; theme files as cards in the picker (drawn from the file's own colours); repository
  colour hues tuned per theme (the new six reuse the standard repo set); a picker filter once
  there are more than ~16 themes; base-theme hints in the theme file help as a list, not a sentence;
  Graphite became the default dark theme (old "Dark" settings follow it): a light partner
  for it if Dawn reads mismatched beside it under Match system, and theme-file `base` falling
  back to Graphite rather than Dusk if owners want files to follow the default

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

### UI review polish

- Palette entries for New repository… and Clone repository… (and suggesting them when no repository is open)
- Translate the command palette's command labels, step titles, prompts and footers (the palette is English-only today)
- Translate the shallow-clone tooltip on Fetch full history (state::shallow::header_tooltip)
- Open a row's menu from the keyboard (Menu key / Shift+F10) below the focused row
- Make the multi-selection direction row's SHAs clickable to jump to each end commit
- Reach the graph's New branch… ghost button with Tab when its row has keyboard focus
- Rename the repository picker's empty placeholder "Open…" in the toolbar
- Suggested palette commands from more state (Pop stash on a clean tree with stashes, Create pull request for a pushed branch without one)
- A key for step 3's Skip (the Continue menu) alongside Continue's
- Deep-link Choose a repository: a "Remember for this repository" choice so the same link opens the same copy next time
- Tab groups: collapse and expand a group from its chip
- Tab groups: the chip follows its group while a tab is dragged (it hides during a drag now)
- Clone dialog: on Your repositories, hide the folder and clone options until a repository is picked
- Dashboard: alternate-row tint as an option alongside the width cap
- Translate the existing English strings of the trust prompt, the Clone dialog, the Git Flow Finish dialog and the deep-link dialogs
- Git Flow Finish: a choice to leave the auto-stash in the stash list instead of putting it back on the development branch
- Git Flow Finish: offer stashing when the dialog is reached with conflicted files (resolve or stash them first)
- Submodules: a legend of the status dots in the section header menu
- Git-not-found banner: a How to install git link to the per-OS download page, like the welcome step's Download git
- Preferences > Git: a way to show the too-old banner again after Don't show again
- Offer git's own updater (update-git-for-windows) from the banner when Git for Windows is the git in use
- Translate the git banner's core texts (GitInfo::banner/problem) and the feature labels
- Use error::advice for the other hosting-service errors (fork, create pull request, create issue, comments, dashboard, sign-in status)
- An Edit remote… fix on the remote-not-found and unreadable toasts
- Repository rows: plain sentences for more git failures (diverged pull, locked index)
- Recognise TLS certificate failures ("SSL certificate problem") with a next step
- Translate the safe.directory dialog and the Publish dialog strings
- A Copy button inside Details disclosures
- Use the shared Details disclosure in the SSH host-key prompt
