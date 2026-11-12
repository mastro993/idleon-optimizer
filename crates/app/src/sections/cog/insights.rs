//! The board insights, the roll ranges and the read-only shelf.
//!
//! Ported from `renderInsights`, `renderRollRanges` and `renderShelf` in
//! `cog-optimizer.html`. The insights name the weakest and strongest cog the
//! optimizer cannot place better, and what to roll to replace the weakest; the
//! ranges show what a roll of each tier can produce at the effective
//! construction level; the shelf lists the pool the optimizer draws from and
//! takes no edits.

use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Hsla, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_core::cog::{BoardInsights, CogSave, Goal, Insight, slot_to_pos};

/// The objective's base and surround stat keys, as the controls name them.
fn stat_keys(goal: Goal) -> (&'static str, &'static str) {
    match goal {
        Goal::Build => ("a", "e"),
        Goal::ConExp => ("d", "f"),
        Goal::Flaggy => ("c", "g"),
    }
}

/// The insights card: the two measured extremes and the rolls they ask for.
pub fn insights_card(insights: Option<&BoardInsights>, goal: Goal, muted: Hsla) -> AnyElement {
    let header = CardHeader::new("Board insights").description(
        "Each cog measured by removing it and rescoring, so a surround's gift to its \
         neighbours counts toward its own contribution.",
    );
    let Some(insights) = insights else {
        return Card::new()
            .header(header)
            .child(div().id("cog-insights-empty").p_3().child(Caption::new(
                "The board scores nothing yet. Place a player cog and run Optimize.",
            )))
            .into_any_element();
    };
    let (weak_col, weak_row) = slot_to_pos(insights.weakest.slot);
    let (best_col, best_row) = slot_to_pos(insights.best.slot);
    let summary = format!(
        "Weakest {} at ({weak_col},{weak_row}), strongest {} at ({best_col},{best_row}) · {} roll target(s)",
        insights.weakest.cog.display_name(),
        insights.best.cog.display_name(),
        insights.roll_targets.len()
    );
    let mut targets = div().flex().flex_col().gap_2();
    for (index, target) in insights.roll_targets.iter().enumerate() {
        let position = format!("({},{})", target.col, target.row);
        targets = targets.child(
            div()
                .id(format!("cog-roll-target-{index}"))
                .role(Role::Label)
                .aria_label(format!(
                    "Roll target at {position}: {} — {} → {}",
                    target.cog, target.why, target.roll
                ))
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(Caption::new(position).text_color(muted))
                        .child(div().text_size(px(12.0)).child(target.cog.clone()))
                        .child(
                            Badge::new(format!("{:.2}% of score", target.pct)).tone(Tone::Neutral),
                        ),
                )
                .child(Caption::new(target.why.clone()).text_color(muted))
                .child(
                    div()
                        .text_size(px(12.0))
                        .child(format!("→ {}", target.roll)),
                ),
        );
    }
    let (base, surround) = stat_keys(goal);
    Card::new()
        .header(header.action(Badge::new(format!(
            "{} movable cogs",
            insights.impacts.len()
        ))))
        .child(
            div()
                .id("cog-insights")
                .role(Role::Label)
                .aria_label(summary)
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_4()
                        .child(extreme(
                            "cog-insights-weakest",
                            "Weakest",
                            &insights.weakest,
                            Tone::Danger,
                            insights.weakest.delta <= 0.0,
                        ))
                        .child(extreme(
                            "cog-insights-strongest",
                            "Strongest",
                            &insights.best,
                            Tone::Success,
                            false,
                        )),
                )
                .when(!insights.roll_targets.is_empty(), |body| {
                    body.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Caption::new(format!(
                                "What to roll for · the objective reads {base} and {surround}"
                            )))
                            .child(targets),
                    )
                }),
        )
        .into_any_element()
}

/// One extreme: its name, position and what removing it costs the objective.
fn extreme(
    id: &'static str,
    label: &'static str,
    insight: &Insight,
    tone: Tone,
    hurts: bool,
) -> AnyElement {
    let (col, row) = slot_to_pos(insight.slot);
    let position = format!("({col},{row})");
    let contribution = if hurts {
        format!("hurting score by {}", format::number(insight.delta.abs()))
    } else {
        format!("contributes {:.2}%", insight.pct)
    };
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!(
            "{label}: {} at {position}, {contribution}",
            insight.cog.display_name()
        ))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(Caption::new(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_size(px(13.0)).child(insight.cog.display_name()))
                .child(Badge::new(position).tone(Tone::Neutral)),
        )
        .child(Badge::new(contribution).tone(tone))
        .into_any_element()
}

