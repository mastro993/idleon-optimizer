//! The Currency tab: the value table, the per-water multipliers, the desired
//! currency simulator and the next unlock tracker.
//!
//! Ported from the `#tab-currency` half of `fountain-calc.html`. The page reads
//! the save's desired currency; here the same table can simulate another choice
//! through the select and the simulator's rows, and the save is never written.

use super::overview::{num, pct};
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use idleon_fountain as fountain;
use idleon_fountain::CurrencyRow;
use idleon_fountain::Fountain;

/// The Currency pane: values, per-water multipliers, desired simulator, unlocks.
pub(super) fn currency_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    let desired = state.fountain_desired();
    div()
        .id("fountain-currency")
        .flex()
        .flex_col()
        .gap_3()
        .child(values(fountain, desired, cx))
        .child(water_multipliers(fountain, cx))
        .child(desired_simulator(fountain, desired, cx))
        .child(unlocks(fountain, cx))
        .into_any_element()
}

/// The desired currency the section simulates, or the save's own choice.
fn desired_select(fountain: &Fountain, desired: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut choices = vec![Choice::new("-1", "Save's choice")];
    for currency in 0..fountain::CURRENCIES {
        if fountain.currency_unlocked(currency) {
            choices.push(Choice::new(
                currency.to_string(),
                fountain::currency_label(currency),
            ));
        }
    }
    let saved = fountain.desired_currency();
    Select::new("fountain-currency-desired", choices)
        .label("Desired currency")
        .selected(if desired == saved {
            "-1".to_owned()
        } else {
            desired.to_string()
        })
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let desired = value.parse::<i64>().ok().filter(|value| *value >= 0);
            entity.update(cx, |state, cx| {
                state.set_fountain_desired(desired.map(|value| value as usize), cx);
            });
        })
        .into_any_element()
}

