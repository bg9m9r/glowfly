# UI: shared widgets, dialogs and states (glowfly-ui, `crates/glowfly-ui/src/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## Painted controls (`widgets/`)

- `widgets/mod.rs`: the shared painted controls.
- `icon.rs`: the `Icon` set.
- `tab_glyph.rs`: the tab bars' painted close × and new-tab + (one stroke and hover box for both).
- `scroll_cue.rs`: the always-visible thin scroll bar and the edge fades of dialog bodies, Preferences and the File view.
- `ghost::close_view`: the × that closes a view (the diff, blame, a history, an issue or pull request), with Close and its Esc cap in the tooltip.
- `widgets::remove_icon`: the weak × that removes a key in Preferences > Keyboard and closes a toast.
- `buttons.rs`: the button family, eight kinds: primary, tonal, secondary, quiet, danger, quiet danger, icon-only and split, sized by a `ControlSize` (`.size`, Regular by default; Compact quiet only).
- Buttons have one disabled look: a neutral fill and weak text, never an outline or egui's fade, and a `why` shown on hover.
- `button_look.rs`: each button kind's colours per state, and `paint_target`, where a toolbar target's state becomes pixels: the quiet layer over its own shape, a single framed control's border going weak while lit, and its focus ring on the same corners.
- `button_split.rs`: the split button (an action and its menu caret as one pill).
- `widgets/tool.rs`: the toolbar's buttons, toggles and split buttons, each lighting its whole slot; `widgets::tool_group` a borderless group and `tool_gap` the divider between groups.
- `widgets/segment_bar.rs`: flush segments in one shape (the toolbar pickers' framed bar, a split button's quiet one), each its own target, with full-height dividers in a framed bar; tests in `segment_bar_tests.rs`.
- `widgets/ghost.rs`: ghost buttons (at a `ControlSize`) and dividers, shared by the diff's file header, the terminal panel's header and the graph's New branch… button.
- `widgets/segmented.rs`: a segmented control at a `ControlSize` whose segments can be disabled with a reason on hover.
- `widgets/quiet_chip.rs`: quiet chips (the commit details' SHA, parents and committer).
- `widgets/details.rs`: a collapsed Details under a confirmation's plain words, also under the Publish dialog's rewritten service errors.

## Rows and lists

- `list_row.rs`: list rows, whose dim right-hand parts paint ahead/behind arrows, and the side panels' row heights and insets.
- `list_row.rs` holds the file lists' fixed-height `file_row` and `file_row_with_end`, whose right-hand part keeps its width while the path is cut, and `row_text` for their unnamed path.
- The file rows are one click target each, with the selection painted across the row and into the panel padding.
- `widgets/row_fill.rs`: one selection fill with a left accent bar and lifted hover, shared by the graph, sidebar rows, file rows and the file history list.
- `widgets/menu.rs`: the one menu style (row metrics from `theme/spacing.rs`'s `menu` tokens, transparent rows, `SM` pills) and the only ways to open a menu: `context_menu`, `submenu`, `popup_below`, `custom_button`, `below`, plus `separator` (never first or doubled), `group_header` (the one group-header style, in place of its separator), `inset` for plain lines at the label column, and `danger_style` / `danger_submenu` for destructive items; `tests/view_checks/menus.rs` checks.
- `widgets/menu_columns.rs`: a menu's shared columns (state, icon, label, reason slot, shortcut, end slot for a chevron or a trailing button), learned from its rows each pass so every row lines up.
- `widgets/menu_row.rs`: the menu row (`menu::item`): a ✓, radio dot or current-choice ✓ in the state column, a dot or swatch in the icon column, the label, the info mark and the shortcut on the menu's columns; `item_named` for a label with cut names, `trailing_button` + `show_split` for a split row.
- `widgets/menu_split.rs`: a split menu row's trailing icon button (its own target, focus, tooltip and name, blocked with its row or by its own reason) and `Part`, which part was clicked.
- `widgets/menu_name.rs`: names inside menu labels cut in the middle to `NAME_MAX` characters (`named`), the full label kept for the tooltip and accessible name.
- `widgets/row_menu.rs` (`RowMenuExt::row_menu`): right-click menus that open just below their row (at the pointer's x), or above it when they don't fit, sharing egui's context-menu state; Shift+F10 opens a focused row's menu (or a selected one's, `row_menu_for`) below it.
- `widgets/menu_open.rs`: opens egui's dropdown menus from their button with the keyboard too (Alt+Down, Right in a popup's menu; egui's Enter and Space), names the button for Esc's focus return, and keeps a press in a keyboard-opened submenu from closing the menu.
- `widgets/menu_submenu_keyed.rs`: a submenu opened from the keyboard (Right, Enter or Space on its row), drawn as its own menu where egui's would be, since egui opens its own only under the pointer.
- `widgets/truncate.rs`: single-line text cut with an ellipsis and its whole text on hover, and names and paths cut in the middle.
- `widgets/section_header.rs`: the left panel's section headers with their hover + (the section's create action) and ⋯ (its menu), and the empty state line under an empty section; each + is described by `state/section_actions.rs`.
- `line_counts.rs`: line counts drawn one way everywhere: a file row's `+N −M` and proportional bar, a total, the WIP row's chips.
- Added, removed and modified share `common::added_color`, `removed_color` and `modified_color` with the status letters.
- `date_tip.rs`: date labels with their full date on hover.

## Disabled reasons

- `widgets/reason.rs` draws disabled reasons and setup suffixes: a weak line beside or under a disabled button.
- A dialog footer's reason sits in the row before Cancel, or above it when the row is full; laid out by `dialog_footer.rs`.
- `widgets/menu_reason.rs` (re-exported by `widgets/reason.rs`): a menu item's states (`ItemState`): a greyed label with a painted info mark and its reason in the shared tooltip on hover or focus (described to screen readers too), "Set up…" in the shortcut column, or a destructive (`Danger`) item in the danger style; drawn by `menu_row.rs`.
- `widgets/menu_submenu.rs` (tests in `menu_submenu_tests.rs`): what `menu::submenu` adds to egui's submenu: a 250 ms open delay on hover (at once on a click, Enter, Space or Right), a 400 ms close grace, and placement right of the menu or flipped left, kept 8 px inside the window; `row_at` maps a submenu row's auto-id slot to its button for `menu_keys`; Right, Enter or Space opens it from the keyboard (`menu_submenu_keyed.rs`), and Left or Esc closes egui's own.

## Tooltips

- `widgets/tooltip.rs`: every rich tooltip shares one frame, timing and layout: title, keycaps or a state line, body, an inset preview and a footer.
- `widgets/tooltip_motion.rs`: the tooltip's fade and rise, off with Reduce motion.

## Empty, loading, failed and banner states

- One style for every empty, loading, failed and banner state.
- `empty_state.rs`: an empty pane's icon, title, hint and buttons; its painted icons in `empty_state_icons.rs`.
- `graph_empty.rs`: the graph's empty state.
- `history_footer.rs`: the file history list's empty state and its footer's.
- `pane_status.rs`: a pane that is loading or failed, with Try again and Copy details; shows an error without its kind prefix (`error::plain_message`).
- `widgets/banner.rs`: the banner in five kinds, Info, Warning, Error, Progress and NeedsYou, with a mark and a coloured stripe.
- `needs_you.rs`: the coral "needs you" badge and pill, always with the warning mark and the only reader of the coral.
- `widgets/severity.rs`: the error, warning and information marks.
- `widgets/form_message.rs`: a form's messages, under a field once it was edited or a submit was tried (`state/touched.rs`), or for the whole form in a tinted box.
- `tests/view_checks/message_styles.rs` checks banners and every dialog's messages.

## Dialogs

- `dialog_frame.rs`: the frame every dialog is drawn in.
- `dialog_header.rs`: a header with the title, an optional icon and subtitle and a painted close button that acts as Esc.
- `dialog_size.rs`: a size class, Small 400–480, Medium 480–560 or Wide 640–760 px, following a long title up to its most and capped by the window.
- The dialog body scrolls or resizes.
- `dialog_guide.rs`: for a less familiar operation, a tinted side pane that explains it in numbered steps under "How it works", beside the form or, in a narrow window, stacked above it; the steps are in `dialog_guides.rs`.
- The footer has one order: extras, then Cancel or Close, then the actions with the primary last.
- Enter runs the default button, and the default gets the focus on the first frame.
- `Dialog::header`: a header the caller draws instead of the title row (the welcome's band, which may paint into the padding).
- `Dialog::width` sets the content width; `Dialog::min_body` a least body height.
- `close_button` places the shared close box.
- `widgets/text_field.rs`: a dialog's text fields: 36 px with the text centred, tinted with `color::field_fill`, a Browse… button beside one with `with_button`, and `entered` for a field's Enter.
- `widgets/field_group.rs`: the label-field-help group a text field sits in: the label above, a weak Optional marker, then one line under the field, its message or else its help.
- `confirm_dialog.rs`: the confirmation dialog and its danger form.
- `dialog_guard.rs`: the "Discard your changes?" question (see Dialogs in [flow-shell.md](flow-shell.md)).