/// The roll ranges card: what each tier's rolls reach at the effective level.
pub fn roll_ranges_card(goal: Goal, level: u32, saved_level: u32) -> AnyElement {
    let ranges = idleon_core::cog::roll_ranges(goal, level);
    let summary = ranges
        .iter()
        .map(|range| {
            format!(
                "{}: per roll {} to {}",
                tier_label(range.tier),
                whole(range.per_roll_min),
                whole(range.per_roll_max)
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let mut table = Table::new([
        "Tier",
        "Rolls",
        "Per roll",
        "Total max",
        "Next min",
        "Next max",
    ]);
    for range in &ranges {
        table = table.row([
            div().child(tier_label(range.tier)).into_any_element(),
            div().child(format!("{}", range.rolls)).into_any_element(),
            div()
                .child(format!(
                    "{} – {}",
                    whole(range.per_roll_min),
                    whole(range.per_roll_max)
                ))
                .into_any_element(),
            div().child(whole(range.total_max)).into_any_element(),
            div()
                .child(next_label(
                    range.next_min_level,
                    level,
                    whole(range.next_min_value),
                ))
                .into_any_element(),
            div()
                .child(next_label(
                    range.next_max_level,
                    level,
                    whole(range.next_max_value),
                ))
                .into_any_element(),
        ]);
    }
    let surrounds = ranges
        .iter()
        .map(|range| {
            format!(
                "{} {:.0}–{:.0}%",
                tier_label(range.tier),
                range.min_surround,
                range.max_surround
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    Card::new()
        .header(
            CardHeader::new("Roll ranges")
                .description(format!(
                    "At the effective construction level {level}, from rolls of 0.4× to 3.0× the tier's magnitude."
                ))
                .action(Badge::new(if level == saved_level {
                    format!("Level {level}")
                } else {
                    format!("Level {level} · saved {saved_level}")
                })),
        )
        .child(
            div()
                .id("cog-roll-ranges")
                .role(Role::Label)
                .aria_label(format!("Roll ranges at level {level} · {summary}"))
                .flex()
                .flex_col()
                .gap_2()
                .child(div().p_3().child(table))
                .child(
                    div().px_3().pb_3().child(Caption::new(format!(
                        "Surrounds are fixed per tier and do not follow the level: {surrounds}. The next columns name the level at which the minimum or maximum per-roll stat rises."
                    ))),
                ),
        )
        .into_any_element()
}

/// A roll value as a whole number; the game's conversions already round it.
fn whole(value: f64) -> String {
    format::count(value.max(0.0).round() as u64)
}

fn next_label(level: u32, current: u32, value: String) -> String {
    let steps = level.saturating_sub(current);
    format!("Lv {level} (+{steps}) · {value}")
}

fn tier_label(tier: usize) -> String {
    match tier {
        3 => "Ulti (T3)".into(),
        4 => "Jewelled (T4)".into(),
        other => format!("T{other}"),
    }
}

/// The shelf card: every placeable shelf cog, with no way to change it.
pub fn shelf_card(save: &CogSave, goal: Goal, muted: Hsla) -> AnyElement {
    let (base_key, surround_key) = stat_keys(goal);
    let header = CardHeader::new("Shelf").description(format!(
        "The optimizer's pool, read straight from the save. Base is this objective's {base_key} and the surround is its {surround_key}."
    ));
    let total = save.shelf_slots.iter().flatten().count();
    if save.shelf.is_empty() {
        return Card::new()
            .header(header.action(Badge::new("Read-only").tone(Tone::Neutral)))
            .child(
                div()
                    .id("cog-shelf")
                    .role(Role::Label)
                    .aria_label("Shelf: empty, read-only")
                    .p_3()
                    .child(Caption::new(
                        "The save's shelf is empty. Rolled cogs land here until the optimizer places them.",
                    )),
            )
            .into_any_element();
    }
    let mut table = Table::new(["Shelf", "Cog", "Base", "Surround"]);
    for (index, cog) in save.shelf.iter().enumerate() {
        let base = match goal {
            Goal::Build => cog.a,
            Goal::ConExp => cog.d,
            Goal::Flaggy => cog.c,
        };
        let surround = match goal {
            Goal::Build => cog.e,
            Goal::ConExp => cog.f,
            Goal::Flaggy => cog.g,
        };
        let surround_text = match cog.direction {
            Some(direction) if surround > 0.0 => format!("{} +{surround:.0}%", direction.name()),
            Some(direction) => direction.name().to_string(),
            None => "—".into(),
        };
        table = table.row([
            div().child(format!("#{}", index + 1)).into_any_element(),
            div().child(cog.display_name()).into_any_element(),
            div()
                .child(if base >= 1e6 {
                    format::fmt_val(base)
                } else {
                    format!("{base:.0}")
                })
                .into_any_element(),
            div().child(surround_text).into_any_element(),
        ]);
    }
    Card::new()
        .header(
            header.action(
                Badge::new(format!("{} cogs · read-only", save.shelf.len())).tone(Tone::Neutral),
            ),
        )
        .child(
            div()
                .id("cog-shelf")
                .role(Role::Label)
                .aria_label(format!(
                    "Shelf: {} placeable cogs of {total} filled slots, read-only",
                    save.shelf.len()
                ))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    ScrollArea::new("cog-shelf-scroll")
                        .vertical()
                        .h(px(260.0))
                        .child(table),
                )
                .child(
                    div().px_3().pb_3().child(
                        Caption::new(
                            "The shelf is the optimizer's pool, so the section only reads it. Small cogs stay on the tiny board and are not listed.",
                        )
                        .text_color(muted),
                    ),
                ),
        )
        .into_any_element()
}
