//! The run's long tables: the queue preview, the comparison and the saved
//! buffs. Each is a kit `DataTable` whose rows are built here when the plan or
//! the run result changes, never in render (`docs/ui-kit/data.md`).
//!
//! The rows carry element ids the harnesses read (`killroy-buff-…`,
//! `killroy-best-…`), so the delegate renders cells as elements instead of the
//! text-only `idleon_ui::widgets::RowsDelegate`.

use crate::sections::killroy::forecast_view::{metric, week_label};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, FontFeatures, SharedString, Window, div};
use gpui_kit::component::table::{Column, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_killroy::{CharacterBuffs, Plan, RowSummary};
use idleon_ui::widgets::{Caption, TEXT_MONO, Tone};
use std::sync::Arc;

/// One column: the kit column, how a row draws in it and how it reads aloud.
pub(crate) struct Cell<T> {
    column: Column,
    draw: fn(&T, &App) -> AnyElement,
    text: fn(&T) -> SharedString,
}

impl<T> Cell<T> {
    fn new(column: Column, draw: fn(&T, &App) -> AnyElement, text: fn(&T) -> SharedString) -> Self {
        Self { column, draw, text }
    }
}

/// A table's rows and columns, owned by its `TableState`.
pub(crate) struct Rows<T> {
    columns: Vec<Cell<T>>,
    rows: Vec<T>,
}

impl<T: 'static> Rows<T> {
    fn new(columns: Vec<Cell<T>>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    fn text(&self, row_ix: usize, col_ix: usize) -> Option<SharedString> {
        Some((self.columns.get(col_ix)?.text)(self.rows.get(row_ix)?))
    }
}

impl<T: 'static> TableDelegate for Rows<T> {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].column.clone()
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        match (self.rows.get(row_ix), self.columns.get(col_ix)) {
            (Some(row), Some(cell)) => (cell.draw)(row, cx),
            _ => div().into_any_element(),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        self.text(row_ix, col_ix)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.text(row_ix, col_ix)
            .map(String::from)
            .unwrap_or_default()
    }
}

/// A table state over `columns`: no selection, sorting or column moves.
pub(crate) fn state<T: 'static>(
    columns: Vec<Cell<T>>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Entity<TableState<Rows<T>>> {
    cx.new(|cx| {
        TableState::new(Rows::new(columns), window, cx)
            .row_selectable(false)
            .col_selectable(false)
            .col_movable(false)
            .sortable(false)
    })
}

/// Replace a table's rows and redraw it.
pub(crate) fn set_rows<T: 'static>(
    table: &gpui::Entity<TableState<Rows<T>>>,
    rows: Vec<T>,
    cx: &mut App,
) {
    table.update(cx, |table, cx| {
        table.delegate_mut().rows = rows;
        cx.notify();
    });
}

/// A number in the mono face with tabular figures, right-aligned by its column.
fn mono(text: &SharedString, cx: &App) -> AnyElement {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(TEXT_MONO)
        .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        .child(text.clone())
        .into_any_element()
}

fn plain(text: &SharedString) -> AnyElement {
    div().child(text.clone()).into_any_element()
}

/// A name over a caption, as a two-line cell.
fn stacked(title: &SharedString, caption: &SharedString) -> AnyElement {
    v_flex()
        .child(div().child(title.clone()))
        .child(Caption::new(caption.clone()))
        .into_any_element()
}

fn tag(text: &'static str, tone: Tone) -> AnyElement {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(text)
        .into_any_element()
}

/// One configuration the queue preview lists.
pub(crate) struct QueueRow {
    week: SharedString,
    map: SharedString,
    detail: SharedString,
    character: SharedString,
    configurations: SharedString,
}

pub(crate) fn queue_columns() -> Vec<Cell<QueueRow>> {
    vec![
        Cell::new(
            Column::new("week", "Week").width(gpui::px(72.)),
            |row, _| plain(&row.week),
            |row| row.week.clone(),
        ),
        Cell::new(
            Column::new("map", "Map / class").width(gpui::px(132.)),
            |row, _| stacked(&row.map, &row.detail),
            |row| format!("{} · {}", row.map, row.detail).into(),
        ),
        Cell::new(
            Column::new("character", "Character").width(gpui::px(80.)),
            |row, _| plain(&row.character),
            |row| row.character.clone(),
        ),
        Cell::new(
            Column::new("configurations", "Configurations").width(gpui::px(140.)),
            |row, _| Caption::new(row.configurations.clone()).into_any_element(),
            |row| row.configurations.clone(),
        ),
    ]
}

