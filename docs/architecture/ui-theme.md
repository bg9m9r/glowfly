# UI: theme, colours, type and tokens (`crates/glowfly-ui/src/theme/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here. What each colour is for is in [COLOUR.md](../COLOUR.md).

## Controls and modals

- `theme/controls.rs`: the themed form controls: text fields with a border and focus ring (`SizedField`: Regular 28 px inline or Large 36 px, padding from the size), and checkboxes and radio buttons drawn as visible boxes and circles; views use these, which `tests/view_checks/controls.rs` checks.
- `theme::modal`: every modal: the themed backdrop and dialog border.
- `theme::show_modal`: shows a modal with its contents under one accessibility node of role Dialog (what screen readers and the text snapshots see).

## Colours

- `theme/contrast.rs`: contrast maths, the brown zone (`brownish`, and `brownish_text` for coloured text) with `unbrown` and `clear` that keep tints, edges, fills and chip letters out of it, and the change and selected-diff-line fills.
- `theme/ref_chip.rs`: the graph's ref chip colours: filled local branches, outlined remotes and tags, the halo on the checked-out branch and HEAD, each standing 3:1 from every row surface.
- `theme/ref_chip_tests.rs`: the ref chips in every built-in and both presets: text on the fill or every row surface, fill or edge 3:1 from every row surface, the halo only where you are.
- `theme/no_brown_tests.rs`: every built-in's (and the Garden pair's) surfaces, bands, chips, tints and washes, composited, and its accent, edges, branch lanes, chip letters, lantern and HEAD glow, in both presets (the colour-safe one with its diff and status colours), stay out of the brown zone, with no exception.
- `theme/code_sel_tests.rs`: the selected-code fills (`code_sel`, `code_sel_dim`) in every built-in: text and syntax contrast on them, their OKLab distance from the row colours and each other, and the designer's measured values.
- `theme/inactive.rs`: `selected_inactive`, the selection of a pane without the keyboard focus: the designer's values for the built-ins, and for other palettes a neutral in `selected`'s hue worked out by the same rules.
- `theme/text_floor_tests.rs`: placeholders, the status bar's `faint`, `weak` on `selected_inactive` and a search-dimmed row's text at 4.5:1 in every built-in; the find fill readable and visible on every row surface; the live-updates marks at 3:1 on `toolbar`.
- `theme/inactive_tests.rs`: the built-ins' `selected_inactive` are the designer's values, and it and the worked-out value are neutral (chroma at most 0.02, `selected`'s hue, never brown), 0.03 (OKLab ΔE) from `selected` and `hover`, and keep `weak` text at 4.5:1.
- `theme/no_brown_text_tests.rs`: every built-in's coloured text (status, syntax, terminal colours) stays out of the text brown zone, and the light themes' amber text reads at 4.5:1 on its surfaces (High contrast light's amber excepted).
- `theme/more_themes_tests.rs`: the lineup's pinned values (Porcelain, and the Midnight, Dusk and Dawn retunes) and the six sleekness rules: one accent, a quiet selection, the accent's chroma, the border on the panel, the surface ladder and distinct themes of a kind.
- `theme/lantern_tests.rs`: the `lantern` token at 3:1 on every row fill, `text` in high contrast, apart from `modified` and `warn`, and the HEAD glow visible but quiet.
- `theme/lineup_contrast_tests.rs`: the lineup's measured contrast, as the Themes v2 spec gives it, within 0.02.
- `snapshot_tests/snapshot_tests_colour_sheet.rs`: an ignored test that renders every built-in's main window and chip strip into `GLOWFLY_SHEET_DIR`, for judging colour by eye.
- `snapshot_tests/snapshot_tests_colour_safe.rs`: the colour-blind safe goldens: a diff with selected and changed-word lines beside A/M/D/R files (Graphite, Porcelain, high contrast light; split in Graphite) and the graph's dashed lanes.
- `snapshot_tests/snapshot_tests_chips.rs`: the chip catalog golden: every chip and badge kind from its real painter, the standing check of the two chip heights.
- `theme/families.rs`: the token families: status, branch, repository, attention, and the terminal's colours.
- `theme/ansi.rs`: the terminal's colours, one table per built-in, with the readability check Preferences warns from.
- `theme/terminal_scheme.rs`: which colours the terminal draws in (the app theme's, another built-in's or an imported scheme over the app theme's), read through `color::term()`.
- `prefs/terminal_colours.rs`: Preferences > Terminal's Colors row.
- `prefs/group_colour_row.rs`: Preferences > UI Customization's New group color row: a combo of Next unused color, None and the hues with their swatches.
- `prefs/accessibility.rs`: Preferences > Accessibility: the zoom and text size rows of their own pages, High contrast, the branch colours, motion, the cursor blink, the shortcut links and the toast and tooltip timings, each control named for screen readers; tests and goldens in `prefs/accessibility_tests.rs`.
- `theme/a11y.rs`: the style Accessibility sets each frame: animation time, tooltip timing, the text cursor's blink and the focus rings' style (Standard or Strong).
- `prefs/theme_picker.rs`: the built-in themes drawn as grouped preview cards with one-line descriptions on Preferences' Theme row; with Match the system on, the group headings say when each is used and both of the pair's cards are ringed.
- `prefs/theme_row.rs`: the Theme row: the picker, then the Custom theme files disclosure (the theme file dropdown, the folder's buttons, the help and the file's problems).
- `prefs/theme_files.rs`: the Custom theme files disclosure's header, a quiet target that stays open while a theme file is in use.
- `theme/branch_palette.rs`: the branch hues in two presets, standard and colour-blind friendly (the light tones trading Olive, Gold and a yellow for Wine, Navy and a clear amber), put into a palette by `applied`.
- `theme/branch_palette_gold.rs`, `theme/branch_palette_more.rs`: the families that make room beside a gold, iris (Dawn) or cobalt accent.
- `theme/branch_names.rs`: the branch hues' names in the language.
- `theme/branch_colours.rs`: a branch swatch's line and chip tones (main's trunk tone among them, and the slots that pass for it, `trunk_near`).
- `theme/initials.rs`: a person's initials colours: eight muted tones by email, `text` initials. Tests in `initials_tests.rs`.
- `theme/colour_safe.rs`: the colour-blind safe preset's diff and status colours (blue added, orange removed, violet modified), each at its theme colour's luminance; `colour_safe_tests.rs` holds them apart under simulated colour vision and at the contrast floors in every built-in.
- `theme/vision.rs`: simulated colour vision (normal, protan, deutan, tritan), the OKLab distance and the floors under which two lines pass for each other.
- The branch colours are read by the graph's per-row colours (`graph/lane_colours.rs`) and the branch dots (`branch_dot.rs`).
- `theme/surfaces.rs`: the colour tokens worked out from a palette, so theme files get them with no keys of their own: the graph's row bands and highlights (`band_color`, `highlight_color`) and every surface a row's chip sits on (`with_row_surfaces`), the field, guide and toolbar-group fills, the framed toolbar bar's edge (`tool_edge`), the overlay surface with its edge and inset, the hover, pressed and latched layers, a destructive menu item's text and hover pill (`menu_danger`, `danger_hover`), the semantic tones with their tints, the tonal fill and ink, and the brown-safe wash (`color::wash`) views use for translucent colour fills.
- `ui::theme::color` holds the active palette (a thread-local, see Rules); `ThemeCache` owns palettes built from theme files, and the slots near the trunk the shell reports to the model.
- `ui/backdrop.rs`: the Mica tints (see Window chrome in [flow-shell.md](flow-shell.md)).
- `theme/builtin.rs`: the built-in palettes, by theme: Graphite and Porcelain (the defaults, `builtin_cobalt.rs`), Midnight and Mist (`builtin_sea.rs`), Dusk and Dawn (`builtin_dusk.rs`) and the high-contrast pair (`builtin_extra.rs`).
- `theme/builtin_cobalt.rs`: Graphite and Porcelain, the cobalt pair and the defaults: near-neutral graphite greys (chroma under 0.01, a hair cool) and neutral white paper with grey-black ink, both under a cobalt accent in its own band (243 to 259 deg) with the cobalt branch families (`branch_palette_more.rs`).
- `theme/builtin_dusk.rs`: Dusk and Dawn, the pair drawn from the app icon: its dusk tile (indigo into plum) tints the neutrals, and the glowfly's lantern gold is Dusk's accent, so the main branch runs in gold as the icon's first lane does; Dusk's selected row is an indigo step under the gold bar.
- `theme/builtin_extra.rs`: the high-contrast palettes, and the repository hues every palette takes (see docs/COLOUR.md for the rules they meet).
- `theme/builtin_garden.rs`: the Garden pair: the standard themes before Dusk and Dawn, kept for those who like the green.
- `theme/builtin_sea.rs`: Midnight and Mist, the sea-glass pair: a deep blue-black night and its light twin of cool fog paper.
- `theme/color.rs`: the active color palette.
- `theme/custom.rs`: palettes from theme files: the base theme with the file's colors on top.

## Fonts and type

- `theme/fonts.rs`: the bundled fonts: Noto Sans, its semibold family and JetBrains Mono ahead of egui's fallbacks, from `assets/fonts/`.
- `theme/fonts.rs` is the one font setup, installed by `GlowflyApp::new`, `theme::apply_style` and the glyph check.
- `theme/fonts.rs` ends both families with the CJK fallbacks: the system's, which the shell hands over at startup (`fonts::use_cjk`, from `platform/system_fonts.rs`), or until then (and in tests) the Noto Sans CJK subset in `assets/fonts/test/`.
- `theme/missing_glyphs.rs`: in debug builds, a warning once per character a frame draws that no loaded font has.
- `theme/type_scale.rs`: the type scale: five steps, caption 11, small 12, body 14, subhead 16 and title 20 px, with nothing under the 11 px floor.
- The type scale has one section-header style, Noto Sans SemiBold at body size, and SemiBold titles.
- `theme::apply_style` installs the steps as egui's text styles.
- `common::with_code` sets prose with an inline monospace SHA.

## Spacing and tokens

- `theme/spacing.rs`: the spacing grid and panel padding, and the menu row metrics (comfortable and compact) and frame margin.
- `theme/sizes.rs`: the control sizes (Compact 22, Small 24, Regular 28, Large 36: height, padding, text, icon, gap, corners, hit area), the chip sizes (Badge 18, Chip 22), `text_pos` and `centred_top` (text, or a stack of it, centred on a whole point) and `fits` (which sizes a row can host).
- `theme/tokens.rs`: the design tokens every theme reads: the radius scale with nested container radii and a segment's corners (`radius::segment`), the border width, the least hit area and a split's caret width, the Float and Dialog elevations, the focus ring, the inset ring for packed controls and a text field's halo (each in the Standard and Strong styles, `focus_style`), and the motion timings and easing, instant with Reduce motion.
