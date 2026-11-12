//! The current opal state: the six overview cards and the villagers table.

use crate::state::AppState;
use crate::ui::layout;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div};
use idleon_opal::Opal;
use idleon_opal::{NAMES, VILLAGERS};
use idleon_ui::format;

/// The overview row: unlocked villagers, the opal totals and the two cosmo
/// bonuses, then the total EXP the save earns.
pub(crate) fn current_state(opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let opals = opal.opals();
    let total_opals = opal.total_opals();
    let min_opals = opal.min_opals(&opals);
    let spread = opal.equal_spread(&opals);
    let total = opal.total_exp_rate(&opals);
    let label = format!(
        "{} unlocked · {} total opals · {} min opals · equal spread {} · total {}",
        opal.unlocked_count(),
        format::value(total_opals),
        format::value(min_opals),
        format::percent(spread / 100.0),
        format::rate(total)
    );
    div()
        .id("opal-current")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("opal-current-line")
                .role(Role::Label)
                .aria_label(label),
        )
        .child(
            div()
                .id("opal-current-row")
                .flex()
                .flex_wrap()
                .gap_3()
                .child(layout::figure(
                    "opal-kpi-unlocked",
                    "Unlocked",
                    opal.unlocked_count().to_string(),
                    "Villagers at level 1 or above",
                    cx,
                ))
                .child(layout::figure(
                    "opal-kpi-total-opals",
                    "Total Opals",
                    format::value(total_opals),
                    "Invested across the villagers",
                    cx,
                ))
                .child(layout::figure(
                    "opal-kpi-min-opals",
                    "Min Opals",
                    format::value(min_opals),
                    "The lowest unlocked villager",
                    cx,
                ))
                .child(layout::figure(
                    "opal-kpi-equal-spread",
                    "Equal Spread Bonus",
                    format::percent(spread / 100.0),
                    format!("Cosmo Lv {}", opal.cosmo_level(1, 5) as i64),
                    cx,
                ))
                .child(layout::figure(
                    "opal-kpi-enthusiasm",
                    "Opal Enthusiasm",
                    format::percent(opal.ext.enthusiasm / 100.0),
                    format!("per 10 opals · Cosmo Lv {}", opal.cosmo_level(1, 0) as i64),
                    cx,
                ))
                .child(layout::figure(
                    "opal-kpi-total",
                    "Total EXP/hr",
                    format::rate(total),
                    "Every unlocked villager, added",
                    cx,
                )),
        )
        .into_any_element()
}

/// The villagers table, in save order, with each villager's share of the total.
///
/// An unlocked villager's row switches the breakdown to that villager; the
/// card's action switches it back to the total.
pub(crate) fn villagers(state: &AppState, opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let opals = opal.opals();
    let total = opal.total_exp_rate(&opals);
    let mut table = Table::new([
        "Villager",
        "Level",
        "Stars",
        "Opals",
        "EXP/hr",
        "% of Total",
    ]);
    for (villager, name) in NAMES.iter().enumerate().take(VILLAGERS) {
        let saved = opal.villagers[villager];
        let rate = opal.exp_rate(villager, &opals);
        let share = opal.share(villager);
        let selected = state.opal_selected == Some(villager);
        let mut label = div().child(*name);
        if !opal.unlocked(villager) {
            label = label.child(div().pl_2().child(Badge::new("Locked").tone(Tone::Neutral)));
        }
        if selected {
            label = label.child(div().pl_2().child(Badge::new("Breakdown").tone(Tone::Info)));
        }
        let label: AnyElement = if opal.unlocked(villager) {
            let entity = cx.entity();
            div()
                .id(format!("opal-villager-{villager}"))
                .role(Role::Button)
                .aria_label(format!("{}: break the tree down for this villager", name))
                .cursor_pointer()
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.set_opal_villager(Some(villager), cx));
                })
                .flex()
                .items_center()
                .child(label)
                .into_any_element()
        } else {
            label.into_any_element()
        };
        table = table.row([
            label,
            div().child(format::value(saved.level)).into_any_element(),
            div()
                .child(format!("★{}", saved.stars as i64))
                .into_any_element(),
            div().child(format::value(saved.opals)).into_any_element(),
            div().child(format::rate(rate)).into_any_element(),
            div()
                .child(if total > 0.0 {
                    format::percent(share)
                } else {
                    "-".to_owned()
                })
                .into_any_element(),
        ]);
    }
    let entity = cx.entity();
    let total_control = CardHeader::new("Villagers")
        .description("Level, stars and invested opals from the save, with the rate each one earns")
        .action(
            Button::new("opal-villager-all", "Show total")
                .size(ControlSize::Sm)
                .variant(if state.opal_selected.is_some() {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.set_opal_villager(None, cx));
                }),
        );
    Card::new()
        .header(total_control)
        .child(
            div()
                .id("opal-villagers")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(
                    Caption::new(format!(
                        "Press a villager to break its own rate down; the total adds them up. \
                         A villager earns nothing without opals, every star doubles its EXP, and \
                         the lowest unlocked villager is at {} opals. Showing: {}.",
                        format::value(opal.min_opals(&opals)),
                        selection(state)
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// What the formula tree is currently rooted at.
fn selection(state: &AppState) -> String {
    match state.opal_selected {
        Some(villager) => format!(
            "{}'s own rate",
            NAMES.get(villager).copied().unwrap_or("Villager")
        ),
        None => "the total across the unlocked villagers".to_owned(),
    }
}
