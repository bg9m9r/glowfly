# Colour

How Trellis uses colour: the families every palette is built from, what each colour is for,
the hue bands and contrast floors the tests hold every built-in theme to, and how to add a
colour. The palettes live in `crates/trellis-app/src/ui/theme/` (`color.rs` declares
`Palette`, `families.rs` the token families, `builtin.rs` and `builtin_extra.rs` the five
built-ins); the tests that check this page are `theme/tests.rs` and `theme/family_tests.rs`.

Views never name a hex value: they read the active palette through `color::text()`,
`color::added()` and so on (see "Rules" in [ARCHITECTURE.md](ARCHITECTURE.md)).

## 1. Families

### Neutrals

| Token | Theme key | What it is for |
|---|---|---|
| chrome | `toolbar` | The frame around the content: toolbar, status bar, terminal header, Preferences side panel and save bar, the active repository tab. In Dark it is the darkest surface. |
| base | `bg` | The canvas behind panels. |
| panel | `panel` | Panels, dialogs, list backgrounds. |
| raised | `raised` | Controls and small fills a step off the panel: egui buttons and combo boxes, disabled styled buttons, chips, cards, tooltips, toasts, segmented tracks, keycaps, pane header strips. |
| input | `input` | Text fields; also the check mark on an accent-filled checkbox. |
| hairline | `border` | Lines between regions. |
| | `input_border`, `dialog_border` | The edge of text fields, checkboxes and radios; the frame of dialogs. |
| | `hover`, `selected` | A hovered row; the selected row (a tint of the accent). |
| text, strong, weak, faint | `text`, `strong`, `weak`, `faint` | Three text steps, plus `strong` for text on selection. |

### Brand

| Token | Theme key | What it is for |
|---|---|---|
| accent | `accent` | The icon's lattice green: focus rings, links, checked controls, the selected row's bar, the active tab's line, column-resize lines, header highlights. |
| primary | `primary`, `primary_border`, `primary_off` | The fill and edge of primary buttons. |
| primary ink | `on_primary` | Text and icons on a primary fill (dark ink in Dark, white elsewhere). |
| danger | `danger`, `danger_off` | Destructive buttons. |
| sand | `tag_bg`, `tag_fg` | Tag chips in the graph: the icon's sand as the second brand colour. |

### Status

| Token | Theme key | Main use | Also marks |
|---|---|---|---|
| added | `added` | Added lines and files | Success, done |
| removed | `removed` | Removed lines, deleted files | Errors, failures |
| modified | `modified` | Modified files | |
| info | `info` | Renames, info banners | Links to services |
| progress | `progress` | A running operation (banner and spinner) | Pull requests, squash and fixup |

Status colours are used for status letters, line counts and bars, the WIP row's change chips
and the matching banners.

### Attention

| Token | Theme key | What it is for |
|---|---|---|
| warn | `warn` | Warnings. Until each job has its own colour it also marks favourites and pending checks. |
| banner | `banner` | The fill of banners. |

The attention band (below) stays free for a later coral "needs you" colour.

### Repository

`repository.groups`: the four worktree tab-group strip colours. Not a theme-file key yet;
theme files always get the base palette's.

### Branch

`branch.lanes`: the ten graph lane colours, the same in every built-in. Branch-led colour
replaces them with hues chosen per branch. Not a theme-file key.

### Diff and syntax

`diff_*`, `hunk_*` and `syn_*` colour diff lines, hunk headers and syntax tokens. They are
outside the families and unchanged by the brand colour.

## 2. Rules

- One job per colour: each colour means one thing (with the related uses listed above), and
  no two families share a colour for different meanings.
- The accent never marks status. Running operations use `progress`, not the accent.
- Only the accent family (`accent`, `primary`, `primary_border`, `selected`) sits in the
  accent band, about 145 to 175 deg OKLCH hue.
- Status and branch colours stay out of the accent band and out of the attention band (about
  35 to 60 deg). Status colours also keep OKLCH chroma at 0.08 or more. Two lanes (6 and 9)
  sit in a band today; the test lists them as known exceptions until Branch-led colour.
- Neutrals in Dark and Light keep OKLCH chroma at or below 0.015: Dark is tinted green (hue
  about 160), Light's surfaces are warm paper (hue about 90).
- In Dark and Light, chrome is darker than the base, and raised is at least 1.08:1 off the
  panel in every theme.

OKLCH is computed from sRGB with Björn Ottosson's OKLab matrices (`contrast::oklch`, test
only).

## 3. Contrast floors

WCAG contrast ratio, with Trellis's 0.03928 linearisation threshold. Where two values are
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
| Change chip text | its chip tint | 4.5 |
| Every `syn_*` | panel, diff_add, diff_del, diff_add_word, diff_del_word | 2.4 / 4.5 |

## 4. Chip tints

A change chip (the WIP row's M / A / D) is an opaque tint of its status colour over the base:
22% in dark palettes and 10% in light ones (`contrast::change_chip`). The chip text is the
status colour itself wherever that reaches 4.5:1 on the tint; otherwise `readable_on` pushes it
5% at a time toward white (dark) or black (light) until it does. In Light that is only
modified (3.9:1 unpushed); in Dark, removed and progress are pushed.

## 5. Values

### Dark

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

### Light

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

High contrast dark, High contrast light and Sand keep their own surfaces, text, diff and syntax
colours (their `raised` is their `toolbar`); their accent family is green at their own
contrast: accent #5fe0a0 / #00613a / #2a7550 and primary #003d26 / #005c36 / #236a48, all
with white ink. High contrast dark's primary is dark enough that white still reads at 7:1 on
a hovered or pressed button.

## 6. Theme-file keys

A theme file (Preferences > UI Customization > Theme files) sets any of these keys, in this
order, to `#rrggbb` or to another key's name; `trellis-core`'s `COLOR_KEYS` and
`Palette::KEYS` hold the same list:

`bg`, `panel`, `toolbar`, `raised`, `input`, `border`, `input_border`, `dialog_border`,
`hover`, `selected`, `accent`, `text`, `strong`, `weak`, `faint`, `added`, `removed`,
`modified`, `info`, `progress`, `diff_add`, `diff_del`, `diff_add_sel`, `diff_del_sel`,
`diff_add_word`, `diff_del_word`, `hunk_bg`, `hunk_fg`, `warn`, `banner`, `primary`,
`on_primary`, `primary_off`, `primary_border`, `danger`, `danger_off`, `tag_bg`, `tag_fg`,
`syn_comment`, `syn_string`, `syn_number`, `syn_constant`, `syn_keyword`, `syn_type`,
`syn_function`, `syn_attr`.

The older names still load, as keys and as values: `green` is `added`, `red` is `removed`,
`amber` is `modified`, `blue` is `info` and `purple` is `progress`. When a file sets both the
old and the new name, the new name wins. Keys a file leaves out (including `raised`,
`on_primary` and `warn` in older files) come from its base theme, with one exception: a file
that sets its own `primary` but no `on_primary` keeps the base's ink only while it reaches 4.5:1
on that primary, and otherwise gets white or black, whichever reads better.

## 7. Adding a colour

1. Add the field to its family in `families.rs`, or to the flat neutrals or brand groups of
   `Palette` in `color.rs`, and give it a value in every built-in.
2. Add its `key => path` entry to `palette_fields!` in `color.rs` and its key to `COLOR_KEYS`
   in `trellis-core/src/settings/themes.rs`, at the same position (`keys_match_theme_files`
   checks the two lists match).
3. Add a contrast or band rule for it to `theme/tests.rs` or `theme/family_tests.rs`.
4. Describe its job on this page.
