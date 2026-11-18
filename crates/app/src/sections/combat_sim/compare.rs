//! The comparison workspace (CS-04): characters, loadouts and maps side by side.
//!
//! The "Compare characters, loadouts & maps" card: the character roster, each
//! character's saved/edited/named loadouts, the map picker, and one row per
//! selected combination with its delta against the first row. The plan and the
//! deltas are [`idleon_combat::compare`]; the run goes through the shared task
//! runner, and the export copies the same document the Killroy section copies.
//!
//! The result rows are formatted once per finished comparison and drawn by a
//! virtualized `DataTable`.

use super::controls::duration_label;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
    px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_combat::compare::{Cell, Comparison, Metric, Plan};
use idleon_combat::encounter;
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, PANEL_GAP, PANEL_PADDING, TEXT_CAPTION, TEXT_MONO, Tone,
};

/// A result row's height: the value and its delta under it.
const ROW_HEIGHT: f32 = 48.0;
/// The result table stops growing here and scrolls.
const MAX_TABLE_HEIGHT: f32 = 420.0;

/// One metric cell: its value, and its delta against the baseline.
#[derive(Clone, Debug, PartialEq)]
struct MetricCell {
    value: SharedString,
    delta: SharedString,
    tone: Tone,
}

/// One compared setup, formatted for the table.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompareRow {
    setup: SharedString,
    label: SharedString,
    map: SharedString,
    metrics: Vec<MetricCell>,
}

/// The formatted rows of the stored comparison and the plan they came from.
#[derive(Default)]
pub(crate) struct Rows {
    plan: Option<Plan>,
    rows: Vec<CompareRow>,
}

impl Rows {
    /// The comparison's rows, built once per finished comparison.
    fn build(comparison: &Comparison) -> Vec<CompareRow> {
        comparison
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| CompareRow {
                setup: format!("{} · {}", row.spec.character_name, row.spec.loadout).into(),
                label: format!(
                    "{} · {} on map {}",
                    row.spec.character_name, row.spec.loadout, row.spec.map
                )
                .into(),
                map: format!("{} · {}", row.spec.map, row.map_name).into(),
                metrics: Metric::ALL
                    .iter()
                    .map(|metric| metric_cell(comparison, index, *metric))
                    .collect(),
            })
            .collect()
    }
}

impl TableDelegate for Rows {
    fn columns_count(&self, _: &App) -> usize {
        2 + Metric::ALL.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("setup", "Setup").width(px(240.0)),
            1 => Column::new("map", "Map").width(px(180.0)),
            _ => {
                let metric = Metric::ALL[col_ix - 2];
                Column::new(metric.short_key(), metric.label()).width(px(150.0))
            }
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut gpui::Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        match col_ix {
            0 => div()
                .id(SharedString::from(format!("combat-compare-row-{row_ix}")))
                .role(Role::Label)
                .aria_label(row.label.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_size(TEXT_MONO).child(row.setup.clone()))
                .when(row_ix == 0, |setup| {
                    setup.child(
                        div().id("combat-compare-baseline").child(
                            Tag::new()
                                .with_variant(Tone::Neutral.into())
                                .outline()
                                .small()
                                .child("Baseline"),
                        ),
                    )
                })
                .into_any_element(),
            1 => div()
                .id(SharedString::from(format!(
                    "combat-compare-map-name-{row_ix}"
                )))
                .text_size(TEXT_MONO)
                .child(row.map.clone())
                .into_any_element(),
            _ => {
                let metric = Metric::ALL[col_ix - 2];
                let cell = &row.metrics[col_ix - 2];
                div()
                    .id(SharedString::from(format!(
                        "combat-compare-{}-{row_ix}",
                        metric.short_key()
                    )))
                    .role(Role::Label)
                    .aria_label(format!(
                        "{}: {}; {}",
                        metric.label(),
                        cell.value,
                        cell.delta
                    ))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(TEXT_MONO)
                            .child(cell.value.clone()),
                    )
                    .child(
                        div()
                            .text_size(TEXT_CAPTION)
                            .text_color(cell.tone.color(cx))
                            .child(cell.delta.clone()),
                    )
                    .into_any_element()
            }
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match col_ix {
            0 => row.setup.clone(),
            1 => row.map.clone(),
            _ => {
                let cell = &row.metrics[col_ix - 2];
                format!("{}; {}", cell.value, cell.delta).into()
            }
        })
    }
}

