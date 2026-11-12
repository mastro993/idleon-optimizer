//! The selected plot's saved row, its rolls and its per-harvest rewards.

use super::field::family_name;
use crate::state::AppState;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_farming::{self as farming, PlotMetrics, Snapshot};
use idleon_ui::format;

/// The selected plot's saved row and formulas.
pub(super) fn detail_pane(
    state: &mut AppState,
    metric: &PlotMetrics,
    snapshot: &Snapshot,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = &cx.theme().colors;
    let (muted, accent) = (colors.fg_muted, colors.accent);
    let family = family_name(metric.seed_family);
    let title = format!("Plot {}", metric.plot + 1);
    let loaded = state.farm.has_plots();
    let card = Card::new()
        .header(
            CardHeader::new(title.clone())
                .description(if loaded {
                    format!("{} · crop #{}", family, metric.crop_id)
                } else {
                    "No crop planted".into()
                })
                .action(Badge::new(family)),
        )
        .child(
            div()
                .id("plot-detail-body")
                .flex()
                .flex_col()
                .gap_0()
                .px_4()
                .pb_4()
                .when(!loaded, |body| {
                    body.child(
                        EmptyState::new("farming-empty", IconName::Map, "No planted plots")
                            .body(
                                "This save has no planted plots. Import one with FarmPlot or farmPlotData to read its crops, ranks and growth.",
                            ),
                    )
                })
                .when(loaded, |body| {
                    let evolution = if metric.evolution_blocked {
                        "Evolution locked".to_string()
                    } else {
                        format!("Evolution depth {}", metric.evolution)
                    };
                    let overgrowth = format!(
                        "OG {} → {}",
                        metric.overgrowth,
                        metric.overgrowth.saturating_add(1)
                    );
                    let value = if metric.crop_value.capped {
                        "Crop value capped".to_string()
                    } else {
                        format!(
                            "base {} · cap {}",
                            format::number(metric.crop_value.base),
                            format::number(metric.crop_value.cap)
                        )
                    };
                    let rank = format!(
                        "{} / {} XP",
                        format::number(metric.rank_progress),
                        format::number(metric.rank_requirement)
                    );
                    let growth = format!(
                        "{} / {}",
                        format::number(metric.growth_progress),
                        format::number(metric.growth_requirement)
                    );
                    let exp = format!("OG ×{}", farming::overgrowth_multiplier(metric.overgrowth));
                    body.child(stat_row(
                        "plot-crop",
                        "Current crop",
                        format!("#{}", metric.crop_id),
                        evolution,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-growth",
                        "Growth remaining",
                        format::duration(metric.growth_seconds),
                        growth,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-evolution",
                        "Next evolution",
                        format::chance(metric.next_crop_chance),
                        if metric.evolution_blocked {
                            "Blocked".into()
                        } else {
                            "Enabled".into()
                        },
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-overgrowth",
                        "Next overgrowth",
                        format::chance(metric.next_overgrowth_chance),
                        overgrowth,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-vine",
                        "Crops on vine",
                        format::number(f64::from(metric.crops_on_vine)),
                        format!(
                            "Overflow {}",
                            format::number(metric.overflow_progress)
                        ),
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-harvest",
                        "Expected harvest",
                        format::number(metric.crop_value.expected_harvest),
                        value,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-rank-xp",
                        "Rank XP / harvest",
                        format::number(metric.rank_xp),
                        rank,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-farming-exp",
                        "Farming EXP / harvest",
                        format::number(metric.farming_exp),
                        exp,
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-rank",
                        "Land Rank",
                        format::number(f64::from(metric.rank)),
                        format!(
                            "{} unspent account-wide",
                            state.farm.land_rank_points_left()
                        ),
                        muted,
                        accent,
                    ))
                    .child(stat_row(
                        "plot-target",
                        "Goal target",
                        state
                            .farming
                            .assignment
                            .rows
                            .get(metric.plot)
                            .map(|row| {
                                if row.target_crop < 0 {
                                    "—".to_string()
                                } else {
                                    format!("#{}", row.target_crop)
                                }
                            })
                            .unwrap_or_else(|| "—".into()),
                        state.farming.objective.label().to_string(),
                        muted,
                        accent,
                    ))
                    .child(
                        div()
                            .pt_2()
                            .text_size(px(11.0))
                            .text_color(muted)
                            .child(format!(
                                "Crop evo ×{} · overgrowth ×{} · growth ×{}",
                                format::number(snapshot.next_crop_common),
                                format::number(snapshot.overgrowth_common),
                                format::number(snapshot.growth_rate)
                            )),
                    )
                }),
        );
    div()
        .id("plot-detail")
        .role(gpui::Role::Group)
        .aria_label(title)
        .flex()
        .flex_col()
        .min_h_0()
        .child(card)
        .into_any_element()
}

/// One labelled figure in the detail rail.
fn stat_row(
    id: &'static str,
    label: &'static str,
    value: String,
    detail: String,
    muted: Hsla,
    accent: Hsla,
) -> AnyElement {
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label}: {value} · {detail}"))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .child(Caption::new(label).text_color(muted))
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(div().text_size(px(13.0)).text_color(accent).child(value))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(muted)
                        .child(SharedString::from(detail)),
                ),
        )
        .into_any_element()
}
