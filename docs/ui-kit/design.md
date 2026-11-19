# UI design: Console

The visual direction for the GPUI Kit UI (#335). The owner picked direction **C · Console**, with
both a dark and a light mode, on [#338](https://github.com/mastro993/idleon-optimizer/issues/338)
(comparison of the three directions: [mocks](https://github.com/mastro993/idleon-optimizer/issues/338#issuecomment-6974555266),
[pick](https://github.com/mastro993/idleon-optimizer/issues/338#issuecomment-6974972543)).

Console is a dense pro tool, in the spirit of Supabase's table editor and Railway's logs: a
near-black graphite ground, one violet accent, flat panels split by hairlines, and a monospace face
for every number. The app is a wall of numbers the player compares, so density and alignment come
first.

Tokens live in [`crates/app/data/theme.json`](../../crates/app/data/theme.json), a gpui-kit 0.10.0 theme set with the two
modes, which the app loads (see [Loading the theme](#loading-the-theme)).

| | Dark | Light |
|---|---|---|
| Home | ![Console dark: Home](https://github.com/user-attachments/assets/aefe467b-1080-49bb-b66d-11b5f3d51a1b) | ![Console light: Home](https://github.com/user-attachments/assets/df741930-ab8b-4e30-aa03-e74e785bd503) |
| Table-heavy: Research › Rates | ![Console dark: Research Rates](https://github.com/user-attachments/assets/0b8004da-3699-466a-bd33-e7d61b63c680) | ![Console light: Research Rates](https://github.com/user-attachments/assets/0f2f0667-5ee1-4e19-8770-089bd5764565) |
| Form-heavy: Farming › Land Rank plan | ![Console dark: Farming Planner](https://github.com/user-attachments/assets/989e8f8d-52cf-4567-88ff-d6b192edd3f8) | ![Console light: Farming Planner](https://github.com/user-attachments/assets/df143152-3e41-4e4b-b5c4-fd8e4082103a) |

The mocks are 1440×900 HTML renders with fixture-like data. They fix the look, not the exact
content of each section.

## Color tokens

Each row is one design token, its value in each mode, and the kit theme keys that carry it.
Sections never write a color literal; they read `cx.theme()` or the `idleon_ui` tokens.

### Surfaces and text

| Token | Dark | Light | Kit keys | Use |
|---|---|---|---|---|
| background | `#0a0a0c` | `#f6f6f8` | `background`, `title_bar.background`, `tab_bar.background` | Window ground behind panels |
| surface | `#111114` | `#ffffff` | `table.background`, `list.background`, `group_box.background`, `accordion.background` | Panels, cards, tables |
| elevated | `#18181c` | `#f0f0f3` | `muted.background`, `popover.background`, `secondary.background`, `table.head.background`, `list.head.background`, `description_list.label.background`, `skeleton.background` | Table heads, popovers, secondary buttons, input wells |
| sunken | `#070708` | `#ececf0` | `sidebar.background`, `tab_bar.segmented.background` | Sidebar, segmented-control track |
| hover | `#1e1e23` | `#e8e8ed` | `accent.background`, `list.hover.background`, `table.hover.background`, `secondary.hover.background` | Hovered rows, menu items, buttons |
| border | `#232328` | `#e1e1e6` | `border`, `table.row.border`, `sidebar.border`, `title_bar.border`, `window.border`, `chart.grid` | Hairlines between panels and rows |
| border strong | `#313137` | `#cbcbd3` | `input.border`, `secondary.active.background`, `switch.background`, `scrollbar.thumb.background` | Input outlines, pressed secondary, switch track |
| foreground | `#ededf0` | `#18181b` | `foreground`, `popover.foreground`, `secondary.foreground`, `tab.active.foreground`, `sidebar.accent.foreground` | Body text, values |
| muted | `#9a9aa4` | `#52525b` | `muted.foreground`, `tab.foreground`, `sidebar.foreground`, `description_list.label.foreground`, `table.foot.foreground` | Labels, descriptions, inactive tabs and nav |
| subtle | `#8a8a94` | `#61616b` | `table.head.foreground` | Table heads, captions, units, group headings |

### Accent and status

| Token | Dark | Light | Kit keys | Use |
|---|---|---|---|---|
| accent | `#a78bfa` | `#6d4ae0` | `primary.background`, `ring`, `caret`, `link`, `list.highlight.foreground`, `progress.bar.background`, `slider.background`, `drag.border`, `list.active.border`, `table.active.border` | The one accent: primary button, focus, selection edge, highlighted value |
| accent hover | `#b9a2fb` | `#7c5cf0` | `primary.hover.background`, `link.hover` | |
| accent active | `#9071f0` | `#5b3bc9` | `primary.active.background`, `link.active` | |
| on accent | `#140c2b` | `#ffffff` | `primary.foreground`, `sidebar.primary.foreground` | Text on a filled accent |
| accent tint | accent at 14% (`#a78bfa24`) | accent at 14% (`#6d4ae024`) | `list.active.background`, `table.active.background`, `sidebar.accent.background` | Selected row, active nav item |
| selection | accent at 35% | accent at 35% | `selection.background` | Text selection |
| success | `#4ade80` | `#166534` | `success.background`, `chart.bullish`, `base.green` | Gains, "found", positive deltas |
| warning | `#facc15` | `#854d0e` | `warning.background`, `base.yellow` | Caps reached, save warnings |
| danger | `#f87171` | `#b91c1c` | `danger.background`, `chart.bearish`, `base.red` | Errors, losses, destructive actions |
| info | `#60a5fa` | `#1d4ed8` | `info.background`, `base.blue` | Neutral notices |
| on status | `#0a0a0c` | `#ffffff` | `success.foreground`, `warning.foreground`, `danger.foreground`, `info.foreground` | Text on a filled status color |
| overlay | `#000000a6` | `#18181b66` | `overlay` | Behind dialogs |

The status colors double as text colors on every surface (a `+38.6%` gain, a warning callout title);
the contrast tables below cover that use. Hover and active variants of each status fill are in the
JSON (`*.hover.background`, `*.active.background`): 12% toward white (dark) or black (light) on
hover, 15% toward black when pressed.

### Charts

| Series | Dark | Light |
|---|---|---|
| `chart.1` | `#a78bfa` | `#7c5cf0` |
| `chart.2` | `#60a5fa` | `#2563eb` |
| `chart.3` | `#4ade80` | `#16a34a` |
| `chart.4` | `#facc15` | `#b45309` |
| `chart.5` | `#f472b6` | `#db2777` |

Bars and series start at `chart.1` and keep the same order across sections, so the same series
reads the same color everywhere. `chart.grid` is the border color.

### World badges

Not kit keys: the app keeps one color per world next to its `Group` enum (`world_color` in
`crates/app/src/ui/theme.rs`), and `WorldBadge` draws it as a small kit `Tag`. A badge is the world
color as text, the same color at 14% as fill, and at 30% as a 1 px outline, in the active mode.

| Group | Dark | Light |
|---|---|---|
| General | `#a1a1aa` | `#52525b` |
| W2 | `#60a5fa` | `#1d4ed8` |
| W3 | `#4ade80` | `#166534` |
| W4 | `#c084fc` | `#7e22ce` |
| W5 | `#818cf8` | `#4f46e5` |
| W6 | `#fb7185` | `#be123c` |
| W7 | `#fbbf24` | `#92400e` |
| Tools | `#5eead4` | `#115e59` |

## Type

- **UI text:** the system UI font (`.SystemUIFont`: SF Pro on macOS, Segoe UI on Windows). The
  theme leaves `font.family` unset so the kit picks it. Nothing to bundle.
- **Numbers:** JetBrains Mono (SIL Open Font License 1.1, free to bundle and redistribute) for every
  value, id, count, stepper value and KPI figure. It is fixed-width, so digits line up down a column
  without a tabular-figures feature. The theme names it in `mono_font.family`; the kit uses a named
  family as-is, so the app bundles Regular, Medium and SemiBold in `crates/app/fonts/` (license
  beside them) and registers them with `add_fonts` before applying the theme.

| Role | Size / weight | Face | Color |
|---|---|---|---|
| Page title | 18 / 600 | UI | foreground |
| Panel title | 13 / 600 | UI | foreground |
| Body, table cell (`font.size`) | 13 / 400 | UI | foreground |
| Number in a cell or stepper (`mono_font.size`) | 12 / 400, 600 for the key column | mono | foreground |
| KPI value | 18 / 500 | mono | foreground (status color for a delta) |
| Label (form field, KPI caption) | 11 / 500 | UI | muted |
| Table head, sidebar group heading | 11 / 500, uppercase, +0.05em tracking | UI | subtle |
| Caption, unit, footnote | 11 / 400 | UI | subtle |

## Density and shape

| Rule | Value |
|---|---|
| Control height (button, input, select, stepper) | 26 px; 22 px for small buttons in panel headers |
| Table row / table head | 28 px / 28 px |
| Sidebar item | 26 px; sidebar width 228 px |
| Panel padding | 12 px |
| Gap between panels | 8 px; 12 px between a page header and its content |
| Page padding | 16 px top and bottom, 20 px left and right |
| Radius (`radius` / `radius.lg`) | 4 px for controls and panels, 6 px for dialogs and popovers |
| Depth | no shadows (`shadow: false`); panels separate by a 1 px border and the surface step |
| Sprites | 16 px in the sidebar, tables and Home; 20 px in a page header; scaled by whole pixels with nearest-neighbor filtering, never blurred |

## Patterns

- **Page header:** sprite, title and world badge on the left; the section's headline metric on the
  right in accent mono (`14.32B EXP/hr`) with its caption under it. Primary actions sit on the
  right in place of the metric when the page has them.
- **Tabs:** underline tabs on the background, muted text, the active tab in foreground with a 2 px
  accent underline.
- **KPI strip:** one bordered surface panel split into cells by vertical hairlines, not one card per
  number. Each cell is a label, a mono value and a subtle mono detail line.
- **Tables:** head row on elevated, uppercase subtle labels; numbers right-aligned in mono; text
  left-aligned in the UI face. Row hairlines, no zebra stripes. The selected row is the accent tint
  with a 2 px accent bar on its left edge; its name may turn accent.
- **Forms:** a panel with a fixed 112 px label column on the left and controls on the right, one
  field per 32 px line. Two or three exclusive options use a segmented control; booleans use
  switches; numbers use mono steppers sized to their content, not stretched.
- **Callouts:** a sunken row with a status dot, a bold status-colored title and muted body text.
- **Home:** one table of every calculator (sprite, name, world badge, what it answers, last opened)
  in place of a card grid, so the 28 calculators fit on one screen.

## Do and don't

| Do | Don't |
|---|---|
| Put every number in the mono face and right-align it | Mix proportional and mono digits in one column |
| Use one accent per screen region, for the thing that matters (selection, primary action, headline metric) | Color headings, icons and borders with the accent "for interest" |
| Split related numbers with hairlines inside one panel | Wrap each number in its own shadowed card |
| Size steppers, selects and segmented controls to their content | Stretch a stepper or a two-option segmented control across the panel |
| Use status colors only for status (gain, cap, error) | Use success green or warning yellow as decoration |
| Keep sprites at 16 or 20 px with whole-pixel scaling | Blur pixel art by scaling it to an odd size |
| Read colors from the theme | Write `rgb(..)` / `rgba(..)` literals in section or shell code |

## Contrast

WCAG 2.x contrast ratios, computed from the values above. AA needs 4.5:1 for text and 3:1 for
graphical objects such as chart series and the focus ring. Every text pair passes.

### Dark

| Text | background `#0a0a0c` | surface `#111114` | elevated `#18181c` | sidebar `#070708` | hover `#1e1e23` |
|---|---:|---:|---:|---:|---:|
| foreground `#ededf0` | 16.93 | 16.13 | 15.15 | 17.23 | 14.21 |
| muted `#9a9aa4` | 7.10 | 6.76 | 6.35 | 7.22 | 5.95 |
| subtle `#8a8a94` | 5.79 | 5.51 | 5.18 | 5.89 | 4.86 |
| accent `#a78bfa` | 7.27 | 6.93 | 6.50 | 7.40 | 6.10 |
| success `#4ade80` | 11.35 | 10.82 | 10.16 | 11.56 | 9.53 |
| warning `#facc15` | 12.92 | 12.31 | 11.56 | 13.15 | 10.84 |
| danger `#f87171` | 7.15 | 6.81 | 6.40 | 7.28 | 6.00 |
| info `#60a5fa` | 7.78 | 7.41 | 6.96 | 7.92 | 6.53 |
| world General `#a1a1aa` | 7.72 | 7.35 | 6.91 | 7.86 | 6.48 |
| world W2 `#60a5fa` | 7.78 | 7.41 | 6.96 | 7.92 | 6.53 |
| world W3 `#4ade80` | 11.35 | 10.82 | 10.16 | 11.56 | 9.53 |
| world W4 `#c084fc` | 7.49 | 7.13 | 6.70 | 7.62 | 6.28 |
| world W5 `#818cf8` | 6.63 | 6.32 | 5.93 | 6.75 | 5.56 |
| world W6 `#fb7185` | 7.35 | 7.00 | 6.58 | 7.48 | 6.17 |
| world W7 `#fbbf24` | 11.85 | 11.29 | 10.60 | 12.06 | 9.94 |
| world Tools `#5eead4` | 13.37 | 12.74 | 11.97 | 13.61 | 11.22 |

On the selected-row tint (`#262234`, accent at 14% over surface): foreground 13.21, muted 5.54,
subtle 4.51, accent 5.67.

| Text on a fill | Ratio |
|---|---:|
| on accent `#140c2b` on primary `#a78bfa` / hover `#b9a2fb` / active `#9071f0` | 6.91 / 8.61 / 5.20 |
| on status `#0a0a0c` on success / warning / danger / info | 11.35 / 12.92 / 7.15 / 7.78 |

| Graphical object on surface `#111114` | Ratio |
|---|---:|
| `chart.1`–`chart.5` | 6.93, 7.41, 10.82, 12.31, 7.12 |
| focus ring (accent) | 6.93 |

### Light

| Text | background `#f6f6f8` | surface `#ffffff` | elevated `#f0f0f3` | sidebar `#ececf0` | hover `#e8e8ed` |
|---|---:|---:|---:|---:|---:|
| foreground `#18181b` | 16.41 | 17.72 | 15.58 | 15.04 | 14.51 |
| muted `#52525b` | 7.16 | 7.73 | 6.80 | 6.56 | 6.33 |
| subtle `#61616b` | 5.67 | 6.12 | 5.38 | 5.20 | 5.01 |
| accent `#6d4ae0` | 5.25 | 5.67 | 4.98 | 4.81 | 4.64 |
| success `#166534` | 6.61 | 7.13 | 6.27 | 6.05 | 5.84 |
| warning `#854d0e` | 6.35 | 6.85 | 6.02 | 5.81 | 5.61 |
| danger `#b91c1c` | 5.99 | 6.47 | 5.69 | 5.49 | 5.30 |
| info `#1d4ed8` | 6.21 | 6.70 | 5.89 | 5.69 | 5.49 |
| world General `#52525b` | 7.16 | 7.73 | 6.80 | 6.56 | 6.33 |
| world W2 `#1d4ed8` | 6.21 | 6.70 | 5.89 | 5.69 | 5.49 |
| world W3 `#166534` | 6.61 | 7.13 | 6.27 | 6.05 | 5.84 |
| world W4 `#7e22ce` | 6.47 | 6.98 | 6.14 | 5.93 | 5.72 |
| world W5 `#4f46e5` | 5.83 | 6.29 | 5.53 | 5.34 | 5.15 |
| world W6 `#be123c` | 5.82 | 6.29 | 5.53 | 5.33 | 5.15 |
| world W7 `#92400e` | 6.57 | 7.09 | 6.23 | 6.02 | 5.81 |
| world Tools `#115e59` | 7.03 | 7.58 | 6.67 | 6.44 | 6.21 |

On the selected-row tint (`#eae5fb`): foreground 14.43, muted 6.29, subtle 4.98, accent 4.61.

| Text on a fill | Ratio |
|---|---:|
| on accent `#ffffff` on primary `#6d4ae0` / hover `#7c5cf0` / active `#5b3bc9` | 5.67 / 4.52 / 7.21 |
| on status `#ffffff` on success / warning / danger / info | 7.13 / 6.85 / 6.47 / 6.70 |

| Graphical object on surface `#ffffff` | Ratio |
|---|---:|
| `chart.1`–`chart.5` | 4.52, 5.17, 3.30, 5.02, 4.60 |
| focus ring (accent) | 5.67 |

Light mode uses darker status and world colors than dark mode on purpose: the dark-mode greens and
ambers fall under 4.5:1 on white.

## The theme file

[`theme.json`](../../crates/app/data/theme.json) is a kit `ThemeSet` named `Idleon Console` with two themes,
`Idleon Console Dark` and `Idleon Console Light`. Every key is a `ThemeConfig` /
`ThemeConfigColors` key from `crates/component/src/theme/schema.rs` in gpui-kit 0.10.0, and the
file validates against the `.theme-schema.json` published with the `v0.10.0` tag. Its `font.size`
is 16, not the 13 of the type scale below, while Ely remains (see [the README](README.md#theme)).

Left to the kit's fallbacks on purpose:

- `button.*` and `status_bar.*`: in `schema.rs` but missing from the tag's published JSON schema,
  so setting them would fail that validation. Default buttons fall back to a mix of `input.border`,
  which is close to `elevated`.
- `is_default`: also absent from the published schema. The app selects the theme by name.
- `accordion.hover.background` and `base.*.light`: the kit derives them from the colors above.
- `highlight` (code syntax colors): the app shows no code.

### Loading the theme

`ui::theme::install` registers JetBrains Mono with `add_fonts`, embeds
`crates/app/data/theme.json` with `include_str!`, adds it with
`ThemeRegistry::global_mut(cx).load_themes_from_str(..)`, sets the kit theme's `dark_theme` and
`light_theme` to its two configs and shows the system's current mode. The app follows the system:
`follow_system` observes the window's appearance and switches the kit (`Theme::change`) and Ely
together. There is no in-app mode setting.