/// The queue preview's rows: every configuration in the plan's order.
pub(crate) fn queue_rows(plan: &Plan) -> Vec<QueueRow> {
    plan.rows
        .iter()
        .map(|row| {
            let configurations = match &row.upgrade {
                Some(upgrade) => format!("+1 {}", upgrade.name),
                None => plan
                    .rows
                    .iter()
                    .filter(|other| other.baseline == row.key)
                    .filter_map(|other| other.upgrade.as_ref())
                    .map(|upgrade| format!("+1 {}", upgrade.name))
                    .collect::<Vec<_>>()
                    .join(" · "),
            };
            QueueRow {
                week: week_label(row.week_offset).into(),
                map: row.encounter.map_name.clone().into(),
                detail: format!(
                    "{} · Encounter {}{}",
                    row.encounter.family_name,
                    row.slot + 1,
                    if row.encounter.used == Some(true) {
                        " (practice)"
                    } else {
                        ""
                    }
                )
                .into(),
                character: row.character.clone().into(),
                configurations: configurations.into(),
            }
        })
        .collect()
}

/// One configuration's averages in the comparison.
pub(crate) struct ResultRow {
    key: String,
    character: SharedString,
    map: SharedString,
    /// The `+1` the row tests; `None` for the baseline reference.
    upgrade: Option<SharedString>,
    best: bool,
    kills: SharedString,
    skulls: SharedString,
    /// Against the row's own baseline; `None` for the baseline itself.
    delta: Option<SharedString>,
    talents: SharedString,
    credits: SharedString,
    pearls: SharedString,
}

fn number_column(key: &'static str, name: &'static str) -> Column {
    Column::new(key, name).text_right().width(gpui::px(76.))
}

pub(crate) fn result_columns() -> Vec<Cell<ResultRow>> {
    vec![
        Cell::new(
            Column::new("character", "Character").width(gpui::px(140.)),
            |row, _| stacked(&row.character, &row.map),
            |row| format!("{} · {}", row.character, row.map).into(),
        ),
        Cell::new(
            Column::new("configuration", "Configuration").width(gpui::px(240.)),
            |row, _| configuration(row),
            |row| match &row.upgrade {
                Some(upgrade) if row.best => format!("{upgrade} · Best option").into(),
                Some(upgrade) => upgrade.clone(),
                None => "Baseline reference".into(),
            },
        ),
        Cell::new(
            number_column("kills", "Kills"),
            |row, cx| mono(&row.kills, cx),
            |row| row.kills.clone(),
        ),
        Cell::new(
            number_column("skulls", "Skulls"),
            |row, cx| mono(&row.skulls, cx),
            |row| row.skulls.clone(),
        ),
        Cell::new(
            number_column("delta", "Δ skulls"),
            |row, cx| match &row.delta {
                Some(delta) => mono(delta, cx),
                None => Caption::new("—").into_any_element(),
            },
            |row| row.delta.clone().unwrap_or_else(|| "—".into()),
        ),
        Cell::new(
            number_column("talents", "Talents"),
            |row, cx| mono(&row.talents, cx),
            |row| row.talents.clone(),
        ),
        Cell::new(
            number_column("credits", "Credits"),
            |row, cx| mono(&row.credits, cx),
            |row| row.credits.clone(),
        ),
        Cell::new(
            number_column("pearls", "Pearls"),
            |row, cx| mono(&row.pearls, cx),
            |row| row.pearls.clone(),
        ),
    ]
}

/// The configuration cell: the `+1` it tests and the best marker.
fn configuration(row: &ResultRow) -> AnyElement {
    let Some(upgrade) = &row.upgrade else {
        return Caption::new("Baseline reference").into_any_element();
    };
    h_flex()
        .id(SharedString::from(format!("killroy-config-{}", row.key)))
        .gap_2()
        .child(div().child(upgrade.clone()))
        .when(row.best, |cell| {
            cell.child(
                div()
                    .id(SharedString::from(format!("killroy-best-{}", row.key)))
                    .role(gpui::Role::Label)
                    .aria_label("Best option")
                    .child(tag("Best", Tone::Success)),
            )
        })
        .into_any_element()
}

