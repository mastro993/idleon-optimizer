//! The depth estimates: the two charts and the per-depth inspector.
//!
//! The charts show per-floor placement odds and the expected stamina to break
//! each object; the inspector below them carries the exact numbers, the
//! reference tool and the over-budget flags, which is also the accessible path
//! the canvas cannot offer.

use super::chart::chart;
use super::controls::{Depth, Derived};
use super::state::records;
use super::{boxed, field, number, select, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use idleon_spelunking::{ConfigError, Inputs, Profile};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, TEXT_MONO, Tone};

/// The estimates card.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let controls = &state.spelunking_controls;
    let range = state.spelunking.range();
    let from = number(
        "spelunk-depth-from",
        &controls.depth_from,
        "From depth",
        range.from as f64,
        0,
        window,
        cx,
    );
    let to = number(
        "spelunk-depth-to",
        &controls.depth_to,
        "Through depth",
        range.to as f64,
        0,
        window,
        cx,
    );
    let tool = select(
        &controls.depth_tool,
        "spelunk-depth-tool",
        "Reference tool",
        &derived.depth_tools,
        &state.spelunking.depth_tool,
        window,
        cx,
    );
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
            .child(field("From depth", from))
            .child(field("Through depth", to))
            .child(field("Reference tool", boxed(tool))),
    );
    let depth: &Depth = &derived.depth;
    let card = match &depth.profile {
        Err(error) => card.child(error_line(error, cx)),
        Ok(profile) => {
            let inputs = &derived.inputs;
            let records = records(inputs, state.spelunking.manic);
            let record = records.get(state.spelunking.cave).copied().unwrap_or(0.0);
            let mode = if profile.config.manic {
                "Manic"
            } else {
                "Normal"
            };
            let summary_text = format!(
                "{} / {mode} · Entry Power {} · Party stamina {} · Record {}",
                profile.cave,
                format::value(profile.power),
                format::value(profile.budget),
                format::value(record)
            );
            let summary = div()
                .id("spelunk-depth-summary")
                .role(Role::Label)
                .aria_label(summary_text.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(div().text_size(TEXT_MONO).child(summary_text))
                .child(tag(format!("{} / {mode}", profile.cave), Tone::Accent))
                .child(tag(
                    format!("Entry Power {}", format::value(profile.power)),
                    Tone::Neutral,
                ))
                .child(tag(
                    format!("Party stamina {}", format::value(profile.budget)),
                    Tone::Neutral,
                ))
                .child(tag(
                    format!("Record {}", format::value(record)),
                    Tone::Neutral,
                ))
                .when(profile.partial, |row| {
                    row.child(tag("Partial account inputs", Tone::Warning))
                });
            let grid = cx.theme().muted_foreground;
            card.child(summary)
                .child(risk_chart(profile, depth, grid))
                .child(stamina_chart(profile, depth, grid))
                .child(notes())
                .child(inspector(state, inputs, profile, window, cx))
        }
    };
    card.into_any_element()
}

fn error_line(error: &ConfigError, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("spelunk-depth-error")
        .role(Role::Alert)
        .text_color(cx.theme().danger)
        .child(format!("Cannot estimate depth limits: {error}"))
        .into_any_element()
}

fn depths(profile: &Profile) -> (usize, usize) {
    (
        profile.rows.first().map_or(1, |row| row.depth),
        profile.rows.last().map_or(1, |row| row.depth),
    )
}

fn risk_chart(profile: &Profile, depth: &Depth, grid: Hsla) -> AnyElement {
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
            depth.risk.clone(),
            depths(profile),
            None,
            false,
            format::percent,
            180.0,
            grid,
        ))
        .child(Caption::new(
            "Extra holes are included: one bad hole does not trap you if another is reachable. \
             Bosses, full-clear requirements and the Grand Discovery descent restriction are \
             excluded from this placement probability.",
        ))
        .into_any_element()
}

