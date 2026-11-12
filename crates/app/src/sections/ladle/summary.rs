//! The Ladles tab: the panel inputs, the summary cards, the character rates and
//! the breakdowns.

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::{Checkbox, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_ladle::{self as ladle, Ladle, Panel, Stats};
use idleon_ui::format;

/// The header's one-line summary.
pub(crate) fn line(ladle: &Ladle, panel: Panel, claim_hours: f64, assigned: &[usize]) -> String {
    let stats = all(ladle, panel, claim_hours);
    let guaranteed = total(&stats, assigned, |row| row.min_per_day);
    let expected = total(&stats, assigned, |row| row.per_day);
    let reset = match ladle.reset_hours {
        Some(hours) => format!("{} to reset", format::hours(hours)),
        None => "reset countdown missing".into(),
    };
    format!(
        "{} ladles owned · {} guaranteed / day · {} expected · {reset}",
        format::number(ladle.ladles.value),
        format::number(guaranteed),
        format::number(expected),
    )
}

/// The whole Ladles tab.
pub(crate) fn pane(
    state: &mut AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let stats = all(
        ladle,
        state.ladle_state.panel,
        state.ladle_state.claim_hours,
    );
    div()
        .id("ladle-ladles-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("ladle-ladles-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(panel_card(state, window, cx))
                        .child(cards(state, ladle, &stats))
                        .child(characters(state, ladle, &stats, cx))
                        .child(super::breakdowns::ladles(state, &stats, cx)),
                ),
        )
        .into_any_element()
}

/// The stat-engine values the section asks for until the shared engine lands.
fn panel_card(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let panel = state.ladle_state.panel;
    let mut controls = div().id("ladle-panel").flex().flex_wrap().gap_3();
    let fields: [(&'static str, f64, f64, f64, f64); 5] = [
        ("Cooking efficiency", panel.efficiency, 0.0, 1e15, 100.0),
        ("AFK gain rate", panel.afk, 0.0, 100.0, 0.05),
        ("EXP multiplier", panel.exp_multi, 0.0, 1000.0, 0.05),
        ("Skill prowess", panel.prowess, 0.0, 0.1, 0.001),
        ("Talent 148 %", panel.talent_148, 0.0, 10_000.0, 1.0),
    ];
    for (id, value, min, max, step) in fields {
        let entity = cx.entity();
        let label = match id {
            "Cooking efficiency" => "efficiency",
            "AFK gain rate" => "afk",
            "EXP multiplier" => "exp",
            "Skill prowess" => "prowess",
            _ => "talent",
        };
        controls = controls.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(id).text_color(cx.theme().colors.fg_muted))
                .child(
                    Stepper::new(format!("ladle-panel-{label}"), value)
                        .range(min, max)
                        .step(step)
                        .precision(if step < 0.01 { 3 } else { 2 })
                        .on_change(move |value, _window, cx| {
                            entity.update(cx, |state, cx| {
                                let panel = &mut state.ladle_state.panel;
                                match label {
                                    "efficiency" => panel.efficiency = value,
                                    "afk" => panel.afk = value,
                                    "exp" => panel.exp_multi = value,
                                    "prowess" => panel.prowess = value,
                                    _ => panel.talent_148 = value,
                                }
                                cx.notify();
                            });
                        }),
                ),
        );
    }
    let note = Callout::new(Severity::Info)
        .title("AFK panel values")
        .child(
            Caption::new(
                "Cooking efficiency, the AFK gain rate, the EXP multiplier, skill prowess and Talent 148 come \
                 from the game's Cooking panel: the shared account stats engine is not built yet, so they are \
                 entered here and apply to every character. The values you read in game give the exact upstream \
                 rates; the neutral defaults are a lower bound.",
            )
            .text_color(cx.theme().colors.fg_muted),
        );
    div()
        .id("ladle-panel-card")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls)
        .child(note)
        .into_any_element()
}

/// The upstream summary cards.
fn cards(state: &AppState, ladle: &Ladle, stats: &[Stats]) -> AnyElement {
    let selected = &state.ladle_state.assigned;
    let guaranteed = total(stats, selected, |row| row.min_per_day);
    let expected = total(stats, selected, |row| row.per_day);
    let checked = selected.len();
    let ladle_hours = state.ladle_state.panel.ladle_hours();
    let prowess = state.ladle_state.panel.prowess;
    let owned = match ladle.ladles.partial {
        true => format!("{} · partial", format::number(ladle.ladles.value)),
        false => format::number(ladle.ladles.value),
    };
    let reset_hours = ladle.reset_hours.unwrap_or(24.0);
    let reset_caption = match ladle.reset_hours {
        Some(_) => "Claim the ladles before it; the reset applies NMLB",
        None => "The save has no reset countdown; assuming a day",
    };
    let reset_badge = match ladle.reset_hours {
        Some(_) => None,
        None => Some(Badge::new("assumed").tone(Tone::Warning)),
    };
    let mut row = div()
        .id("ladle-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("ladle-kpi-owned", "Ladles owned", ladle.ladles.value).decimals(0),
            )
            .caption(owned),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "ladle-kpi-guaranteed",
                    "Guaranteed ladles / day",
                    guaranteed,
                )
                .decimals(0),
            )
            .caption(format!(
                "{checked} checked characters · claims may give ~{} with the 19c roll",
                format::number(expected)
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new("ladle-kpi-reset", "Daily reset", reset_hours)
                    .decimals(1)
                    .suffix("h"),
            )
            .caption(reset_caption),
        )
        .child(
            KpiCard::new(
                Statistic::new("ladle-kpi-hours", "Ladle hours", ladle_hours)
                    .suffix("×")
                    .decimals(3),
            )
            .caption(format!(
                "Talent 148 Overflowing Ladle {}%",
                format::number(state.ladle_state.panel.talent_148)
            )),
        )
        .child(
            KpiCard::new(Statistic::new("ladle-kpi-prowess", "Skill prowess", prowess).decimals(3))
                .caption("Account-wide · ladle exponent 0.25 + min(0.1, prowess)"),
        );
    if let Some(badge) = reset_badge {
        row = row.child(div().id("ladle-reset-note").child(badge));
    }
    row.into_any_element()
}

