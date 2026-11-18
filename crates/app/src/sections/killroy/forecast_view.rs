//! The forecast card: each week's three encounters and who can enter them.
//!
//! The week rows, the family and room of every encounter, the save's records,
//! the class eligibility and the character selector that queues an encounter
//! for the run.

use crate::sections::killroy::Mode;
use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use idleon_killroy::{Forecast, ForecastWeek, Selection};
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone, show_number};

/// One forecast week as the table draws it, built when the forecast changes.
pub(crate) struct WeekRow {
    offset: usize,
    label: SharedString,
    /// The week and its local dates, read aloud.
    aria: SharedString,
    /// The local dates under the check, when the timestamps are valid.
    dates: Option<SharedString>,
    slots: Vec<Option<SlotRow>>,
}

/// One encounter slot of a week: its family, rooms, records and who can enter.
struct SlotRow {
    family: SharedString,
    aria: SharedString,
    /// Each room on offer and its record; empty when every room is locked.
    maps: Vec<(SharedString, SharedString)>,
    /// This week's encounter was already used: practice only.
    practice: bool,
    /// Why nobody can enter, when nobody can.
    blocked: Option<SharedString>,
    /// The characters who can enter, by name and index.
    eligible: Vec<(SharedString, usize)>,
}

/// The table rows for `forecast`: dates, labels and records formatted once.
pub(crate) fn week_rows(forecast: &Forecast) -> Vec<WeekRow> {
    forecast
        .weeks
        .iter()
        .map(|week| {
            let label = week_label(week.week_offset);
            let dates = match (
                jiff::Timestamp::from_second(week.starts_at.floor() as i64),
                jiff::Timestamp::from_second(week.ends_at.floor() as i64),
            ) {
                (Ok(start), Ok(end)) => {
                    let zone = jiff::tz::TimeZone::system();
                    Some(
                        format!(
                            "{} – {}",
                            start.to_zoned(zone.clone()).strftime("%b %-d"),
                            end.to_zoned(zone).strftime("%b %-d, %Y")
                        )
                        .into(),
                    )
                }
                _ => None,
            };
            WeekRow {
                offset: week.week_offset,
                aria: format!(
                    "{label}: {} to {}",
                    stamp(week.starts_at),
                    stamp(week.ends_at)
                )
                .into(),
                label: label.into(),
                dates,
                slots: (0..3).map(|slot| slot_row(week, slot)).collect(),
            }
        })
        .collect()
}

fn slot_row(week: &ForecastWeek, slot: usize) -> Option<SlotRow> {
    let entries: Vec<&idleon_killroy::Encounter> = week
        .characters
        .iter()
        .filter_map(|character| character.encounters.get(slot))
        .collect();
    // A room is drawn for every slot not known to be locked, so an
    // unknown unlock still shows its map and record.
    let offered: Vec<&idleon_killroy::Encounter> = entries
        .iter()
        .copied()
        .filter(|encounter| encounter.available != Some(false))
        .collect();
    let first = offered
        .first()
        .copied()
        .or_else(|| entries.first().copied())?;
    let mut maps: Vec<&idleon_killroy::Encounter> = Vec::new();
    for encounter in &offered {
        if encounter
            .map_idx
            .is_some_and(|map_idx| !maps.iter().any(|seen| seen.map_idx == Some(map_idx)))
        {
            maps.push(encounter);
        }
    }
    let eligible: Vec<(SharedString, usize)> = week
        .characters
        .iter()
        .filter(|character| {
            character
                .encounters
                .get(slot)
                .is_some_and(|encounter| encounter.eligible)
        })
        .map(|character| (character.name.clone().into(), character.ci))
        .collect();
    Some(SlotRow {
        family: first.family_name.clone().into(),
        aria: format!(
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
        )
        .into(),
        maps: maps
            .iter()
            .map(|encounter| {
                (
                    encounter.map_name.clone().into(),
                    metric(encounter.best).into(),
                )
            })
            .collect(),
        practice: week.week_offset == 0 && first.used == Some(true),
        blocked: eligible.is_empty().then(|| {
            if first.reason.is_empty() {
                "No eligible saved character".into()
            } else {
                first.reason.clone().into()
            }
        }),
        eligible,
    })
}

