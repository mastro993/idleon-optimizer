//! Current-state summary: the save's coral metrics and Coral Kid levels.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::v_flex;
use idleon_coral::{Unlock, unlocks};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Statistic, Tone};

/// The saved budget, rank and unlock headline above the plan.
pub(crate) fn kpis(state: &mut AppState) -> AnyElement {
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
        .role(Role::Label)
        .aria_label(format!(
            "Budget {} · god rank {} · offer tier {} · Coral Kid levels {}",
            format::value(inputs.budget),
            inputs.god_rank,
            inputs.tier,
            levels
        ))
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "kpi-coral-budget",
                        "Budget",
                        format!("{budget:.budget_decimals$}{budget_unit}"),
                    )
                    .caption("Divinity points"),
                )
                .child(
                    Statistic::new("kpi-coral-rank", "God rank", inputs.god_rank.to_string())
                        .caption("Divinity[25]"),
                )
                .child(
                    Statistic::new("kpi-coral-tier", "Offer tier", inputs.tier.to_string())
                        .caption("Offering odds row"),
                )
                .child(
                    Statistic::new("kpi-coral-levels", "Coral Kid levels", levels.to_string())
                        .caption(format!("{} of 6 unlocked", inputs.unlocked_count())),
                )
                .child(
                    Statistic::new("kpi-coral-y4", "Y4", format!("{:.1}%", inputs.y4))
                        .caption("Free offering chance"),
                ),
        )
        .into_any_element()
}

/// The save's current coral metrics and Coral Kid levels.
pub(crate) fn current_pane(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let inputs = &state.coral.inputs;
    let metrics = inputs.metrics();
    let muted = cx.theme().muted_foreground;

    let metric_rows = [
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
    ];
    let coral_metrics = table(
        &[("Metric", false), ("Current", true)],
        metric_rows
            .into_iter()
            .enumerate()
            .map(|(index, (label, value))| {
                TableRow::new()
                    .id(("coral-metric-row", index))
                    .child(TableCell::new().child(label))
                    .child(number_cell(value))
            }),
    );

    let levels_table = table(
        &[
            ("Index", true),
            ("Per level", true),
            ("Level", true),
            ("Effect", true),
            ("Next cost", true),
            ("Status", false),
        ],
        inputs
            .level_rows()
            .into_iter()
            .zip(unlocks(inputs))
            .map(|(row, unlock)| {
                TableRow::new()
                    .id(("coral-level-row", row.index))
                    .child(number_cell(row.index.to_string()))
                    .child(number_cell(format::value(row.per_level)))
                    .child(number_cell(row.level.to_string()))
                    .child(number_cell(format::value(row.effect)))
                    .child(number_cell(format::value(row.next_cost)))
                    .child(TableCell::new().child(unlock_status(unlock)))
            }),
    );
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
        levels_card = levels_card.child(
            Alert::new(
                "coral-levels-missing",
                format!(
                    "The save did not carry {}, so those levels read 0. Import a save with the Coral Kid page.",
                    missing_levels.join(" and ")
                ),
            )
            .with_variant(Tone::Warning.into())
            .banner(),
        );
    }

    v_flex()
        .id("coral-summary")
        .gap(PANEL_GAP)
        .child(
            Card::new()
                .header(
                    CardHeader::new("Current coral state")
                        .description("Coral Kid upgrades and Singularity bonuses from the save."),
                )
                .child(coral_metrics)
                .child(
                    Caption::new(format!(
                        "Singularity levels 6-11: {}",
                        singularity.join(", ")
                    ))
                    .text_color(muted),
                ),
        )
        .child(levels_card)
}

/// The status column: the unlock reason, or `Unlocked` when there is none.
fn unlock_status(unlock: Unlock) -> &'static str {
    unlock.reason.unwrap_or("Unlocked")
}

/// A kit table with these `(title, numeric)` columns over `rows`. Number
/// columns sit right-aligned, as `docs/ui-kit/data.md` sets them.
pub(super) fn table(
    columns: &[(&'static str, bool)],
    rows: impl IntoIterator<Item = TableRow>,
) -> Table {
    Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(columns.iter().map(
                |(title, numeric)| {
                    TableHead::new()
                        .when(*numeric, TableHead::text_right)
                        .child(*title)
                },
            ))),
        )
        .child(TableBody::new().children(rows))
}

/// A right-aligned number cell.
pub(super) fn number_cell(text: String) -> TableCell {
    TableCell::new().text_right().child(text)
}
