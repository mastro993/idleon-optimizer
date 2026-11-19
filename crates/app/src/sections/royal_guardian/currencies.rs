//! The Currencies tab: each slot's balance, the nodes that pay it and what the
//! daily reset collects.
//!
//! The "Collection Breakdown" tree belongs to RG-03, so this pane shows the
//! sources with the same figures the tree resolves to.

use super::overview::{figure_text, kpi, number_cell, rate_cell, tag};
use super::table::{table, text, text_id};
use super::world_rail;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div, px};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Sizable as _, v_flex};
use idleon_royal_guardian::RoyalGuardian;
use idleon_royal_guardian::{self as royal_guardian, CurrencyIncome};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, TableColumn, Tone,
};

/// The Currencies pane.
pub(crate) fn currencies(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let slots = royal_guardian::currency_slots();
    let slot = selected_currency(&slots, state.royal_state.currency);
    let income = state.royal_controls.derived.income(royal, slot);
    v_flex()
        .id("royal-currencies")
        .gap(PANEL_GAP)
        .child(world_rail(world, cx))
        .child(picker(state, &slots, slot, window, cx))
        .child(summary(&income))
        .child(source_table(royal, &income, cx))
        .child(partials(&income))
        .into_any_element()
}

/// The currency slot selector.
fn picker(
    state: &AppState,
    slots: &[i64],
    slot: i64,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let choices: Vec<Choice<i64>> = slots
        .iter()
        .map(|value| Choice::new(*value, format!("Currency slot {value}")))
        .collect();
    let select = &state.royal_controls.currency;
    select.show(choices, Some(&slot), window, cx);
    div()
        .id("royal-currency-picker")
        .child(
            Select::new(select.state())
                .id("royal-currency")
                .accessibility_label("Currency")
                .small()
                .w(px(220.0)),
        )
        .into_any_element()
}

/// The inputs this save or this build leaves out of the rate, when any are.
fn partials(income: &CurrencyIncome) -> AnyElement {
    if !income.partial {
        return div().id("royal-currency-partial").into_any_element();
    }
    Card::new()
        .header(
            CardHeader::new("Rate inputs left out")
                .description("These factors read as one instead of being guessed"),
        )
        .child(
            v_flex()
                .id("royal-currency-partial")
                .gap_1()
                .children(income.missing.iter().cloned().map(Caption::new)),
        )
        .into_any_element()
}

/// The selected slot's headline figures.
fn summary(income: &CurrencyIncome) -> AnyElement {
    let reset_caption = if income.reset.available {
        match income.reset.hours_remaining {
            Some(hours) => format!("{} until the daily reset", format::hours(hours)),
            None => "No reset countdown".to_owned(),
        }
    } else {
        "The save carries no TimeAway.ShopRestock".to_owned()
    };
    let before_reset = income
        .collectable_before_reset
        .map(figure_text)
        .unwrap_or_else(|| "—".to_owned());
    let at_reset = income
        .projected_available_at_reset
        .map(figure_text)
        .unwrap_or_else(|| "—".to_owned());
    let strip = KpiStrip::new()
        .child(kpi(
            "royal-kpi-currency-balance",
            "Balance",
            income.balance,
            "",
            format!("Currency slot {}", income.slot),
        ))
        .child(kpi(
            "royal-kpi-currency-rate",
            "Nominal /hr",
            income.nominal_per_hour,
            "",
            format!(
                "{} · {}",
                plural(income.active_streams, "stream"),
                plural(income.active_nodes, "node")
            ),
        ))
        .child(kpi(
            "royal-kpi-currency-window",
            "Collectable in window",
            income.collectable_within_window,
            "",
            format!(
                "{} remaining now",
                format::value(income.remaining_now.unwrap_or_default())
            ),
        ))
        .child(kpi(
            "royal-kpi-currency-reset",
            "Collectable before reset",
            income.collectable_before_reset.unwrap_or(0.0),
            "",
            format!("{before_reset} by the reset · {at_reset} after it"),
        ))
        .child(
            v_flex()
                .id("royal-currency-reset-note")
                .role(gpui::Role::Label)
                .aria_label(reset_caption.clone())
                .gap_1()
                .child(Caption::new("Reset"))
                .child(reset_caption),
        );
    div()
        .id("royal-currency-summary")
        .child(strip)
        .into_any_element()
}

/// The nodes that pay the selected slot.
fn source_table(
    royal: &RoyalGuardian,
    income: &CurrencyIncome,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let card = Card::new().header(CardHeader::new(format!(
        "Currency slot {} source nodes",
        income.slot
    )));
    if income.nodes.is_empty() {
        return card
            .child(
                div()
                    .id("royal-currency-nodes-empty")
                    .child(Caption::new("No resource node produces this currency.")),
            )
            .into_any_element();
    }
    let rows = income.nodes.iter().map(|node| {
        let resource = node.resource;
        let detail = &node.allocation;
        let state = if detail.drained {
            tag("Drained", Tone::Warning)
        } else if detail.remaining == 0.0 {
            tag("Empty", Tone::Neutral)
        } else if detail.rate > 0.0 {
            tag("Draining", Tone::Success)
        } else {
            tag("No stream", Tone::Danger)
        };
        vec![
            text_id(
                SharedString::from(format!("royal-currency-node-{resource}")),
                format!("Resource {}", resource + 1),
            ),
            number_cell(royal.resource_grade(resource)),
            text(format!(
                "{} / {}",
                format::value(detail.remaining),
                format::value(royal.resource_capacity(resource))
            )),
            state.into_any_element(),
            rate_cell(node.currency_rate),
            number_cell(node.collectable_within_window),
            text(match detail.refill_boundary {
                "saved-drained" => "Refills",
                "collect-before-reset" => "Collect first",
                _ => "—",
            }),
        ]
    });
    let table = table(
        "royal-currency-node",
        &[
            TableColumn::text("Resource"),
            TableColumn::number("Grade"),
            TableColumn::number("Remaining"),
            TableColumn::text("State"),
            TableColumn::number("Nominal /hr"),
            TableColumn::number("Collectable"),
            TableColumn::text("Reset"),
        ],
        rows,
        cx,
    );
    card.child(div().id("royal-currency-nodes").child(table))
        .into_any_element()
}

/// A count and its word, with the plural the count asks for.
fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{count} {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// The slot the pane lists: the saved one while a node pays it, else the first.
pub(super) fn selected_currency(slots: &[i64], saved: Option<i64>) -> i64 {
    if let Some(saved) = saved
        && slots.contains(&saved)
    {
        return saved;
    }
    slots.first().copied().unwrap_or(0)
}