/// The import action both empty states offer.
fn import_button(label: &'static str, cx: &mut Context<AppState>) -> Button {
    let entity = cx.entity();
    Button::new("killroy-import")
        .label(label)
        .icon(IconName::Upload)
        .on_click(move |_event, window, cx| {
            entity.update(cx, |state, cx| state.open_import(window, cx));
        })
}

/// The forecast card around an empty state: an icon, a title, a line and the
/// import action.
fn empty_card(
    id: &'static str,
    icon: IconName,
    title: &'static str,
    body: impl Into<SharedString>,
    action: Button,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(
            CardHeader::new("Weekly forecast")
                .description("Encounters, monster records and class eligibility"),
        )
        .child(
            div().id(id).child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(EmptyMedia::new().child(Icon::new(icon).text_color(muted)))
                            .title(EmptyTitle::new().child(title))
                            .description(EmptyDescription::new().child(body.into())),
                    )
                    .content(EmptyContent::new().child(action)),
            ),
        )
        .into_any_element()
}

/// No save has been imported yet.
pub(crate) fn empty_state(_window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let action = import_button("Import a save", cx);
    empty_card(
        "killroy-empty",
        IconName::CalendarRange,
        "Import a save to forecast Killroy",
        "The forecast reads each character's class, encounter unlocks and monster records \
         from the save, plus the server's Killroy rotation from a wrapped it.json.",
        action,
        cx,
    )
}

/// A save that cannot answer the forecast, with the way to replace it.
pub(crate) fn error_state(
    error: String,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let action = import_button("Import another save", cx);
    empty_card(
        "killroy-error",
        IconName::TriangleAlert,
        "The forecast needs more from the save",
        error,
        action,
        cx,
    )
}

/// The forecast table and the `+1` upgrade list beside it.
pub(crate) fn forecast_card(
    state: &AppState,
    forecast: &Forecast,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let table_height = layout::table_height(f32::from(window.viewport_size().height));
    let weeks = state.killroy.weeks;
    let rotation = forecast.weeks.first().map(|week| week.week).unwrap_or(0);
    let comparing = state.killroy.mode == Mode::Upgrades;
    let field = &state.killroy_controls.weeks;
    show_number(field, weeks as f64, 0, window, cx);

    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .id("killroy-forecast-head")
                    .child(TableHead::new().child("Week"))
                    .child(TableHead::new().child("Encounter 1"))
                    .child(TableHead::new().child("Encounter 2"))
                    .child(TableHead::new().child("Encounter 3")),
            ),
        )
        .child(
            TableBody::new().children(state.killroy_controls.weeks_rows.iter().map(|week| {
                TableRow::new()
                    .id(("killroy-forecast-row", week.offset))
                    .child(TableCell::new().child(week_cell(state, week, cx)))
                    .children(week.slots.iter().enumerate().map(|(slot, row)| {
                        TableCell::new().child(encounter_cell(state, week, slot, row, cx))
                    }))
            })),
        );

    Card::new()
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
                    h_flex()
                        .gap_2()
                        .child(Caption::new("Upcoming weeks"))
                        .child(crate::ui::number_field(
                            "killroy-weeks",
                            field,
                            "Upcoming weeks",
                        )),
                ),
        )
        .child(
            div().id("killroy-forecast").min_h_0().child(
                div()
                    .h(table_height.min(px(300.0)))
                    .child(table)
                    .overflow_y_scrollbar()
                    .id("killroy-forecast-scroll"),
            ),
        )
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .text_color(cx.theme().muted_foreground)
                .child(if comparing {
                    "Check a character to compare its +1 upgrades. Used encounters are practice \
                     only; map records are shared across classes."
                } else {
                    "Check a character to queue its week. Used encounters are practice only; map \
                     records are shared across classes."
                }),
        )
        .into_any_element()
}

/// The week's label, its check and its local dates.
fn week_cell(state: &AppState, week: &WeekRow, cx: &mut Context<AppState>) -> AnyElement {
    let offset = week.offset;
    let entity = cx.entity();
    let check = Checkbox::new(format!("killroy-week-check-{offset}"))
        .label(week.label.clone())
        .checked(state.killroy.week_checked(offset))
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| state.toggle_killroy_week(offset, on, cx));
        });
    v_flex()
        .id(("killroy-week", offset))
        .role(gpui::Role::Label)
        .aria_label(week.aria.clone())
        .gap_1()
        .child(check)
        .children(week.dates.clone().map(Caption::new))
        .into_any_element()
}