/// The Currency Values table and the multipliers behind its totals.
fn values(fountain: &Fountain, desired: usize, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let rows = fountain.currency_rows(desired);
    let mut table = Table::new([
        "Currency",
        "Base",
        "Lucky Multi",
        "Total",
        "Full Stack",
        "Royal Stack",
        "Status",
        "Create %",
    ]);
    for row in &rows {
        table = table.row([
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(fountain::currency_name(row.currency))
                .when(row.currency == desired, |cell| {
                    cell.child(Badge::new("Desired").tone(Tone::Accent))
                })
                .into_any_element(),
            number_cell(row.base),
            div()
                .flex()
                .flex_col()
                .items_end()
                .w_full()
                .child(if row.lucky > 1.0 {
                    format!("{:.2}×", row.lucky)
                } else {
                    "1.00×".to_owned()
                })
                .into_any_element(),
            bold_cell(row.total),
            number_cell(row.full_stack),
            number_cell(row.royal_stack),
            status_cell(row),
            create_cell(row),
        ]);
    }
    let saved = fountain.desired_currency();
    let caption = format!(
        "All multi {:.2}× · Royal {:.2}× · Desire {:.2}×{}",
        fountain.currency_all_multi(),
        fountain.effective_royal_multi(),
        fountain.desire_multi(),
        if desired == saved {
            " (the save's choice)".to_owned()
        } else {
            format!(" (simulating {})", fountain::currency_name(desired))
        }
    );
    Card::new()
        .header(
            CardHeader::new("Currency Values")
                .description(
                    "One coin at the simulated desired currency; every type shares the fills",
                )
                .action(desired_select(fountain, desired, cx)),
        )
        .child(
            div()
                .id("fountain-currency-values")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(Caption::new(caption).text_color(muted))
                .child(
                    Caption::new(
                        "Locked currencies cannot appear yet; ignored ones are hidden with Turn \
                         and Push. Create % is the share of fills that makes each active type.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The per-water POW-10 log table.
fn water_multipliers(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let rows = fountain.water_multipliers();
    let names = ["Mythril Transience", "Sharp Surrogate", "Rupie Monie"];
    let mut table = Table::new([
        "Water",
        "Shared All Multi",
        "Mythril Transience",
        "Sharp Surrogate",
        "Rupie Monie",
    ]);
    for row in &rows {
        let own = |water: usize| {
            if row.water == water {
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .w_full()
                    .child(format!("{:.4}×", row.own))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .w_full()
                    .child("—")
                    .into_any_element()
            }
        };
        table = table.row([
            div()
                .child(fountain::water_names()[row.water].clone())
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .items_end()
                .w_full()
                .child(format!("{}×", num(row.shared)))
                .into_any_element(),
            own(0),
            own(1),
            own(2),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Per-Water Multipliers").description(
            "The shared currency multiplier, and the POW 10 log each water's currencies add",
        ))
        .child(
            div()
                .id("fountain-water-multipliers")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(
                    Caption::new(format!(
                        "{} scale with POW 10 Mythril, Sharp Notes and Red Rupie at the \
                         Transience upgrades' saved levels.",
                        names.join(", ")
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The Optimal Desired Currency simulator; a row switches the simulation.
fn desired_simulator(
    fountain: &Fountain,
    desired: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let hover = cx.theme().colors.hover;
    let mut table = Table::new([
        "Currency",
        "Avg Coin Value",
        "Royal-Weighted",
        "Full Ground Value",
        "Status",
    ]);
    for row in fountain.desired_rows(desired) {
        let entity = cx.entity();
        let currency = row.currency;
        table = table.row([
            div()
                .id(("fountain-desired", currency))
                .role(gpui::Role::Button)
                .aria_label(format!(
                    "Simulate {} as the desired currency",
                    fountain::currency_name(currency)
                ))
                .aria_selected(row.current)
                .rounded(px(3.0))
                .px_1()
                .cursor_pointer()
                .when(row.current, |cell| cell.text_color(accent))
                .hover(move |style| style.bg(hover))
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.set_fountain_desired(Some(currency), cx);
                    });
                })
                .child(fountain::currency_name(currency).to_owned())
                .into_any_element(),
            number_cell(row.average),
            number_cell(row.weighted),
            bold_cell(row.ground),
            div()
                .flex()
                .items_center()
                .gap_1()
                .when(row.current, |cell| {
                    cell.child(Badge::new("Current").tone(Tone::Accent))
                })
                .when(row.best, |cell| {
                    cell.child(Badge::new("Best").tone(Tone::Success))
                })
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Optimal Desired Currency")
                .description("What a full ground redeems for at each unlocked desired currency"),
        )
        .child(
            div()
                .id("fountain-desired")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(
                    Caption::new(
                        "Pick a row to simulate it. Desired is a save setting; this section never \
                         writes one.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The currencies the save has not unlocked, and the upgrades that grant them.
fn unlocks(fountain: &Fountain, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let rows = fountain.next_unlocks();
    let mut card = Card::new().header(
        CardHeader::new("Next Unlock Tracker")
            .description("Every currency still locked, with the upgrade that unlocks it"),
    );
    if rows.is_empty() {
        card = card.child(
            div()
                .id("fountain-unlocks")
                .child(Caption::new("All currencies unlocked.").text_color(muted)),
        );
        return card.into_any_element();
    }
    let mut table = Table::new([
        "Currency",
        "Unlock Upgrade",
        "Water",
        "Current Lv",
        "Status",
    ]);
    for row in &rows {
        table = table.row([
            div()
                .child(fountain::currency_name(row.currency).to_owned())
                .into_any_element(),
            div().child(row.upgrade).into_any_element(),
            div()
                .child(fountain::water_names()[row.water].clone())
                .into_any_element(),
            number_cell(row.level),
            div()
                .flex()
                .items_center()
                .when(row.prerequisite_unlocked, |cell| {
                    cell.child(Badge::new("Available").tone(Tone::Warning))
                })
                .when(!row.prerequisite_unlocked, |cell| {
                    cell.child(Badge::new("Prerequisite locked").tone(Tone::Danger))
                })
                .into_any_element(),
        ]);
    }
    card.child(
        div()
            .id("fountain-unlocks")
            .flex()
            .flex_col()
            .gap_2()
            .child(table)
            .child(
                Caption::new(
                    "An Available row only needs its own levels; its prerequisite is met.",
                )
                .text_color(muted),
            ),
    )
    .into_any_element()
}

// ----- shared cells -----

/// A right-aligned number.
pub(super) fn number_cell(value: f64) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .child(num(value))
        .into_any_element()
}

/// A right-aligned, emphasised number.
fn bold_cell(value: f64) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .font_weight(FontWeight::SEMIBOLD)
        .child(num(value))
        .into_any_element()
}

/// The Locked, Active or Ignored badge.
fn status_cell(row: &CurrencyRow) -> AnyElement {
    div()
        .flex()
        .items_center()
        .when(!row.unlocked, |cell| {
            cell.child(Badge::new("Locked").tone(Tone::Danger))
        })
        .when(row.unlocked && row.active, |cell| {
            cell.child(Badge::new("Active").tone(Tone::Success))
        })
        .when(row.unlocked && !row.active, |cell| {
            cell.child(Badge::new("Ignored").tone(Tone::Neutral))
        })
        .into_any_element()
}

/// The share of fills a currency is created from, or a dash while locked.
fn create_cell(row: &CurrencyRow) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w_full()
        .child(if row.unlocked {
            pct(row.create)
        } else {
            "—".to_owned()
        })
        .into_any_element()
}
