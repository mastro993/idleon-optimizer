# The app draws with GPUI Kit alone

Supersedes the Ely parts of [ADR 0003](0003-sidebar-items-are-built-in-the-app-from-ely-parts.md)
and of the stack in `docs/plan.md` §6 and §8.

The app drew with Ely (`ely-gpui-component`, pinned to a git revision). It now draws with GPUI Kit
(`gpui-kit` 0.10.0, Apache-2.0) only, and Ely is no longer a dependency (#361).

Why GPUI Kit (#335):

- It is published on crates.io and pinned exactly (`=0.10.0`), so the GPUI snapshot moves only when
  the pin does. Ely came from a git revision, the one git source `deny.toml` had to allow.
- One theme file drives every component: `crates/app/data/theme.json`, a kit `ThemeSet` with a dark
  and a light Console theme that follow the system (`docs/ui-kit/design.md`).
- The window's `Root` hosts dialogs, sheets, notifications, popovers and tooltips, and the kit has a
  virtualized `DataTable` for the long tables (`docs/ui-kit/data.md`).
- It had the components the #338 design needed, so the Console design is applied once, through the
  theme and the `idleon_ui` composites, instead of in every section.

How the switch went: both libraries lived in one graph on one GPUI (`gpui-pre =0.3.12`, patched by
`vendor/gpui-mcp`) while the screens moved one issue at a time (#336–#360). During that time Ely's
`Palette` was derived from the kit theme's tokens, so both libraries drew one palette, and one asset
source served Ely's fonts before the kit's icons. Every moved file stopped importing Ely and kept
its element ids, so the `script/mcp-*.mjs` harnesses ran unchanged across the migration. Once no
screen used Ely, #361 removed the dependency, its init, the palette derivation, the combined asset
source, the Ely `TextInput` case of `state::redraw_on_edit` and Ely's test init (tests now call
`gpui_kit::init`).

Left out on purpose:

- **`font.size` stays 16.** The kit's `Root` sets the window rem from it, and the kit's component
  heights and the app's spacing are in rems, set at 16 px. The 13 px body text comes from the
  `idleon_ui` type scale in pixels. A smaller rem would rescale every screen; that is a design
  change for the visual QA pass (#362), not part of removing a dependency.
- **The sidebar items and the running mark stay the app's own** ([ADR 0005](0005-sidebar-items-stay-in-the-app-on-the-kit-theme.md)).
- **The mapping pages stay.** `docs/ui-kit/forms.md`, `display.md` and `data.md` keep their
  Ely → kit tables as the record of what each kit component replaced, and as the guide to which
  component a new screen takes.

Considered: a single rewrite of every screen onto the kit. It would have serialized the work behind
one change to about 190 files and left no working app to compare against, where the side-by-side path
kept every screen shippable and checked by the harnesses at each step.