/// One encounter slot: its family, maps and records, and who can enter.
fn encounter_cell(
    state: &AppState,
    week: &WeekRow,
    slot: usize,
    row: &Option<SlotRow>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(row) = row else {
        return div().into_any_element();
    };
    let (muted, accent) = (cx.theme().muted_foreground, cx.theme().primary);
    let index = week.offset * 3 + slot;
    v_flex()
        .id(("killroy-encounter", index))
        .role(gpui::Role::Label)
        .aria_label(row.aria.clone())
        .items_start()
        .gap_1()
        .child(
            Tag::new()
                .with_variant(Tone::Accent.into())
                .outline()
                .small()
                .child(row.family.clone()),
        )
        .when(row.maps.is_empty(), |cell| {
            cell.child(Caption::new("Locked").text_color(muted))
        })
        .children(row.maps.iter().map(|(map, record)| {
            h_flex()
                .gap_1()
                .child(div().child(map.clone()))
                .child(Caption::new("· Record").text_color(muted))
                .child(Caption::new(record.clone()).text_color(accent))
        }))
        .when(row.practice, |cell| {
            cell.child(Caption::new("Already used — practice only").text_color(muted))
        })
        .when_some(row.blocked.clone(), |cell, reason| {
            cell.child(Caption::new(reason).text_color(muted))
        })
        .when(!row.eligible.is_empty(), |cell| {
            cell.child(
                h_flex()
                    .id(("killroy-selectors", index))
                    .flex_wrap()
                    .gap_2()
                    .children(
                        row.eligible
                            .iter()
                            .map(|(name, ci)| selector(state, *ci, name, week.offset, slot, cx)),
                    ),
            )
        })
        .into_any_element()
}

/// One character's check for one encounter slot.
fn selector(
    state: &AppState,
    ci: usize,
    name: &SharedString,
    week_offset: usize,
    slot: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let selection = Selection {
        week_offset,
        ci,
        slot,
    };
    let entity = cx.entity();
    Checkbox::new(format!("killroy-select-{week_offset}-{ci}-{slot}"))
        .label(name.clone())
        .checked(state.killroy.selected(week_offset, ci, slot))
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| {
                state.toggle_killroy_selection(selection, on, cx);
            });
        })
        .into_any_element()
}

/// The right rail: family and map appearances over the upcoming weeks and the
/// assumptions the numbers rest on.
pub(crate) fn rotation_card(
    forecast: &Forecast,
    table_height: gpui::Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let families = h_flex()
        .id("killroy-families")
        .flex_wrap()
        .gap_1()
        .children(forecast.family_counts.iter().map(|family| {
            Tag::new()
                .with_variant(Tone::Neutral.into())
                .outline()
                .small()
                .child(format!("{}: {}", family.name, family.count))
        }));
    let maps = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .id("killroy-maps-head")
                    .child(TableHead::new().child("Map"))
                    .child(TableHead::new().text_right().child("Weeks")),
            ),
        )
        .child(
            TableBody::new().children(forecast.map_counts.iter().enumerate().map(|(ix, map)| {
                TableRow::new()
                    .id(("killroy-map-row", ix))
                    .child(TableCell::new().child(map.name.clone()))
                    .child(TableCell::new().text_right().child(map.count.to_string()))
            })),
        );
    Card::new()
        .header(CardHeader::new("Upcoming rotation").description("Weeks after this one only"))
        .child(
            v_flex().gap_2().child(families).child(
                div()
                    .h(table_height.min(px(320.0)))
                    .child(maps)
                    .overflow_y_scrollbar()
                    .id("killroy-maps-scroll"),
            ),
        )
        .child(
            v_flex()
                .id("killroy-notes")
                .gap_1()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(
                    "Rotation, characters, unlocks, upgrades and records stay at the imported \
                     snapshot; future progress and server changes are not predicted.",
                )
                .child(
                    "Counts include only encounters whose unlocks are known, and a \
                     character-dependent map alternative counts once per encounter.",
                )
                .child("Dates are local."),
        )
        .into_any_element()
}

pub(crate) fn week_label(offset: usize) -> String {
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
pub(crate) fn metric(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    if value.fract() == 0.0 && value.abs() < u64::MAX as f64 {
        idleon_ui::format::count(value as u64)
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
