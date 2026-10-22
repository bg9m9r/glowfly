# Ideas: Navigation, menus and shortcuts

The command palette, keyboard shortcuts, tabs and the menu bar. Untriaged; see [README.md](README.md).

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

### Menu bar

- A key to show or hide the menu bar, and Alt showing it for one use while it is hidden
- About Glowfly and Quit as command palette entries
- A Show menu bar button in the status bar while the menu bar is hidden
- Repository and Branch menus (fetch, pull, push, stash, new branch and tag)
- Recently closed tabs as a submenu, to reopen any of them
- The OS and architecture in the About dialog's copied version info
- Open the submenus (Open recent, Theme) from the keyboard
- F10 focuses the first menu
- Mnemonics for the items inside menus
- Let translators pick each menu's mnemonic in the catalog
- Preferences > Keyboard warns when an Alt+letter key hides a menu's mnemonic
- Hide the underlines once the pointer is used in a menu opened from the keyboard
- Ask before quitting when tabs hold unsaved work (reuse `state/tab_losses.rs`) or Preferences has unsaved repository settings
- Quit by itself once the running operation ends while the quit question shows
- Quit anyway stops a stoppable job (fetch) first, so its lock files are cleaned up
- Also ask before quitting while a sparse clone, a terminal program or other background work runs
- Keep the open dialog under the quit question instead of replacing it
- On macOS, the Dock menu's Quit and logging out go through the quit question (needs `applicationShouldTerminate:`, which winit's application delegate owns)
- macOS menu keys (Cmd+Q, Cmd+T…) work while the window is minimized and there is no key window
- Cut, Copy, Paste and Select All in the macOS Edit menu, sent to the focused text field
- Minimize (Cmd+M), Zoom and Bring All to Front in the macOS Window menu
- Keep AppKit from adding its own Enter Full Screen, dictation or emoji items beside Glowfly's
- Mark folders that are gone in the macOS Open recent menu (the drawn rows' "not found" tooltip), and remove a recent entry from it
- Preferences > Keyboard warns on macOS that Cmd+H and Option+Cmd+H hide Glowfly instead of running a binding
- Section headers (macOS 14's `NSMenuItem.sectionHeader`) instead of disabled title items for Diff layout, Diff shows, Favorites and Recent
- A Dock menu listing the open tabs and recent repositories
