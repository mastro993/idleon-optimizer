//! The overview tab: the save's account snapshot and the operation estimate.

use super::estimate;
use super::parts::{stat, strips};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use idleon_jelly::{Jelly, boss_hp};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP};

pub(crate) fn overview(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    div()
        .id("jelly-overview")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(snapshot(state, jelly))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Operation estimate").description(
                        "The seeded Monte Carlo of the saved board, its cell contribution and the outlook.",
                    ),
                )
                .child(estimate::pane(state, window, cx)),
        )
        .into_any_element()
}

/// The account snapshot, minus the fields that need an
/// estimate; those live under the estimate itself.
fn snapshot(state: &AppState, jelly: &Jelly) -> AnyElement {
    let progress = &jelly.progress;
    let metrics = &state.jelly_state.save.snapshot;
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
                idleon_jelly::fever_mode_name(progress.fever),
                jelly.layout.len()
            )),
        )
        .child(strips(
            "jelly-snapshot",
            vec![
                stat(
                    "jelly-obstruction",
                    "Obstruction",
                    (progress.obstruction + 1) as f64,
                )
                .caption(format!(
                    "{} HP · {} sec normal phase",
                    format::value(boss_hp(progress.obstruction)),
                    idleon_jelly::boss_time(progress.obstruction)
                )),
                stat("jelly-steady-dps", "Steady DPS", metrics.dps)
                    .caption(format!("{} placed cells", metrics.cells.len())),
                stat("jelly-bloodcells", "Bloodcells", progress.bloodcells).caption(format!(
                    "Daily: {}",
                    format::value(jelly.daily_bloodcells())
                )),
                stat("jelly-attempts", "Attempts banked", progress.tries).caption(format!(
                    "Daily floor: {}",
                    format::number(jelly.daily_tries())
                )),
                stat("jelly-best-dps", "Best DPS", progress.best_dps).caption(format!(
                    "×{:.3} Bloodcells",
                    idleon_jelly::metrics::best_dps_multiplier_from_value(progress.best_dps)
                )),
                stat("jelly-cell-levels", "Cell levels", jelly.total_cell_level()).caption(
                    format!(
                        "Cell EXP ×{:.2}",
                        jelly.cell_exp_multiplier(Some(progress.fever))
                    ),
                ),
                stat(
                    "jelly-slot-unlocks",
                    "Slot unlocks left",
                    idleon_jelly::board::slot_purchases_left(jelly),
                )
                .caption(format!(
                    "{} grid slots available",
                    state
                        .jelly_state
                        .save
                        .unlocked
                        .iter()
                        .filter(|open| **open)
                        .count()
                )),
            ],
        ))
        .child(Caption::new(format!(
            "Research EXP reward: {reward_text} · Legend plot tokens: {} · the board is the save's own",
            format::number(jelly.legend_slot_tokens())
        )))
        .into_any_element()
}