/// The character table: who counts, and what each row's rates are.
fn characters(
    state: &AppState,
    ladle: &Ladle,
    stats: &[Stats],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut table = Table::new([
        "Use",
        "Character",
        "Cook Lv",
        "Efficiency",
        "AFK rate",
        "Ladle multi",
        "Panel / day",
        "Guaranteed / day",
        "Expected / day",
        "/ hr",
        "Next gain",
    ]);
    for row in stats {
        let index = row.index;
        let entity = cx.entity();
        let checked = state.ladle_state.is_assigned(index);
        let use_cell = Checkbox::new(("ladle-use", index), checked)
            .label(format!("Count {}", row.name))
            .on_change(move |_, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.ladle_state.toggle_assigned(index);
                    cx.notify();
                });
            });
        let mut name = div().flex().items_center().gap_2().child(row.name.clone());
        if row.cooking {
            name = name.child(Badge::new("cooking").tone(Tone::Success));
        }
        let guaranteed = match row.claim_capped {
            true => div()
                .flex()
                .items_center()
                .gap_1()
                .child(format::number(row.min_per_day))
                .child(Badge::new("10h cap").tone(Tone::Danger))
                .into_any_element(),
            false => div()
                .child(format::number(row.min_per_day))
                .into_any_element(),
        };
        let mut guaranteed_cell = div().flex().flex_col().child(guaranteed);
        if row.pending_known {
            guaranteed_cell = guaranteed_cell.child(
                Caption::new(format!("{} pending", format::hours(row.pending_hours)))
                    .text_color(cx.theme().colors.fg_muted),
            );
        }
        table = table.row([
            use_cell.into_any_element(),
            name.into_any_element(),
            div().child(format::number(row.level)).into_any_element(),
            div()
                .child(format::number(row.efficiency))
                .into_any_element(),
            div().child(format!("×{:.2}", row.afk)).into_any_element(),
            div()
                .child(format!("×{}", format::number(row.ladle_multi)))
                .into_any_element(),
            div()
                .child(format::number(row.panel_per_day))
                .into_any_element(),
            guaranteed_cell.into_any_element(),
            div().child(format::number(row.per_day)).into_any_element(),
            div().child(format::number(row.per_hour)).into_any_element(),
            div().child(row.outlook_label()).into_any_element(),
        ]);
    }
    let selected = &state.ladle_state.assigned;
    let guaranteed = total(stats, selected, |row| row.min_per_day);
    let expected = total(stats, selected, |row| row.per_day);
    table = table.row([
        div().into_any_element(),
        div()
            .child(format!("Checked total ({} characters)", selected.len()))
            .into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().into_any_element(),
        div().child(format::number(guaranteed)).into_any_element(),
        div().child(format::number(expected)).into_any_element(),
        div()
            .child(format::number(expected / 24.0))
            .into_any_element(),
        div().into_any_element(),
    ]);
    let note = match ladle.reset_hours {
        Some(hours) => format!(
            "Each claim may give more (Post Office 19c doubling), so expect extra ladles roughly half the time. \
             Next reset in {}.",
            format::hours(hours),
        ),
        None => "Each claim may give more (Post Office 19c doubling), so expect extra ladles roughly half the \
                 time. The save carries no reset countdown."
            .to_owned(),
    };
    Card::new()
        .header(CardHeader::new("Characters").description(
            "Check the characters AFK-ing Cooking to count their ladles · rates assume Cooking as the AFK target",
        ))
        .child(
            div()
                .id("ladle-characters")
                .role(Role::Label)
                .aria_label(format!(
                    "{} characters, {} checked, {} guaranteed ladles a day",
                    stats.len(),
                    selected.len(),
                    format::number(guaranteed)
                ))
                .child(table),
        )
        .child(
            div().px_3().pb_3().child(
                Caption::new(note).text_color(cx.theme().colors.fg_muted),
            ),
        )
        .into_any_element()
}

/// Every character's rates for the panel the section holds.
pub(crate) fn all(ladle: &Ladle, panel: Panel, claim_hours: f64) -> Vec<Stats> {
    ladle
        .characters
        .iter()
        .map(|character| ladle::stats(character, panel, claim_hours))
        .collect()
}

/// Sum one column over the checked characters.
pub(crate) fn total(stats: &[Stats], selected: &[usize], column: impl Fn(&Stats) -> f64) -> f64 {
    stats
        .iter()
        .filter(|row| selected.contains(&row.index))
        .map(column)
        .sum()
}
