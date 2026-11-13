//! The comparison workspace (CS-04): characters, loadouts and maps side by side.
//!
//! Ported from the "Compare characters, loadouts & maps" card in
//! `combat-sim-calc.html` and `combat-sim-setup-compare.js`: the character
//! roster, each character's saved/edited/named loadouts, the map picker, and
//! one row per selected combination with its delta against the first row.
//! The plan and the deltas are [`idleon_combat::compare`]; the run goes through
//! the shared task runner, and the export copies the same document the Killroy
//! section copies.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Checkbox, Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Hsla, IntoElement, ParentElement, Role, SharedString, Styled, div, px,
};
use idleon_combat::compare::{Cell, Comparison, Metric, Plan};
use idleon_combat::encounter;
use idleon_ui::format;
use idleon_ui::runner::TaskState;

/// The comparison pane: the setup picker, the run and its result table.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.combat_comparison(cx);
    let plan = state.combat_comparison_plan();
    let mut body = div()
        .id("combat-compare-panel")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(selection_card(state, &plan, &task, cx));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "combat-compare-run-progress",
            "Comparing the setups",
            "Runs off the UI thread; cancelling keeps the previous comparison.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_combat_comparison(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "combat-compare-cancelled",
            task.result.is_some(),
        ));
    }
    match task.result.clone() {
        Some(result) => body = body.child(results_card(&result, task.stale, cx)),
        None if !task.running && !task.cancelled => {
            body = body.child(
                Banner::new(
                    "combat-compare-empty",
                    Severity::Info,
                    "No comparison has run yet. Pick at least two character, loadout and map setups, then Compare.",
                )
                .into_any_element(),
            );
        }
        None => {}
    }
    body = body.child(
        Caption::new(
            "Every row runs the same player and duration, so a difference between maps comes \
             from the map. The auto-mode player AI is not ported yet: the stand-in run ignores \
             the loadout's stats, so two loadouts on one map share their kill outcome and differ \
             in their gear totals.",
        )
        .text_color(muted),
    );
    ScrollArea::new("combat-compare-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The roster, the loadouts and the maps the plan reads.
fn selection_card(
    state: &mut AppState,
    plan: &Plan,
    task: &TaskState<Comparison>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let characters: Vec<(usize, String)> = match &state.save {
        Some(save) => (0..save.character_count())
            .map(|index| (index, save.character_name(index)))
            .collect(),
        None => Vec::new(),
    };
    let map_count = state.combat.compare_maps.len();
    let setups = plan
        .rows
        .len()
        .checked_div(map_count)
        .unwrap_or(plan.rows.len());
    let rows = plan.rows.len();
    let invalid = plan.validate().err();
    let running = task.running;

    let mut roster = div().flex().flex_wrap().items_center().gap_3();
    for (index, name) in &characters {
        let selected = state.combat.compare_characters.contains(index);
        let entity = cx.entity();
        let (character, label) = (*index, name.clone());
        roster = roster.child(
            Checkbox::new(
                SharedString::from(format!("combat-compare-char-{character}")),
                selected,
            )
            .label(SharedString::from(label))
            .on_change(move |on, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_combat_compare_character(character, on, cx);
                });
            }),
        );
    }
    if characters.is_empty() {
        roster =
            roster.child(Caption::new("Import a save to choose characters.").text_color(muted));
    }

    let mut loadout_rows = div()
        .id("combat-compare-loadouts")
        .flex()
        .flex_col()
        .gap_2();
    for (index, name) in &characters {
        if !state.combat.compare_characters.contains(index) {
            continue;
        }
        let keys = state.combat.compare_loadout_keys(*index);
        let mut choices = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(loadout_check(
                *index,
                "saved",
                "Imported save".into(),
                keys.iter().any(|key| key == "saved"),
                cx,
            ));
        if *index == state.combat.character && state.combat.loadout.changed() > 0 {
            choices = choices.child(loadout_check(
                *index,
                "edited",
                format!("Working edits ({} changes)", state.combat.loadout.changed()),
                keys.iter().any(|key| key == "edited"),
                cx,
            ));
        }
        let presets: Vec<(String, String)> = state
            .combat
            .presets
            .items
            .iter()
            .filter(|preset| preset.character == *index)
            .map(|preset| (format!("preset:{}", preset.name), preset.name.clone()))
            .collect();
        for (key, name) in presets {
            choices = choices.child(loadout_check(*index, &key, name, keys.contains(&key), cx));
        }
        loadout_rows = loadout_rows.child(
            div()
                .id(SharedString::from(format!(
                    "combat-compare-loadout-row-{index}"
                )))
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(12.0))
                        .child(SharedString::from(name.clone())),
                )
                .child(choices),
        );
    }

    let map_choices: Vec<Choice> = encounter::maps()
        .iter()
        .map(|map| {
            Choice::new(
                map.index.to_string(),
                format!("{} · {}", map.index, encounter::map_name(map)),
            )
        })
        .collect();
    let add_entity = cx.entity();
    let add_map = Select::new("combat-compare-map-add", map_choices)
        .label("Add a map")
        .placeholder("Add a map…")
        .selected(SharedString::from(""))
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                add_entity.update(cx, |state, cx| {
                    state.add_combat_compare_map(index, cx);
                });
            }
        });
    let maps = state.combat.compare_maps.clone();
    let mut map_rows = div().id("combat-compare-maps").flex().flex_col().gap_1();
    if maps.is_empty() {
        map_rows = map_rows.child(Caption::new("Add at least one map.").text_color(muted));
    }
    for map in maps {
        let name = encounter::map(map).map_or_else(
            || format!("Map {map}"),
            |entry| format!("{} · {}", map, encounter::map_name(entry)),
        );
        let entity = cx.entity();
        map_rows = map_rows.child(
            div()
                .id(SharedString::from(format!("combat-compare-map-{map}")))
                .role(Role::Label)
                .aria_label(name.clone())
                .flex()
                .items_center()
                .gap_2()
                .h(px(26.0))
                .child(div().text_size(px(12.0)).child(SharedString::from(name)))
                .child(
                    Button::new(
                        SharedString::from(format!("combat-compare-map-remove-{map}")),
                        "Remove",
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.remove_combat_compare_map(map, cx));
                    }),
                ),
        );
    }

    let run = cx.entity();
    let cancel = cx.entity();
    let controls = div()
        .id("combat-compare-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            div()
                .id("combat-compare-count")
                .role(Role::Label)
                .aria_label(format!(
                    "{setups} setups across {map_count} maps, {rows} runs"
                ))
                .text_size(px(12.0))
                .text_color(muted)
                .child(format!(
                    "{setups} setup(s) × {map_count} map(s) = {rows} run(s)"
                )),
        )
        .child(div().flex_1())
        .child(
            Button::new("combat-compare-run", "Compare")
                .variant(ButtonVariant::Primary)
                .size(ControlSize::Sm)
                .icon(IconName::Play)
                .disabled(running || invalid.is_some())
                .on_click(move |_event, _window, cx| {
                    run.update(cx, |state, cx| state.run_combat_comparison(cx));
                }),
        )
        .child(
            Button::new("combat-compare-cancel", "Cancel")
                .size(ControlSize::Sm)
                .icon(IconName::X)
                .disabled(!running)
                .on_click(move |_event, _window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_combat_comparison(cx));
                }),
        );

    Card::new()
        .header(
            CardHeader::new("Compared setups")
                .description(
                    "Pick characters, the loadouts to run and the maps. Each selected \
                     combination is a row, and the first row is the baseline.",
                )
                .action(Badge::new(format!("{rows} row(s)")).tone(Tone::Accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .when(!characters.is_empty(), |card| {
                    card.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Caption::new("Characters").text_color(muted))
                            .child(roster),
                    )
                })
                .child(loadout_rows)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Maps").text_color(muted))
                        .child(add_map)
                        .child(map_rows),
                )
                .child(controls)
                .when_some(invalid, |card, error| {
                    card.child(Caption::new(error).text_color(muted))
                }),
        )
        .into_any_element()
}

