# glowfly-app UI: theme, colours, type and tokens (`crates/glowfly-app/src/ui/theme/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. What each colour is for is in [COLOUR.md](../COLOUR.md).

## Controls and modals

- `theme/controls.rs`: the themed form controls: text fields with a border and focus ring, and checkboxes and radio buttons drawn as visible boxes and circles; views use these, which `tests/controls.rs` checks.
- `theme::modal`: every modal: the themed backdrop and dialog border.
- `theme::show_modal`: shows a modal with its contents under one accessibility node of role Dialog (what screen readers and the text snapshots see).

## Colours

- `theme/contrast.rs`: contrast maths, the brown zone (`brownish`) with `unbrown`, `clear` and `clean_ink` that keep tints, edges, fills and chip letters out of it, and the graph's ref chip colours (neutral, edged chips).
- `theme/no_brown_tests.rs`: every built-in's surfaces, bands, chips, tints and washes, composited, and its accent, edges, branch lanes and chip letters, in both presets, stay out of the brown zone (Dusk's bronze selection excepted).
- `snapshot_tests/snapshot_tests_colour_sheet.rs`: an ignored test that renders every built-in's main window and chip strip into `GLOWFLY_SHEET_DIR`, for judging colour by eye.
- `theme/families.rs`: the token families: status, branch, repository, attention, and the terminal's colours.
- `theme/ansi.rs`: the terminal's colours, one table per built-in, with the readability check Preferences warns from.
- `theme/terminal_scheme.rs`: which colours the terminal draws in (the app theme's, another built-in's or an imported scheme over the app theme's), read through `color::term()`.
- `prefs/terminal_colours.rs`: Preferences > Terminal's Colors row.
- `prefs/theme_picker.rs`: the built-in themes drawn as grouped preview cards with one-line descriptions on Preferences' Theme row.
- `theme/branch_palette.rs`: the branch hues in two presets, standard and colour-blind friendly (the light tones trading Olive, Gold and a yellow for Wine, Navy and a clear amber), put into a palette by `applied`.
- `theme/branch_palette_gold.rs`, `theme/branch_palette_more.rs`: the families that make room beside a gold, iris (Dawn), flame or cobalt accent.
- `theme/branch_names.rs`: the branch hues' names in the language.
- `theme/branch_colours.rs`: a branch swatch's line and chip tones, with a person's initials colours.
- The branch colours are read by the graph's per-row colours (`graph/lane_colours.rs`) and the branch dots (`branch_dot.rs`).
- `theme/surfaces.rs`: the colour tokens worked out from a palette, so theme files get them with no keys of their own: the field, guide and toolbar-group fills, the overlay surface with its edge and inset, the hover, pressed and latched layers, a destructive menu item's text and hover pill (`menu_danger`, `danger_hover`), the semantic tones with their tints, the tonal fill and ink, and the brown-safe wash (`color::wash`) views use for translucent colour fills.
- `ui::theme::color` holds the active palette (a thread-local, see Rules); `ThemeCache` owns palettes built from theme files.
- `ui/backdrop.rs`: the Mica tints (see Window chrome in [flow-shell.md](flow-shell.md)).

## Fonts and type

- `theme/fonts.rs`: the bundled fonts: Noto Sans, its semibold family and JetBrains Mono ahead of egui's fallbacks, from `assets/fonts/`.
- `theme/fonts.rs` is the one font setup, installed by `GlowflyApp::new`, `theme::apply_style` and the glyph check.
- `theme/fonts.rs` ends both families with the CJK fallbacks: the system's (`platform/system_fonts.rs`), or in tests the Noto Sans CJK subset in `assets/fonts/test/`.
- `theme/missing_glyphs.rs`: in debug builds, a warning once per character a frame draws that no loaded font has.
- `theme/type_scale.rs`: the type scale: five steps, caption 11, small 12, body 14, subhead 16 and title 20 px, with nothing under the 11 px floor.
- The type scale has one section-header style, Noto Sans SemiBold at body size, and SemiBold titles.
- `theme::apply_style` installs the steps as egui's text styles.
- `common::with_code` sets prose with an inline monospace SHA.

## Spacing and tokens

- `theme/spacing.rs`: the spacing grid and panel padding, and the menu row metrics (comfortable and compact) and frame margin.
- `theme/tokens.rs`: the design tokens every theme reads: the radius scale with nested container radii, the Float and Dialog elevations, the focus ring, the inset ring for packed controls and a text field's halo, and the motion timings and easing, instant with Reduce motion.
