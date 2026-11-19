//! The Captains tab: roll quality, EXP and leveling, and the boats left uncrewed.
//!
//! A bonus gain is rolled once when the captain is recruited and stays fixed, so
//! the quality bar reads where inside the recruitment range the roll landed. The
//! boat column follows the section's assignment mode.

use super::controls::cached;
use super::table::{self, Cell};
use super::{captain_code, hours_or_ready};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, SharedString, Styled, div, px,
};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_sailing::{
    FleetPlan, RollQuality, captain_bonus, captain_bonus_name, level_time, roll_quality,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone, fit_columns};

/// The figures the tab shows, read once per key.
pub(crate) struct View {
    count: usize,
    assigned: usize,
    unassigned: usize,
    summary: SharedString,
    /// Whether any boat or captain is flagged.
    flagged: bool,
    /// One line per flagged boat or captain.
    lines: Vec<SharedString>,
    boats_missing: bool,
}

/// One rolled bonus: its name, what it gives now and where the roll landed.
#[derive(Clone)]
pub(crate) struct Bonus {
    id: SharedString,
    meter: SharedString,
    name: SharedString,
    total: SharedString,
    tip: SharedString,
    quality: f32,
    quality_text: SharedString,
}

/// One captain, formatted.
pub(crate) struct Row {
    captain: usize,
    code: SharedString,
    boat: SharedString,
    level: SharedString,
    bonuses: [Option<Bonus>; 2],
    exp: SharedString,
    exp_rate: SharedString,
    eta: SharedString,
    fastest: SharedString,
}

pub(crate) fn columns() -> Vec<Cell<Row>> {
    vec![
        Cell::element(
            "Captain",
            100.0,
            |row, _| {
                div()
                    .id(("captain-code", row.captain))
                    .child(row.code.clone())
                    .into_any_element()
            },
            |row| row.code.clone(),
        ),
        Cell::text("Boat", 100.0, |row| row.boat.clone()),
        Cell::number("Level", 60.0, |row| row.level.clone()),
        Cell::element(
            "First bonus",
            300.0,
            |row, cx| bonus_cell(row.bonuses[0].as_ref(), cx),
            |row| bonus_text(row.bonuses[0].as_ref()),
        ),
        Cell::element(
            "Second bonus",
            300.0,
            |row, cx| bonus_cell(row.bonuses[1].as_ref(), cx),
            |row| bonus_text(row.bonuses[1].as_ref()),
        ),
        Cell::number("EXP", 210.0, |row| row.exp.clone()),
        Cell::number("EXP/hr", 90.0, |row| row.exp_rate.clone()),
        Cell::number("Destination ETA", 120.0, |row| row.eta.clone()),
        Cell::text("Fastest level route", 160.0, |row| row.fastest.clone()),
    ]
}

/// The captains tab: the flags, the KPI row and the captain table.
pub(crate) fn captains_tab(
    state: &mut AppState,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let key = state.sailing_key();
    let view = cached(state, |controls| &mut controls.captains, key, cx, build);
    let muted = cx.theme().muted_foreground;
    let flags = Card::new()
        .header(CardHeader::new("Fleet crew").description(
            "A boat whose saved captain row is missing loses that pairing; the optimizer still sails the boat.",
        ))
        .child(
            v_flex()
                .id("captains-missing")
                .role(gpui::Role::Label)
                .aria_label(view.summary.clone())
                .gap_1()
                .child(if view.flagged {
                    h_flex()
                        .gap_2()
                        .child(table::tag("Missing captains", Tone::Warning))
                        .child(Caption::new(view.summary.clone()).text_color(muted))
                        .into_any_element()
                } else {
                    Caption::new(view.summary.clone())
                        .text_color(muted)
                        .into_any_element()
                })
                .children(view.lines.iter().cloned().map(Caption::new)),
        );
    let kpis = div()
        .id("captains-kpis")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{} captains · {} assigned",
            view.count, view.assigned
        ))
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new("captains-count", "Captains", view.count.to_string())
                        .caption("saved rows; the assignment limits how many sail"),
                )
                .child(
                    Statistic::new("captains-assigned", "Assigned", view.assigned.to_string())
                        .caption(state.sailing_state.mode.label()),
                )
                .child(
                    div()
                        .id("captains-missing-count")
                        .role(gpui::Role::Label)
                        .aria_label(view.unassigned.to_string())
                        .child(
                            Statistic::new(
                                "captains-unassigned",
                                "Unassigned captains",
                                view.unassigned.to_string(),
                            )
                            .caption(if view.boats_missing {
                                "and boats with a missing captain"
                            } else {
                                "flagged in the crew card"
                            }),
                        ),
                ),
        );
    let table = Card::new()
        .header(CardHeader::new("Captain Quality and Leveling").description(
            "Each bonus gain is rolled once at recruitment and stays fixed as the captain levels.",
        ))
        .child(
            div()
                .id("captains-table")
                .h(table::data_height(view.count, 33.0, table_height))
                .child(fit_columns(DataTable::new(&state.sailing_controls.captains_table), &state.sailing_controls.captains_table)),
        )
        .child(
            Caption::new(
                "Quality reads the roll inside its recruitment range for that rarity; the tag shows the bonus the captain actually gives at their level. The boat column follows the selected assignment. EXP needs the trip rate and the island's captain EXP; account multipliers outside the save model are excluded.",
            )
            .text_color(muted),
        );
    v_flex()
        .id("captains-body")
        .gap_3()
        .child(flags)
        .child(kpis)
        .child(table)
        .into_any_element()
}

