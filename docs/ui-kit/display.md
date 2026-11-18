# Layout, display and feedback

What replaces each Ely layout, typography, display and feedback item when a screen moves to GPUI
Kit (#340). Take the kit component when one exists; use an `idleon_ui::widgets` composite only
where the kit has none. The composites live in
[`crates/ui/src/widgets/`](../../crates/ui/src/widgets/), each a `RenderOnce` builder styled only
from `cx.theme()`, in the Console sizes from [`design.md`](design.md).

Paths: `kit::` is `gpui_kit::component::`, `assets::` is `gpui_kit::assets::`, `widgets::` is
`idleon_ui::widgets::`.

## Mapping

Uses are the counts across `crates/` when #340 was written (the tab strip row: when #344 was).

| Ely item | Uses | Replacement | Notes |
|---|---:|---|---|
| `typography::Caption` | 157 | `widgets::Caption` | 11 px, subtle. `Styled`, so `.text_color(..)` still overrides it. |
| `layout::Card` | 155 | `widgets::Card` | The kit's `GroupBox` puts its title above the surface; a Console panel keeps it inside. |
| `layout::CardHeader` | 154 | `widgets::CardHeader` | Title, `.description(..)`, any number of `.action(..)`s. |
| `theme::ControlSize` / `TextSize` / `IconSize` / `Radius` | ≈120 | `kit::Size` + `Sizable`; `widgets::TEXT_*`; `cx.theme().radius` / `radius_lg` | See [Sizes](#sizes). |
| `data_display::Tone` | 93 | `widgets::Tone` | `TagVariant::from(tone)`, `AlertVariant::from(tone)`, `tone.color(cx)`. |
| `data_display::Badge` | 87 | `kit::tag::Tag` | `Tag::new().with_variant(tone.into()).outline().small().child(label)`. The kit's `Badge` is a count or dot pinned over another element, not a label. |
| `primitives::IconName` | 70 | `assets::IconName` | The full Lucide catalog. Ely's `Trash2` is `Trash` in Lucide 1.43. An icon outside the kit's defaults draws only once it is in `widgets::Icons` (see [Icons](#icons)). |
| `primitives::Severity` | 50 | `widgets::Tone` | `Info`, `Success`, `Warning`, `Danger` keep their names. |
| `data_display::KpiCard` | 37 | `widgets::KpiStrip` | One panel split by hairlines, one `Statistic` per cell. A lone card is a strip of one. `.caption(..)` moves to the `Statistic`. |
| `data_display::Statistic` | 35 | `widgets::Statistic` | The value arrives formatted (`idleon_ui::format`) and draws in the mono face. `.delta(text, tone)` replaces `TrendIndicator`. Sizes: `.small()`, default, `.large()`. |
| `feedback::Banner` | 30 | `kit::alert::Alert` | `Alert::new(id, message).with_variant(tone.into()).banner()`; `.on_close(..)` for dismiss. Alert has no action slot: put the kit `Button` next to it in an `h_flex`. |
| `feedback::EmptyState` | 23 | `kit::empty::Empty` | `EmptyHeader` with `EmptyMedia` (icon), `EmptyTitle`, `EmptyDescription`; actions in `EmptyContent`. `Empty` takes no id: wrap it in `div().id(..)` to keep the old one. |
| `feedback::Callout` | 20 | `widgets::Callout` | `Callout::new(tone, title).child(..)`. The kit's `Alert` tints the whole box and takes text only; a callout body holds elements. |
| `primitives::Tooltip` | 19 | `kit::tooltip::Tooltip` | `.tooltip(\|window, cx\| Tooltip::new(text).build(window, cx))`; kit buttons take `.tooltip(text)`. |
| `motion::ProgressBar` | 6 | `kit::progress::Progress` | `Progress::new(id).value(fraction * 100.)`: the kit takes percent, Ely a fraction. |
| `typography::format` | 3 | `idleon_ui::format`, or `format!` | `format::percent(r, d, false)` is `format!("{:.d$}%", r * 100.)`. |
| `primitives::Disclosure` | 3 | `kit::Icon` | `Icon::new(IconName::ChevronRight).rotate(radians(if open { FRAC_PI_2 } else { 0. }))`; for a whole expanding section, `kit::collapsible::Collapsible` or `kit::accordion::Accordion`. |
| `primitives::Icon` | 2 | `kit::Icon` | `Icon::new(name).with_size(Size::Small).text_color(..)`. |
| `data_display::Meter` | 2 | `kit::progress::Progress` | A `Caption` row for the label and value over the bar; `.color(tone.color(cx))` for a threshold color. |
| `typography::DateTimeText` | 1 | `jiff` + text | `start.to_zoned(TimeZone::system()).strftime("%b %-d").to_string()` in a `Caption` or `div`. |
| `primitives::FocusRing` | 1 | `kit::ThemeStyled::focus_ring_style` | `.when(focused, \|el\| el.focus_ring_style(window, cx))`. |
| `data_display::Tag` | 1 | `kit::tag::Tag` | As `Badge`; a remove action is a kit ghost `Button` inside the tag. |
| `data_display::DotBadge` | 1 | `kit::badge::Badge` or a dot `div` | Over an element: `Badge::new().dot().color(tone.color(cx)).child(el)`. Alone: `div().size_1p5().rounded_full().bg(tone.color(cx))`. |
| `data_display::Timeline` / `TimelineItem` | 1 | `widgets::Timeline` / `TimelineItem` | Same builders: `.item(..)`, `.time(..)`, `.tone(..)`, `.child(..)`. |
| `shell::TabBar` / `WindowTab` | 22 | `ui::layout::tab_bar` | A kit underline `TabBar`, each tab sized to its label and scrolling sideways on overflow. Takes `(id, label)` pairs, the selected index and an `on_select(index)`; each tab keeps its Ely id (the kit numbers its tabs, so the id sits on the label). Drop the `layout::scrolling_tabs` wrapper: the kit bar scrolls itself. Research's `app-tabs` uses it. |
| `motion::LoadingOverlay` | 1 | `kit::spinner::Spinner` | A centered `Spinner::new()` in an absolute `div` over the content. |
| `layout::SimpleGrid` | 1 | GPUI grid | `div().grid().grid_cols(n).gap(..)`, `n` from the workspace width beside the sidebar (`ui/dashboard.rs`, `columns`). Grid items stretch to their row's height. Home becomes one table in the Console design. |
| `layout::bring_into_view` | 1 | `widgets::bring_into_view` | Same signature. `ScrollHandle::scroll_to_item(ix)` when the item is a direct child of the scroll container. |

## Sizes

Ely sized controls with `ControlSize` and text with `TextSize`; the kit sizes components with
`Size` through `Sizable` (`.xsmall()`, `.small()`, default `Medium`, `.large()`), and text in pixels
from the Console scale. Pixels, not rems, because the kit's `Root` sets the window rem from the
theme's `font.size`.

| Ely | Replacement |
|---|---|
| `ControlSize::Sm` / `Md` / `Lg` | `.small()` / default / `.large()` |
| `TextSize::Xs` (11) | `widgets::TEXT_CAPTION` (11 px) |
| `TextSize::Sm` (12) | `widgets::TEXT_MONO` (12 px) for numbers, `TEXT_CAPTION` for labels |
| `TextSize::Base` (13), `Md` (14) | `widgets::TEXT_BODY` (13 px) |
| `TextSize::Lg` (16), `Xl` (20) | `widgets::TEXT_TITLE` (18 px) for a page title, `TEXT_KPI` (18 px) for a figure |
| `TextSize::Xxl` (24) | `Statistic::large()` |
| `IconSize::Xs` / `Sm` / `Md` / `Lg` | `Icon::with_size(Size::XSmall)` (12) / `Small` (14) / `Medium` (16) / `Large` (24) |
| `Radius::Sm`, `Md` | `cx.theme().radius` (controls, panels) |
| `Radius::Lg`, `Xl` | `cx.theme().radius_lg` (dialogs, popovers) |

Spacing inside and between panels: `widgets::PANEL_PADDING` (12 px) and `widgets::PANEL_GAP` (8 px).

## Icons

`assets::Assets` embeds only the ~100 icons the kit's components draw. `widgets::Icons`, the source
behind `assets::source()` in the app, adds the Lucide icons the app draws beyond those (`SearchX`,
`Upload`, `X`, …). Another `assets::IconName` renders blank until it is added to the list in
[`icons.rs`](../../crates/ui/src/widgets/icons.rs); `the_icon_source_serves_the_kit_and_the_app_icons`
checks a sample.

## Before and after

The five most used items.

### Caption

```rust
// Ely
use ely_gpui_component::typography::Caption;
Caption::new(progress.message).text_color(muted)

// Kit
use idleon_ui::widgets::Caption;
Caption::new(progress.message).text_color(cx.theme().muted_foreground)
```

### Card and CardHeader

```rust
// Ely
use ely_gpui_component::layout::{Card, CardHeader};
Card::new()
    .header(CardHeader::new(title).description(description).action(cancel))
    .child(body)

// Kit
use idleon_ui::widgets::{Card, CardHeader};
Card::new()
    .header(CardHeader::new(title).description(description).action(cancel))
    .child(body)
```

### Sizes

```rust
// Ely
Button::new("run-cancel", "Cancel").size(ControlSize::Sm);
div().text_size(theme.text_size(TextSize::Sm)).rounded(theme.radius(Radius::Md));

// Kit
use gpui_kit::component::Sizable as _;
use idleon_ui::widgets::TEXT_CAPTION;
Button::new("run-cancel").label("Cancel").small();
div().text_size(TEXT_CAPTION).rounded(cx.theme().radius);
```

### Tone and Badge

```rust
// Ely
use ely_gpui_component::data_display::{Badge, Tone};
Badge::new("Stale").tone(Tone::Warning)

// Kit
use gpui_kit::component::{Sizable as _, tag::Tag};
use idleon_ui::widgets::Tone;
Tag::new().with_variant(Tone::Warning.into()).outline().small().child("Stale")
```

### KpiCard and Statistic

```rust
// Ely: one card per figure
div().flex().gap_3()
    .child(KpiCard::new(Statistic::new("insight-high-value", "High value", high as f64)).caption("Gain ≥ 10%"))
    .child(KpiCard::new(Statistic::new("insight-low", "Low or none", low as f64)))

// Kit: one strip, a hairline between figures
use idleon_ui::{format, widgets::{KpiStrip, Statistic}};
KpiStrip::new()
    .child(Statistic::new("insight-high-value", "High value", format::count(high as u64)).caption("Gain ≥ 10%"))
    .child(Statistic::new("insight-low", "Low or none", format::count(low as u64)))
```

`Statistic` takes its value formatted. Ely's `AnimatedNumber` grouped thousands with commas and
rolled the digits when the value changed; `format::count` groups the same way, and the value no
longer animates.
