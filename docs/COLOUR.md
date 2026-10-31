# Colour

How Glowfly uses colour: the families every palette is built from, what each colour is for,
the hue bands and contrast floors the tests hold every built-in theme to, and how to add a
colour. The palettes live in `crates/glowfly-ui/src/theme/` (`color.rs` declares
`Palette`, `families.rs` the token families, `builtin_dusk.rs`, `builtin_garden.rs`,
`builtin_extra.rs`, `builtin_sea.rs`, `builtin_warm.rs` and `builtin_cobalt.rs` the thirteen
built-ins, `builtin.rs` which theme takes which); the tests that check this page are
`theme/tests.rs`, `theme/family_tests.rs`, `theme/vivid_tests.rs`, `theme/more_themes_tests.rs`
and the branch tests, with the shared hue bands in `theme/bands.rs`.

## 0. The built-in themes

| Theme | Kind | Character | bg / panel / accent / text |
|---|---|---|---|
| Dusk (`dark`) | dark | The icon's dusk tile: indigo neutrals under the lantern gold | #1b1829 / #221f31 / #f6c159 / #e6e5ef |
| Dawn (the default light; `light`) | light | Faintly lavender paper, indigo ink, an iris accent and lantern-gold buttons | #f7f6fc / #fbfafe / #5a47cf / #211f38 |
| Garden Dark (`garden-dark`) | dark | The earlier standard dark: green-tinted neutrals, lattice green | #151b18 / #1b221e / #4dc588 / #e0e6e3 |
| Garden Light (`garden-light`) | light | The earlier standard light: warm paper, deep green | #f9f7f1 / #fdfcf9 / #14764a / #161c19 |
| High Contrast Dark, High Contrast Light | | White on black and black on white, the green accent at 7:1 text | |
| Sand | light | Warm charcoal ink on cream sand paper, a green accent | |
| Midnight (`midnight`) | dark | Deep blue-black night under a cool sea-glass accent | #0d1827 / #14202f / #6eddb2 / #e2e8f0 |
| Mist (`mist`) | light | Midnight's light twin: cool fog paper, blue-grey ink, deep sea glass | #f2f8fc / #f8fcfe / #007258 / #142533 |
| Graphite (the default dark; `graphite`) | dark | Near-neutral graphite greys (a hair cool) under a cobalt accent | #191a1d / #202225 / #7ebbff / #e6e7e9 |
| Ember (`ember`) | dark | Red-leaning charcoal under a vivid flame-orange accent, a clean deep-red selection | #211819 / #281f20 / #ff9b2a / #ebe6e6 |
| Pine (`pine`) | dark | Spruce-slate charcoal under the lantern gold, a petrol-blue selection | #111d1f / #182426 / #f6c159 / #e3e8ea |
| Linen (`linen`) | light | Clean warm-white paper, soft charcoal ink and deep cobalt | #f9f7f3 / #fcfbf9 / #004da8 / #28211c |

