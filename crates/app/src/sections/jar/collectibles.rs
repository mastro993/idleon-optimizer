//! The Collectibles tab: every collectible, its saved bonus and the order an
//! enchant should target them in.

use crate::sections::jar::metric;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_core::Jar;
use idleon_core::jar::Collectible;

pub fn collectibles(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    div()
        .id("jar-collectibles")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar, cx))
        .child(grid(jar, cx))
        .child(priority(jar, cx))
        .into_any_element()
}

/// The four figures the upstream collectibles tab leads with.
fn summary(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let found = jar.collectibles_found();
    let slots = jar.collectible_slots();
    let mut row = div()
        .id("jar-collectibles-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-collectibles-found",
            "Found",
            format!("{found} / {slots}"),
            match slots == 40 {
                true => "Page two is unlocked; every collectible can drop".to_owned(),
                false => "Advanced Collection unlocks the other twenty-four".to_owned(),
            },
            Tone::Accent,
            cx,
        ))
        .child(metric(
            "jar-collectibles-enchants",
            "Total enchant levels",
            format::number(f64::from(jar.enchant_levels())),
            "The sum of level − 1 across every enchanted collectible".to_owned(),
            Tone::Info,
            cx,
        ))
        .child(metric(
            "jar-collectibles-page-two",
            "Page two",
            match jar.page_two_unlocked() {
                true => "Unlocked".to_owned(),
                false => "Locked".to_owned(),
            },
            "Advanced Collection schematic".to_owned(),
            match jar.page_two_unlocked() {
                true => Tone::Success,
                false => Tone::Warning,
            },
            cx,
        ));
    if jar.ext.legend_pts > 0.0 {
        row = row.child(metric(
            "jar-collectibles-legend",
            "What's in your Jar?",
            format!("+{}%", format::number(jar.ext.legend_pts)),
            "Bonus to every collectible from legend talent 29".to_owned(),
            Tone::Success,
            cx,
        ));
    }
    row.into_any_element()
}

/// `_renderCollectibles`: the tiles grouped by effect category.
fn grid(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let groups = jar.collectible_groups();
    let found = jar.collectibles_found();
    let mut body = div().id("jar-collectible-grid").flex().flex_col().gap_4();
    for group in groups {
        let colors = cx.theme().colors.clone();
        let color = format::hex(group.color);
        let count = group.items.len();
        let mut tiles = div().flex().flex_wrap().gap_2();
        for item in group.items {
            tiles = tiles.child(tile(item, color, &colors));
        }
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(8.0)).rounded_full().bg(color))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child(group.label),
                        )
                        .child(
                            Caption::new(format!("{count} collectibles"))
                                .text_color(colors.fg_muted),
                        ),
                )
                .child(tiles),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Collectibles").description(format!(
                "{found} found. A faded tile is one the save has not found yet; the border marks its enchant tier.",
            )),
        )
        .child(body)
        .into_any_element()
}

/// One collectible tile: its level, its current effect and its enchant tier.
fn tile(
    item: Collectible,
    color: gpui::Hsla,
    colors: &ely_gpui_component::theme::Palette,
) -> AnyElement {
    let unmapped = colors.fg_muted;
    let border = match item.enchant_levels {
        20.. => format::hex("#ffd700"),
        5..=19 => format::hex("#aaaaaa"),
        1..=4 => format::hex("#cd7f32"),
        _ => colors.border,
    };
    let level = match item.level {
        0 => "—".to_owned(),
        level if item.enchant_levels >= 20 => format!("+{level}"),
        level => level.to_string(),
    };
    let effect = match item.level {
        0 => "Not found".to_owned(),
        _ => item.effect.clone(),
    };
    div()
        .id(("jar-collectible", item.index))
        .role(Role::Label)
        .aria_label(format!(
            "{}: level {}, {}, enchant levels {}",
            item.name, item.level, item.effect, item.enchant_levels
        ))
        .flex()
        .flex_col()
        .gap_1()
        .w(px(150.0))
        .flex_none()
        .p_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .when(item.level == 0, |tile| tile.opacity(0.55))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(item.name),
        )
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .child(level),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(unmapped)
                .child(match item.level {
                    0 => effect,
                    _ => format!("{effect} · per level {}", item.per_level_effect),
                }),
        )
        .into_any_element()
}

/// `enchantPriority`: found collectibles sorted by the next level's marginal
/// value.
fn priority(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let rows = jar.enchant_priority();
    if rows.is_empty() {
        return div()
            .id("jar-collectible-priority")
            .child(
                Card::new()
                    .header(CardHeader::new("Enchant priority"))
                    .child(
                        Caption::new(
                            "No collectible has been found yet, so there is nothing to enchant.",
                        )
                        .text_color(cx.theme().colors.fg_muted),
                    ),
            )
            .into_any_element();
    }
    let mut table = Table::new([
        "#",
        "Collectible",
        "Category",
        "Level",
        "Enchant",
        "Current bonus",
        "+1 enchant",
    ]);
    for row in rows {
        table = table.row([
            div().child(row.index.to_string()).into_any_element(),
            div().child(row.name).into_any_element(),
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(format::hex(row.category_color)),
                )
                .child(row.category_label)
                .into_any_element(),
            div().child(row.level.to_string()).into_any_element(),
            div()
                .child(row.enchant_levels.to_string())
                .into_any_element(),
            div().child(format!("+{:.1}", row.bonus)).into_any_element(),
            div()
                .child(format!("+{:.1}", row.marginal))
                .into_any_element(),
        ]);
    }
    div()
        .id("jar-collectible-priority")
        .child(
            Card::new()
                .header(CardHeader::new("Enchant priority").description(
                    "Highest marginal value first; ties go to the collectible with the \
                         fewest enchant levels",
                ))
                .child(table),
        )
        .into_any_element()
}
