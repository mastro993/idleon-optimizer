//! The Killroy Calculator: the weekly encounter forecast.
//!
//! Ported from the baseline half of `js/ui/killroy.js` and the forecast in
//! `js/stats/systems/common/killroy-planner.js`. Kills, skulls, upgrade
//! comparisons and the combat results tables arrive with KL-02 and KL-03; this
//! pane only explains what the imported save can run in each upcoming week.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::{Caption, DateTimeText};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_core::killroy::{self, Encounter, Forecast, ForecastWeek};

/// The section's live state: how many upcoming weeks the forecast covers.
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub weeks: usize,
}

impl Default for State {
    fn default() -> Self {
        Self { weeks: 12 }
    }
}

/// The section as the registry opens it.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let forecast = state
        .save
        .as_ref()
        .map(|save| killroy::forecast(save, state.killroy.weeks));
    match forecast {
        Some(Ok(forecast)) => forecast_pane(state, forecast, window, cx),
        Some(Err(error)) => error_state(error.to_string(), window, cx),
        None => empty_state(window, cx),
    }
}

/// No save has been imported yet.
fn empty_state(_window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(
            CardHeader::new("Weekly forecast")
                .description("Encounters, monster records and class eligibility"),
        )
        .child(
            EmptyState::new(
                "killroy-empty",
                IconName::CalendarRange,
                "Import a save to forecast Killroy",
            )
            .body(
                "The forecast reads each character's class, encounter unlocks and monster records \
                 from the save, plus the server's Killroy rotation from a wrapped it.json.",
            )
            .action(
                Button::new("killroy-import", "Import a save")
                    .icon(IconName::Upload)
                    .on_click(move |_event, window, cx| {
                        entity.update(cx, |state, cx| state.open_import(window, cx));
                    }),
            ),
        )
        .into_any_element()
}

/// A save that cannot answer the forecast, with the way to replace it.
fn error_state(error: String, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(
            CardHeader::new("Weekly forecast")
                .description("Encounters, monster records and class eligibility"),
        )
        .child(
            EmptyState::new(
                "killroy-error",
                IconName::TriangleAlert,
                "The forecast needs more from the save",
            )
            .body(error)
            .action(
                Button::new("killroy-import", "Import another save")
                    .icon(IconName::Upload)
                    .on_click(move |_event, window, cx| {
                        entity.update(cx, |state, cx| state.open_import(window, cx));
                    }),
            ),
        )
        .into_any_element()
}

/// The forecast table and the rotation's upcoming statistics.
fn forecast_pane(
    state: &mut AppState,
    forecast: Forecast,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let breakpoint = Breakpoint::of(window);
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let weeks = state.killroy.weeks;
    let entity = cx.entity();
    let rotation = forecast.weeks.first().map(|week| week.week).unwrap_or(0);

    let table = forecast.weeks.iter().fold(
        Table::new(["Week", "Encounter 1", "Encounter 2", "Encounter 3"]),
        |table, week| {
            table.row([
                week_cell(week),
                encounter_cell(week, 0, cx),
                encounter_cell(week, 1, cx),
                encounter_cell(week, 2, cx),
            ])
        },
    );

    let forecast_card = Card::new()
        .header(
            CardHeader::new("Weekly forecast")
                .description(format!(
                    "Rotation week {rotation} · {weeks} upcoming weeks · {} character(s)",
                    forecast
                        .weeks
                        .first()
                        .map(|week| week.characters.len())
                        .unwrap_or(0)
                ))
                .action(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Caption::new("Upcoming weeks"))
                        .child(
                            NumberInput::new("killroy-weeks", weeks as f64)
                                .range(1.0, 52.0)
                                .precision(0)
                                .size(ControlSize::Sm)
                                .label("Upcoming weeks")
                                .on_change(move |value, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        state.set_killroy_weeks(value, cx);
                                    });
                                }),
                        ),
                ),
        )
        .child(
            div().id("killroy-forecast").min_h_0().p_3().child(
                ScrollArea::new("killroy-forecast-scroll")
                    .vertical()
                    .h(table_height)
                    .child(table),
            ),
        )
        .child(
            div()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(cx.theme().colors.fg_muted)
                .child(
                    "A character appears under an encounter when its class and unlocks allow it. \
                     Map records are shared across classes; used encounters are practice only.",
                ),
        );

    div()
        .id("killroy-pane")
        .flex()
        .size_full()
        .min_h_0()
        .child(layout::split_pane(
            "killroy-body",
            breakpoint,
            forecast_card,
            rotation_card(&forecast, table_height, cx),
            px(340.0),
        ))
        .into_any_element()
}

/// The week's label and local dates.
fn week_cell(week: &ForecastWeek) -> AnyElement {
    let label = SharedString::from(week_label(week.week_offset));
    let mut cell = div()
        .id(("killroy-week", week.week_offset))
        .role(gpui::Role::Label)
        .aria_label(format!(
            "{}: {} to {}",
            label,
            stamp(week.starts_at),
            stamp(week.ends_at)
        ))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(label));
    if let (Ok(start), Ok(end)) = (
        jiff::Timestamp::from_second(week.starts_at.floor() as i64),
        jiff::Timestamp::from_second(week.ends_at.floor() as i64),
    ) {
        cell = cell.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(DateTimeText::new(start).pattern("%b %-d"))
                .child(Caption::new("–"))
                .child(DateTimeText::new(end).pattern("%b %-d, %Y")),
        );
    }
    cell.into_any_element()
}

