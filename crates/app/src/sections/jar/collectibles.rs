//! The Collectibles tab: every collectible, its saved bonus and the order an
//! enchant should target them in. The groups and the priority are read once
//! per applied save ([`super::Controls::sync`]).

use super::table::{Col, table, text};
use crate::sections::jar::{metric, row};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, ParentElement, Role, Styled, div, px,
};
use gpui_kit::component::ActiveTheme as _;
use idleon_jar::{Collectible, EnchantPriority, Jar};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, TEXT_CAPTION, Tone};

/// Tiles to a row: a row never wraps (`docs/ui-kit/display.md`).
const TILES_PER_ROW: usize = 6;

pub(crate) fn collectibles(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let figures = state.jar_controls.figures();
    div()
        .id("jar-collectibles")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar))
        .child(grid(jar, &figures.groups, cx))
        .child(priority(&figures.priority, cx))
        .into_any_element()
}

/// The four figures the collectibles tab leads with.
fn summary(jar: &Jar) -> AnyElement {
    let found = jar.collectibles_found();
    let slots = jar.collectible_slots();
    let mut row = row("jar-collectibles-summary")
        .child(metric(
            "jar-collectibles-found",
            "Found",
            format!("{found} / {slots}"),
            match slots == 40 {
                true => "Page two is unlocked; every collectible can drop".to_owned(),
                false => "Advanced Collection unlocks the other twenty-four".to_owned(),
            },
            Tone::Accent,
        ))
        .child(metric(
            "jar-collectibles-enchants",
            "Total enchant levels",
            format::number(f64::from(jar.enchant_levels())),
            "The sum of level − 1 across every enchanted collectible".to_owned(),
            Tone::Info,
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
        ));
    if jar.ext.legend_pts > 0.0 {
        row = row.child(metric(
            "jar-collectibles-legend",
            "What's in your Jar?",
            format!("+{}%", format::number(jar.ext.legend_pts)),
            "Bonus to every collectible from legend talent 29".to_owned(),
            Tone::Success,
        ));
    }
    row.into_any_element()
}

/// The tiles grouped by effect category.
fn grid(jar: &Jar, groups: &[idleon_jar::CollectibleGroup], cx: &App) -> AnyElement {
    let found = jar.collectibles_found();
    let body = div()
        .id("jar-collectible-grid")
        .flex()
        .flex_col()
        .gap_4()
        .children(groups.iter().map(|group| {
            let color = format::hex(group.color);
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
                                .text_size(TEXT_BODY)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child(group.label),
                        )
                        .child(Caption::new(format!("{} collectibles", group.items.len()))),
                )
                .children(group.items.chunks(TILES_PER_ROW).map(|chunk| {
                    div()
                        .flex()
                        .gap_2()
                        .children(chunk.iter().map(|item| tile(item, color, cx)))
                }))
        }));
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
fn tile(item: &Collectible, color: Hsla, cx: &App) -> AnyElement {
    let theme = cx.theme();
    // The game's enchant tier borders: gold, silver and bronze.
    let border = match item.enchant_levels {
        20.. => format::hex("#ffd700"),
        5..=19 => format::hex("#aaaaaa"),
        1..=4 => format::hex("#cd7f32"),
        _ => theme.border,
    };
    let level = match item.level {
        0 => "—".to_owned(),
        level if item.enchant_levels >= 20 => format!("+{level}"),
        level => level.to_string(),
    };
    let effect = match item.level {
        0 => "Not found".to_owned(),
        _ => format!("{} · per level {}", item.effect, item.per_level_effect),
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
        .rounded(theme.radius)
        .border_1()
        .border_color(border)
        .when(item.level == 0, |tile| tile.opacity(0.55))
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(item.name),
        )
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .child(level),
        )
        .child(Caption::new(effect))
        .into_any_element()
}

/// Found collectibles sorted by the next level's marginal value.
fn priority(rows: &[EnchantPriority], cx: &App) -> AnyElement {
    if rows.is_empty() {
        return div()
            .id("jar-collectible-priority")
            .child(
                Card::new()
                    .header(CardHeader::new("Enchant priority"))
                    .child(Caption::new(
                        "No collectible has been found yet, so there is nothing to enchant.",
                    )),
            )
            .into_any_element();
    }
    let cols = [
        Col::Number("#"),
        Col::Text("Collectible"),
        Col::Text("Category"),
        Col::Number("Level"),
        Col::Number("Enchant"),
        Col::Number("Current bonus"),
        Col::Number("+1 enchant"),
    ];
    let rows = rows.iter().map(|row| {
        vec![
            text(row.index.to_string()),
            text(row.name),
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
            text(row.level.to_string()),
            text(row.enchant_levels.to_string()),
            text(format!("+{:.1}", row.bonus)),
            text(format!("+{:.1}", row.marginal)),
        ]
    });
    div()
        .id("jar-collectible-priority")
        .child(
            Card::new()
                .header(CardHeader::new("Enchant priority").description(
                    "Highest marginal value first; ties go to the collectible with the fewest \
                     enchant levels",
                ))
                .child(table("jar-priority", &cols, rows, cx)),
        )
        .into_any_element()
}
