//! The overview tab: the save's account snapshot and the operation estimate.

use super::estimate;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::KpiCard;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use idleon_core::jelly::metrics::MetricOptions;
use idleon_core::jelly::{Jelly, boss_hp};

pub fn overview(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    div()
        .id("jelly-overview")
        .flex()
        .flex_col()
        .gap_3()
        .child(snapshot(cx, jelly))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Operation estimate").description(
                        "The seeded Monte Carlo of the saved board, its cell contribution and the outlook.",
                    ),
                )
                .child(
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(estimate::pane(state, cx, jelly)),
                ),
        )
        .into_any_element()
}

/// The page's account snapshot (`summaryCards`), minus the fields that need an
/// estimate; those live under the estimate itself.
fn snapshot(cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let progress = &jelly.progress;
    let metrics = jelly.layout_metrics(
        &jelly.layout,
        MetricOptions {
            fever: Some(progress.fever),
            ..MetricOptions::default()
        },
    );
    let muted = cx.theme().colors.fg_muted;
    let research_reward = jelly.completion_bonus(0);
    let reward_text = if research_reward > 0.0 {
        format!("+{research_reward}%")
    } else {
        "—".to_string()
    };
    Card::new()
        .header(
            CardHeader::new("Account snapshot").description(format!(
                "Obstruction {} of 72 · Fever {} · {} cells in the saved board",
                progress.obstruction + 1,
                idleon_core::jelly::fever_mode_name(progress.fever),
                jelly.layout.len()
            )),
        )
        .child(
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id("jelly-snapshot")
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-obstruction",
                                "Obstruction",
                                (progress.obstruction + 1) as f64,
                            ))
                            .caption(format!(
                                "{} HP · {} sec normal phase",
                                format::value(boss_hp(progress.obstruction)),
                                idleon_core::jelly::boss_time(progress.obstruction)
                            )),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-steady-dps",
                                "Steady DPS",
                                metrics.dps,
                            ))
                            .caption(format!("{} placed cells", metrics.cells.len())),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-bloodcells",
                                "Bloodcells",
                                progress.bloodcells,
                            ))
                            .caption(format!(
                                "Daily: {}",
                                format::value(
                                    progress.best_bloodcells * jelly.upgrade_quantity(38) / 100.0
                                )
                            )),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-attempts",
                                "Attempts banked",
                                progress.tries,
                            ))
                            .caption(format!(
                                "Daily floor: {}",
                                format::number(jelly.daily_tries())
                            )),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-best-dps",
                                "Best DPS",
                                progress.best_dps,
                            ))
                            .caption(format!(
                                "×{:.3} Bloodcells",
                                idleon_core::jelly::metrics::best_dps_multiplier_from_value(
                                    progress.best_dps
                                )
                            )),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-cell-levels",
                                "Cell levels",
                                jelly.total_cell_level(),
                            ))
                            .caption(format!(
                                "Cell EXP ×{:.2}",
                                jelly.cell_exp_multiplier(Some(progress.fever))
                            )),
                        )
                        .child(
                            KpiCard::new(estimate::stat(
                                "jelly-slot-unlocks",
                                "Slot unlocks left",
                                idleon_core::jelly::board::slot_purchases_left(jelly),
                            ))
                            .caption(format!(
                                "{} grid slots available",
                                idleon_core::jelly::board::unlocked_count(jelly)
                            )),
                        ),
                )
                .child(
                    Caption::new(format!(
                        "Research EXP reward: {reward_text} · Legend plot tokens: {} · the board is the save's own",
                        format::number(jelly.legend_slot_tokens())
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}
