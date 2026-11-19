//! The current opal state: the six overview cards and the villagers table.

use super::table::{Col, figure, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use idleon_opal::Opal;
use idleon_opal::{NAMES, VILLAGERS};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, KpiStrip, PANEL_GAP, Tone};

/// The overview row: unlocked villagers, the opal totals and the two cosmo
/// bonuses, then the total EXP the save earns.
pub(crate) fn current_state(opal: &Opal) -> AnyElement {
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
    v_flex()
        .id("opal-current")
        .gap(PANEL_GAP)
        .child(
            div()
                .id("opal-current-line")
                .role(Role::Label)
                .aria_label(label),
        )
        .child(
            v_flex()
                .id("opal-current-row")
                .gap(PANEL_GAP)
                .child(
                    KpiStrip::new()
                        .child(figure(
                            "opal-kpi-unlocked",
                            "Unlocked",
                            opal.unlocked_count().to_string(),
                            "Villagers at level 1 or above",
                        ))
                        .child(figure(
                            "opal-kpi-total-opals",
                            "Total Opals",
                            format::value(total_opals),
                            "Invested across the villagers",
                        ))
                        .child(figure(
                            "opal-kpi-min-opals",
                            "Min Opals",
                            format::value(min_opals),
                            "The lowest unlocked villager",
                        )),
                )
                .child(
                    KpiStrip::new()
                        .child(figure(
                            "opal-kpi-equal-spread",
                            "Equal Spread Bonus",
                            format::percent(spread / 100.0),
                            format!("Cosmo Lv {}", opal.cosmo_level(1, 5) as i64),
                        ))
                        .child(figure(
                            "opal-kpi-enthusiasm",
                            "Opal Enthusiasm",
                            format::percent(opal.ext.enthusiasm / 100.0),
                            format!("per 10 opals · Cosmo Lv {}", opal.cosmo_level(1, 0) as i64),
                        ))
                        .child(figure(
                            "opal-kpi-total",
                            "Total EXP/hr",
                            format::rate(total),
                            "Every unlocked villager, added",
                        )),
                ),
        )
        .into_any_element()
}

/// The villagers table, in save order, with each villager's share of the total.
///
/// An unlocked villager's row switches the breakdown to that villager; the
/// card's action switches it back to the total.
pub(crate) fn villagers(state: &AppState, opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let opals = opal.opals();
    let total = opal.total_exp_rate(&opals);
    let entity = cx.entity();
    let rows = NAMES
        .iter()
        .enumerate()
        .take(VILLAGERS)
        .map(|(villager, name)| {
            let saved = opal.villagers[villager];
            let unlocked = opal.unlocked(villager);
            let label = h_flex()
                .gap_2()
                .child(*name)
                .when(!unlocked, |label| label.child(tag("Locked", Tone::Neutral)))
                .when(state.opal.selected == Some(villager), |label| {
                    label.child(tag("Breakdown", Tone::Info))
                });
            let label: AnyElement = if unlocked {
                let entity = entity.clone();
                div()
                    .id(format!("opal-villager-{villager}"))
                    .role(Role::Button)
                    .aria_label(format!("{}: break the tree down for this villager", name))
                    .cursor_pointer()
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.opal.set_villager(Some(villager), cx));
                    })
                    .child(label)
                    .into_any_element()
            } else {
                label.into_any_element()
            };
            [
                label,
                text(format::value(saved.level)),
                text(format!("★{}", saved.stars as i64)),
                text(format::value(saved.opals)),
                text(format::rate(opal.exp_rate(villager, &opals))),
                text(if total > 0.0 {
                    format::percent(opal.share(villager))
                } else {
                    "-".to_owned()
                }),
            ]
        });
    let table = table(
        "opal-villagers",
        &[
            Col::Text("Villager"),
            Col::Number("Level"),
            Col::Number("Stars"),
            Col::Number("Opals"),
            Col::Number("EXP/hr"),
            Col::Number("% of Total"),
        ],
        rows,
        cx,
    );
    let show_total = entity.clone();
    let header = CardHeader::new("Villagers")
        .description("Level, stars and invested opals from the save, with the rate each one earns")
        .action(
            Button::new("opal-villager-all")
                .label("Show total")
                .small()
                .when(state.opal.selected.is_none(), |button| button.ghost())
                .on_click(move |_event, _window, cx| {
                    show_total.update(cx, |state, cx| state.opal.set_villager(None, cx));
                }),
        );
    Card::new()
        .header(header)
        .child(
            v_flex()
                .id("opal-villagers")
                .gap_2()
                .child(table)
                .child(note(
                    format!(
                        "Press a villager to break its own rate down; the total adds them up. \
                         A villager earns nothing without opals, every star doubles its EXP, and \
                         the lowest unlocked villager is at {} opals. Showing: {}.",
                        format::value(opal.min_opals(&opals)),
                        selection(state)
                    ),
                    cx,
                )),
        )
        .into_any_element()
}

/// What the formula tree is currently rooted at.
fn selection(state: &AppState) -> String {
    match state.opal.selected {
        Some(villager) => format!(
            "{}'s own rate",
            NAMES.get(villager).copied().unwrap_or("Villager")
        ),
        None => "the total across the unlocked villagers".to_owned(),
    }
}
