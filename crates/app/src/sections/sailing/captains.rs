//! The Captains tab: roll quality, EXP and leveling, and the boats left uncrewed.
//!
//! A bonus gain is rolled once when the captain is recruited and stays fixed, so
//! the quality bar reads where inside the recruitment range the roll landed. The
//! boat column follows the section's assignment mode.

use super::{captain_code, hours_or_ready};
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, KpiCard, Meter, Statistic, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, div, px};
use idleon_core::sailing::{
    CaptainLevelTime, FleetPlan, RollQuality, captain_bonus, captain_bonus_name, level_time,
    roll_quality,
};

/// One captain row: their bonuses, their boat and their level time.
struct Row {
    captain: usize,
    boat: Option<usize>,
    rolls: [RollQuality; 2],
    time: Option<CaptainLevelTime>,
    fastest: Option<(usize, f64)>,
}

/// The captains tab: the flags, the KPI row and the captain table.
pub fn captains_tab(
    state: &AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let missing = Missing::of(state, &plan);
    let rows = rows(state, &plan);
    div()
        .id("captains-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(flags(state, &missing, cx))
        .child(kpis(state, &rows, &missing, cx))
        .child(table(state, &rows, table_height, cx))
        .into_any_element()
}

/// Everything the save leaves uncrewed or unassigned.
struct Missing {
    /// Boats whose saved captain row is not in the save.
    boats: Vec<usize>,
    /// Captains with no boat in the current plan.
    captains: Vec<usize>,
}

impl Missing {
    fn of(state: &AppState, plan: &FleetPlan) -> Self {
        let boats = state
            .fleet
            .boats
            .iter()
            .enumerate()
            .filter_map(|(boat, entry)| {
                let captain = entry.captain?;
                (captain >= state.fleet.captains.len()).then_some(boat)
            })
            .collect();
        let assigned: Vec<usize> = plan
            .routes
            .iter()
            .filter_map(|route| route.captain)
            .collect();
        let captains = (0..state.fleet.captains.len())
            .filter(|captain| !assigned.contains(captain))
            .collect();
        Self { boats, captains }
    }

    fn any(&self) -> bool {
        !self.boats.is_empty() || !self.captains.is_empty()
    }

    fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.boats.is_empty() {
            parts.push(format!(
                "{} boat(s) point at a captain the save does not have",
                self.boats.len()
            ));
        }
        if !self.captains.is_empty() {
            parts.push(format!("{} captain(s) sail no boat", self.captains.len()));
        }
        parts.join(" · ")
    }
}

fn rows(state: &AppState, plan: &FleetPlan) -> Vec<Row> {
    let boat_of = |captain: usize| {
        plan.routes
            .iter()
            .find(|route| route.captain == Some(captain))
            .map(|route| route.boat)
    };
    (0..state.fleet.captains.len())
        .map(|captain| {
            let boat = boat_of(captain);
            let time = boat.map(|boat| {
                level_time(
                    &state.fleet,
                    captain,
                    boat,
                    state.sailing_state.island,
                    &state.sailing,
                )
            });
            let fastest = boat.and_then(|boat| {
                let available = state.fleet.available_islands().max(1);
                (0..available)
                    .map(|island| {
                        (
                            island,
                            level_time(&state.fleet, captain, boat, island, &state.sailing).hours,
                        )
                    })
                    .min_by(|left, right| left.1.total_cmp(&right.1))
            });
            let entry = &state.fleet.captains[captain];
            Row {
                captain,
                boat,
                rolls: [roll_quality(entry, 0), roll_quality(entry, 1)],
                time,
                fastest,
            }
        })
        .collect()
}

fn flags(state: &AppState, missing: &Missing, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let summary = if missing.any() {
        missing.summary()
    } else {
        "Every boat has a captain and every captain sails a boat.".into()
    };
    let mut body = div()
        .id("captains-missing")
        .role(gpui::Role::Label)
        .aria_label(summary.clone())
        .flex()
        .flex_col()
        .gap_1()
        .p_3();
    if missing.any() {
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Badge::new("Missing captains").tone(Tone::Warning))
                .child(Caption::new(summary).text_color(muted)),
        );
    } else {
        body = body.child(Caption::new(summary).text_color(muted));
    }
    for boat in &missing.boats {
        body = body.child(Caption::new(format!(
            "Boat {} · captain {} is missing from the save",
            boat + 1,
            state.fleet.boats[*boat].captain.unwrap_or(0)
        )));
    }
    for captain in &missing.captains {
        body = body.child(Caption::new(format!(
            "Captain {} · sails no boat in the {} assignment",
            captain_code(*captain),
            state.sailing_state.mode.label().to_lowercase()
        )));
    }
    Card::new()
        .header(CardHeader::new("Fleet crew").description(
            "A boat whose saved captain row is missing loses that pairing; the optimizer still sails the boat.",
        ))
        .child(body)
        .into_any_element()
}

