//! The Next Tier tab: the power-of-ten milestone each jar type should chase and
//! the off-map accumulation that decides the highest tier it can reach. The
//! ranking and the projections are read once per applied save
//! ([`super::Controls::sync`]).

use super::table::{Col, table, text};
use crate::sections::jar::{metric, row, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Role, Styled, div};
use gpui_kit::component::ActiveTheme as _;
use idleon_jar::Jar;
use idleon_jar::{MilestoneRow, TierProjection};
use idleon_ui::format;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, Tone};

pub(crate) fn next_tier(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let figures = state.jar_controls.figures();
    if !jar.production_line_unlocked() {
        return locked(jar, &figures.projections, cx);
    }
    div()
        .id("jar-next-tier")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar))
        .child(milestones(&figures.milestones, cx))
        .child(recommendation(&figures.milestones))
        .child(projections(jar, &figures.projections, cx))
        .into_any_element()
}

/// The tab needs Big Jar Mach's production line before its ranking
/// means anything.
fn locked(jar: &Jar, rows: &[TierProjection], cx: &App) -> AnyElement {
    div()
        .id("jar-next-tier")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Callout::new(Tone::Warning, "Jar Production Line is not owned").child(Caption::new(
                "This tab ranks jar types by how fast they reach the next power of ten in their \
                 produced count, which reduces the next type's production requirement. Buy Big \
                 Jar Mach's production line to use it.",
            )),
        )
        .child(projections(jar, rows, cx))
        .into_any_element()
}

/// The shared production rate and the per-tick progress the off-map loop adds.
fn headline(jar: &Jar) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            row("jar-next-summary")
                .child(metric(
                    "jar-next-production",
                    "Production / hr",
                    format::rate(jar.production_per_hour()),
                    "Progress every slot shares".to_owned(),
                    Tone::Info,
                ))
                .child(metric(
                    "jar-next-tick",
                    "Per 5s tick",
                    format::number(jar.per_tick()),
                    "Progress one off-map tick adds".to_owned(),
                    Tone::Accent,
                )),
        )
        .child(Caption::new(
            "Off-map the game ticks every five seconds and produces one jar per slot per tick.",
        ))
        .into_any_element()
}

/// The pow10 ranking, fastest first.
fn milestones(rows: &[MilestoneRow], cx: &App) -> AnyElement {
    let cols = [
        Col::Number("#"),
        Col::Text("Jar type"),
        Col::Number("Current count"),
        Col::Number("Next milestone"),
        Col::Number("Deficit"),
        Col::Number("Time (3 slots)"),
        Col::Text("Benefits"),
        Col::Text("Line now → next"),
    ];
    let cells = rows.iter().enumerate().map(|(rank, row)| {
        vec![
            text((rank + 1).to_string()),
            milestone_name(row, rank == 0),
            text(format::number(row.count)),
            text(format::number(row.next)),
            text(format::number(row.deficit)),
            text(format::projection(row.hours)),
            text(row.benefits.unwrap_or("—")),
            text(format!("{:.1}% → {:.1}%", row.line_now, row.line_next)),
        ]
    });
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
                .child(table("jar-milestones", &cols, cells, cx)),
        )
        .into_any_element()
}

fn milestone_name(row: &MilestoneRow, fastest: bool) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(row.name)
        .when(fastest, |cell| cell.child(tag("Fastest", Tone::Success)))
        .into_any_element()
}

/// The recommendation line: the fastest milestone, its deficit and the
/// type it speeds up.
fn recommendation(rows: &[MilestoneRow]) -> AnyElement {
    let Some(best) = rows.first() else {
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
    div()
        .id("jar-next-recommendation")
        .role(Role::Label)
        .aria_label(text.clone())
        .child(Callout::new(Tone::Success, "Recommendation").child(Caption::new(text)))
        .into_any_element()
}

/// The off-map accumulation: sustained tier, accumulation peak
/// and what the next tier still needs.
fn projections(jar: &Jar, rows: &[TierProjection], cx: &App) -> AnyElement {
    let per_hour = jar.production_per_hour();
    let max_tiers: Vec<String> = rows
        .iter()
        .map(|row| format!("{} T{}", row.name, row.max_tier + 1))
        .collect();
    let mono = cx.theme().mono_font_family.clone();
    let cols = [
        Col::Text("Jar type"),
        Col::Number("Requirement"),
        Col::Text("Sustained tier"),
        Col::Text("Max tier"),
        Col::Number("Time / max jar"),
        Col::Number("Next tier net / tick"),
        Col::Text("Next via production line"),
        Col::Text("Next via production"),
    ];
    let cells = rows.iter().map(|row| {
        let (label, tone) = match row.max_tier > 0 {
            true => ("accumulation", Tone::Success),
            false => ("base", Tone::Neutral),
        };
        vec![
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(row.name)
                .into_any_element(),
            text(format::number(row.requirement)),
            text(match row.sustained {
                Some(tier) => format!(
                    "T{} ({}×)",
                    tier + 1,
                    format::number(10_f64.powi(tier as i32))
                ),
                None => "T1 only".to_owned(),
            }),
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .font_family(mono.clone())
                        .font_weight(FontWeight::BOLD)
                        .child(format!("T{}", row.max_tier + 1)),
                )
                .child(tag(label, tone))
                .into_any_element(),
            text(tick_time(row.seconds_between)),
            text(format!(
                "{} (T{})",
                format::number(row.net_next),
                row.next_tier + 1
            )),
            text(line_path(row)),
            text(production_path(row, per_hour)),
        ]
    });
    div()
        .id("jar-next-projections")
        .role(Role::Label)
        .aria_label(format!("Max tier: {}", max_tiers.join(", ")))
        .child(
            Card::new()
                .header(CardHeader::new("Off-map tier accumulation").description(
                    "Max tier is the highest jar the 500-tick loop reaches: one jar per slot per \
                     tick, and surplus progress banks until it clears the next requirement",
                ))
                .child(table("jar-projections", &cols, cells, cx))
                .child(Caption::new(
                    "Sustained tier is the tier the per-tick progress covers every tick; max tier \
                     is the accumulation peak above it.",
                )),
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
