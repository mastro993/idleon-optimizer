//! The pool KPI strip, the every-location table and the clickable pool switch.

use super::spot_text;
use super::table::{self, Kpi};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_fishing::{Evaluation, Objective, PoolOutcome, ties};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, Tone};

/// The character's fishing context: its summary and two strips of figures.
pub(crate) struct Kpis {
    summary: SharedString,
    figures: [Kpi; 10],
}

/// The character's fishing context, formatted once per evaluation.
pub(crate) fn kpis(
    evaluation: &Evaluation,
    ranking: &[Evaluation],
    objective: Objective,
    hours: f64,
) -> Kpis {
    let outcome = &evaluation.outcome;
    let account = &evaluation.account;
    let ratio = ratio(outcome);
    let best = &ranking[0];
    let reference = objective.metric(outcome);
    let gain = if reference > 0.0 {
        (objective.metric(&best.outcome) / reference - 1.0) * 100.0
    } else {
        0.0
    };
    let is_selected = (best.lure.idx, best.line.idx) == (evaluation.lure.idx, evaluation.line.idx);
    let tied = ties(ranking, objective);
    let best_caption = if is_selected && tied > 1 {
        format!("The selected lure + line · ties with {} others", tied - 1)
    } else if is_selected {
        "The selected lure + line".to_string()
    } else {
        format!("{} {gain:+.1}% vs selected", objective.unit())
    };
    let figure = |id, label: &str, value: String, caption: String| -> Kpi {
        (id, label.to_string().into(), value.into(), caption.into())
    };
    Kpis {
        summary: format!(
            "{} catches/hr · {} AFK catches · {} fish · {} EXP · real over panel {:.3}",
            format::number(evaluation.catches_per_hour),
            format::number(outcome.catches),
            format::number(outcome.fish_total),
            format::number(outcome.exp),
            ratio
        )
        .into(),
        figures: [
            figure(
                "fishing-kpi-depths",
                "Depths",
                format!(
                    "D1 {} · D2 {} · D3 {}",
                    format::number(evaluation.depths.d1),
                    format::number(evaluation.depths.d2),
                    format::number(evaluation.depths.d3)
                ),
                format!(
                    "D0 {} · equipment {} · spot {}",
                    format::number(evaluation.depths.d0),
                    account
                        .equipment_depth
                        .iter()
                        .map(|value| format::number(*value))
                        .collect::<Vec<_>>()
                        .join("/"),
                    spot_text(&evaluation.spot_bonus)
                ),
            ),
            figure(
                "fishing-kpi-efficiency",
                "Fishing efficiency",
                format::number(account.efficiency),
                "Entered from the Skills panel".into(),
            ),
            figure(
                "fishing-kpi-catches",
                "Catches / hr",
                format::number(evaluation.catches_per_hour),
                format!(
                    "Catch time {:.3}s + 0.4s · attack ×{:.2}",
                    evaluation.speed, evaluation.attack_factor_used
                ),
            ),
            figure(
                "fishing-kpi-afk",
                &format!("AFK catches · {hours}h"),
                format::number(outcome.catches),
                format!("Catches/hr × AFK {:.1}%", account.afk_rate * 100.0),
            ),
            figure(
                "fishing-kpi-per-catch",
                "Fish / catch",
                format::number(outcome.fish_per_catch),
                format!(
                    "Ore value {} × Multi-Fish",
                    format::number(outcome.ore_value)
                ),
            ),
            figure(
                "fishing-kpi-fish",
                "Fish",
                format::number(outcome.fish_total),
                format!(
                    "{} rolled · {} overwritten",
                    format::number(outcome.rolled_fish_total),
                    format::number(outcome.overwritten_fish)
                ),
            ),
            figure(
                "fishing-kpi-exp",
                "Fishing EXP",
                format::number(outcome.exp),
                format!("×{} EXP multiplier", format::number(account.exp_multi)),
            ),
            figure(
                "fishing-kpi-best",
                "Best lure + line",
                format!("{} + {}", best.lure.name, best.line.name),
                best_caption,
            ),
            figure(
                "fishing-kpi-panel",
                "Panel EXP / hr",
                format::number(outcome.panel_exp_per_hour),
                format!(
                    "{} EXP/catch",
                    format::number(outcome.pool.panel_exp_per_catch)
                ),
            ),
            figure(
                "fishing-kpi-ratio",
                "Real ÷ panel",
                format!("×{ratio:.3}"),
                "The panel omits ore value and per-fish EXP".into(),
            ),
        ],
    }
}

/// The figures as two strips of five.
pub(crate) fn kpis_view(kpis: &Kpis) -> AnyElement {
    v_flex()
        .id("fishing-kpis")
        .role(Role::Label)
        .aria_label(kpis.summary.clone())
        .gap_3()
        .child(table::kpis(&kpis.figures[..5]))
        .child(table::kpis(&kpis.figures[5..]))
        .into_any_element()
}