/// Read the crew once and fill the table.
fn build(state: &AppState, cx: &mut App) -> View {
    let plan = state.fleet_plan(state.sailing_state.island, state.sailing_state.goal);
    let rows: Vec<Row> = (0..state.fleet.captains.len())
        .map(|captain| row(state, &plan, captain))
        .collect();
    let assigned = plan
        .routes
        .iter()
        .filter(|route| route.captain.is_some())
        .count();
    let boats: Vec<usize> = state
        .fleet
        .boats
        .iter()
        .enumerate()
        .filter_map(|(boat, entry)| {
            let captain = entry.captain?;
            (captain >= state.fleet.captains.len()).then_some(boat)
        })
        .collect();
    let sailing: Vec<usize> = plan
        .routes
        .iter()
        .filter_map(|route| route.captain)
        .collect();
    let idle: Vec<usize> = (0..state.fleet.captains.len())
        .filter(|captain| !sailing.contains(captain))
        .collect();

    let mut parts = Vec::new();
    if !boats.is_empty() {
        parts.push(format!(
            "{} boat(s) point at a captain the save does not have",
            boats.len()
        ));
    }
    if !idle.is_empty() {
        parts.push(format!("{} captain(s) sail no boat", idle.len()));
    }
    let flagged = !parts.is_empty();
    let summary = if flagged {
        parts.join(" · ")
    } else {
        "Every boat has a captain and every captain sails a boat.".into()
    };
    let mode = state.sailing_state.mode.label().to_lowercase();
    let lines = boats
        .iter()
        .map(|boat| {
            format!(
                "Boat {} · captain {} is missing from the save",
                boat + 1,
                state.fleet.boats[*boat].captain.unwrap_or(0)
            )
            .into()
        })
        .chain(idle.iter().map(|captain| {
            format!(
                "Captain {} · sails no boat in the {mode} assignment",
                captain_code(*captain)
            )
            .into()
        }))
        .collect();
    let view = View {
        count: rows.len(),
        assigned,
        unassigned: idle.len(),
        summary: summary.into(),
        flagged,
        lines,
        boats_missing: !boats.is_empty(),
    };
    table::set_rows(&state.sailing_controls.captains_table, rows, cx);
    view
}

fn row(state: &AppState, plan: &FleetPlan, captain: usize) -> Row {
    let boat = plan
        .routes
        .iter()
        .find(|route| route.captain == Some(captain))
        .map(|route| route.boat);
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
    let progress = time.map_or(0.0, |time| {
        if time.requirement > 0.0 {
            (time.progress / time.requirement).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    Row {
        captain,
        code: format!("Captain {}", captain_code(captain)).into(),
        boat: match boat {
            Some(boat) => format!("Boat {}", boat + 1),
            None => "not assigned".to_string(),
        }
        .into(),
        level: entry.level.to_string().into(),
        bonuses: [0, 1].map(|slot| bonus(state, captain, roll_quality(entry, slot), slot)),
        exp: match time {
            Some(time) => format!(
                "{} / {} ({}%)",
                format::rate(time.progress),
                format::rate(time.requirement),
                (progress * 100.0 * 10.0).round() / 10.0
            ),
            None => "—".into(),
        }
        .into(),
        exp_rate: time
            .map_or_else(|| "—".into(), |time| format::rate(time.exp_per_hour))
            .into(),
        eta: time
            .map_or_else(|| "—".into(), |time| hours_or_ready(time.hours))
            .into(),
        fastest: fastest
            .and_then(|(island, hours)| {
                hours
                    .is_finite()
                    .then(|| format!("Island {} · {}", island + 1, hours_or_ready(hours)))
            })
            .unwrap_or_else(|| "—".into())
            .into(),
    }
}

/// One bonus slot, or `None` when the captain rolled nothing there.
fn bonus(state: &AppState, captain: usize, quality: RollQuality, slot: usize) -> Option<Bonus> {
    if quality.bonus_type < 0 {
        return None;
    }
    let name = captain_bonus_name(quality.bonus_type);
    let total = captain_bonus(&state.fleet, Some(captain), quality.bonus_type);
    let percent = (quality.quality * 100.0).round();
    Some(Bonus {
        id: format!("captain-bonus-{captain}-{slot}").into(),
        meter: format!("captain-quality-{captain}-{slot}").into(),
        name: name.clone().into(),
        total: format!("+{}%", format::value(total)).into(),
        tip: format!(
            "{name}\nGain per level: +{}%\nPossible rolls: +{}% to +{}%\nQuality: {percent}%",
            format::value(quality.roll),
            format::value(quality.min),
            format::value(quality.max),
        )
        .into(),
        quality: quality.quality as f32,
        quality_text: format!("{percent}%").into(),
    })
}

/// A bonus cell: the name, the total it gives and the roll's quality bar.
fn bonus_cell(bonus: Option<&Bonus>, cx: &App) -> AnyElement {
    let Some(bonus) = bonus else {
        return div().child("—").into_any_element();
    };
    let muted = cx.theme().muted_foreground;
    let body = h_flex()
        .gap_2()
        .child(Caption::new(bonus.name.clone()))
        .child(table::tag(bonus.total.clone(), Tone::Accent))
        .child(
            div()
                .w(px(48.0))
                .child(Progress::new(bonus.meter.clone()).value(bonus.quality * 100.0)),
        )
        .child(Caption::new(bonus.quality_text.clone()).text_color(muted));
    table::tip(bonus.id.clone(), bonus.tip.clone(), body)
}

fn bonus_text(bonus: Option<&Bonus>) -> SharedString {
    match bonus {
        Some(bonus) => format!(
            "{} {} · quality {}",
            bonus.name, bonus.total, bonus.quality_text
        )
        .into(),
        None => "—".into(),
    }
}
