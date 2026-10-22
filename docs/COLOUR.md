# Colour

How Glowfly uses colour: the families every palette is built from, what each colour is for,
the hue bands and contrast floors the tests hold every built-in theme to, and how to add a
colour. The palettes live in `crates/glowfly-app/src/ui/theme/` (`color.rs` declares
`Palette`, `families.rs` the token families, `builtin_dusk.rs`, `builtin_garden.rs` and
`builtin_extra.rs` the seven built-ins, `builtin.rs` which theme takes which); the tests that
check this page are `theme/tests.rs`, `theme/family_tests.rs`, `theme/vivid_tests.rs` and the
branch tests, with the shared hue bands in `theme/bands.rs`.

## 0. The built-in themes

| Theme | Kind | Character | bg / panel / accent / text |
|---|---|---|---|
| Dusk (the default dark; `dark`) | dark | The icon's dusk tile: indigo neutrals under the lantern gold | #1b1829 / #221f31 / #f6c159 / #e6e5ef |
| Dawn (the default light; `light`) | light | Faintly lavender paper, indigo ink, a deep gold accent and lantern-gold buttons | #f7f6fc / #fbfafe / #714900 / #211f38 |
| Garden Dark (`garden-dark`) | dark | The earlier standard dark: green-tinted neutrals, lattice green | #151b18 / #1b221e / #4dc588 / #e0e6e3 |
| Garden Light (`garden-light`) | light | The earlier standard light: warm paper, deep green | #f9f7f1 / #fdfcf9 / #14764a / #161c19 |
| High Contrast Dark, High Contrast Light | | White on black and black on white, the green accent at 7:1 text | |
| Sand | light | Brown ink on sand, a green accent | |

Why Dusk and Dawn look as they do:

- The app icon is a dusk tile (indigo into plum) with a gold lantern-glowfly and branch lanes in
  gold, magenta and azure, the branch palette's own Gold, Magenta and Azure. Dusk takes the
  tile for its neutrals and the lantern for its accent, so the main branch runs in gold as the
  icon's first lane does, and the other lanes keep their colours.
- One accent, not gold plus a second colour for links and focus: a second accent hue would be a
  fourth hue family on the chrome (see Rules) and would take one more slot from the branch hues.
  The magenta and azure of the icon appear as branch lanes, where the icon shows them.
- The gold sits in the crowded warm side of the wheel, beside the coral, the amber `modified`
  and `warn` and the Orange and Gold branch hues. So in Dusk and Dawn `modified` and `warn`
  move to the orange side (OKLCH hue about 64, out of the gold band, 25 deg off the coral), Jade
  takes the Gold branch slot, the colour-blind preset swaps two dark hues (below), and tags take
  the plum of the tile's lower edge instead of sand.