/// One character's loadout checkbox.
fn loadout_check(
    character: usize,
    key: &str,
    label: String,
    selected: bool,
    cx: &mut Context<AppState>,
) -> Checkbox {
    let entity = cx.entity();
    let key = key.to_string();
    Checkbox::new(
        SharedString::from(format!("combat-compare-loadout-{character}-{key}")),
        selected,
    )
    .label(SharedString::from(label))
    .on_change(move |on, _window, cx| {
        entity.update(cx, |state, cx| {
            state.set_combat_compare_loadout(character, &key, on, cx);
        });
    })
}

/// Copy the stored comparison as JSON.
fn export_button(cx: &mut Context<AppState>) -> Button {
    let entity = cx.entity();
    Button::new("combat-compare-export", "Copy comparison JSON")
        .icon(IconName::Copy)
        .size(ControlSize::Sm)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.copy_combat_comparison(cx));
        })
}

/// The stored comparison: one row per setup and a value plus delta per metric.
fn results_card(comparison: &Comparison, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let success = cx.theme().colors.success;
    let danger = cx.theme().colors.danger;
    let mut header = CardHeader::new("Comparison results").description(format!(
        "{} rows · {} · {}",
        comparison.rows.len(),
        player_label(comparison.plan.player),
        duration_label(comparison.plan.duration_ms)
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("combat-compare"));
    } else {
        header = header.action(export_button(cx));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card
            .child(idleon_ui::run_chrome::stale_banner("combat-compare"))
            .child(div().px_3().pt_3().child(export_button(cx)));
    }
    let mut table = Table::new(
        ["Setup", "Map"]
            .into_iter()
            .chain(Metric::ALL.iter().map(|metric| metric.label()))
            .collect::<Vec<_>>(),
    );
    for (index, row) in comparison.rows.iter().enumerate() {
        let mut setup = div()
            .id(SharedString::from(format!("combat-compare-row-{index}")))
            .role(Role::Label)
            .aria_label(format!(
                "{} · {} on map {}",
                row.spec.character_name, row.spec.loadout, row.spec.map
            ))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().text_size(px(12.0)).child(SharedString::from(format!(
                "{} · {}",
                row.spec.character_name, row.spec.loadout
            ))));
        if index == 0 {
            setup = setup.child(
                div()
                    .id("combat-compare-baseline")
                    .child(Badge::new("Baseline").tone(Tone::Neutral)),
            );
        }
        let mut cells: Vec<AnyElement> = vec![setup.into_any_element()];
        cells.push(
            div()
                .id(SharedString::from(format!(
                    "combat-compare-map-name-{index}"
                )))
                .text_size(px(12.0))
                .child(SharedString::from(format!(
                    "{} · {}",
                    row.spec.map, row.map_name
                )))
                .into_any_element(),
        );
        for metric in Metric::ALL {
            cells.push(metric_cell(
                comparison, index, metric, muted, success, danger,
            ));
        }
        table = table.row(cells);
    }
    card.child(
        div().id("combat-compare-results").p_3().child(
            ScrollArea::new("combat-compare-table-scroll")
                .vertical()
                .max_h(px(420.0))
                .child(table),
        ),
    )
    .child(
        div()
            .id("combat-compare-notes")
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .pb_3()
            .text_size(px(11.0))
            .text_color(muted)
            .child(format!(
                "{} rows compared. Deltas are against the first row ({} · {} · map {}); the kill \
                 interval is the one metric where a smaller value is better.",
                format::count(comparison.rows.len() as u64),
                comparison.rows[0].spec.character_name,
                comparison.rows[0].spec.loadout,
                comparison.rows[0].spec.map
            ))
            .child(
                "The Export button copies this comparison as JSON: every setup, its outcome, its \
                 gear totals and each metric's value and delta.",
            ),
    )
    .into_any_element()
}

