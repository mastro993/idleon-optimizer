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
- one asset source, `assets::source()`: Ely's fonts, then the kit's icons;
- the window through `gpui_kit::open_window`, which wraps `AppState` in the kit's `Root`. Kit
  dialogs, sheets, notifications, popovers and tooltips need it. The gpui-mcp bridge still
  installs from `AppState::new`.

A screen not migrated yet keeps drawing with Ely. A migrated file imports nothing from
`ely_gpui_component`.

## Theme

The kit theme lives in [`crates/app/data/theme.json`](../../crates/app/data/theme.json), a kit
theme set embedded with `include_str!` and applied in dark mode at startup. Until #360 it carries
the same colors as Ely's palette (`palette()` in `crates/app/src/ui/theme.rs`), so both libraries
draw one palette; `the_kit_theme_draws_ely_s_palette` fails when the two drift, and
`kit_components_paint_the_app_colors` renders a kit `Button` and `Label` and checks the painted
colors. #360 swaps in the Console design from [`theme.json`](theme.json) next to this page.

The kit's `Root` sets the window's rem to the theme's `font.size` on every frame, and Ely sizes
everything in rems of 16 px. So `font.size` stays 16 while any Ely screen remains; a smaller value
shrinks the whole app (the same test checks the rem). The Console theme's 13 px body text has to
come another way until Ely is gone. The `Root` also sets the window's font family from
`font.family`, so the theme leaves it unset: the kit then uses the system UI font, which the app's
plain text already draws in.

Sections read colors, radii and fonts from `cx.theme()` (the kit's `ActiveTheme`) or the
`idleon_ui` tokens, never a literal.

## Pages

| Page | What it covers |
|---|---|
| [`design.md`](design.md) | The Console visual direction: tokens, type, density, patterns |
| [`theme.json`](theme.json) | The Console theme set, dark and light |
| [`performance.md`](performance.md) | Frame-time harness, the Ely baseline and the budget |