/// Real EXP per hour over the panel's.
pub(super) fn ratio(outcome: &PoolOutcome) -> f64 {
    if outcome.panel_exp_per_hour > 0.0 {
        (outcome.exp / outcome.hours) / outcome.panel_exp_per_hour
    } else {
        0.0
    }
}

/// One location, formatted.
struct LocationRow {
    id: SharedString,
    location: String,
    label: SharedString,
    selected: bool,
    pool: SharedString,
    spot: SharedString,
    shares: [SharedString; 4],
    per_catch: SharedString,
    fish: SharedString,
    exp: SharedString,
    panel: SharedString,
    ratio: SharedString,
    ratio_good: bool,
}

/// Every location, formatted once per evaluation.
pub(crate) struct Locations {
    description: SharedString,
    rows: Vec<LocationRow>,
}

pub(crate) fn locations(rows: &[Evaluation], selected: &str, hours: f64) -> Locations {
    Locations {
        description: format!(
            "Each spot has its own pool and depth bonus · totals {} · click a row to switch the pool",
            if hours == 1.0 {
                "/hr".to_string()
            } else {
                format!("/{hours}h")
            }
        )
        .into(),
        rows: rows
            .iter()
            .map(|evaluation| {
                let outcome = &evaluation.outcome;
                let ratio = ratio(outcome);
                LocationRow {
                    id: eval_id(&evaluation.location.id).into(),
                    location: evaluation.location.id.clone(),
                    label: evaluation.location.label.clone().into(),
                    selected: evaluation.location.id == selected,
                    pool: evaluation.location.pool_name.clone().into(),
                    spot: spot_text(&evaluation.spot_bonus).into(),
                    shares: std::array::from_fn(|tier| share(outcome, tier).into()),
                    per_catch: format::number(outcome.fish_per_catch).into(),
                    fish: format::number(outcome.fish_total).into(),
                    exp: format::number(outcome.exp).into(),
                    panel: format::number(outcome.panel_exp_per_hour).into(),
                    ratio: format!("×{ratio:.3}").into(),
                    ratio_good: ratio >= 1.0,
                }
            })
            .collect(),
    }
}

/// Every location, each row switching the selected pool. The selected row is
/// marked, and its first cell is the harness/click target
/// `fishing-location-<id with : as ->`.
pub(crate) fn location_table(locations: &Locations, cx: &mut Context<AppState>) -> AnyElement {
    let theme = cx.theme();
    let (muted, success, warning) = (theme.muted_foreground, theme.success, theme.warning);
    let rows = locations.rows.iter().enumerate().map(|(index, row)| {
        let entity = cx.entity();
        let location = row.location.clone();
        let mut first = h_flex()
            .id(row.id.clone())
            .role(Role::Button)
            .aria_label(format!("{}: select this pool", row.label))
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_fishing_location(location.clone(), cx)
                });
            })
            .gap_2()
            .child(if row.selected {
                SharedString::from(format!("{} · selected", row.label))
            } else {
                row.label.clone()
            });
        if row.selected {
            first = first.child(table::tag("Selected", Tone::Accent));
        }
        table::Row::new(("fishing-location-row", index))
            .child(table::text(first))
            .child(table::text(row.pool.clone()))
            .child(table::text(div().text_color(muted).child(row.spot.clone())))
            .children(
                row.shares
                    .iter()
                    .map(|share| table::number(div().text_color(muted).child(share.clone()))),
            )
            .child(table::number(row.per_catch.clone()))
            .child(table::number(row.fish.clone()))
            .child(table::number(row.exp.clone()))
            .child(table::number(row.panel.clone()))
            .child(table::number(
                div()
                    .text_color(if row.ratio_good { success } else { warning })
                    .child(row.ratio.clone()),
            ))
    });
    Card::new()
        .header(
            CardHeader::new("Every fishing location").description(locations.description.clone()),
        )
        .child(div().id("fishing-table").child(table::table(
            "fishing-table-scroll",
            &table::columns(&[
                ("Location", false, 290.0),
                ("Pool", false, 110.0),
                ("Spot depth", false, 140.0),
                ("Fish 1", true, 90.0),
                ("Fish 2", true, 90.0),
                ("Fish 3", true, 90.0),
                ("Fish 4", true, 90.0),
                ("Fish / catch", true, 100.0),
                ("Fish", true, 90.0),
                ("Fishing EXP", true, 110.0),
                ("Panel EXP/hr", true, 120.0),
                ("Real ÷ panel", true, 100.0),
            ]),
            rows,
        )))
        .into_any_element()
}

fn share(outcome: &PoolOutcome, tier: usize) -> String {
    let fish = &outcome.fish[tier];
    if !fish.unlocked {
        "locked".to_string()
    } else if !fish.active {
        "skipped".to_string()
    } else {
        format!("{:.2}%", outcome.shares[tier])
    }
}

/// `54:0` becomes `fishing-location-54-0` for the automation bridge.
pub(crate) fn eval_id(id: &str) -> String {
    format!("fishing-location-{}", id.replace(':', "-"))
}