/// What the result card says about the stored comparison.
struct Summary {
    rows: usize,
    player: idleon_combat::encounter::Player,
    duration_ms: u64,
    baseline: (String, String, usize),
}

/// The comparison pane: the setup picker, the run and its result table.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let plan = state.combat_comparison_plan();
    let table = state.combat.controls.comparison.clone();
    // Read the stored comparison in place: the table's rows are rebuilt only
    // when another comparison finished, and nothing is cloned per frame.
    let (running, cancelled, progress, stale, summary, rows) = {
        let runner = state.combat_compare.read(cx);
        let result = runner.result();
        let shown = table.read(cx).delegate().plan.as_ref();
        let rows = result
            .filter(|comparison| shown != Some(&comparison.plan))
            .map(|comparison| (comparison.plan.clone(), Rows::build(comparison)));
        let summary = result.map(|comparison| Summary {
            rows: comparison.rows.len(),
            player: comparison.plan.player,
            duration_ms: comparison.plan.duration_ms,
            baseline: comparison.rows.first().map_or_else(
                || (String::new(), String::new(), 0),
                |row| {
                    (
                        row.spec.character_name.clone(),
                        row.spec.loadout.clone(),
                        row.spec.map,
                    )
                },
            ),
        });
        (
            runner.running(),
            runner.cancelled(),
            runner.progress().cloned(),
            runner.is_stale(&plan),
            summary,
            rows,
        )
    };
    if let Some((shown, rows)) = rows {
        table.update(cx, |table, cx| {
            let delegate = table.delegate_mut();
            delegate.plan = Some(shown);
            delegate.rows = rows;
            cx.notify();
        });
    }

    let mut body = div()
        .id("combat-compare-panel")
        .flex()
        .flex_col()
        .gap_3()
        .p(PANEL_PADDING)
        .child(selection_card(state, &plan, running, window, cx));
    if running && let Some(progress) = progress {
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
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "combat-compare-cancelled",
            summary.is_some(),
        ));
    }
    match summary {
        Some(summary) => body = body.child(results_card(&summary, &table, stale, cx)),
        None if !running && !cancelled => {
            body = body.child(
                Alert::new(
                    "combat-compare-empty",
                    "No comparison has run yet. Pick at least two character, loadout and map setups, then Compare.",
                )
                .with_variant(Tone::Info.into())
                .banner(),
            );
        }
        None => {}
    }
    body = body.child(
        Caption::new(
            "Every row runs the same player and duration, so a difference between maps comes \
             from the map. The auto-mode player AI is not modelled yet: the stand-in run ignores \
             the loadout's stats, so two loadouts on one map share their kill outcome and differ \
             in their gear totals.",
        )
        .text_color(muted),
    );
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("combat-compare-scroll")
        .into_any_element()
}