- Gold can't be a line or text colour on a light background without turning brown, and between
  `warn` (L 0.58) and the colour-blind hues (L 0.47 to 0.60) there is no room for a mid gold.
  Dawn's accent is a deep gold (#714900, L 0.44), a step darker than everything near its hue,
  and the lantern gold itself fills primary buttons (#efb146 with indigo ink) and tints the
  selected row (#f8e4bb).
- Old settings files say `Dark` and `Light`, and theme files `"base": "dark"` / `"light"`: they
  stay the standard pair's names, so they open in Dusk and Dawn. The Garden pair is there for
  anyone who prefers the green. A theme file on Dusk or Dawn whose accent isn't the base's (a
  file written from the green Dark, say) takes the Garden branch families, which leave a green
  accent's band free.
- View snapshots draw in the Garden pair (so a palette change doesn't redraw every image); the
  theme snapshots (`lineup_*`, `vivid_*`, the design and controls sheets) show each palette.

Views never name a hex value: they read the active palette through `color::text()`,
`color::added()` and so on (see "Rules" in [ARCHITECTURE.md](ARCHITECTURE.md)).

## 1. Families

### Neutrals

| Token | Theme key | What it is for |
|---|---|---|
| chrome | `toolbar` | The frame around the content: title bar (and the system frame's tab bar), status bar, terminal header, Preferences side panel and save bar. In dark themes it is the darkest surface. |
| base | `bg` | The canvas behind panels. |
| panel | `panel` | Panels, dialogs, list backgrounds; the toolbar and the active repository tab, which runs into it. |
| raised | `raised` | Controls and small fills a step off the panel: egui buttons and combo boxes, disabled styled buttons, chips, cards, tooltips, toasts, segmented tracks, keycaps, pane header strips. |
| input | `input` | Text fields; also the check mark on an accent-filled checkbox. |
| hairline | `border` | Lines between regions. |
| | `input_border`, `dialog_border` | The edge of text fields, checkboxes and radios; the frame of dialogs. |
| | `hover`, `selected` | A hovered row; the selected row (a tint of the accent). |
| text, strong, weak, faint | `text`, `strong`, `weak`, `faint` | Three text steps, plus `strong` for text on selection. |

### Brand

| Token | Theme key | What it is for |
|---|---|---|
| accent | `accent` | The theme's own colour: the lantern gold in Dusk and Dawn, the lattice green in the Garden pair, high contrast and Sand. Focus rings, links, checked controls, the selected row's bar, the active tab's line, the main branch's line, column-resize lines, header highlights. |
| primary | `primary`, `primary_border`, `primary_off` | The fill and edge of primary buttons. |
| primary ink | `on_primary` | Text and icons on a primary fill (dark ink on the gold of Dusk and Dawn and the green of Garden dark, white elsewhere). |
| danger | `danger`, `danger_off` | Destructive buttons. |
| tag | `tag_bg`, `tag_fg` | Tag chips in the graph: the plum of the icon's lower edge in Dusk and Dawn, sand in the Garden pair, high contrast and Sand. |

### Status

| Token | Theme key | Main use | Also marks |
|---|---|---|---|
| added | `added` | Added lines and files | Success, done |
| removed | `removed` | Removed lines, deleted files | Errors, failures |
| modified | `modified` | Modified files | |
| info | `info` | Renames, info banners | Links to services |
| progress | `progress` | A running operation (banner and spinner) | Pull requests, squash and fixup |

In diffs and file lists, status colours appear only as letter badges (the file status chips,
the WIP row's M / A / D chips, the WIP list's letters) and the `+N` / `−M` counts, besides the
matching banners. A file row's change bar is neutral: the added share in `weak`, the removed
share in `faint`, on the `border` track.

### Attention

| Token | Theme key | What it is for |
|---|---|---|
| warn | `warn` | Warnings. Until each job has its own colour it also marks favourites and pending checks. |
| banner | `banner` | The fill of banners. |
| needs you | (none) | The coral: the one solid-filled status, for moments Glowfly waits on you (a conflicted file, a merge stopped on conflicts, a repository whose trust is undecided, a sign-in). Always drawn with the warning mark, by `ui/needs_you.rs` alone: a 16 px badge on conflicted rows and in the conflicted operation banner (`BannerKind::NeedsYou`, with a coral stripe), a pill for the toolbar's conflict and Untrusted chips (shaded 10% toward white in dark themes and black in light ones on hover), a 12 px badge before a status-bar sign-in error. |
| ink on needs you | (none) | The warning mark and text on the coral. |

The attention band (below) belongs to the coral. `needs_you` and `on_needs_you` are not
theme-file keys yet: theme files take their base theme's.

### Repository

`repository.hues`: 8 quiet hues at OKLCH chroma 0.08, in slot order Rose, Ochre, Fern, Teal,
Sky, Slate, Iris, Plum (hues 10, 95, 128, 200, 235, 265, 298 and 335 deg, one lightness per
theme). They mark repositories, only on the window chrome:

- a colour you give a repository (tab menu or welcome row > Color; none by default), saved
  per working folder (`Settings.repo_colours`, by `repo_keys`, as the names `rose` … `plum`):
  an 8 px square swatch on its tab, a 2 px line along the window's top edge for the active tab
  (`ui/repo_line.rs`), and a 10 px swatch before its path in the repository lists, which keep a
  swatch slot on every row once any repository has a colour;
- the worktree tab-group strips, which stay automatic and walk the hues in `GROUP_ORDER` (Rose,
  Teal, Iris, Ochre, Sky, Plum, Fern, Slate), each at least 98 deg from the next.

Swatches are squares, so they never read as a branch's round dot. Not theme-file keys yet:
theme files take their base theme's hues.

### Branch

Each branch has one colour wherever it shows: its graph lines, node and row band, its chips,
its sidebar dot, the Branch picker's dot, the line under the active tab and the commit details'
branch row. Branch colours appear only as lines, dots and name pills: the Branch picker shows a
4 px dot in the branch's line tone (a 1.5 px `weak` ring while HEAD is detached), never a tinted
icon. The repository's main branch (the home remote's recorded HEAD, else `main`, then
`master`) takes `accent`; every other branch takes one of 14 hues (P1), in slot order Rose,
Lime, Violet, Orange, Cyan, Magenta, Olive, Azure, Red, Teal, Orchid, Gold, Sky, Indigo. They
keep out of the theme's accent band and the attention band, and neighbouring hues alternate
between two lightness tiers (OKLCH L 0.70 / 0.80 in the dark tones). Beside Dusk and Dawn's gold
accent, Jade (hue 165, at Gold's tier) takes the Gold slot, in the green the gold leaves free
(`theme/branch_palette_gold.rs`); each family names its own slots for the Color submenu.

- `branch.lanes`: line tones, for lines, dots and bands (3:1 on bg and panel).
- `branch.chips`: chip tones, which ref chips are tinted with and their text pushed from.

Dusk, Garden dark and High contrast dark use the dark tones for both. Dawn and Garden light use
bright line tones and darker text tones for chips. High contrast light and Sand use the text
tones for both, as the line tones fall below 3:1 on their surfaces. Not a theme-file key: theme files get their base
theme's family.

A branch's slot comes from its name: FNV-1a (32-bit) of its UTF-8 bytes, mod 14, the same in
every session and on every machine. Remote copies share their local namesake's colour
(`origin/feature` and `feature` are one key); case counts, as in git. Lines no branch names
(a merge's other parent without a recognisable merge message) hash their first commit's id.

Neighbour rule: two branches that ever sit in side-by-side lanes never share a hue. In order of
first appearance from the top, each keeps its name's slot unless an already-placed neighbour
has it, else takes the next slot (wrapping) no placed neighbour uses; with all 14 taken it keeps
its own. The colour then holds on every surface, so a branch's colour changes only when its
neighbours in the graph do. `glowfly-core`'s `graph/owners.rs` works out which branch owns each
lane and `branch_colour.rs` picks the colours.

Pins: a branch menu's Color submenu (local and remote branches, in the sidebar and the graph)
pins a branch to a hue, or with Automatic lets it pick its own again. A pin is saved per
repository (`Settings.branch_pins`, by branch key, so it colours the remote copies too) and
recolours every surface at once. A pinned branch keeps its hue whatever its neighbours; an
automatic neighbour moves off it, and two pinned neighbours may share a hue. Main has no pin:
its Color item is disabled. A pin keeps its slot number, so under the colour-blind preset it
shows as that number modulo 8.

People: the initials in graph nodes and the commit details avatar take a person's own slot,
FNV-1a of their trimmed, lowercased email (their name when the email is blank) modulo the
preset's hue count, never the accent. The fill is the background mixed 30% toward their line
tone, and the ink their chip tone pushed to 4.5:1 on that fill
(`theme/branch_colours.rs::initials_tones`). The ring around the initials, or around an avatar
image, stays the branch's colour.

Colour-blind safe preset (Preferences > UI Customization > Commit graph > Branch colors): 8
hues that stay apart under protan, deutan and tritan vision as well as normal vision, and keep
out of the accent and attention bands. Dark themes use its dark tones for lines and chips;
light ones use its line tones for lines and its text tones for chips. Under protan or deutan
vision Yellow and Lime pass for Dusk's gold, so Dusk's set (`Palette::colour_blind`) has Orange
(#eb8f13) and a paler Lime (#c8feac) in their slots; Dawn's deep gold stays clear of the light
tones, which it keeps. Theme files take their base's set. The preset switches with the profile.
Branch names always appear beside their colour, and main keeps the accent.

### Terminal

`ansi`: the built-in terminal's colours, one table per built-in theme in `ui/theme/ansi.rs`:
its background (`ansi.background`, the theme's `input`), default text (`ansi.foreground`,
`text`), cursor and selection (`ansi.cursor`, `ansi.selection`, both the accent; the selection is
drawn at 35% over the background, a filled block cursor at 60%), and its 16 basic colours
(`ansi.colors`, 0-7 and their bright 8-15). Bold text in 0-7 uses 8-15; the 256-colour cube,
the grey ramp and truecolour are not themed. Theme files set them with the `terminal_*` keys
(section 7). See section 6 for the values and section 3 for the floors.

Preferences > Terminal > Colors chooses what the terminal draws in: **Match app theme** (the
default: the palette's own `ansi`), any built-in theme's terminal colours, or a scheme imported
with **Import scheme…** (settings: `terminal.colours`, `MatchTheme`, `Theme("<id>")` or
`Scheme("<stem>")`). The view reads them through `color::term()`, which the shell's
`ThemeCache` points at the chosen colours each frame (`ui/theme/terminal_scheme.rs`); a scheme or
theme that is gone falls back to the app theme's. When the chosen colours break the text rules
of section 3 (at 4.5:1, and 3:1 for 8, against the scheme's own background, with dark or light
decided by that background), a quiet line under the picker names the colours that are hard to
read (`ansi::hard_to_read`); the scheme can still be used.

Imported schemes live in the themes folder's `terminal` folder, one JSON file each, holding a
name and the `terminal_*` keys it sets; keys it leaves out come from the app theme's terminal
colours:

```json
{ "name": "Harbour Night", "colors": { "terminal_background": "#101820", "terminal_red": "#e0565b" } }
```

Import reads (in `glowfly-core`, `settings/terminal_schemes/`):

- a Windows Terminal scheme: a JSON object with `name`, `background`, `foreground`,
  `cursorColor`, `selectionBackground` and `black`, `red`, `green`, `yellow`, `blue`, `purple`
  (or `magenta`), `cyan`, `white`, `brightBlack` … `brightWhite`, as `#rrggbb`, `#rgb` or
  `#rrggbbaa` (alpha ignored); a list of such objects; or a whole `settings.json` (comments and
  trailing commas allowed) whose `schemes` list is imported scheme by scheme, passing over the
  ones with a colour that isn't one;
- an iTerm2 `.itermcolors` XML property list: `Ansi 0 Color` … `Ansi 15 Color`, `Background
  Color`, `Foreground Color`, `Cursor Color` and `Selection Color`, each with `Red`, `Green` and
  `Blue Component` from 0 to 1 (a colour whose components go above 1 is read as 0-255; alpha
  and the colour space are ignored). Binary property lists are refused with a note to export as
  XML.

Each scheme is saved under a name made from its own (`harbour-night.json`, then `-2`…, never
over an existing file) and the first one becomes the terminal's colours. Files over 1 MiB,
empty files, other formats and schemes without a usable colour are refused with the reason.

### Diff and syntax

`diff_*`, `hunk_*` and `syn_*` colour diff lines, hunk headers and syntax tokens. They are
outside the families and unchanged by the brand colour.

## 2. Rules

- One job per colour: each colour means one thing (with the related uses listed above), and
  no two families share a colour for different meanings. Each kind keeps to its forms: status
  colours to letter badges and counts, branch colours to lines, dots and name pills, repository
  colours to the window chrome, and the coral to "needs you" moments.
- The coral is the only solid-filled status, and it always comes with the warning mark.
- The toolbar's groups sit on a neutral backing, `toolbar` tinted 9% toward the accent
  (`Palette::tool_backing`, a tint as below); a hovered tool is the backing mixed 8% toward `text`, a pressed or
  open one 16% (`tool_hover`, `tool_pressed`). Icons are `text`, or `faint` when disabled; no
  icon has its own colour.
- The chrome's own marks (fills, icons, chips, strips, lines) use at most three hue families at
  once: the accent (focus, the active tab's line, the terminal-open bar, the toolbar backing),
  the repository hues (tab swatches, group strips, the top line) and the coral when something
  needs you. A branch's own line or dot beside its name (the line under the active tab, the
  Branch picker's dot) belongs to the branch, not the chrome; banners under the toolbar are
  messages, not chrome. `theme/vivid_tests.rs` keeps status, attention and lane colours out of
  the chrome's files.
- Tinting a surface: the faint tints of a surface (graph row bands up to 7% toward the row's
  lane, the toolbar groups' backing 9% toward the accent, the dialog guide 7% and text fields
  4%) go through `contrast::tint`. It is a plain mix, except on a dark palette whose surface is
  a coloured field (OKLCH chroma 0.02 or more: Dusk's indigo). There a tint toward a far hue
  would cancel the field's colour: gold, orange, lime or olive mixed into indigo comes out a
  warm grey of chroma 0.01 to 0.02, which reads as brown or mud. So the tint keeps the mix's
  lightness, moves the field's chroma the same fraction toward the tint colour's, and turns the
  field's hue at most 20 deg toward the mix's. In Dusk every band, the toolbar groups and the
  guide and field fills stay in the indigo-violet family (hue 270 to 315): warm lanes give a
  plum-violet band, cool lanes a blue-indigo one, and only the gold accent, the selected row
  and the lanes' own marks are warm. Light palettes, and dark ones on a near-grey field
  (Garden dark, high contrast dark), keep the plain mix: a warm tint on pale paper reads as
  cream, not brown. `theme/tint_tests.rs` checks both.
- The selected row's message is drawn in `strong`, in the graph as in the side panels.
- The accent never marks status. Running operations use `progress`, not the accent.
- Only the accent family (`accent`, `primary`, `primary_border`, `selected`) sits in the
  theme's accent band: the gold band, 72 to 92 deg OKLCH hue, in Dusk and Dawn; the green band,
  145 to 175 deg, in the others. The gold band is narrower as the warm side of the wheel is
  crowded; there the gold keeps its distance by lightness too (Orange at 70 deg is 0.12 OKLab
  from Dusk's gold, and Dawn's deep gold sits 0.12 to 0.15 in L below `warn` and Orange).
- Status, branch and repository colours stay out of the theme's accent band and out of the
  attention band (about 35 to 60 deg), which belongs to the coral `needs_you`. Status colours
  also keep OKLCH chroma at 0.08 or more; repository hues sit at 0.08 and at least 30 deg apart.
- Neutrals: Garden dark and light keep OKLCH chroma at or below 0.015, tinted green (hue about
  160) and warm paper (hue about 90). Dusk's and Dawn's take the icon's indigo (hue 280 to 300):
  Dusk's at up to 0.045 chroma, Dawn's paper at up to 0.018 under indigo ink of up to 0.05.
- In Dusk, Dawn and the Garden pair, chrome is darker than the base, and raised is at least
  1.08:1 off the panel in every theme.

OKLCH is computed from sRGB with Björn Ottosson's OKLab matrices (`contrast::oklch`, and back
with `contrast::from_oklch` for `contrast::tint`).

## 3. Contrast floors

WCAG contrast ratio, with Glowfly's 0.03928 linearisation threshold. Where two values are
given, the second is for the high-contrast themes.

| What | On | Floor |
|---|---|---|
| `text` | bg, panel, toolbar, raised, input, hover, selected, hunk_bg, every diff background | 4.5 / 7 |
| `strong` | selected | 4.5 / 7 |
| `weak` | bg, panel, toolbar, raised, input, hover, selected | 4.5 |
| `faint` | bg, panel, input (never text on the toolbar) | 4.5, and 1.0 below weak on panel |
| `input_border` | bg, panel, input, toolbar | 3 / 4.5 |
| `dialog_border` | panel | 3 / 4.5 |
| `accent` | bg, panel, input | 3 |
| `on_primary` | primary, and primary shaded 10% and 18% (hover, press) | 4.5 / 7 |
| `tag_fg` | tag_bg | 4.5 / 7 |
| Status and `warn` | bg, panel, hover, selected, every lane band | 3 / 4.5 |
| Branch line tones (`branch.lanes`) and `accent` as main's line | bg, panel | 3 |
| Repository hues | toolbar, bg, panel, raised, hover | 3 / 4.5 |
| `needs_you` | bg, panel, toolbar, selected, banner, hover | 3 |
| `on_needs_you` | needs_you, and needs_you shaded 10% (hover) | 4.5 / 7 |
| `text` | tool backing, tool hover, tool pressed | 4.5 / 7 |
| `weak` | tool backing | 4.5 |
| `text` | overlay, inset | 4.5 / 7 |
| `weak` | overlay, inset | 4.5 |
| `text` | hover, pressed and latched layers on panel, raised, overlay and tool backing | 4.5 / 7 |
| Tonal ink | tonal fill at rest, hovered and pressed | 4.5 / 7 |
| `accent` (the focus ring) | bg, panel, input, raised, overlay, tool backing | 3 |
| Tone tint ink (accent, success, warning, danger, info) | its tint | 4.5 |
| Light's chip tones (`branch.chips`) | bg, panel, toolbar, hover, selected, raised | 4.5 |
| Colour-blind line tones, applied to every built-in | bg, panel | 3 |
| Colour-blind light text tones (chips on every light built-in) | bg, panel, toolbar, hover, selected, raised | 4.5 |
| Colour-blind dark tones, light line tones | each other and the accents, in OKLab under normal, protan, deutan and tritan vision | distance 0.08 (dark), 0.05 (light lines) |
| Change chip text | its chip tint | 4.5 |
| Every `syn_*` | panel, diff_add, diff_del, diff_add_word, diff_del_word | 2.4 / 4.5 |
| Terminal text colours (below) and `ansi.foreground` | `ansi.background` (input) | 4.5 / 7 |
| Terminal 8 (bright black, dim hints) | `ansi.background` (input) | 3 / 4.5 |
| Terminal background colours (below) | `ansi.background` (input) | 1.25 off it, with `text` on them at 4.5 / 7 |

The terminal's text colours are 1-7 and 9-15 in dark themes, and 0-6 and 9-14 in light ones;
its background colours are 0 (black) in dark themes, and 7 and 15 (white) in light ones.
Colours 1-6 and 9-14 keep their hue's OKLCH band (red 0-45 or 345-360, green 115-165, yellow
60-115, blue 225-285, magenta 290-345, cyan 165-225), and each bright colour is at least 0.04
OKLCH lightness off its normal one: lighter in dark themes, darker in light ones, so bold text
gets stronger rather than fainter. `theme/ansi_tests.rs` checks all of this.

## 4. Overlay, state layers and tones

These are worked out from the palette in `theme/surfaces.rs`, so every theme, theme files
included, gets them; none is a theme-file key. `theme/token_tests.rs` checks them over every
built-in and two custom palettes with odd colours.

| Token | What it is | What it is for |
|---|---|---|
| overlay | Dark: `raised` 6% toward `text`. Light: `panel`, and the shadow does the lifting. | Menus, tooltips, egui popups and combo lists, the tour card and toasts, with the Float shadow. Dialogs and the command palette stay on `panel` with the Dialog shadow. |
| overlay edge | Dark: overlay 12% toward `text`. Light: `border`. | The 1 px edge of everything on the overlay. |
| inset | Dark: `panel`. Light: `raised`. | A block set into an overlay (a tooltip's preview list). |
| hover, pressed, latched | A surface 8%, 12% or 16% toward `text` (`hover_on(s)` and so on), or `text` at that opacity as a translucent layer for a control that doesn't know its surface. | Quiet buttons, ghost controls and menu items (`hover_on(overlay)`); latched is a toggle that stays on or an open menu's caret. The toolbar's hover and toggle fills are these on the tool backing. |
| filled shading | A filled button's fill 10% (hover) or 18% (pressed) toward white in dark palettes, black in light ones. | Primary, secondary and danger buttons. |
| tones | Accent, success (`added`), warning (`warn`), danger (`removed`) and info (`info`), each with a tint: the change-chip tint of the colour, with the colour pushed to read on it (below). | Toast outlines, the commit caret while hooks are skipped. |
| tonal | The accent's tint as the fill (moved 10% / 18% toward the accent on hover / press) with the accent as ink, pushed per state until it reads at 4.5:1 (7:1 in a high-contrast palette). | Tonal buttons: important actions that aren't the view's main one (Stage all). |

## 5. Chip tints

A change chip (the WIP row's M / A / D) is an opaque tint of its status colour over the base:
22% in dark palettes and 10% in light ones (`contrast::change_chip`). The chip text is the
status colour itself wherever that reaches 4.5:1 on the tint; otherwise `readable_on` pushes it
5% at a time toward white (dark) or black (light) until it does. In Garden light that is
only modified (3.9:1 unpushed); in Garden dark, removed and progress are pushed.

## 6. Values

### Dusk

| Token | Value | Notes |
|---|---|---|
| chrome (`toolbar`) | #131220 | |
| base (`bg`) | #1b1829 | The icon tile's indigo, lifted so the backdrop dim still reads |
| panel | #221f31 | |
| raised | #312d41 | |
| input | #161423 | |
| hairline (`border`) | #373347 | |
| input_border, dialog_border | #89839e | |
| hover | #332d44 | |
| selected | #573b11 | A bronze wash of the gold, turned to the amber edge of the gold band (hue 73, chroma 0.069): a darker straight gold (#503e1d, hue 81) read olive against the indigo |
| text / strong | #e6e5ef / #ffffff | text 12.9:1 on panel |
| weak / faint | #b6b3c8 / #9b96ae | 7.9 and 5.6:1 on panel |
| accent, primary | #f6c159 | The lantern gold, OKLCH 0.84 0.135 82; 9.7:1 on panel |
| on_primary | #151229 | 11.0:1 on the gold |
| primary_border | #d5a13c | |
| tag_bg / tag_fg | #4a283f / #f4c9e4 | Plum, 8.6:1 |
| added / removed / modified / info / progress | #7bc85f / #ea6a64 / #f69d3d / #60aaf3 / #a689f1 | modified on the orange side (hue 64), out of the gold band |
| warn / banner | #fba541 / #46290b | |
| syn_comment | #a29eb6 | The other syntax colours are Garden dark's |

### Dawn

| Token | Value | Notes |
|---|---|---|
| chrome (`toolbar`) | #e7e5ef | |
| base (`bg`) | #f7f6fc | |
| panel | #fbfafe | |
| raised | #edebf4 | |
| input | #ffffff | |
| hairline (`border`) | #d8d6e1 | |
| input_border, dialog_border | #747188 | |
| hover | #ece9f5 | |
| selected | #f8e4bb | The lantern gold as a wash; branch chips keep 4.5:1 on it |
| text / strong | #211f38 / #100d26 | text 15.3:1 on panel |
| weak / faint | #54526d / #6b6882 | 7.2 and 5.1:1 on panel |
| accent | #714900 | Deep gold, OKLCH 0.44 0.094 73; 7.6:1 on panel |
| primary / on_primary | #efb146 / #1b1933 | The lantern gold with indigo ink, 9.0:1 |
| primary_border | #bf851e | |
| tag_bg / tag_fg | #f5d8ea / #763463 | Plum, 6.5:1 |
| added / removed / modified / info / progress | #357518 / #b63132 / #a96000 / #1f68bc / #6d47b8 | |
| warn / banner | #ae6704 / #fee3c5 | |
| danger / danger_off | #c0332a / #fedadc | |
| syn_comment | #5f5d78 | The other syntax colours are Garden light's |

### Garden dark

| Token | Value | Notes |
|---|---|---|
| chrome (`toolbar`) | #0f1612 | |
| base (`bg`) | #151b18 | |
| panel | #1b221e | |
| raised | #262e29 | |
| input | #111814 | |
| hairline (`border`) | #2d3530 | |
| input_border, dialog_border | #737d77 | |
| hover | #29312c | |
| selected | #244534 | |
| text / strong | #e0e6e3 / #ffffff | text 12.8:1 on panel |
| weak | #a4aea8 | 7.1:1 on panel |
| faint | #808a84 | The review's #7f8983 reaches only 4.49:1 on the panel, so it is nudged one step (4.55:1). |
| accent, primary | #4dc588 | OKLCH 0.74 0.14 158 |
| on_primary | #0f1a14 | 8.2:1 on the accent |
| primary_border | #3fae76 | |
| tag_bg / tag_fg | #3d3628 / #e4d6b6 | 8.3:1 |
| added / removed / modified / info / progress | #7bc85f / #ea6a64 / #edb345 / #60aaf3 / #a689f1 | |
| warn / banner | #e8b339 / #4a3812 | |

### Garden light

| Token | Value | Notes |
|---|---|---|
| chrome (`toolbar`) | #ece9e1 | |
| base (`bg`) | #f9f7f1 | |
| panel | #fdfcf9 | |
| raised | #f2efe8 | |
| input | #ffffff | |
| hairline (`border`) | #dcd9d0 | |
| input_border, dialog_border | #767d79 | |
| hover | #ebe8e0 | |
| selected | #cfeddb | |
| text / strong | #161c19 / #0b100d | |
| weak | #4c5550 | |
| faint | #68716c | 4.70 on base, 4.91 on panel, only 4.15 on chrome |
| accent, primary | #14764a | OKLCH 0.50 0.11 158; white on it 5.65:1 |
| on_primary | #ffffff | |
| primary_border | #0f5c3a | |
| tag_bg / tag_fg | #e3dac2 / #6f562a | 4.96:1 |
| added / removed / modified / info / progress | #357518 / #b63132 / #a06604 / #1f68bc / #6d47b8 | modified is 3.9 to 4.1:1 on chrome and hover (status letters need 3:1) |
| warn / banner | #a86b00 / #fbecc8 | |

### Branch

Slot by slot (`theme/branch_palette.rs`): dark tones (Dusk, Garden dark and High contrast
dark), light text and chip tones (Dawn's and Garden light's chips; every line and chip in High
contrast light and Sand) and light line tones (Dawn's and Garden light's lines, dots and
bands). In Dusk and Dawn slot 11 is Jade (#56d1a3 dark, #176449 light text, #2d9570 light line,
hue 165) in place of Gold.

| Slot | Hue | Dark | Light text | Light line |
|---|---|---|---|---|
| 0 | Rose | #e77492 | #861f43 | #c04d6e |
| 1 | Lime | #a3cf64 | #4d7002 | #719c27 |
| 2 | Violet | #c5aeff | #6d4ea8 | #9776db |
| 3 | Orange | #d78c1a | #6c4302 | #a86b06 |
| 4 | Cyan | #04d4f1 | #057080 | #0b9db3 |
| 5 | Magenta | #fb97db | #953d7c | #c664a8 |
| 6 | Olive | #9ea827 | #4d5201 | #7a8201 |
| 7 | Azure | #93c1fe | #2462b1 | #498de5 |
| 8 | Red | #fea195 | #a43c33 | #d86357 |
| 9 | Teal | #0ab5b1 | #005957 | #0a8c89 |
| 10 | Orchid | #c37fd8 | #6b2d7d | #9e59b3 |
| 11 | Gold | #dfba3b | #786101 | #a98904 |
| 12 | Sky | #0aabea | #005475 | #0685b6 |
| 13 | Indigo | #8794f7 | #3b4097 | #646fd1 |

The colour-blind safe preset, slot by slot: dark tones (Dark and High contrast dark), light line
tones (lines, dots and bands on every light theme) and light text tones (chips on every light
theme), with the OKLCH hue the three share within 1.5 deg. Simulated with Machado et al. (2009)
at full severity on linear sRGB, distance in OKLab: the least distance between any two dark
tones or a dark tone and a dark accent is 0.087, between light line tones or a light line tone
and a light accent 0.055 (the standard dark tones get as close as 0.001, Violet and Azure, under
deutan vision). Chips carry the branch name, so text tones have no distance floor.

| Slot | Hue | Dark | Light line | Light text | Hue (deg) |
|---|---|---|---|---|---|
| 0 | Red | #ed485a | #b17272 | #8b0422 | 19 |
| 1 | Aqua | #45fafc | #0d9295 | #0a4b4b | 196 |
| 2 | Yellow | #d1c038 | #70640c | #474219 | 102 |
| 3 | Blue | #2898c9 | #207aa0 | #106082 | 233 |
| 4 | Plum | #ba7294 | #b2397b | #ac1c6f | 350 |
| 5 | Lime | #ccee6e | #668014 | #56680f | 122 |
| 6 | Purple | #9d62f4 | #9169d5 | #482b74 | 299 |
| 7 | Periwinkle | #a2b5f2 | #3a45cf | #4158bd | 271 |

Dusk's colour-blind set swaps two dark tones: slot 2 is Orange #eb8f13 (hue 65) and slot 5 a
paler Lime #c8feac (hue 135), as Yellow and Lime fall within 0.08 of the gold under protan and
deutan vision. Dawn keeps the light tones above.

High contrast dark, High contrast light and Sand keep their own surfaces, text, diff and syntax
colours (their `raised` is their `toolbar`); their accent family is green at their own
contrast: accent #5fe0a0 / #00613a / #2a7550 and primary #003d26 / #005c36 / #236a48, all
with white ink. High contrast dark's primary is dark enough that white still reads at 7:1 on
a hovered or pressed button.

### Repository hues and the coral

In slot order Rose, Ochre, Fern, Teal, Sky, Slate, Iris, Plum, as `#rrggbb` without the `#`
(OKLCH lightness 0.70, 0.57, 0.78, 0.47 and 0.56):

| Theme | Hues | Lowest on chrome |
|---|---|---|
| Dusk | cb8a93 ae9e64 90a872 5caeb2 6ba7c9 869ed1 a494ca be8cb2 | 6.68 |
| Dawn | a0636d 86773d 6a814c 31868a 447fa0 6077a7 7d6da1 95658a | 3.43 |
| Garden dark | cb8a93 ae9e64 90a872 5caeb2 6ba7c9 869ed1 a494ca be8cb2 | 4.83 |
| Garden light | a0636d 86773d 6a814c 31868a 447fa0 6077a7 7d6da1 95658a | 3.49 |
| High contrast dark | e5a2ac c7b77c a9c28a 76c7cc 84c0e4 9eb7eb bdade5 d8a4cc | 7.09 |
| High contrast light | 814750 695a1f 4e6330 00686c 256281 455a88 605182 76496d | 5.15 |
| Sand | 9d606a 83743a 677e49 2d8387 417c9d 5d74a4 7a6a9e 926287 | 3.36 |

The coral (OKLCH hue about 40 deg) and its ink, with the ink's contrast at rest and on hover:

| Theme | needs_you / on_needs_you | Ink contrast |
|---|---|---|
| Dusk, Garden dark | #fb794a / #22110b | 6.88, 7.52 |
| Dawn, Garden light, Sand | #bf4306 / #ffffff | 5.22, 6.17 |
| High contrast dark | #ff8a5c / #22110b | 7.84, 8.53 |
| High contrast light | #9c3604 / #ffffff | 7.14, 8.22 |

The coral reads at least 3.89:1 on its surroundings (lowest: Dusk's selected row). The tool
backing is #281f32 (Dusk, a violet step: the plain mix was the grey-brown #272225), #dcd7d9 (Dawn), #15261d (Garden dark), #d9dfd3 (Garden light),
#1d2a27 (High contrast dark), #d4dedd (High contrast light) and #d7d5bf (Sand), at most OKLCH
chroma 0.040.

### Terminal colours

0-7 then 8-15, as `#rrggbb` without the `#`. Every built-in's terminal background, text, cursor
and selection are its `input`, `text`, `accent` and `accent`:

| Theme | 0-7 | 8-15 |
|---|---|---|
| Dusk | 2b2939 f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 ceccda | 8c899d ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f5f4fa |
| Dawn | 1d1c31 c0362f 2f7d1c 8a6400 2a5fc4 8a3fb8 087c80 d8d6e1 | 686679 a3241e 226614 6e5000 1d4ba3 70299b 006468 e3e2ec |
| Garden dark | 2a322d f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 c9d1cc | 86918a ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f4f7f5 |
| Garden light | 1c2420 c0362f 2f7d1c 8a6400 2a5fc4 8a3fb8 087c80 d3d9d5 | 6b746f a3241e 226614 6e5000 1d4ba3 70299b 006468 e2e7e4 |
| High contrast dark | 303030 ff6b6b 5fe36a ffd84a 7ab4ff e08aff 4fe0e0 e6e6e6 | a6a6a6 ff9c9c 9cf0a2 ffec96 a8ccff eeb5ff 96f0f0 ffffff |
| High contrast light | 000000 a31515 08621a 6b4c00 0646a8 6a1fa0 005f63 d0d0d0 | 55595f 7f0d0d 084f15 523900 043682 52157d 004a4d e5e5e5 |
| Sand | 2e2519 b23a2b 4a7a1a 8a6200 2d65a8 84429e 1b7570 e3d9c6 | 76695a 93291c 3a6312 6d4c00 1f4f8a 6a2f82 0f5d59 eae2d2 |

The lowest ratios of a chromatic colour on `input` are 4.99 (Dawn and Garden light 6) and 5.01 (Sand 2), and
7.36 in High contrast light.

## 7. Theme-file keys

A theme file (Preferences > UI Customization > Theme files) sets any of these keys, in this
order, to `#rrggbb` or to another key's name; `glowfly-core`'s `COLOR_KEYS` and
`Palette::KEYS` hold the same list:

`bg`, `panel`, `toolbar`, `raised`, `input`, `border`, `input_border`, `dialog_border`,
`hover`, `selected`, `accent`, `text`, `strong`, `weak`, `faint`, `added`, `removed`,
`modified`, `info`, `progress`, `diff_add`, `diff_del`, `diff_add_sel`, `diff_del_sel`,
`diff_add_word`, `diff_del_word`, `hunk_bg`, `hunk_fg`, `warn`, `banner`, `primary`,
`on_primary`, `primary_off`, `primary_border`, `danger`, `danger_off`, `tag_bg`, `tag_fg`,
`syn_comment`, `syn_string`, `syn_number`, `syn_constant`, `syn_keyword`, `syn_type`,
`syn_function`, `syn_attr`, then the terminal's: `terminal_background`, `terminal_foreground`,
`terminal_cursor`, `terminal_selection`, `terminal_black`, `terminal_red`, `terminal_green`,
`terminal_yellow`, `terminal_blue`, `terminal_magenta`, `terminal_cyan`, `terminal_white`,
`terminal_bright_black`, `terminal_bright_red`, `terminal_bright_green`,
`terminal_bright_yellow`, `terminal_bright_blue`, `terminal_bright_magenta`,
`terminal_bright_cyan`, `terminal_bright_white` (`glowfly-core`'s `TERMINAL_KEYS`).

The older names still load, as keys and as values: `green` is `added`, `red` is `removed`,
`amber` is `modified`, `blue` is `info` and `purple` is `progress`. When a file sets both the
old and the new name, the new name wins. Keys a file leaves out (including `raised`,
`on_primary` and `warn` in older files) come from its base theme, with one exception: a file
that sets its own `primary` but no `on_primary` keeps the base's ink only while it reaches 4.5:1
on that primary, and otherwise gets white or black, whichever reads better. The terminal's body
keys follow the file's own surfaces when it leaves them out: `terminal_background` is its
`input`, `terminal_foreground` its `text`, and `terminal_cursor` and `terminal_selection` its
`accent`, as the built-ins' are; the 16 basic colours come from the base. Create from current
writes every key, the terminal's included.

## 8. Adding a colour

1. Add the field to its family in `families.rs`, or to the flat neutrals or brand groups of
   `Palette` in `color.rs`, and give it a value in every built-in.
2. Add its `key => path` entry to `palette_fields!` in `color.rs` and its key to `COLOR_KEYS`
   in `glowfly-core/src/settings/themes.rs`, at the same position (`keys_match_theme_files`
   checks the two lists match).
3. Add a contrast or band rule for it to `theme/tests.rs` or `theme/family_tests.rs`.
4. Describe its job on this page.

A new built-in theme also needs a terminal table in `ui/theme/ansi.rs` (its `input`, `text` and
`accent`, then 16 basic colours: start from Dusk's or Dawn's and adjust them until
`theme/ansi_tests.rs` passes), set as the palette's `ansi`.