Graphite is the default dark theme and Dawn the default light one: new settings start in
Graphite, and Match the system shows Graphite or Dawn. Preferences > Appearance shows the
built-ins as cards in two groups, Dark and Light (the default one first in each, then Dusk and
the menu order), each a small picture of the main window in
that theme with its name under it and its description on hover; Match the system sits above
them, and theme files in a dropdown below (`ui/prefs/theme_picker.rs`). View > Theme lists the
same groups under small Dark and Light labels.

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
- Gold can't be a line or text colour on a light background without turning brown (the old
  deep-gold accent, #714900, read brown; see "No brown"), and the one clean warm line on paper,
  a saturated amber at hue 62, would sit on top of `warn` and Orange. So Dawn's accent is an
  iris violet (#5a47cf, OKLCH 0.50 0.20 283: the dawn sky over the lavender paper, opposite the
  gold), and the lantern gold itself fills primary buttons (#efb146 with indigo ink, edged
  #d19006) and tints the selected row (#f8e4bb), where the iris bar sits on it.
- Old settings files say `Dark` and `Light`. `Dark` always meant "the default dark theme" (the
  green one, then Dusk), so it now opens in Graphite, the default; Dusk picked today is saved as
  `Dusk`. `Light` stays Dawn. Theme files' `"base": "dark"` / `"light"` stay Dusk and Dawn, so a
  file keeps the colours it was written against. The Garden pair is there for
  anyone who prefers the green. A theme file on Dusk or Dawn whose accent isn't the base's (a
  file written from the green Dark, say) takes the Garden branch families, which leave a green
  accent's band free.
- View snapshots draw in the Garden pair (so a palette change doesn't redraw every image); the
  theme snapshots (`lineup_*`, `vivid_*`, `theme_*`, the design and controls sheets) show each
  palette.

Why the six newer themes look as they do (each follows the same recipe as Dusk: one quiet hue
for every surface, stepped in lightness, and one accent across the wheel from it):

- Midnight and Mist: a deep blue-black (OKLCH hue 255, chroma up to 0.045) and a cool fog paper
  (hue 235, chroma under 0.02 under blue-grey ink), with a sea-glass accent (#6eddb2, hue 166;
  Mist's deep #007258) at the cool edge of the green band, so the Garden branch families fit as
  they are. Midnight is darker than Dusk (base L 0.21) and lifted just enough that the dialog
  backdrop still dims it.
- Graphite and Linen: for people who want almost no tint. Graphite's greys sit under chroma
  0.008 (hue 265, a hair cool so they never read warm-grey); Linen's paper is warm by a hair
  (chroma at most 0.012, hue 80) so it reads as clean white, not cream or sepia, under charcoal
  ink. Both take a cobalt accent (#7ebbff, #004da8) in its own band, with Jade in the Azure
  slot and `info` moved to hue about 224.
- Ember: red and orange, never brown. The field is a charcoal with a faint red lean (hue 10,
  chroma about 0.014): an earlier oxblood field (chroma 0.03) read as chocolate brown, and so
  did every warm tint on it. The accent is a vivid flame orange (#ff9b2a, hue 63, chroma 0.165)
  in its own band just above the coral's, a step lighter than the coral (L 0.78 against 0.72)
  and 23 deg round from it. The selected row is a clean deep red (#7c1d1e, hue 25, chroma
  0.13), saturated enough to read red, not brick. `modified` and `warn` move up to amber (hue
  about 90), Jade takes the Orange slot, and tags are a wine rose.
- Pine: green as a hint, not a wash. The field is a spruce-slate charcoal (hue 212, chroma
  about 0.017; it was a teal at 0.026 and read too green) under the lantern gold, so the gold
  is the one vivid thing. It takes Dusk's gold families and status colours; its selected row is
  a petrol blue (#0e4b5c) under the gold accent bar: a gold wash on a dark field is brown.

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
| accent | `accent` | The theme's own colour: the lantern gold in Dusk (Dawn's iris beside its gold buttons), the lattice green in the Garden pair, high contrast and Sand. Focus rings, links, checked controls, the selected row's bar, the active tab's line, the main branch's line, column-resize lines, header highlights. |
| primary | `primary`, `primary_border`, `primary_off` | The fill and edge of primary buttons. |
| primary ink | `on_primary` | Text and icons on a primary fill (dark ink on the gold of Dusk and Dawn and the green of Garden dark, white elsewhere). |
| danger | `danger`, `danger_off` | Destructive buttons. |
| tag | `tag_bg`, `tag_fg` | Tag chips in the graph: the plum of the icon's lower edge in Dusk and Dawn, and in the Garden pair, high contrast and Sand too (their sand tags read brown). |

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

`repository.hues`: 8 quiet hues at OKLCH chroma 0.08, in slot order round the wheel Rose, Fern,
Mint, Teal, Sky, Slate, Iris, Plum (hues 10, 128, 176.5, 207, 237.5, 267.5, 298 and 335 deg, one
lightness per theme). No quiet hue between Rose and Fern is clean (40 to 125 deg reads khaki
or olive at chroma 0.08, and 35 to 60 is the coral's), so Mint, which replaced Ochre, sits just
above the green accent band and Teal, Sky and Slate make room, each about 30 deg from the next.
They mark repositories, only on the window chrome:

- a colour you give a repository (tab menu or welcome row > Color; none by default), saved
  per working folder (`Settings.repo_colours`, by `repo_keys`, as the names `rose` … `plum`;
  an older file's `ochre` loads as Mint):
  an 8 px square swatch on its tab, a 2 px line along the window's top edge for the active tab
  (`ui/repo_line.rs`), and a 10 px swatch before its path in the repository lists, which keep a
  swatch slot on every row once any repository has a colour;
- the worktree tab-group strips, which stay automatic and walk the hues in `GROUP_ORDER` (Rose,
  Teal, Plum, Sky, Fern, Iris, Mint, Slate), each at least 90 deg from the next.

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
between two lightness tiers (OKLCH L 0.70 / 0.80 in the dark tones). Beside Dusk's gold
accent, Jade (hue 165, at Gold's tier) takes the Gold slot, in the green the gold leaves free
(`theme/branch_palette_gold.rs`); Pine takes the same families. Beside Ember's flame, Jade takes
the Orange slot, and beside Graphite's and Linen's cobalt the Azure slot
(`theme/branch_palette_more.rs`); each family names its own slots for the Color submenu.

On paper a gold or olive dark enough for a 3:1 line reads brown, so the light tones differ:
Olive's slot is a deep Wine (hue 352), Gold's a Navy (hue 262), both darker than their
neighbours, and Orange is a clear amber at hue 62 (#c47100, at the sRGB edge: the one warm line
that stays clean at 3:1, even on Sand). Beside Linen's cobalt, Forest (hue 142) takes Navy's
slot; on Dawn, Jade takes it and Forest takes Indigo's, beside the iris accent. A chip whose
tone would letter it in brown (the amber's text tone) is lettered in the ink instead
(`contrast::clean_ink`), its fill and edge keeping the hue.

- `branch.lanes`: line tones, for lines, dots and bands (3:1 on bg and panel).
- `branch.chips`: chip tones, which ref chips are edged and lettered in (see Chip tints).

Dusk, Garden dark and High contrast dark use the dark tones for both. Dawn and Garden light use
bright line tones and darker text tones for chips. High contrast light and Sand use the text
tones for both (the amber line for Orange), as the line tones fall below 3:1 on their surfaces. Not a theme-file key: theme files get their base
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

Colour-blind friendly preset (Preferences > Appearance > Commit graph > Branch colors): 8
hues that stay apart under protan, deutan and tritan vision as well as normal vision, and keep
out of the accent and attention bands. Dark themes use its dark tones for lines and chips;
light ones use its line tones for lines and its text tones for chips. Under protan or deutan
vision Yellow and Lime pass for Dusk's gold, so Dusk's set (`Palette::colour_blind`) has Orange
(#eb8f13) and a paler Lime (#c8feac) in their slots. On paper a yellow line reads olive, so the
light set's Yellow is the clear amber (named Orange) and its Lime the deeper #4d7002, which stays
apart from the amber under protan vision; Dawn takes Linen's Wine for Periwinkle, which would pass
for the iris. Theme files take their base's set. The preset switches with the profile.
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
- No brown (#26): nothing a built-in theme paints reads as brown, tan, khaki, olive or rust.
  `contrast::brownish` defines the zone in OKLCH, judged from a rendered grid of swatches: a
  warm hue (40 to 125 deg; 55 to 125 at lightness 0.75 and up, where 40 to 55 is peach) with
  chroma 0.025 or more (0.010 darker than lightness 0.5) and lightness under 0.90 (paler is
  cream), unless it is a clear, saturated orange, amber, gold or yellow (chroma 0.15 at
  lightness 0.60, falling to 0.074 at 0.88; never under lightness 0.55); and the dull rust side
  of red (20 to 40 deg, chroma under 0.08, darker than 0.62). The consequence: a warm colour
  washed or tinted onto a dark surface is brown at any strength, and so is a strong warm tint
  on paper. So no view paints one:
  - Tints and washes go through `contrast::tint`, `contrast::unbrown` and `Palette::wash` /
    `color::wash` (never `gamma_multiply` of a status, accent or repository colour). A result
    in the zone keeps its lightness and takes the surface's own hue and chroma (a grey where
    that is warm too): the fill lifts the surface, and the colour moves to the text, edge,
    stripe or mark beside it.
  - Lines, edges and solid fills that would land in the zone go through `contrast::clear`,
    which lifts them in their own hue to a clear colour (OKLCH lightness 0.80, chroma 0.13).
  - Branch chips are neutral, edged and lettered in the branch colour (see Chip tints), never
    tinted.
  - Banners sit on a neutral `banner` fill; the stripe carries the kind's colour.
  - `theme/no_brown_tests.rs` composites every surface, band, highlighted band, chip and chip
    edge, initials fill, change chip, tone tint, tonal state and wash (each semantic and
    repository colour at 3% to 80% on bg, panel, toolbar and raised) and fails on any in the
    zone, in both branch presets. Dusk's bronze selected row (#573b11) is the one allowed
    exception (the owner's call on #25). It also checks every line, edge and letter drawn in a
    colour of its own: the accent, `primary_border`, the field and dialog edges, each branch
    lane, chip text and initials ink.
  - `theme/no_brown_text_tests.rs` checks every coloured text (status, `warn`, tag and hunk
    text, syntax but `syn_comment`, the terminal's red to cyan, normal and bright) against
    `contrast::brownish_text`. Text dark enough for 4.5:1 on paper can't take the chroma a warm
    fill needs (sRGB holds about 0.13 at that darkness), so a warm text colour is clean only
    as an orange at the gamut's edge: hue 40 to 66 deg (past that, amber turns ochre),
    lightness 0.50 and up and at least 95% of the chroma its lightness and hue hold. So the
    light themes' `warn`, `modified`, `syn_type` and terminal yellows are an orange-leaning
    amber (Dawn's #985700) that reaches 4.5:1 on the surfaces it sits on, and `syn_number` a
    vermilion (hue 34, Dawn's #c92d00) to stay apart from it. High contrast light's amber text
    is the one exception: at 7:1 on white any amber is chocolate. Neutral inks (body text,
    comments, the terminal's black, white and foreground) are judged as greys, not hues.
  - `snapshot_tests/snapshot_tests_colour_sheet.rs` renders every theme for judging by eye
    (`GLOWFLY_SHEET_DIR=<dir> cargo test -p glowfly-ui --lib colour_sheet -- --ignored`).
- Tinting a surface: the faint tints of a surface (graph row bands up to 7% toward the row's
  lane, the toolbar groups' backing 9% toward the accent, the dialog guide 7% and text fields
  4%) go through `contrast::tint`. It is a plain mix, except on a dark palette whose surface is
  a coloured field (OKLCH chroma 0.012 or more: Dusk's indigo, Midnight's blue, Pine's slate,
  Ember's charcoal). There a tint toward a far hue would cancel the field's colour into mud or
  an olive-green, so the tint keeps the mix's lightness, moves the field's chroma the same
  fraction toward the tint colour's, and turns the field's hue at most 20 deg toward the
  mix's. Whatever still lands in the brown zone is `unbrown`ed. A highlighted row (its branch
  hovered) is its band 16% toward the lane, `unbrown`ed the same way. `theme/tint_tests.rs`
  checks the family rule.
- The selected row's message is drawn in `strong`, in the graph as in the side panels.
- The accent never marks status. Running operations use `progress`, not the accent.
- Only the accent family (`accent`, `primary`, `primary_border`, `selected`; Ember's `selected`
  is its deep ember red instead, outside both bands) sits in the
  theme's accent band: the gold band, 72 to 92 deg OKLCH hue, in Dusk and Pine; the iris band,
  274 to 292 deg (between the Slate and Lavender repository hues), for Dawn's accent, whose
  buttons and selected row stay in the gold band; the flame
  band, 61 to 75 deg, in Ember; the cobalt band, 243 to 259 deg (between the Sky and Slate
  repository hues), in Graphite and Linen; the green band, 145 to 175 deg, in the others. The gold band is narrower as the warm side of the wheel is
  crowded; there the gold keeps its distance by lightness too (Orange at 70 deg is 0.12 OKLab
  from Dusk's gold).
- Status, branch and repository colours stay out of the theme's accent band and out of the
  attention band (about 35 to 60 deg), which belongs to the coral `needs_you`. Status colours
  also keep OKLCH chroma at 0.08 or more; repository hues sit at 0.08 and at least 30 deg apart.
- Neutrals: Garden dark and light keep OKLCH chroma at or below 0.015, tinted green (hue about
  160) and warm paper (hue about 90). Dusk's and Dawn's take the icon's indigo (hue 280 to 300):
  Dusk's at up to 0.045 chroma, Dawn's paper at up to 0.018 under indigo ink of up to 0.05. The
  six newer themes keep their surfaces and greys within 12 deg of one hue each (Midnight 255,
  Mist 235 under ink at 245, Ember 10, Pine 212, Linen 80 under ink at 55; Graphite's are under
  chroma 0.008 for surfaces and 0.012 for greys).
- No brown or olive dark surface: in every dark theme, a surface with chroma 0.008 or more keeps
  its hue out of 40 to 125 deg (see "No brown" above). Warmth comes from the accent; a selected
  row is a clean colour of its own (Graphite's cobalt, Ember's deep red, Pine's petrol blue),
  never a dark wash of a warm accent, except Dusk's bronze.
  Linen's paper keeps chroma at or under 0.012 so it reads white, not cream.
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
| danger hover, menu danger | The overlay's hover pill tinted 12% toward danger (`removed`, through `contrast::tint`, so a dark field keeps its hue and a warm tint on a grey stays grey); the danger colour pushed until it reads at 4.5:1 (7:1 in a high-contrast palette) on the overlay and on that pill. | A destructive menu item (Force push…, Delete…, Discard changes…): its label and hover pill (`widgets/menu.rs`'s `danger_style`). `danger` itself is a button fill under white text, not a text colour. |
| filled shading | A filled button's fill 10% (hover) or 18% (pressed) toward white in dark palettes, black in light ones. | Primary, secondary and danger buttons. |
| tones | Accent, success (`added`), warning (`warn`), danger (`removed`) and info (`info`), each with a tint: the change-chip tint of the colour, with the colour pushed to read on it (below). | Toast outlines, the commit caret while hooks are skipped. |
| tonal | The accent's tint as the fill (moved 10% / 18% toward the accent on hover / press) with the accent as ink, pushed per state until it reads at 4.5:1 (7:1 in a high-contrast palette). | Tonal buttons: important actions that aren't the view's main one (Stage all). |

## 5. Chip tints

Ref chips in the graph (`contrast::ref_chip`, painted by `graph/labels.rs`'s `paint_box`), every
fill opaque with its text at 4.5:1 or more:

- A branch or remote branch on a dark palette: a neutral fill, the background 10% toward
  `text`, with a 1 px edge (the fill 70% toward the branch's chip tone, `clear`ed) and text in
  the chip tone. A tint of the branch colour would be brown for every warm branch (orange,
  gold, lime, olive), so no chip is tinted, and all chips look alike whatever their hue.
- On a light palette: a pale wash of the branch's hue (OKLCH lightness 0.955, chroma at most
  0.035), with the same edge and text. Paler than 0.90, a warm wash reads cream.
- The checked-out branch: on a dark palette a solid fill of its chip tone (`clear`ed, so a
  warm one is a bright orange or gold) with dark text; on a light one a deeper wash (lightness
  0.90, chroma at most 0.09) with a full-colour edge.
- Tags keep the palette's `tag_bg` / `tag_fg` (plum in most themes), a detached HEAD the
  background 16% toward `text`.

A change chip (the WIP row's M / A / D) is an opaque tint of its status colour over the base:
22% in dark palettes and 10% in light ones (`contrast::change_chip`), `unbrown`ed: a warm one
(modified) on a dark palette is a neutral step with the amber letters. The chip text is the
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
| warn / banner | #fba541 / #2b273b | |
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
| accent | #5a47cf | Iris, OKLCH 0.50 0.20 283; 6.1:1 on panel, 5.2:1 on selected |
| primary / on_primary | #efb146 / #1b1933 | The lantern gold with indigo ink, 9.0:1 |
| primary_border | #d19006 | A clear gold edge |
| tag_bg / tag_fg | #f5d8ea / #763463 | Plum, 6.5:1 |
| added / removed / modified / info / progress | #357518 / #b63132 / #985700 / #1f68bc / #6d47b8 | modified and warn: one orange-leaning amber at the gamut's edge, 4.5:1 on every row surface |
| warn / banner | #985700 / #fee3c5 | |
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
| tag_bg / tag_fg | #4a283f / #f4c9e4 | 8.6:1 |
| added / removed / modified / info / progress | #7bc85f / #ea6a64 / #edb345 / #60aaf3 / #a689f1 | |
| warn / banner | #e8b339 / #222925 | |

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
| tag_bg / tag_fg | #f5d8ea / #763463 | 6.5:1 |
| added / removed / modified / info / progress | #357518 / #b63132 / #975700 / #1f68bc / #6d47b8 | modified and warn 4.56:1 at least, on selected |
| warn / banner | #975700 / #fbecc8 | |

### The six newer themes

Surfaces, text and brand (`theme/builtin_sea.rs`, `builtin_warm.rs`, `builtin_cobalt.rs`). Their
diff colours are Dusk's (dark) or Dawn's (light), their syntax colours Garden dark's or Garden
light's with `syn_comment` in their own grey, and `danger` Dusk's or Dawn's.

| Token | Midnight | Mist | Graphite | Ember | Pine | Linen |
|---|---|---|---|---|---|---|
| chrome (`toolbar`) | #08111d | #dee9f0 | #121315 | #181112 | #0b1517 | #ebe7e0 |
| base (`bg`) | #0d1827 | #f2f8fc | #191a1d | #211819 | #111d1f | #f9f7f3 |
| panel | #14202f | #f8fcfe | #202225 | #281f20 | #182426 | #fcfbf9 |
| raised | #222f40 | #e5eff5 | #2f3134 | #372e2f | #273336 | #f0ede7 |
| input | #0a1421 | #ffffff | #141618 | #1b1415 | #0d181a | #ffffff |
| hairline (`border`) | #283545 | #cfdae1 | #35373a | #3e3435 | #2c393c | #dcd7d1 |
| input_border, dialog_border | #798ba2 | #687683 | #868990 | #938587 | #7b8c90 | #7a736f |
| hover | #213044 | #e2eef5 | #303236 | #3a2e30 | #263538 | #efebe3 |
| selected | #174a39 | #c7eddd | #1f3d62 | #7c1d1e | #0e4b5c | #d6e8fe |
| text | #e2e8f0 | #142533 | #e6e7e9 | #ebe6e6 | #e3e8ea | #28211c |
| weak / faint | #adbacc / #8f9eb3 | #475867 / #5e6f7d | #b7b9bc / #9a9da2 | #c0b6b7 / #a5999b | #afbcbf / #91a0a3 | #5c544f / #726b66 |
| accent, primary | #6eddb2 | #007258 | #7ebbff | #ff9b2a | #f6c159 | #004da8 |
| on_primary | #08121f | #ffffff | #0a1018 | #1b1113 | #091619 | #ffffff |
| primary_border | #4eb891 | #005641 | #5695e0 | #d97f01 | #d5a13c | #003a7f |
| tag_bg / tag_fg | #3d2d4e / #e0cef8 | #e7dbf7 / #5e3d80 | #3e314d / #e0cff5 | #4f2a3c / #f7c8dd | #4a283f / #f4c9e4 | #f8dae2 / #813450 |
| added / removed | Garden dark's | Garden light's | Garden dark's | Garden dark's | Dusk's | Garden light's |
| modified / warn | #edb345 / #e8b339 | #975600 / #975600 | #edb345 / #e8b339 | #e3b842 / #e9c447 | Dusk's | #985700 / #985700 |
| info / progress | #60aaf3 / #a689f1 | #1f68bc / #6d47b8 | #4fc0e7 / #a689f1 | #60aaf3 / #a689f1 | Dusk's | #006e91 / #6d47b8 |
| banner | #1c2939 | #fbecc8 | #292b2e | #312829 | #212d30 | #fbecc8 |

Text on the panel reaches 12.8 to 13.3:1 in the dark ones and 15.2 to 15.3:1 in the light ones;
the accent 5.7 to 9.9:1. The branch families: Midnight takes Garden dark's, Mist Garden light's,
Pine Dusk's, Ember the flame ones and Graphite and Linen the cobalt ones (below).

### Branch

Slot by slot (`theme/branch_palette.rs`): dark tones (Dusk, Garden dark and High contrast
dark), light text and chip tones (Dawn's and Garden light's chips; every line and chip in High
contrast light and Sand) and light line tones (Dawn's and Garden light's lines, dots and
bands). In Dusk slot 11 is Jade (#56d1a3 dark) in place of Gold; in Dawn it is Jade (#176449
light text, #2d9570 light line, hue 165) in place of Navy, and slot 13 Forest (#1d4f19 / #2c6b27,
hue 142) in place of Indigo. Linen's slot 11 is Forest. The light slots 3, 6 and 11 differ in hue
from the dark ones (see Branch above).

| Slot | Hue | Dark | Light text | Light line |
|---|---|---|---|---|
| 0 | Rose | #e77492 | #861f43 | #c04d6e |
| 1 | Lime | #a3cf64 | #4d7002 | #719c27 |
| 2 | Violet | #c5aeff | #6d4ea8 | #9776db |
| 3 | Orange | #d78c1a | #7c4600 (lettered in the ink) | #c47100 |
| 4 | Cyan | #04d4f1 | #057080 | #0b9db3 |
| 5 | Magenta | #fb97db | #953d7c | #c664a8 |
| 6 | Olive (dark) / Wine (light) | #9ea827 | #6f1647 | #952c63 |
| 7 | Azure | #93c1fe | #2462b1 | #498de5 |
| 8 | Red | #fea195 | #a43c33 | #d86357 |
| 9 | Teal | #0ab5b1 | #005957 | #0a8c89 |
| 10 | Orchid | #c37fd8 | #6b2d7d | #9e59b3 |
| 11 | Gold (dark) / Navy (light) | #dfba3b | #17397b | #2750a2 |
| 12 | Sky | #0aabea | #005475 | #0685b6 |
| 13 | Indigo | #8794f7 | #3b4097 | #646fd1 |

The colour-blind friendly preset, slot by slot: dark tones (Dark and High contrast dark), light line
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
| 2 | Yellow (dark) / Orange (light) | #d1c038 | #c47100 | #7c4600 | 102 / 62 |
| 3 | Blue | #2898c9 | #207aa0 | #106082 | 233 |
| 4 | Plum | #ba7294 | #b2397b | #ac1c6f | 350 |
| 5 | Lime | #ccee6e | #4d7002 | #426101 | 122 / 128 |
| 6 | Purple | #9d62f4 | #9169d5 | #482b74 | 299 |
| 7 | Periwinkle | #a2b5f2 | #3a45cf | #4158bd | 271 |

Dusk's colour-blind set swaps two dark tones: slot 2 is Orange #eb8f13 (hue 65) and slot 5 a
paler Lime #c8feac (hue 135), as Yellow and Lime fall within 0.08 of the gold under protan and
deutan vision. Dawn takes Linen's set (Wine for Periwinkle). Pine takes Dusk's set. Beside the flame,
Yellow falls within 0.04 of Ember's accent under deutan vision, so Ember's set has a light Jade
(#57dcb4, hue 170) in slot 2. Beside the cobalt, Periwinkle falls within 0.05 of the accent under
protan and deutan vision, so Graphite's set has a light Rose (#f49194, hue 18) in slot 7 and
Linen's a deep Wine (#800953, hue 350) as both line and chip tone. Midnight and Mist take the
standard sets.

High contrast dark, High contrast light and Sand keep their own surfaces, text, diff and syntax
colours (their `raised` is their `toolbar`); their accent family is green at their own
contrast: accent #5fe0a0 / #00613a / #2a7550 and primary #003d26 / #005c36 / #236a48, all
with white ink. High contrast dark's primary is dark enough that white still reads at 7:1 on
a hovered or pressed button.

### Repository hues and the coral

In slot order Rose, Fern, Mint, Teal, Sky, Slate, Iris, Plum, as `#rrggbb` without the `#`
(OKLCH lightness 0.70, 0.57, 0.78, 0.47 and 0.56):

| Theme | Hues | Lowest on toolbar, bg, panel, raised and hover |
|---|---|---|
| Dusk | cb8a93 90a872 65af9d 5dadb8 6ea6ca 899ed2 a494ca be8cb2 | 4.74 |
| Dawn | a0636d 6a814c 3a8776 2e8590 477ea0 6376a7 7d6da1 95658a | 3.43 |
| Garden dark | cb8a93 90a872 65af9d 5dadb8 6ea6ca 899ed2 a494ca be8cb2 | 4.83 |
| Garden light | a0636d 6a814c 3a8776 2e8590 477ea0 6376a7 7d6da1 95658a | 3.50 |
| High contrast dark | e5a2ac a9c28a 7cc8b5 76c6d1 86bfe4 a1b6ea bdade5 d8a4cc | 7.09 |
| High contrast light | 814750 4e6330 156a5a 0a6771 286182 475988 605182 76496d | 5.08 |
| Sand | 9d606a 677e49 368473 2a828d 447b9d 6073a4 7a6a9e 926287 | 3.36 |
| Midnight, Ember, Pine, Graphite | Dusk's | 4.60 |
| Mist, Linen | Dawn's | 3.47 |

The coral (OKLCH hue about 40 deg) and its ink, with the ink's contrast at rest and on hover:

| Theme | needs_you / on_needs_you | Ink contrast |
|---|---|---|
| Dusk, Garden dark | #fb794a / #22110b | 6.88, 7.52 |
| Dawn, Garden light, Sand | #bf4306 / #ffffff | 5.22, 6.17 |
| High contrast dark | #ff8a5c / #22110b | 7.84, 8.53 |
| High contrast light | #9e3301 / #ffffff | 7.17, 8.26 |
| Midnight, Ember, Pine, Graphite | #fb794a / #22110b | 6.88, 7.52 |
| Mist, Linen | #bf4306 / #ffffff | 5.22, 6.17 |

The coral reads at least 3.89:1 on its surroundings (lowest: Dusk's selected row). The tool
backing is #281f32 (Dusk, a violet step: the plain mix was the grey-brown #272225), #dcd7d9 (Dawn), #15261d (Garden dark), #d9dfd3 (Garden light),
#1d2a27 (High contrast dark), #d4dedd (High contrast light) and Sand's near-grey step, at most OKLCH
chroma 0.040.

### Terminal colours

0-7 then 8-15, as `#rrggbb` without the `#`. Every built-in's terminal background, text, cursor
and selection are its `input`, `text`, `accent` and `accent`:

| Theme | 0-7 | 8-15 |
|---|---|---|
| Dusk | 2b2939 f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 ceccda | 8c899d ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f5f4fa |
| Dawn | 1d1c31 c0362f 2f7d1c ab6500 2a5fc4 8a3fb8 087c80 d8d6e1 | 686679 a3241e 226614 9d5900 1d4ba3 70299b 006468 e3e2ec |
| Garden dark | 2a322d f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 c9d1cc | 86918a ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f4f7f5 |
| Garden light | 1c2420 c0362f 2f7d1c ab6500 2a5fc4 8a3fb8 087c80 d3d9d5 | 6b746f a3241e 226614 9d5900 1d4ba3 70299b 006468 e2e7e4 |
| High contrast dark | 303030 ff6b6b 5fe36a ffd84a 7ab4ff e08aff 4fe0e0 e6e6e6 | a6a6a6 ff9c9c 9cf0a2 ffec96 a8ccff eeb5ff 96f0f0 ffffff |
| High contrast light | 000000 a31515 08621a 824c00 0646a8 6a1fa0 005f63 d0d0d0 | 55595f 7f0d0d 084f15 744000 043682 52157d 004a4d e5e5e5 |
| Sand | 2e2519 b23a2b 4a7a1a a96400 2d65a8 84429e 1b7570 e3d9c6 | 76695a 93291c 3a6312 9b5800 1f4f8a 6a2f82 0f5d59 eae2d2 |
| Midnight | 253141 f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 c7cfd9 | 8793a3 ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f1f5fb |
| Mist | 10212f c0362f 2f7d1c ab6500 2a5fc4 8a3fb8 087c80 d0dbe2 | 606e79 a3241e 226614 9d5900 1d4ba3 70299b 006468 dae4ea |
| Ember | 3d2b2c f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 d7cacb | 9f8d8e ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede faf3f4 |
| Pine | 203435 f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 c5d0d1 | 849697 ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f0f6f7 |
| Graphite | 2f3033 f0726b 7fcc63 e3bd4f 6aa8f5 c28af2 4fc4c4 cdced0 | 919295 ff9a92 a3e388 f5d77a 9cc6ff dcb0ff 80dede f4f5f6 |
| Linen | 251e19 c0362f 2f7d1c ab6500 2a5fc4 8a3fb8 087c80 ddd8d1 | 716a66 a3241e 226614 9d5900 1d4ba3 70299b 006468 e5e1db |

The lowest ratios of a chromatic colour on `input` are 4.99 (Dawn and Garden light 6) and 5.01 (Sand 2), and
7.36 in High contrast light.

## 7. Theme-file keys

A theme file (Preferences > Appearance > Theme files) sets any of these keys, in this
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

The rest of a new built-in: a `BuiltinTheme` variant with its id, label and kind and a
`ThemeChoice` variant listed in `ThemeChoice::BUILT_IN` (`glowfly-core/src/settings/builtin_themes.rs`),
its name and one-line description (`prefs.ui.theme.name.*`, `prefs.ui.theme.hint.*`) in every
locale, its accent band in `theme/bands.rs` (and a branch family that keeps out of it), its
palette in `builtin.rs`, its values in the pinned tables of the colour tests, and a `theme_*`
snapshot. The Theme menu and Preferences' cards pick it up from `BUILT_IN`.
