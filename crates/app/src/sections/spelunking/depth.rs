//! The depth estimates: the two charts and the per-depth inspector.
//!
//! Ported from the upstream Depth limits tab. The charts show per-floor
//! placement odds and the expected stamina to break each object; the inspector
//! below them carries the exact numbers, the reference tool and the over-budget
//! flags, which is also the accessible path the canvas cannot offer.

use super::chart::{Series, chart};
use super::state::records;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, IntoElement, ParentElement, Styled, div, px};
use idleon_spelunking::{ConfigError, Inputs, Profile, ReferenceTool, profile};
use idleon_ui::format::{self, hex};

/// The upstream per-rock-type chart colours.
const DEPTH_COLORS: [&str; 16] = [
    "#4dd0e1", "#ffb74d", "#a5d66f", "#df91f0", "#fb8299", "#82aaff", "#ffe082", "#80cbc4",
    "#dca978", "#adadff", "#e4ceff", "#91c4a0", "#f6956a", "#b6d6ee", "#ed95bb", "#ffffff",
];

/// The estimates card.
pub(crate) fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let border = cx.theme().colors.border;
    let entity = cx.entity();
    let from_entity = entity.clone();
    let to_entity = entity.clone();
    let tool_entity = entity.clone();
    let inspect_entity = entity.clone();
    let range = state.spelunking.range();
    let from = NumberInput::new("spelunk-depth-from", range.from as f64)
        .range(1.0, 10_000.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("From depth")
        .on_change(move |value, _window, cx| {
            from_entity.update(cx, |state, cx| {
                state.set_spelunk_depth_from(value as usize, cx)
            });
        });
    let to = NumberInput::new("spelunk-depth-to", range.to as f64)
        .range(1.0, 10_000.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Through depth")
        .on_change(move |value, _window, cx| {
            to_entity.update(cx, |state, cx| {
                state.set_spelunk_depth_to(value as usize, cx)
            });
        });
    let mut tools = vec![Choice::new("best", "Lowest isolated cost (optimistic)")];
    tools.extend(
        inputs
            .tools
            .iter()
            .map(|tool| Choice::new(tool.id.to_string(), tool.name.clone())),
    );
    let tool_select = Select::new("spelunk-depth-tool", tools)
        .label("Reference tool")
        .selected(state.spelunking.depth_tool.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(tool) = ReferenceTool::from_id(value.as_ref()) {
                tool_entity.update(cx, |state, cx| state.set_spelunk_depth_tool(tool, cx));
            }
        });
    let requested = state.spelunking.requested();
    let outcome = profile(inputs, &requested, &range);
    let header = CardHeader::new("Depth estimates").description(
        "Per-floor placement odds, not cumulative risk: an object is over budget only when its \
         expected stamina exceeds the whole party's starting stamina.",
    );
    let card = Card::new().header(header).child(
        div()
            .id("spelunk-depth-controls")
            .flex()
            .flex_wrap()
            .items_end()
            .gap_4()
            .child(crate::sections::spelunking::field(
                "From depth",
                from.into_any_element(),
            ))
            .child(crate::sections::spelunking::field(
                "Through depth",
                to.into_any_element(),
            ))
            .child(crate::sections::spelunking::field(
                "Reference tool",
                tool_select.into_any_element(),
            )),
    );
    let card = match outcome {
        Err(error) => card.child(error_line(&error, cx)),
        Ok(profile) => {
            let records = records(inputs, state.spelunking.manic);
            let record = records.get(state.spelunking.cave).copied().unwrap_or(0.0);
            let summary_text = format!(
                "{} / {} · Entry Power {} · Party stamina {} · Record {}",
                profile.cave,
                if profile.config.manic {
                    "Manic"
                } else {
                    "Normal"
                },
                format::value(profile.power),
                format::value(profile.budget),
                format::value(record)
            );
            let summary = div()
                .id("spelunk-depth-summary")
                .role(gpui::Role::Label)
                .aria_label(summary_text.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(div().text_size(px(12.0)).child(summary_text))
                .child(
                    Badge::new(format!(
                        "{} / {}",
                        profile.cave,
                        if profile.config.manic {
                            "Manic"
                        } else {
                            "Normal"
                        }
                    ))
                    .tone(Tone::Accent),
                )
                .child(Badge::new(format!(
                    "Entry Power {}",
                    format::value(profile.power)
                )))
                .child(Badge::new(format!(
                    "Party stamina {}",
                    format::value(profile.budget)
                )))
                .child(Badge::new(format!("Record {}", format::value(record))))
                .when(profile.partial, |row| {
                    row.child(Badge::new("Partial account inputs").tone(Tone::Warning))
                });
            let risk = risk_chart(&profile, border);
            let stamina = stamina_chart(&profile, border);
            let inspector = inspector(state, inputs, &profile, inspect_entity, border, muted, cx);
            card.child(summary)
                .child(risk)
                .child(stamina)
                .child(notes(muted))
                .child(inspector)
        }
    };
    card.into_any_element()
}

fn error_line(error: &ConfigError, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("spelunk-depth-error")
        .role(gpui::Role::Alert)
        .text_color(cx.theme().colors.danger)
        .child(format!("Cannot estimate depth limits: {error}"))
        .into_any_element()
}

fn risk_chart(profile: &Profile, border: gpui::Hsla) -> AnyElement {
    let blocked: Vec<Option<f64>> = profile
        .rows
        .iter()
        .map(|row| Some(row.blocked_exits))
        .collect();
    let hard: Vec<Option<f64>> = profile.rows.iter().map(|row| Some(row.hard)).collect();
    let series = vec![
        Series::new("All descent holes over budget", hex("#ffb74d"), blocked),
        Series::new("Normal rocks over budget", hex("#fb8299"), hard).dashed(),
    ];
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(subheading("1. Effectively unbreakable exits"))
        .child(chart(
            "spelunk-depth-risk",
            "Per-floor chance that all descent holes sit behind over-budget rocks, and the share \
             of normal rocks over budget."
                .into(),
            series,
            (
                profile.rows.first().map_or(1, |row| row.depth),
                profile.rows.last().map_or(1, |row| row.depth),
            ),
            None,
            false,
            format::percent,
            180.0,
            border,
        ))
        .child(
            Caption::new(
                "Extra holes are included: one bad hole does not trap you if another is reachable. \
                 Bosses, full-clear requirements and the Grand Discovery descent restriction are \
                 excluded from this placement probability.",
            )
            .text_color(border),
        )
        .into_any_element()
}

fn stamina_chart(profile: &Profile, border: gpui::Hsla) -> AnyElement {
    let mut kinds: Vec<usize> = profile
        .rows
        .iter()
        .flat_map(|row| row.objects.iter().map(|object| object.kind))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    let series: Vec<Series> = kinds
        .iter()
        .map(|kind| {
            let name = profile
                .rows
                .iter()
                .flat_map(|row| row.objects.iter())
                .find(|object| object.kind == *kind)
                .map_or_else(|| format!("Rock {kind}"), |object| object.name.clone());
            let values = profile
                .rows
                .iter()
                .map(|row| {
                    row.objects
                        .iter()
                        .find(|object| object.kind == *kind)
                        .map(|object| object.expected)
                        .filter(|expected| expected.is_finite())
                })
                .collect();
            Series::new(name, hex(DEPTH_COLORS[kind % DEPTH_COLORS.len()]), values)
        })
        .collect();
    let budget = profile
        .budget
        .is_finite()
        .then_some((profile.budget, "party starting stamina".to_string()));
    let mut note = String::from(
        "Logarithmic scale. Costs include resists, rounded successful-hit counts, damage upgrades, \
         Manic's 30x cost and the memorized entry effects",
    );
    if profile.refund > 0.0 {
        note.push_str(&format!(
            ", including the {}% stamina refund",
            format::value(profile.refund * 100.0)
        ));
    }
    note.push_str(
        ". Each object is targeted alone with the reference tool repeated throughout; area damage, \
         Shadow and adjacent strikes, Nova, later belt drinks and member switches are not modeled.",
    );
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(subheading("2. Stamina to break each object"))
        .child(chart(
            "spelunk-depth-stamina",
            "Expected stamina to break each rock type by depth, against the party's starting \
             stamina."
                .into(),
            series,
            (
                profile.rows.first().map_or(1, |row| row.depth),
                profile.rows.last().map_or(1, |row| row.depth),
            ),
            budget,
            true,
            format::value,
            200.0,
            border,
        ))
        .child(Caption::new(note).text_color(border))
        .into_any_element()
}

fn inspector(
    state: &AppState,
    inputs: &Inputs,
    profile: &Profile,
    entity: gpui::Entity<AppState>,
    border: gpui::Hsla,
    muted: gpui::Hsla,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let first = profile.rows.first().map_or(1, |row| row.depth);
    let last = profile.rows.last().map_or(first, |row| row.depth);
    let inspect = state.spelunking.inspect.clamp(first, last);
    let input = NumberInput::new("spelunk-depth-inspect", inspect as f64)
        .range(first as f64, last as f64)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Inspect depth")
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_spelunk_inspect(value as usize, cx)
            });
        });
    let row = profile
        .rows
        .iter()
        .find(|row| row.depth == inspect)
        .or_else(|| profile.rows.first());
    let mut table = Table::new([
        "Object",
        "Normal-rock share",
        "Power requirement",
        "Hit chance",
        "Hits needed",
        "Reference tool",
        "Expected stamina",
    ]);
    if let Some(row) = row {
        for object in &row.objects {
            let expected = if object.expected.is_finite() {
                format::value(object.expected)
            } else {
                "Beyond numeric range".to_string()
            };
            table = table.row([
                div()
                    .when(object.over_budget, |cell: Div| {
                        cell.text_color(cx.theme().colors.warning)
                    })
                    .child(format!(
                        "{}{}",
                        object.name,
                        if object.boss { " (boss)" } else { "" }
                    ))
                    .into_any_element(),
                div()
                    .child(if object.boss {
                        "Boss floor only".to_string()
                    } else {
                        format::percent(object.probability)
                    })
                    .into_any_element(),
                div()
                    .child(format::value(object.requirement))
                    .into_any_element(),
                div()
                    .child(format::percent(object.hit_chance))
                    .into_any_element(),
                div().child(format::value(object.hits)).into_any_element(),
                div()
                    .child(tool_name(inputs, object.tool))
                    .into_any_element(),
                div()
                    .when(object.over_budget, |cell: Div| {
                        cell.text_color(cx.theme().colors.warning)
                    })
                    .child(format!(
                        "{expected}{}",
                        if object.over_budget {
                            " (over budget)"
                        } else {
                            ""
                        }
                    ))
                    .into_any_element(),
            ]);
        }
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(subheading("Depth inspector"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(input)
                .when_some(row, |line, row| {
                    line.child(div().text_size(px(12.0)).text_color(muted).child(format!(
                        "Depth {}: all descent holes over budget {}, normal rocks over budget {}.",
                        row.depth,
                        format::percent(row.blocked_exits),
                        format::percent(row.hard)
                    )))
                }),
        )
        .child(table)
        .child(
            Caption::new(
                "Tool names are the reference tools the estimate used; the expected stamina \
                 column flags every object beyond the party's whole starting budget.",
            )
            .text_color(border),
        )
        .into_any_element()
}

fn notes(muted: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            Caption::new(
                "Earlier floors already spend stamina: the full starting budget is an optimistic \
                 reference, not the remaining stamina at that depth. Expected cost is not a \
                 guarantee of success or an exact stopping depth.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

fn subheading(label: &'static str) -> AnyElement {
    div().text_size(px(13.0)).child(label).into_any_element()
}

/// The reference tool's name, for the inspector.
fn tool_name(inputs: &Inputs, id: usize) -> String {
    inputs
        .tools
        .iter()
        .find(|tool| tool.id == id)
        .map_or_else(|| format!("Tool {id}"), |tool| tool.name.clone())
}
