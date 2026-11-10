//! Current-state summary: the save's coral metrics and Coral Kid levels.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div};

pub fn summary_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let inputs = &state.coral.inputs;
    let metrics = inputs.metrics();
    let muted = cx.theme().colors.fg_muted;
    let (budget, budget_unit) = format::scaled(inputs.budget);
    let budget_decimals = match budget_unit.as_str() {
        "" => 0,
        "K" => 1,
        _ => 2,
    };
    let levels: u32 = inputs.levels.iter().sum();

    let kpis = div()
        .id("coral-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Budget {} · god rank {} · offer tier {} · Coral Kid levels {}",
            format::value(inputs.budget),
            inputs.god_rank,
            inputs.tier,
            levels
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("kpi-coral-budget", "Budget", budget)
                    .decimals(budget_decimals)
                    .suffix(budget_unit),
            )
            .caption("Divinity points"),
        )
        .child(
            KpiCard::new(Statistic::new(
                "kpi-coral-rank",
                "God rank",
                f64::from(inputs.god_rank),
            ))
            .caption("Divinity[25]"),
        )
        .child(
            KpiCard::new(Statistic::new(
                "kpi-coral-tier",
                "Offer tier",
                f64::from(inputs.tier),
            ))
            .caption("Offering odds row"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-coral-levels", "Coral Kid levels", f64::from(levels))
                    .size(TextSize::Xl),
            )
            .caption(format!("{} of 6 unlocked", inputs.unlocked_count())),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-coral-y4", "Y4", inputs.y4)
                    .decimals(1)
                    .suffix("%"),
            )
            .caption("Free offering chance"),
        );

    let mut coral_metrics = Table::new(["Metric", "Current"]);
    for (label, value) in [
        (
            "Divinity Points",
            format!("{}×", format::value(metrics.divinity_points)),
        ),
        (
            "Divinity EXP",
            format!("{}×", format::value(metrics.divinity_exp)),
        ),
        (
            "Class EXP",
            format!("{}×", format::value(metrics.class_exp)),
        ),
        (
            "Minor Links",
            format!("{}×", format::value(metrics.minor_links)),
        ),
        ("Blessing Cap", format::value(metrics.blessing_cap)),
        (
            "Daily Reef Coral",
            format!("{}×", format::value(metrics.daily_reef_coral)),
        ),
    ] {
        coral_metrics = coral_metrics.row([label.to_string(), value]);
    }

    let mut levels_table = Table::new(["Upgrade", "Per level", "Level", "Effect", "Next cost"]);
    for row in inputs.level_rows() {
        levels_table = levels_table.row([
            format!("Upgrade {}", row.index),
            format::value(row.per_level),
            row.level.to_string(),
            format::value(row.effect),
            format::value(row.next_cost),
        ]);
    }

    let singularity: Vec<String> = inputs
        .singularity_levels
        .iter()
        .map(u32::to_string)
        .collect();

    ScrollArea::new("coral-summary-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("coral-summary")
                .flex()
                .flex_col()
                .gap_3()
                .child(kpis)
                .child(
                    Card::new()
                        .header(CardHeader::new("Current coral state").description(
                            "Coral Kid upgrades and Singularity bonuses from the save.",
                        ))
                        .child(coral_metrics)
                        .child(
                            div().px_3().py_2().child(
                                Caption::new(format!(
                                    "Singularity levels 6-11: {}",
                                    singularity.join(", ")
                                ))
                                .text_color(muted),
                            ),
                        ),
                )
                .child(
                    Card::new()
                        .header(
                            CardHeader::new("Current Coral Kid levels")
                                .description("Upgrades 0-5, read from OptLacc[427+i]."),
                        )
                        .child(levels_table),
                ),
        )
}
