//! Current-state summary: the save's coral metrics and Coral Kid levels.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_core::coral::{Unlock, unlocks};

/// The saved budget, rank and unlock headline above the plan.
pub fn kpis(state: &mut AppState) -> AnyElement {
    let inputs = &state.coral.inputs;
    let (budget, budget_unit) = format::scaled(inputs.budget);
    let budget_decimals = match budget_unit.as_str() {
        "" => 0,
        "K" => 1,
        _ => 2,
    };
    let levels: u32 = inputs.levels.iter().sum();
    div()
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
        )
        .into_any_element()
}

/// The save's current coral metrics and Coral Kid levels.
pub fn current_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let inputs = &state.coral.inputs;
    let metrics = inputs.metrics();
    let muted = cx.theme().colors.fg_muted;

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

    let mut levels_table = Table::new([
        "Index",
        "Per level",
        "Level",
        "Effect",
        "Next cost",
        "Status",
    ]);
    for (row, unlock) in inputs.level_rows().into_iter().zip(unlocks(inputs)) {
        levels_table = levels_table.row([
            row.index.to_string(),
            format::value(row.per_level),
            row.level.to_string(),
            format::value(row.effect),
            format::value(row.next_cost),
            unlock_status(unlock).to_string(),
        ]);
    }
    let levels_summary = format!(
        "Coral Kid levels: {}",
        inputs
            .levels
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );

    let singularity: Vec<String> = inputs
        .singularity_levels
        .iter()
        .map(u32::to_string)
        .collect();

    let mut levels_card = Card::new()
        .header(
            CardHeader::new("Current Coral Kid levels")
                .description("Upgrades 0-5, read from OptLacc[427+i]."),
        )
        .child(
            div()
                .id("coral-levels")
                .role(Role::Label)
                .aria_label(levels_summary)
                .w_full()
                .child(levels_table),
        );
    let missing_levels = state.coral.save.levels_missing();
    if state.save_loaded && !missing_levels.is_empty() {
        levels_card = levels_card.child(Banner::new(
            "coral-levels-missing",
            Severity::Warning,
            format!(
                "The save did not carry {}, so those levels read 0. Import a save with the Coral Kid page.",
                missing_levels.join(" and ")
            ),
        ));
    }

    div()
        .id("coral-summary")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Card::new()
                .header(
                    CardHeader::new("Current coral state")
                        .description("Coral Kid upgrades and Singularity bonuses from the save."),
                )
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
        .child(levels_card)
}

/// The status column: the unlock reason, or `Unlocked` when there is none.
fn unlock_status(unlock: Unlock) -> &'static str {
    unlock.reason.unwrap_or("Unlocked")
}