/// The roster, the loadouts and the maps the plan reads.
fn selection_card(
    state: &mut AppState,
    plan: &Plan,
    running: bool,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
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

    let mut roster = div().flex().flex_wrap().items_center().gap_3();
    for (index, name) in &characters {
        let selected = state.combat.compare_characters.contains(index);
        let entity = cx.entity();
        let character = *index;
        roster = roster.child(
            Checkbox::new(SharedString::from(format!(
                "combat-compare-char-{character}"
            )))
            .label(SharedString::from(name.clone()))
            .checked(selected)
            .on_click(move |on, _window, cx| {
                let on = *on;
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
        .gap(PANEL_GAP);
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
                        .text_size(TEXT_MONO)
                        .child(SharedString::from(name.clone())),
                )
                .child(choices),
        );
    }

    // The picker adds a map and goes back to its placeholder.
    let controls = &state.combat.controls;
    controls
        .compare_map
        .show(controls.compare_maps.clone(), None, window, cx);
    let add_map = Select::new(controls.compare_map.state())
        .id("combat-compare-map-add")
        .accessibility_label("Add a map")
        .placeholder("Add a map…")
        .small()
        .w(px(320.0));
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
                .child(div().text_size(TEXT_MONO).child(SharedString::from(name)))
                .child(
                    Button::new(SharedString::from(format!(
                        "combat-compare-map-remove-{map}"
                    )))
                    .label("Remove")
                    .small()
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
        .gap(PANEL_GAP)
        .child(
            div()
                .id("combat-compare-count")
                .role(Role::Label)
                .aria_label(format!(
                    "{setups} setups across {map_count} maps, {rows} runs"
                ))
                .text_size(TEXT_MONO)
                .text_color(muted)
                .child(format!(
                    "{setups} setup(s) × {map_count} map(s) = {rows} run(s)"
                )),
        )
        .child(div().flex_1())
        .child(
            Button::new("combat-compare-run")
                .label("Compare")
                .primary()
                .small()
                .icon(IconName::Play)
                .disabled(running || invalid.is_some())
                .on_click(move |_event, _window, cx| {
                    run.update(cx, |state, cx| state.run_combat_comparison(cx));
                }),
        )
        .child(
            Button::new("combat-compare-cancel")
                .label("Cancel")
                .small()
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
                .action(
                    Tag::new()
                        .with_variant(Tone::Accent.into())
                        .outline()
                        .small()
                        .child(format!("{rows} row(s)")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p(PANEL_PADDING)
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
    Checkbox::new(SharedString::from(format!(
        "combat-compare-loadout-{character}-{key}"
    )))
    .label(SharedString::from(label))
    .checked(selected)
    .on_click(move |on, _window, cx| {
        let on = *on;
        entity.update(cx, |state, cx| {
            state.set_combat_compare_loadout(character, &key, on, cx);
        });
    })
}

/// Copy the stored comparison as JSON.
fn export_button(cx: &mut Context<AppState>) -> Button {
    let entity = cx.entity();
    Button::new("combat-compare-export")
        .label("Copy comparison JSON")
        .icon(IconName::Copy)
        .small()
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.copy_combat_comparison(cx));
        })
}

/// The stored comparison: one row per setup and a value plus delta per metric.
fn results_card(
    summary: &Summary,
    table: &gpui::Entity<TableState<Rows>>,
    stale: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let mut header = CardHeader::new("Comparison results").description(format!(
        "{} rows · {} · {}",
        summary.rows,
        player_label(summary.player),
        duration_label(summary.duration_ms / 1000)
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
            .child(
                div()
                    .px(PANEL_PADDING)
                    .pt(PANEL_PADDING)
                    .child(export_button(cx)),
            );
    }
    // The table fits its rows up to a cap, then scrolls them.
    let height = (summary.rows as f32 * ROW_HEIGHT + 40.0).min(MAX_TABLE_HEIGHT);
    let (character, loadout, map) = &summary.baseline;
    card.child(
        div()
            .id("combat-compare-results")
            .p(PANEL_PADDING)
            .h(px(height + 24.0))
            .child(DataTable::new(table).with_size(px(ROW_HEIGHT))),
    )
    .child(
        div()
            .id("combat-compare-notes")
            .flex()
            .flex_col()
            .gap_1()
            .px(PANEL_PADDING)
            .pb(PANEL_PADDING)
            .text_size(TEXT_CAPTION)
            .text_color(muted)
            .child(format!(
                "{} rows compared. Deltas are against the first row ({character} · {loadout} · \
                 map {map}); the kill interval is the one metric where a smaller value is better.",
                format::count(summary.rows as u64),
            ))
            .child(
                "The Export button copies this comparison as JSON: every setup, its outcome, its \
                 gear totals and each metric's value and delta.",
            ),
    )
    .into_any_element()
}

/// One metric: its value, and its delta against the baseline.
fn metric_cell(comparison: &Comparison, index: usize, metric: Metric) -> MetricCell {
    let cell = comparison.cell(index, metric);
    let value = cell
        .mean
        .map_or_else(|| "—".to_string(), |value| format_metric(metric, value));
    let (delta, tone) = if index == 0 {
        ("Baseline".to_string(), Tone::Neutral)
    } else {
        match cell.delta {
            Some(0.0) => ("No change".to_string(), Tone::Neutral),
            Some(delta) => (
                delta_label(cell, metric, delta),
                if (delta > 0.0) == metric.higher_is_better() {
                    Tone::Success
                } else {
                    Tone::Danger
                },
            ),
            None => ("—".to_string(), Tone::Neutral),
        }
    };
    MetricCell {
        value: value.into(),
        delta: delta.into(),
        tone,
    }
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
