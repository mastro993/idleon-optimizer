//! The Next Tier tab: the power-of-ten milestone each jar type should chase and
//! the off-map accumulation that decides the highest tier it can reach.

use crate::sections::jar::metric;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_core::Jar;
use idleon_core::jar::{MilestoneRow, TierProjection};

pub fn next_tier(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    if !jar.production_line_unlocked() {
        return locked(jar, cx);
    }
    div()
        .id("jar-next-tier")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar, cx))
        .child(milestones(jar))
        .child(recommendation(jar, cx))
        .child(projections(jar, cx))
        .into_any_element()
}

/// The upstream tab needs Big Jar Mach's production line before its ranking
/// means anything.
fn locked(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jar-next-tier")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Callout::new(Severity::Warning)
                .title("Jar Production Line is not owned")
                .child(
                    Caption::new(
                        "This tab ranks jar types by how fast they reach the next power of ten in \
                         their produced count, which reduces the next type's production \
                         requirement. Buy Big Jar Mach's production line to use it.",
                    )
                    .text_color(muted),
                ),
        )
        .child(projections(jar, cx))
        .into_any_element()
}

/// The shared production rate and the per-tick progress the off-map loop adds.
fn headline(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jar-next-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-next-production",
            "Production / hr",
            format::rate(jar.production_per_hour()),
            "Progress every slot shares".to_owned(),
            Tone::Info,
            cx,
        ))
        .child(metric(
            "jar-next-tick",
            "Per 5s tick",
            format::number(jar.per_tick()),
            "Progress one off-map tick adds".to_owned(),
            Tone::Accent,
            cx,
        ))
        .child(
            Caption::new(
                "Off-map the game ticks every five seconds and produces one jar per slot per tick.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

/// `_renderNextTier`: the pow10 ranking, fastest first.
fn milestones(jar: &Jar) -> AnyElement {
    let rows = jar.milestone_ranking();
    let mut table = Table::new([
        "#",
        "Jar type",
        "Current count",
        "Next milestone",
        "Deficit",
        "Time (3 slots)",
        "Benefits",
        "Line now → next",
    ]);
    for (rank, row) in rows.iter().enumerate() {
        table = table.row([
            div().child((rank + 1).to_string()).into_any_element(),
            milestone_name(row, rank == 0),
            div().child(format::number(row.count)).into_any_element(),
            div().child(format::number(row.next)).into_any_element(),
            div().child(format::number(row.deficit)).into_any_element(),
            div()
                .child(format::projection(row.hours))
                .into_any_element(),
            div().child(row.benefits.unwrap_or("—")).into_any_element(),
            div()
                .child(format!("{:.1}% → {:.1}%", row.line_now, row.line_next))
                .into_any_element(),
        ]);
    }
    div()
        .id("jar-next-milestones")
        .child(
            Card::new()
                .header(
                    CardHeader::new("Next power-of-ten milestone ranking").description(
                        "Jar Production Line cuts the next type's requirement by 5% per power of \
                         ten of the type below; each milestone is one more +5%",
                    ),
                )
                .child(table),
        )
        .into_any_element()
}

fn milestone_name(row: &MilestoneRow, fastest: bool) -> AnyElement {
    let mut cell = div().flex().items_center().gap_2().child(row.name);
    if fastest {
        cell = cell.child(Badge::new("Fastest").tone(Tone::Success));
    }
    cell.into_any_element()
}

/// The upstream recommendation line: the fastest milestone, its deficit and the
/// type it speeds up.
fn recommendation(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let Some(best) = jar.milestone_ranking().into_iter().next() else {
        return div().into_any_element();
    };
    let mut text = format!(
        "Produce {} to reach {} fastest ({} more needed, {} with all three slots).",
        best.name,
        format::number(best.next),
        format::number(best.deficit),
        format::projection(best.hours)
    );
    if let Some(benefits) = best.benefits {
        text.push_str(&format!(" This speeds up {benefits} production by +5%."));
    }
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jar-next-recommendation")
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(px(6.0))
        .border_l_4()
        .border_color(format::hex("#4caf50"))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Recommendation"),
        )
        .child(Caption::new(text).text_color(muted))
        .into_any_element()
}

/// `_renderNextTier`'s off-map accumulation: sustained tier, accumulation peak
/// and what the next tier still needs.
fn projections(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let per_hour = jar.production_per_hour();
    let rows = jar.tier_projections();
    let max_tiers: Vec<String> = rows
        .iter()
        .map(|row| format!("{} T{}", row.name, row.max_tier + 1))
        .collect();
    let mut table = Table::new([
        "Jar type",
        "Requirement",
        "Sustained tier",
        "Max tier",
        "Time / max jar",
        "Next tier net / tick",
        "Next via production line",
        "Next via production",
    ]);
    for row in &rows {
        table = table.row([
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(row.name)
                .into_any_element(),
            div()
                .child(format::number(row.requirement))
                .into_any_element(),
            div()
                .child(match row.sustained {
                    Some(tier) => format!(
                        "T{} ({}×)",
                        tier + 1,
                        format::number(10_f64.powi(tier as i32))
                    ),
                    None => "T1 only".to_owned(),
                })
                .into_any_element(),
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format!("T{}", row.max_tier + 1)),
                )
                .child(
                    Badge::new(match row.max_tier > 0 {
                        true => "accumulation",
                        false => "base",
                    })
                    .tone(match row.max_tier > 0 {
                        true => Tone::Success,
                        false => Tone::Neutral,
                    }),
                )
                .into_any_element(),
            div()
                .child(tick_time(row.seconds_between))
                .into_any_element(),
            div()
                .child(format!(
                    "{} (T{})",
                    format::number(row.net_next),
                    row.next_tier + 1
                ))
                .into_any_element(),
            div().child(line_path(row)).into_any_element(),
            div()
                .child(production_path(row, per_hour))
                .into_any_element(),
        ]);
    }
    div()
        .id("jar-next-projections")
        .role(Role::Label)
        .aria_label(format!("Max tier: {}", max_tiers.join(", ")))
        .child(
            Card::new()
                .header(CardHeader::new("Off-map tier accumulation").description(
                    "Max tier is the highest jar the 500-tick loop reaches: one jar per slot \
                         per tick, and surplus progress banks until it clears the next requirement",
                ))
                .child(table)
                .child(
                    Caption::new(
                        "Sustained tier is the tier the per-tick progress covers every tick; max \
                         tier is the accumulation peak above it.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// A max-tier jar's interval, as seconds or the shared compact duration.
fn tick_time(seconds: Option<f64>) -> String {
    match seconds {
        Some(seconds) if seconds.is_finite() && seconds >= 60.0 => {
            format::projection(seconds / 3_600.0)
        }
        Some(seconds) if seconds.is_finite() => format!("{}s", seconds.round()),
        _ => "—".to_owned(),
    }
}

/// The previous-type count the production line needs to sustain the next tier.
fn line_path(row: &TierProjection) -> String {
    if !row.needed_line.is_finite() || row.needed_line > 1e20 {
        return "Production line alone cannot reach it".into();
    }
    if row.needed_line <= row.previous_count {
        return format!("Have {}", format::number(row.previous_count));
    }
    format!(
        "Need {} ({} now)",
        format::number(row.needed_line),
        format::number(row.previous_count)
    )
}

/// The production per hour the next tier needs.
fn production_path(row: &TierProjection, per_hour: f64) -> String {
    if row.needed_production <= per_hour {
        return format!("Have {}", format::rate(per_hour));
    }
    format!(
        "Need {} (+{} /hr)",
        format::rate(row.needed_production),
        format::rate(row.needed_production - per_hour)
    )
}