fn stamina_chart(profile: &Profile, depth: &Depth, grid: Hsla) -> AnyElement {
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
            depth.stamina.clone(),
            depths(profile),
            budget,
            true,
            format::value,
            200.0,
            grid,
        ))
        .child(Caption::new(note))
        .into_any_element()
}

fn inspector(
    state: &AppState,
    inputs: &Inputs,
    profile: &Profile,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let (first, last) = depths(profile);
    let inspect = state.spelunking.inspect.clamp(first, last.max(first));
    let input = number(
        "spelunk-depth-inspect",
        &state.spelunking_controls.inspect,
        "Inspect depth",
        inspect as f64,
        0,
        window,
        cx,
    );
    let warning = cx.theme().warning;
    let muted = cx.theme().muted_foreground;
    let row = profile
        .rows
        .iter()
        .find(|row| row.depth == inspect)
        .or_else(|| profile.rows.first());
    // One row per object type on the floor: the game's handful of rock kinds.
    let rows: Vec<TableRow> = row
        .map(|row| {
            row.objects
                .iter()
                .enumerate()
                .map(|(index, object)| {
                    let expected = if object.expected.is_finite() {
                        format::value(object.expected)
                    } else {
                        "Beyond numeric range".to_string()
                    };
                    let over = |cell: TableCell| {
                        cell.when(object.over_budget, |cell| cell.text_color(warning))
                    };
                    TableRow::new()
                        .id(("spelunk-inspect-row", index))
                        .child(over(TableCell::new()).child(format!(
                            "{}{}",
                            object.name,
                            if object.boss { " (boss)" } else { "" }
                        )))
                        .child(TableCell::new().text_right().child(if object.boss {
                            "Boss floor only".to_string()
                        } else {
                            format::percent(object.probability)
                        }))
                        .child(
                            TableCell::new()
                                .text_right()
                                .child(format::value(object.requirement)),
                        )
                        .child(
                            TableCell::new()
                                .text_right()
                                .child(format::percent(object.hit_chance)),
                        )
                        .child(
                            TableCell::new()
                                .text_right()
                                .child(format::value(object.hits)),
                        )
                        .child(TableCell::new().child(tool_name(inputs, object.tool)))
                        .child(over(TableCell::new().text_right()).child(format!(
                            "{expected}{}",
                            if object.over_budget {
                                " (over budget)"
                            } else {
                                ""
                            }
                        )))
                })
                .collect()
        })
        .unwrap_or_default();
    let heads = [
        ("Object", false),
        ("Normal-rock share", true),
        ("Power requirement", true),
        ("Hit chance", true),
        ("Hits needed", true),
        ("Reference tool", false),
        ("Expected stamina", true),
    ];
    let table = Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(heads.map(|(head, numeric)| {
                TableHead::new()
                    .when(numeric, |head| head.text_right())
                    .child(head)
            }))),
        )
        .child(TableBody::new().children(rows));
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
                    line.child(div().text_size(TEXT_MONO).text_color(muted).child(format!(
                        "Depth {}: all descent holes over budget {}, normal rocks over budget {}.",
                        row.depth,
                        format::percent(row.blocked_exits),
                        format::percent(row.hard)
                    )))
                }),
        )
        .child(div().id("spelunk-depth-table").child(table))
        .child(Caption::new(
            "Tool names are the reference tools the estimate used; the expected stamina column \
             flags every object beyond the party's whole starting budget.",
        ))
        .into_any_element()
}

fn notes() -> AnyElement {
    Caption::new(
        "Earlier floors already spend stamina: the full starting budget is an optimistic \
         reference, not the remaining stamina at that depth. Expected cost is not a guarantee of \
         success or an exact stopping depth.",
    )
    .into_any_element()
}

fn subheading(label: &'static str) -> AnyElement {
    div().text_size(TEXT_BODY).child(label).into_any_element()
}

/// The reference tool's name, for the inspector.
fn tool_name(inputs: &Inputs, id: usize) -> String {
    inputs
        .tools
        .iter()
        .find(|tool| tool.id == id)
        .map_or_else(|| format!("Tool {id}"), |tool| tool.name.clone())
}