/// One metric: its value, and its delta against the baseline below it.
fn metric_cell(
    comparison: &Comparison,
    index: usize,
    metric: Metric,
    muted: Hsla,
    success: Hsla,
    danger: Hsla,
) -> AnyElement {
    let cell = comparison.cell(index, metric);
    let value = cell
        .mean
        .map_or_else(|| "—".to_string(), |value| format_metric(metric, value));
    let (delta, color) = if index == 0 {
        ("Baseline".to_string(), muted)
    } else {
        match cell.delta {
            Some(0.0) => ("No change".to_string(), muted),
            Some(delta) => (
                delta_label(cell, metric, delta),
                if (delta > 0.0) == metric.higher_is_better() {
                    success
                } else {
                    danger
                },
            ),
            None => ("—".to_string(), muted),
        }
    };
    div()
        .id(SharedString::from(format!(
            "combat-compare-{}-{index}",
            metric.short_key()
        )))
        .role(Role::Label)
        .aria_label(format!("{}: {value}; {delta}", metric.label()))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(12.0)).child(SharedString::from(value)))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(color)
                .child(SharedString::from(delta)),
        )
        .into_any_element()
}

fn delta_label(cell: Cell, metric: Metric, delta: f64) -> String {
    let absolute = match metric {
        Metric::KillIntervalSeconds => format!("{:+.2}s", delta),
        Metric::TalentPoints => format!("{delta:+.0}"),
        _ if delta.abs() >= 1_000.0 => format!("{delta:+.0}"),
        _ => format!("{delta:+.1}"),
    };
    match cell.delta_pct {
        Some(percent) => format!("{absolute} ({:+.1}%)", percent * 100.0),
        None => absolute,
    }
}

fn format_metric(metric: Metric, value: f64) -> String {
    match metric {
        Metric::KillIntervalSeconds => format!("{value:.2}s"),
        Metric::TalentPoints => format!("{value:.0}"),
        _ => format::value(value),
    }
}

fn player_label(player: idleon_combat::encounter::Player) -> &'static str {
    match player {
        idleon_combat::encounter::Player::StandIn { .. } => "fixed kill interval",
        idleon_combat::encounter::Player::Idle => "idle player",
    }
}

fn duration_label(duration_ms: u64) -> String {
    let seconds = duration_ms / 1000;
    match seconds {
        60 => "1 minute".into(),
        300 => "5 minutes".into(),
        600 => "10 minutes".into(),
        1800 => "30 minutes".into(),
        3600 => "1 hour".into(),
        21_600 => "6 hours".into(),
        43_200 => "12 hours".into(),
        _ => format!("{seconds}s"),
    }
}
