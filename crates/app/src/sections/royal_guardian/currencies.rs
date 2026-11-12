//! The Currencies tab: each slot's balance, the nodes that pay it and what the
//! daily reset collects.
//!
//! Upstream splits the tab into "Sources" and a "Collection Breakdown" tree; the
//! tree belongs to RG-03, so this pane shows the sources with the same figures
//! the tree resolves to.

use super::overview::{figure_text, kpi, number_cell, rate_cell};
use super::world_rail;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div, px};
use idleon_core::RoyalGuardian;
use idleon_core::royal_guardian::{self, CurrencyIncome};

/// The Currencies pane.
pub fn currencies(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let world = state.royal_state.world.min(royal_guardian::WORLDS - 1);
    let slots = royal_guardian::currency_slots();
    let slot = selected_currency(&slots, state.royal_state.currency);
    let income = royal.currency_income(slot);
    div()
        .id("royal-currencies")
        .flex()
        .flex_col()
        .gap_3()
        .child(world_rail(world, cx))
        .child(picker(&slots, slot, cx))
        .child(summary(&income))
        .child(source_table(royal, &income))
        .child(partials(&income, cx))
        .into_any_element()
}

/// The currency slot selector.
fn picker(slots: &[i64], slot: i64, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let choices: Vec<Choice> = slots
        .iter()
        .map(|value| Choice::new(value.to_string(), format!("Currency slot {value}")))
        .collect();
    let select = Select::new("royal-currency", choices)
        .label("Currency")
        .selected(slot.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(slot) = value.parse::<i64>() {
                entity.update(cx, |state, cx| state.set_royal_currency(slot, cx));
            }
        });
    div()
        .id("royal-currency-picker")
        .flex()
        .items_end()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(220.0))
                .child(select),
        )
        .into_any_element()
}

/// The inputs this save or this port leaves out of the rate, when any are.
fn partials(income: &CurrencyIncome, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    if !income.partial {
        return div().id("royal-currency-partial").into_any_element();
    }
    let mut lines = div().flex().flex_col().gap_1();
    for source in &income.missing {
        lines = lines.child(Caption::new(source.clone()).text_color(muted));
    }
    Card::new()
        .header(
            CardHeader::new("Rate inputs left out")
                .description("These factors read as one instead of being guessed"),
        )
        .child(div().id("royal-currency-partial").p_3().child(lines))
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
    div()
        .id("royal-currency-summary")
        .flex()
        .flex_wrap()
        .gap_3()
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
            div()
                .id("royal-currency-reset-note")
                .role(gpui::Role::Label)
                .aria_label(reset_caption.clone())
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Reset"))
                .child(reset_caption),
        )
        .into_any_element()
}

/// The nodes that pay the selected slot.
fn source_table(royal: &RoyalGuardian, income: &CurrencyIncome) -> AnyElement {
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
    let mut table = Table::new([
        "Resource",
        "Grade",
        "Remaining",
        "State",
        "Nominal /hr",
        "Collectable",
        "Reset",
    ]);
    for node in &income.nodes {
        let resource = node.resource;
        let detail = &node.allocation;
        let state = if detail.drained {
            Badge::new("Drained").tone(Tone::Warning)
        } else if detail.remaining == 0.0 {
            Badge::new("Empty").tone(Tone::Neutral)
        } else if detail.rate > 0.0 {
            Badge::new("Draining").tone(Tone::Success)
        } else {
            Badge::new("No stream").tone(Tone::Danger)
        };
        table = table.row([
            div()
                .id(format!("royal-currency-node-{resource}"))
                .child(format!("Resource {}", resource + 1))
                .into_any_element(),
            number_cell(royal.resource_grade(resource)),
            div()
                .flex()
                .flex_col()
                .items_end()
                .w_full()
                .child(format!(
                    "{} / {}",
                    format::value(detail.remaining),
                    format::value(royal.resource_capacity(resource))
                ))
                .into_any_element(),
            div().child(state).into_any_element(),
            rate_cell(node.currency_rate),
            number_cell(node.collectable_within_window),
            div()
                .child(match detail.refill_boundary {
                    "saved-drained" => "Refills".to_owned(),
                    "collect-before-reset" => "Collect first".to_owned(),
                    _ => "—".to_owned(),
                })
                .into_any_element(),
        ]);
    }
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