/// One encounter slot: its family, maps and records, and who can enter.
fn encounter_cell(week: &ForecastWeek, slot: usize, cx: &mut Context<AppState>) -> AnyElement {
    let (muted, accent) = {
        let colors = &cx.theme().colors;
        (colors.fg_muted, colors.accent)
    };
    let entries: Vec<&Encounter> = week
        .characters
        .iter()
        .filter_map(|character| character.encounters.get(slot))
        .collect();
    // Upstream draws a room for every slot not known to be locked, so an
    // unknown unlock still shows its map and record.
    let offered: Vec<&Encounter> = entries
        .iter()
        .copied()
        .filter(|encounter| encounter.available != Some(false))
        .collect();
    let Some(first) = offered
        .first()
        .copied()
        .or_else(|| entries.first().copied())
    else {
        return div().into_any_element();
    };
    let mut maps: Vec<&Encounter> = Vec::new();
    for encounter in &offered {
        if encounter
            .map_idx
            .is_some_and(|map_idx| !maps.iter().any(|seen| seen.map_idx == Some(map_idx)))
        {
            maps.push(encounter);
        }
    }
    let eligible: Vec<(&str, &Encounter)> = week
        .characters
        .iter()
        .filter_map(|character| {
            character
                .encounters
                .get(slot)
                .filter(|encounter| encounter.eligible)
                .map(|encounter| (character.name.as_str(), encounter))
        })
        .collect();

    let mut cell = div()
        .id(("killroy-encounter", week.week_offset * 3 + slot))
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Week {} encounter {}: {} · {}{}",
            week.week_offset,
            slot + 1,
            first.family_name,
            if maps.is_empty() {
                "Locked".to_string()
            } else {
                maps.iter()
                    .map(|encounter| encounter.map_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            if first.reason.is_empty() {
                String::new()
            } else {
                format!(" · {}", first.reason)
            }
        ))
        .flex()
        .flex_col()
        .items_start()
        .gap_1()
        .child(Badge::new(first.family_name.clone()).tone(Tone::Accent));

    if maps.is_empty() {
        cell = cell.child(Caption::new("Locked").text_color(muted));
    }
    for encounter in maps {
        cell = cell.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().child(encounter.map_name.clone()))
                .child(Caption::new("· Record").text_color(muted))
                .child(Caption::new(metric(encounter.best)).text_color(accent)),
        );
    }
    if week.week_offset == 0 && first.used == Some(true) {
        cell = cell.child(Caption::new("Already used — practice only").text_color(muted));
    }
    if eligible.is_empty() {
        cell = cell.child(
            Caption::new(if first.reason.is_empty() {
                "No eligible saved character".into()
            } else {
                first.reason.clone()
            })
            .text_color(muted),
        );
    } else {
        cell = cell.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(Caption::new("Eligible:"))
                .children(
                    eligible
                        .iter()
                        .map(|(name, _)| div().child(Caption::new(*name))),
                ),
        );
    }
    cell.into_any_element()
}

/// The right rail: family and map appearances over the upcoming weeks and the
/// assumptions the numbers rest on.
fn rotation_card(
    forecast: &Forecast,
    table_height: gpui::Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let families = div()
        .id("killroy-families")
        .flex()
        .flex_wrap()
        .gap_1()
        .children(forecast.family_counts.iter().map(|family| {
            Badge::new(format!("{}: {}", family.name, family.count)).tone(Tone::Neutral)
        }));
    let maps = forecast
        .map_counts
        .iter()
        .fold(Table::new(["Map", "Weeks"]), |table, map| {
            table.row([
                div().child(map.name.clone()),
                div().child(map.count.to_string()),
            ])
        });
    Card::new()
        .header(CardHeader::new("Upcoming rotation").description("Weeks after this one only"))
        .child(
            div().flex().flex_col().gap_2().p_3().child(families).child(
                ScrollArea::new("killroy-maps-scroll")
                    .vertical()
                    .h(table_height.min(px(320.0)))
                    .child(maps),
            ),
        )
        .child(
            div()
                .id("killroy-notes")
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Rotation, characters, unlocks, upgrades and records stay at the imported \
                     snapshot; future progress and server changes are not predicted.",
                )
                .child(
                    "Counts include only encounters whose unlocks are known, and a \
                     character-dependent map alternative counts once per encounter.",
                )
                .child(
                    "Dates are local. Kills and skulls come from combat runs, not this forecast.",
                ),
        )
        .into_any_element()
}

fn week_label(offset: usize) -> String {
    match offset {
        0 => "This week".into(),
        1 => "+1 week".into(),
        offset => format!("+{offset} weeks"),
    }
}

/// A local date for an aria label.
fn stamp(seconds: f64) -> String {
    jiff::Timestamp::from_second(seconds.floor() as i64)
        .ok()
        .and_then(|at| {
            let local = at.to_zoned(jiff::tz::TimeZone::system());
            jiff::fmt::strtime::format("%b %-d, %Y", local.datetime()).ok()
        })
        .unwrap_or_else(|| "unknown".into())
}

/// A record or count with thousands separators.
fn metric(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    if value.fract() == 0.0 && value.abs() < u64::MAX as f64 {
        crate::ui::format::count(value as u64)
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_labels_read_from_this_week() {
        assert_eq!(week_label(0), "This week");
        assert_eq!(week_label(1), "+1 week");
        assert_eq!(week_label(3), "+3 weeks");
    }

    #[test]
    fn metrics_keep_records_readable() {
        assert_eq!(metric(Some(1_234.0)), "1,234");
        assert_eq!(metric(Some(12.5)), "12.50");
        assert_eq!(metric(None), "—");
    }
}