/// The comparison's rows in reading order.
pub(crate) fn result_rows(rows: &[RowSummary]) -> Vec<ResultRow> {
    ordered(rows)
        .into_iter()
        .map(|row| {
            let mean = row.mean.unwrap_or_default();
            ResultRow {
                key: row.row.key.clone(),
                character: row.row.character.clone().into(),
                map: row.row.encounter.map_name.clone().into(),
                upgrade: row.row.upgrade.as_ref().map(|upgrade| {
                    format!(
                        "+1 {} ({} → {})",
                        upgrade.name,
                        idleon_ui::format::count(upgrade.level as u64),
                        idleon_ui::format::count(upgrade.next as u64),
                    )
                    .into()
                }),
                best: row.best,
                kills: number(mean.kills),
                skulls: number(mean.skulls_picked),
                delta: row.delta.map(|delta| number(delta.skulls_picked)),
                talents: number(mean.talent_points),
                credits: number(mean.dungeon_credits),
                pearls: number(mean.pearls),
            }
        })
        .collect()
}

/// The rows in reading order: each baseline, then its options best first.
fn ordered(rows: &[RowSummary]) -> Vec<&RowSummary> {
    let mut ordered: Vec<&RowSummary> = Vec::new();
    for base in rows.iter().filter(|row| !row.comparing()) {
        ordered.push(base);
        let mut options: Vec<&RowSummary> = rows
            .iter()
            .filter(|row| row.comparing() && row.row.baseline == base.row.key)
            .collect();
        options.sort_by(|a, b| {
            let key = |row: &RowSummary| {
                let mean = row.mean.unwrap_or_default();
                (
                    std::cmp::Reverse(ordered_key(mean.skulls_picked)),
                    row.row.key.clone(),
                )
            };
            key(a).cmp(&key(b))
        });
        ordered.extend(options);
    }
    ordered
}

fn ordered_key(value: f64) -> u64 {
    (value.max(0.0) * 1000.0).round() as u64
}

/// A mean, rounded the way the tables print it.
fn number(value: f64) -> SharedString {
    if value.fract().abs() < 0.005 {
        idleon_ui::format::count(value.round() as u64).into()
    } else {
        metric(Some(value)).into()
    }
}

/// One saved buff a selected character's run starts with.
pub(crate) struct BuffRow {
    id: SharedString,
    form: Option<SharedString>,
    label: SharedString,
    character: SharedString,
    slot: SharedString,
    buff: SharedString,
    stacks: SharedString,
    time: SharedString,
    kept: bool,
}

pub(crate) fn buff_columns() -> Vec<Cell<BuffRow>> {
    vec![
        Cell::new(
            Column::new("character", "Character").width(gpui::px(150.)),
            |row, _| {
                div()
                    .id(row.id.clone())
                    .role(gpui::Role::Label)
                    .aria_label(row.label.clone())
                    .child(stacked(&row.character, &row.slot))
                    .into_any_element()
            },
            |row| row.label.clone(),
        ),
        Cell::new(
            Column::new("buff", "Buff").width(gpui::px(170.)),
            |row, _| {
                h_flex()
                    .gap_1()
                    .child(div().child(row.buff.clone()))
                    .children(
                        row.form
                            .clone()
                            .map(|id| div().id(id).child(tag("Form toggle", Tone::Neutral))),
                    )
                    .into_any_element()
            },
            |row| {
                if row.form.is_some() {
                    format!("{} · Form toggle", row.buff).into()
                } else {
                    row.buff.clone()
                }
            },
        ),
        Cell::new(
            number_column("stacks", "Stacks").width(gpui::px(90.)),
            |row, cx| mono(&row.stacks, cx),
            |row| row.stacks.clone(),
        ),
        Cell::new(
            number_column("time", "Time left").width(gpui::px(90.)),
            |row, cx| mono(&row.time, cx),
            |row| row.time.clone(),
        ),
        Cell::new(
            Column::new("start", "Start").width(gpui::px(100.)),
            |row, _| {
                if row.kept {
                    tag("Kept", Tone::Success)
                } else {
                    tag("Dropped", Tone::Neutral)
                }
            },
            |row| if row.kept { "Kept" } else { "Dropped" }.into(),
        ),
    ]
}

