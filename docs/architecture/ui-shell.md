# glowfly-app UI: the shell, window layout, toolbar, tabs and terminal panel (`crates/glowfly-app/src/ui/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. `ui/` holds the egui views, one file or folder per region; the
other UI files are in [ui-views.md](ui-views.md), [ui-widgets.md](ui-widgets.md) and
[ui-theme.md](ui-theme.md). How the shell's pieces behave is in [flow-shell.md](flow-shell.md).

## The shell

- `ui/app.rs` is the shell: it owns the runner and dispatches messages.
- `ui/app_testing.rs` builds the shell for the journeys, without eframe and with a sandboxed runner.
- `ui/app_frame.rs` times each frame's phases and each message's handling, kept by `state/frame_times.rs`: frames of 50 ms or more go to the performance log.
- `frame_overlay.rs`: the frame-time overlay Preferences > General can show.
- `ui/access.rs` gives painted buttons and hint-only text fields their accessible names (a toolbar button's name describes it and differs from the palette's rows).
- `central.rs` holds the central-area routing (which view fills it: the pull request panel and the issue panel (`issue/`) share one place) and the Esc chain.
- `panel_header.rs`: the pull request and issue panels' header links (Copy link, Open in browser), folded into a ⋯ menu when they would squeeze the title.
- `ui/probe.rs`: the smoke probe's state file (see Test-only modules in [app-layers.md](app-layers.md)).

## Window layout

- `frame_layout.rs` draws the window's panels in order.
- `panel_rails.rs` draws the side panels' rails.
- `toasts.rs` draws the toasts.
- `menu_anchor.rs` opens a menu that drops below a custom-drawn button (the title bar's menu button, the pickers).
- `menu_keys/`: the keyboard in every menu, run on each menu level's contents by `widgets::menu::styled` (menu spec §9): `level.rs` the keys (Up and Down wrapping, Home, End, Right and Left between a row's parts, typeahead, Esc and Left closing one level and giving the focus back), `items.rs` the level's focusable items in rows, `stash.rs` the keys kept from the views while a menu is open, `memory.rs` what is kept between frames; tests in `tests.rs`, `bar_tests.rs` and `view_tests.rs`.
- `repo_line.rs`: the 2 px line in the active tab's repository colour along the window's top edge; which tab shows which colour is `state/repo_colours.rs`.

## Toolbar

- `toolbar.rs` draws its buttons in groups, each on a recessed surface (`widgets::tool_group`; the buttons, toggles and split buttons in `widgets/tool.rs`).
- `toolbar_menus.rs`: the toolbar's menus.
- `toolbar_groups.rs`: the middle groups' buttons, the split buttons' menus and the groups' widths.
- `toolbar_fold.rs`: which parts fold into the More menu in a narrow window, and in what order.
- `toolbar_more.rs`: the More button and its menu of the folded buttons.
- `toolbar_pickers.rs`: the repository and branch pickers in fixed slots so the bar never moves when the names change.
- Each toolbar button's tooltip comes from `toolbar_tips.rs`.

## Repository tabs

- `tabs.rs` lays the repository tabs out: the system frame's tab bar, or the strip a drawn title bar holds.
- `tab_paint.rs` paints them, with the repository's swatch.
- `tab_drag.rs` drags them.

## Terminal panel

- `terminal/header.rs`: the terminal panel's 40 px header.
- `terminal/pill.rs`: its pill tabs, named by `state/terminal_names.rs`.
- `terminal/tabs.rs`: + and the shell picker.
- `icons_panel.rs`: the header's quiet icon buttons, painted.
- `icons_lists.rs`: the changed-file lists' List / Tree toggle icons, painted.
- The panel is drawn under the central area or, maximised, in its place (`terminal::show_in_main`); renaming and maximising are handled in `update/terminal_header.rs`.
- `terminal/metrics.rs`: the terminal grid's cell size, the monospace advance rounded to whole device pixels where epaint places each glyph, so text, cursor and mouse share one grid.
- The header's ghost buttons and dividers are shared with the diff header (`widgets/ghost.rs`, see [ui-views.md](ui-views.md)).
