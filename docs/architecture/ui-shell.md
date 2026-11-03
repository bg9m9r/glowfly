# UI: the shell, window layout, toolbar, tabs and terminal panel (`crates/glowfly-app/src/shell/`, glowfly-ui)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. `ui/` holds the egui views, one file or folder per region: it
is the glowfly-ui crate (`crates/glowfly-ui/src/`, which the app names `crate::ui`); the
other UI files are in [ui-views.md](ui-views.md), [ui-widgets.md](ui-widgets.md) and
[ui-theme.md](ui-theme.md). How the shell's pieces behave is in [flow-shell.md](flow-shell.md).

## The shell

- `shell/mod.rs`: the shell, outside `ui/` (it stays in the binary); it draws with `ui::draw` and `UiCache`.
- `ui/lib.rs` (glowfly-ui's root): the views' modules, `draw` and `UiCache`; it names the model's modules as the app does, and `use crate as ui` keeps the views' `crate::ui::…` paths.
- `shell/app.rs` is the shell: it owns the runner and dispatches messages.
- `shell/app_effects.rs` carries out the effects that need the window (`Effect::Ui`): the clipboard and the Linux PRIMARY selection, the title, attention, closing and window commands.
- `shell/app_testing.rs` builds the shell for the journeys, without eframe and with a sandboxed runner.
- `shell/app_start.rs`: the startup steps in order (a test app leaves out those reaching outside the process), with a settings file the load couldn't read held before the first, since some save.
- `shell/texture_side.rs` caps the texture side egui may use (the `raw_input_hook`), so the font atlas is narrower and grows in smaller steps.
- `shell/app_frame.rs` times each frame's phases and each message's handling, kept by `state/frame_times.rs`: frames of 50 ms or more go to the performance log.
- `shell/app_native.rs` and `shell/app_menu.rs`: the shell's side of the drawn Windows frame's OS glue and of the macOS system menu bar (see [flow-shell.md](flow-shell.md)).
- `ui/frame_overlay.rs`: the frame-time overlay Preferences > General can show, drawn by the shell.
- `ui/app_icon.rs`: the app icon, for the window (`main.rs`) and the welcome screen's mark (`ui/app_mark.rs`).
- `ui/access.rs` gives painted buttons and hint-only text fields their accessible names (a toolbar button's name describes it and differs from the palette's rows), lets a form row's label name the unnamed fields, combos and boxes drawn under it (`with_label`, `from_label`), and makes a label a live region under a stable key (`announce`).
- `snapshot_tests/snapshot_tests_announce.rs`: the toasts' and the status bar job's live regions, and none with Announcements off.
- `central.rs` holds the central-area routing (which view fills it: the pull request panel and the issue panel (`issue/`) share one place) and the Esc chain.
- `panel_header.rs`: the pull request and issue panels' header links (Copy link, Open in browser), folded into a ⋯ menu when they would squeeze the title.
- `ui/probe.rs`: the smoke probe's state file (see Test-only modules in [app-layers.md](app-layers.md)).

## Window layout

- `frame_layout.rs` draws the window's panels in order.
- `panel_rails.rs` draws the side panels' rails.
- `toasts.rs` draws the toasts.
- `status.rs` draws the status bar (a running job's label, its transfer's progress and its Cancel, disabled with the reason while it merges); `live_updates.rs` its live-updates mark and label beside the version (a click refreshes only when updates are slow or off).
- `menu_anchor.rs` opens a menu that drops below a custom-drawn button (the title bar's menu button, the pickers).
- `menu_keys/`: the keyboard in every menu, run on each menu level's contents by `widgets::menu::styled` (menu spec §9): `level.rs` the keys (Up and Down wrapping, Home, End, Right and Left between a row's parts, typeahead, Esc and Left closing one level and giving the focus back), `items.rs` the level's focusable items in rows, `stash.rs` the keys kept from the views while a menu is open, `memory.rs` what is kept between frames; tests in `tests.rs`, `bar_tests.rs` and `view_tests.rs`.
- `repo_line.rs`: the 2 px line in the active tab's repository colour along the window's top edge; which tab shows which colour is `state/repo_colours.rs`.
- `panel_size.rs`: hands the drawers' fitted sizes (`state::panel_layout`) to egui.

## Toolbar

- `toolbar.rs` draws its buttons in borderless groups of quiet buttons set apart by dividers (`widgets::tool_group`, `tool_gap`; the buttons, toggles and split buttons in `widgets/tool.rs`).
- `toolbar_menus.rs`: the toolbar's menus.
- `toolbar_groups.rs`: the middle groups' buttons, Pull's and Push's count badges and named counts, the split buttons' menus (the push menu led by where Push goes, with Push to) and the groups' widths; `toolbar_sync_tests.rs` checks them at each ahead/behind.
- `toolbar_fold.rs`: which parts fold into the More menu in a narrow window, and in what order.
- `toolbar_more.rs`: the More button and its menu of the folded buttons.
- `toolbar_pickers.rs`: the repository and branch pickers, one framed segment bar whose segments each light whole, their name slots fitted to the names and shrunk only by a narrow window; the branch's ahead/behind counts sit inside its slot.
- `toolbar_fit.rs`: the pickers' fitted slots: the repository's name, and the widest local branch name (measured once per branch list) so a checkout never moves the bar.
- Each toolbar button's tooltip comes from `toolbar_tips.rs`.

## Repository tabs

- `tabs.rs` lays the repository tabs out: the system frame's tab bar, or the strip a drawn title bar holds.
- `tab_paint.rs` paints them, with the repository's swatch.
- `tab_drag.rs` drags them.
- Each tab is a Tab node in one TabList, named after it (selected while active); its × is "Close tab {name}" and closes on a middle-click or opens the tab menu on a right-click; `tabs_access_tests.rs` checks these and that Tab walks the bar's buttons.
- `tab_list.rs`: the tab list right after the tab bar's + button: a chevron button whose popup lists the open tabs (click one to select it) and the recently closed ones, newest first (click one to reopen it).

## Terminal panel

- `terminal/header.rs`: the terminal panel's 40 px header.
- `terminal/pill.rs`: its pill tabs, named by `state/terminal_names.rs`.
- `terminal/tabs.rs`: + and the shell picker.
- `icons_panel.rs`: the header's quiet icon buttons, painted.
- `icons_lists.rs`: the changed-file lists' List / Tree toggle icons and Accessibility's figure, painted.
- The panel is drawn under the central area or, maximised, in its place (`terminal::show_in_main`); renaming and maximising are handled in `update/terminal_header.rs`.
- `terminal/input.rs`: the grid's keys as shell input; Alt with a letter, digit or punctuation key sends ESC and the character (on macOS only with Option as Meta).
- `terminal/mouse.rs`: the wheel, selecting (a finished selection is reported for PRIMARY and Copy on select) and, on Linux, middle-click paste of PRIMARY.
- `terminal/metrics.rs`: the terminal grid's cell size, the monospace advance rounded to whole device pixels where epaint places each glyph, so text, cursor and mouse share one grid.
- The header's ghost buttons and dividers are shared with the diff header (`widgets/ghost.rs`, see [ui-views.md](ui-views.md)).
- `terminal/colors.rs`: terminal colours: the background, text and 16 basic colours the terminal draws in (`color::term`: the theme's own or the scheme chosen in Preferences), the xterm 256-colour cube and grey ramp, and truecolour as given.
- `terminal/grid.rs`: the terminal's screen grid: painting the active tab's cells, selection and cursor, reporting the grid size, the mouse (mouse.rs), the right-click menu, and forwarding the keyboard while it has focus (Shift with Page Up/Down, Home and End scroll the main screen's history).
- `terminal/held.rs`: the terminal panel's body while the active tab's shell is held (`SessionStatus::Held`): git is held in the repository until the user trusts it, and a shell prompt often runs git, so no shell starts until then.
- `terminal/paint.rs`: painting over the terminal's cells: the selection, the cursor in its chosen style (both in the terminal's colours, `color::term`), the dim overlay and the history's scroll thumb.