/// The saved-buff rows, character by character.
pub(crate) fn buff_rows(characters: &[CharacterBuffs]) -> Vec<BuffRow> {
    characters
        .iter()
        .flat_map(|character| {
            character.buffs.iter().map(move |buff| BuffRow {
                id: format!("killroy-buff-{}-{}", character.ci, buff.id).into(),
                form: buff
                    .form
                    .then(|| format!("killroy-form-{}-{}", character.ci, buff.id).into()),
                label: format!(
                    "{} buff {}: {} stacks, {} time left — {}",
                    character.name,
                    buff.id,
                    metric(buff.stacks),
                    metric(buff.time),
                    if buff.kept { "kept" } else { "dropped" }
                )
                .into(),
                character: character.name.clone().into(),
                slot: format!("Character {}", character.ci + 1).into(),
                buff: buff.id.to_string().into(),
                stacks: metric(buff.stacks).into(),
                time: metric(buff.time).into(),
                kept: buff.kept,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_import::import;
    use idleon_killroy::{
        Comparisons, Delta, Means, PlanInput, Selection, plan as build, trial_seeds,
    };
    use idleon_save::Save;
    use serde_json::json;

    fn save() -> Save {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        import(&json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string())
            .expect("the wrapped fixture imports")
            .save
    }

    fn plan(comparisons: Comparisons, trials: usize) -> Plan {
        build(
            &save(),
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds: trial_seeds(1, trials),
                comparisons,
                kills_per_second: 3.0,
                keep_buffs: false,
                unlimited_inventory: false,
            },
        )
        .expect("the plan builds")
    }

    fn rows() -> Vec<RowSummary> {
        idleon_killroy::run(&plan(Comparisons::Only(vec![0, 2]), 3), |_, _| false)
            .expect("the run finishes")
    }

    #[test]
    fn the_saved_buff_rows_read_the_selected_characters() {
        let plan = plan(Comparisons::Off, 1);
        let buffs = idleon_killroy::character_buffs(&save(), &[0], plan.keep_buffs);
        assert_eq!(buffs.len(), 1);
        assert!(!buffs[0].buffs.is_empty());
        assert_eq!(buffs[0].buffs, plan.characters[0].buffs);
        let rows = buff_rows(&buffs);
        assert_eq!(rows.len(), buffs[0].buffs.len());
        assert_eq!(
            rows[0].id.as_ref(),
            format!("killroy-buff-0-{}", buffs[0].buffs[0].id)
        );
    }

    #[test]
    fn the_reading_order_puts_each_baseline_before_its_options() {
        let rows = rows();
        let ordered = ordered(&rows);
        assert_eq!(ordered.len(), rows.len());
        let baseline = ordered.first().expect("the plan has a baseline");
        assert!(!baseline.comparing());
        assert!(ordered[1..].iter().all(|row| row.comparing()));
        assert!(
            ordered[1..]
                .iter()
                .all(|row| row.row.baseline == baseline.row.key)
        );
    }

    #[test]
    fn the_best_option_sorts_first_and_is_marked() {
        let rows = rows();
        let ordered = ordered(&rows);
        let best = ordered
            .iter()
            .find(|row| row.best)
            .expect("a comparison marks its best option");
        assert!(
            ordered[1..]
                .iter()
                .all(|row| row.mean.unwrap().skulls_picked <= best.mean.unwrap().skulls_picked)
        );
        let table = result_rows(&rows);
        assert!(table[0].upgrade.is_none(), "the baseline leads");
        assert!(table[1].best, "the best option follows its baseline");
    }

    #[test]
    fn a_zero_delta_prints_without_a_sign() {
        let base = RowSummary {
            row: rows()[0].row.clone(),
            runs: 1,
            complete: true,
            totals: Means::default(),
            mean: Some(Means::default()),
            delta: Some(Delta {
                kills: 0.0,
                skulls_picked: 0.0,
            }),
            best: false,
        };
        let row = &result_rows(&[base])[0];
        assert_eq!(row.delta.as_deref(), Some("0"));
    }

    #[test]
    fn the_queue_preview_counts_the_options() {
        let plan = plan(Comparisons::All, 2);
        let rows = queue_rows(&plan);
        assert_eq!(rows.len(), plan.rows.len());
        let baseline = plan
            .rows
            .iter()
            .position(|row| row.upgrade.is_none())
            .expect("a baseline row");
        assert_eq!(
            rows[baseline].configurations.split(" · ").count(),
            idleon_killroy::UPGRADES
        );
        assert_eq!(plan.attempts(), (idleon_killroy::UPGRADES + 1) * 2);
    }
}
