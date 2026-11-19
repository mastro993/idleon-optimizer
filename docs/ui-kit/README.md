# UI kit

The app is moving from Ely (`ely-gpui-component`) to [GPUI Kit](https://gpui-kit.com)
(`gpui-kit`, [API docs](https://docs.rs/gpui-kit/0.10.0)). Tracking issue:
[#335](https://github.com/mastro993/idleon-optimizer/issues/335).

## Coexistence

Until #361 removes Ely, both libraries are dependencies on one GPUI (`gpui-pre =0.3.12`, patched
by `vendor/gpui-mcp`; `cargo tree -i gpui-pre` lists one). `gpui-kit` is pinned exactly in the
root `Cargo.toml`: kit releases are weekly and each can move the GPUI snapshot.

`crates/app/src/main.rs` sets up both:

- `ely_gpui_component::init` and `gpui_kit::init`, then `ui::theme::install`, which applies Ely's
  palette and the kit theme;
- one asset source, `assets::source()`: Ely's fonts, then the kit's icons and the extra Lucide
  icons the app draws (`idleon_ui::widgets::Icons`);
- the window through `gpui_kit::open_window`, which wraps `AppState` in the kit's `Root`. Kit
  dialogs, sheets, notifications, popovers and tooltips need it. The gpui-mcp bridge still
  installs from `AppState::new`.

A screen not migrated yet keeps drawing with Ely. A migrated file imports nothing from
`ely_gpui_component`.

## Theme

The kit theme lives in [`crates/app/data/theme.json`](../../crates/app/data/theme.json), the Console design's theme set
([`design.md`](design.md)) in a dark and a light mode, embedded with `include_str!`. The app
follows the system's appearance: `ui::theme::install` applies the current mode at startup and
`follow_system` switches both libraries when the appearance changes. Ely's palette for each mode
is derived from the same kit tokens (`palette()` in `crates/app/src/ui/theme.rs`), so both
libraries draw one palette; `ely_draws_the_kit_tokens_in_both_modes` fails when the mapping drifts,
`every_text_color_reads_on_every_surface` checks the AA contrast of every text color on every
surface in both modes, and
`kit_components_paint_the_console_colors_and_both_libraries_switch_mode` renders a kit `Button`
and `Label`, checks the painted colors and switches mode.

The theme names JetBrains Mono as `mono_font.family`. The app bundles it in `crates/app/fonts/`
(Regular, Medium and SemiBold, with its SIL Open Font License next to it) and registers it with
`add_fonts` before the theme is applied.

The kit's `Root` sets the window's rem to the theme's `font.size` on every frame, and Ely sizes
everything in rems of 16 px. So `font.size` stays 16 while any Ely screen remains; a smaller value
shrinks the whole app (the same test checks the rem). The Console theme's 13 px body text comes
from the `idleon_ui` type scale in pixels until Ely is gone; #361 can then set `font.size` to 13. The `Root` also sets the window's font family from
`font.family`, so the theme leaves it unset: the kit then uses the system UI font, which the app's
plain text already draws in.

Sections read colors, radii and fonts from `cx.theme()` (the kit's `ActiveTheme`) or the
`idleon_ui` tokens, never a literal.

## Pages

| Page | What it covers |
|---|---|
| [`design.md`](design.md) | The Console visual direction: tokens, type, density, patterns |
| [`forms.md`](forms.md) | Buttons and form controls: the Ely → kit mapping and the state-ownership pattern |
| [`performance.md`](performance.md) | Frame-time harness, the Ely baseline and the budget |
| [`display.md`](display.md) | Layout, typography, display and feedback: each Ely item's kit or `idleon_ui` replacement |
| [`data.md`](data.md) | Tables (`Table` or virtualized `DataTable`), scrolling and charts |
