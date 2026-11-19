# UI kit

The app draws with [GPUI Kit](https://gpui-kit.com) (`gpui-kit`,
[API docs](https://docs.rs/gpui-kit/0.10.0)). It replaced Ely (`ely-gpui-component`) screen by
screen; tracking issue [#335](https://github.com/mastro993/idleon-optimizer/issues/335), and
[ADR 0006](../adr/0006-the-app-draws-with-gpui-kit-alone.md) for why and how.

## Setup

`gpui-kit` is pinned exactly in the root `Cargo.toml`, on the GPUI the bridge vendors
(`gpui-pre =0.3.12`, patched by `vendor/gpui-mcp`; `cargo tree -i gpui-pre` lists one): kit
releases are weekly and each can move the GPUI snapshot.

`crates/app/src/main.rs` sets it up:

- `gpui_kit::init`, then `ui::theme::install`, which applies the kit theme;
- one asset source, `idleon_ui::widgets::Icons`: the kit's icons and the extra Lucide icons the
  app draws;
- the window through `gpui_kit::open_window`, which wraps `AppState` in the kit's `Root`. Kit
  dialogs, sheets, notifications, popovers and tooltips need it. The gpui-mcp bridge still
  installs from `AppState::new`.

Tests that build a window state call `gpui_kit::init` under GPUI's test platform.

## Theme

The kit theme lives in [`crates/app/data/theme.json`](../../crates/app/data/theme.json), the Console design's theme set
([`design.md`](design.md)) in a dark and a light mode, embedded with `include_str!`. The app
follows the system's appearance: `ui::theme::install` applies the current mode at startup and
`follow_system` switches the kit theme when the appearance changes.
`every_text_color_reads_on_every_surface` checks the AA contrast of every text color on every
surface in both modes, and `kit_components_paint_the_console_colors_and_switch_mode` renders a kit
`Button` and `Label`, checks the painted colors and switches mode.

The theme names JetBrains Mono as `mono_font.family`. The app bundles it in `crates/app/fonts/`
(Regular, Medium and SemiBold, with its SIL Open Font License next to it) and registers it with
`add_fonts` before the theme is applied.

The kit's `Root` sets the window's rem to the theme's `font.size` on every frame. The kit's
component heights and the app's spacing are in rems, and the Console sizes were set at 16 px, so
`font.size` stays 16; a smaller value shrinks the whole app (the same test checks the rem). The
Console theme's 13 px body text comes from the `idleon_ui` type scale in pixels. The `Root` also sets the window's font family from
`font.family`, so the theme leaves it unset: the kit then uses the system UI font, which the app's
plain text already draws in.

Sections read colors, radii and fonts from `cx.theme()` (the kit's `ActiveTheme`) or the
`idleon_ui` tokens, never a literal.

## Pages

| Page | What it covers |
|---|---|
| [`design.md`](design.md) | The Console visual direction: tokens, type, density, patterns |
| [`forms.md`](forms.md) | Buttons and form controls: the Ely → kit mapping the migration used and the state-ownership pattern |
| [`performance.md`](performance.md) | Frame-time harness and the Ely baseline |
| [`display.md`](display.md) | Layout, typography, display and feedback: the kit or `idleon_ui` component for each need, and the Ely item it replaced |
| [`data.md`](data.md) | Tables (`Table` or virtualized `DataTable`), scrolling and charts |