fn kpis(
    state: &AppState,
    rows: &[Row],
    missing: &Missing,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let count = rows.len();
    let assigned = rows.iter().filter(|row| row.boat.is_some()).count();
    div()
        .id("captains-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!("{count} captains · {assigned} assigned"))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(Statistic::new("captains-count", "Captains", count as f64).decimals(0))
                .caption("saved rows; the assignment limits how many sail"),
        )
        .child(
            KpiCard::new(Statistic::new("captains-assigned", "Assigned", assigned as f64).decimals(0))
                .caption(state.sailing_state.mode.label()),
        )
        .child(
            div()
                .id("captains-missing-count")
                .role(gpui::Role::Label)
                .aria_label(missing.captains.len().to_string())
                .child(
                    KpiCard::new(
                        Statistic::new(
                            "captains-unassigned",
                            "Unassigned captains",
                            missing.captains.len() as f64,
                        )
                        .decimals(0)
                        .size(TextSize::Lg),
                    )
                    .caption(if missing.boats.is_empty() {
                        "flagged in the crew card"
                    } else {
                        "and boats with a missing captain"
                    }),
                ),
        )
        .child(Caption::new(
            "EXP needs the trip rate and the island's captain EXP; account multipliers outside the save model are excluded.",
        )
        .text_color(muted)
        .into_any_element())
        .into_any_element()
}

fn table(
    state: &AppState,
    rows: &[Row],
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut table = Table::new([
        "Captain",
        "Boat",
        "Level",
        "First bonus",
        "Second bonus",
        "EXP",
        "EXP/hr",
        "Destination ETA",
        "Fastest level route",
    ]);
    for row in rows {
        let entry = &state.fleet.captains[row.captain];
        let progress = row.time.map_or(0.0, |time| {
            if time.requirement > 0.0 {
                (time.progress / time.requirement).clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let fastest = row.fastest.and_then(|(island, hours)| {
            hours
                .is_finite()
                .then(|| format!("Island {} · {}", island + 1, hours_or_ready(hours)))
        });
        table = table.row([
            div()
                .id(("captain-code", row.captain))
                .child(format!("Captain {}", captain_code(row.captain)))
                .into_any_element(),
            div()
                .child(match row.boat {
                    Some(boat) => format!("Boat {}", boat + 1),
                    None => "not assigned".to_string(),
                })
                .into_any_element(),
            div().child(entry.level.to_string()).into_any_element(),
            bonus_cell(state, row.captain, row.rolls[0], 0),
            bonus_cell(state, row.captain, row.rolls[1], 1),
            div()
                .child(match row.time {
                    Some(time) => format!(
                        "{} / {} ({}%)",
                        format::rate(time.progress),
                        format::rate(time.requirement),
                        (progress * 100.0 * 10.0).round() / 10.0
                    ),
                    None => "—".into(),
                })
                .into_any_element(),
            div()
                .child(match row.time {
                    Some(time) => format::rate(time.exp_per_hour),
                    None => "—".into(),
                })
                .into_any_element(),
            div()
                .child(match row.time {
                    Some(time) => hours_or_ready(time.hours),
                    None => "—".into(),
                })
                .into_any_element(),
            div()
                .child(fastest.unwrap_or_else(|| "—".into()))
                .into_any_element(),
        ]);
    }

    let table = ScrollArea::new("captains-table-scroll")
        .vertical()
        .h(table_height)
        .child(div().min_w_0().child(table));
    Card::new()
        .header(
            CardHeader::new("Captain Quality and Leveling").description(
                "Each bonus gain is rolled once at recruitment and stays fixed as the captain levels.",
            ),
        )
        .child(
            div()
                .id("captains-table")
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(table),
        )
        .child(
            div()
                .px_3()
                .pb_2()
                .text_size(px(11.0))
                .text_color(cx.theme().colors.fg_muted)
                .child(
                    "Quality reads the roll inside its recruitment range for that rarity; the badge shows the bonus the captain actually gives at their level. The boat column follows the selected assignment.",
                ),
        )
        .into_any_element()
}

/// One bonus slot: its name, the roll's quality and the total it provides.
fn bonus_cell(state: &AppState, captain: usize, quality: RollQuality, slot: usize) -> AnyElement {
    if quality.bonus_type < 0 {
        return div().child("—").into_any_element();
    }
    let name = captain_bonus_name(quality.bonus_type);
    let total = captain_bonus(&state.fleet, Some(captain), quality.bonus_type);
    let share = quality.quality as f32;
    div()
        .id(format!("captain-bonus-{captain}-{slot}"))
        .tooltip(Tooltip::text(format!(
            "{name}\nGain per level: +{}%\nPossible rolls: +{}% to +{}%\nQuality: {}%",
            format::value(quality.roll),
            format::value(quality.min),
            format::value(quality.max),
            (quality.quality * 100.0).round()
        )))
        .flex()
        .flex_col()
        .gap_1()
        .w(px(170.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new(name))
                .child(Badge::new(format!("+{}%", format::value(total))).tone(Tone::Accent)),
        )
        .child(
            Meter::new(format!("captain-quality-{captain}-{slot}"), "", share)
                .thresholds(1.0, 1.0)
                .detail(format!("{}%", (quality.quality * 100.0).round())),
        )
        .into_any_element()
}
