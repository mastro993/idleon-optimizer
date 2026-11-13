//! The manual simulator tab: an opal count per unlocked villager and the rate
//! the split earns against the save.
//!
//! `opal-calc.html` keeps this in `renderSimulatorTab` and `recalcSim`: one
//! number input per unlocked villager, a warning while the typed total leaves
//! the save's, the lowest count and the Equal Spread it buys, and each
//! villager's rate against what it earns today. The section adds the reset the
//! contract asks for. Every figure is [`idleon_opal::Simulation`]; this module
//! only draws it.

use super::MAX_OPALS;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_opal::{NAMES, Opal, Simulation, VILLAGERS};
use idleon_ui::format;

/// The pane: the typed counts over the rates they earn.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.clone() else {
        return div().into_any_element();
    };
    let simulation = opal.simulation(&state.opal_manual);
    ScrollArea::new("opal-manual-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("opal-manual")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(inputs(state, &opal, cx))
                .child(result(&opal, &simulation, cx)),
        )
        .into_any_element()
}

/// One whole opal count per unlocked villager, and the reset to the save.
fn inputs(state: &AppState, opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let saved = opal.opals();
    let mut counts = div().id("opal-manual-inputs").flex().flex_wrap().gap_3();
    for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
        let entity = cx.entity();
        counts = counts.child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_1()
                .child(
                    Caption::new(format!(
                        "{} (★{})",
                        NAMES[villager], opal.villagers[villager].stars as i64
                    ))
                    .text_color(muted),
                )
                .child(
                    NumberInput::new(
                        format!("opal-manual-{villager}"),
                        state.opal_manual[villager],
                    )
                    .range(0.0, MAX_OPALS.max(opal.total_opals()))
                    .step(1.0)
                    .precision(0)
                    .size(ControlSize::Sm)
                    .label(format!("{} opals", NAMES[villager]))
                    .on_change(move |value, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.set_opal_manual(villager, value, cx);
                        });
                    }),
                ),
        );
    }
    let entity = cx.entity();
    let reset = Button::new("opal-manual-reset", "Use saved opals")
        .icon(IconName::RotateCcw)
        .size(ControlSize::Sm)
        .disabled(state.opal_manual == saved)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.reset_opal_manual(cx));
        });
    Card::new()
        .header(
            CardHeader::new("Manual Opal Simulator")
                .description(
                    "Edit opal counts to see how EXP changes. Total must stay the same for fair \
                     comparison.",
                )
                .action(reset),
        )
        .child(div().p_3().child(counts))
        .into_any_element()
}

/// The typed split's total, the warning while it leaves the save's total and
/// each villager's rate against what it earns today.
fn result(opal: &Opal, simulation: &Simulation, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let spent = simulation.spent();
    let saved = simulation.saved_spent();
    let total = simulation.total(opal);
    let lowest = simulation.min_opals(opal);
    let spread = simulation.equal_spread(opal);
    let change = simulation.total_change(opal);
    let label = format!(
        "{} opals typed of the save's {} · lowest {} · equal spread {} · total {} · {} against \
         the saved split",
        format::value(spent),
        format::value(saved),
        format::value(lowest),
        format::percent(spread / 100.0),
        format::rate(total),
        signed(change)
    );

    let mut table = Table::new(["Villager", "Opals", "EXP/hr", "vs Saved"]);
    for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
        let count = simulation.opals[villager];
        let rate = simulation.rate(opal, villager);
        let reading = signed(simulation.change(opal, villager));
        table = table.row([
            div()
                .id(format!("opal-manual-row-{villager}"))
                .role(Role::Label)
                .aria_label(format!(
                    "{}: {} opals · {} EXP/hr · {} vs the saved split",
                    NAMES[villager],
                    format::value(count),
                    format::rate(rate),
                    reading
                ))
                .child(NAMES[villager])
                .into_any_element(),
            div().child(format::value(count)).into_any_element(),
            div().child(format::rate(rate)).into_any_element(),
            div().child(reading).into_any_element(),
        ]);
    }

    let mut card = Card::new()
        .header(CardHeader::new("Simulated Split").description(
            "One whole opal count per unlocked villager, read against the save's own split",
        ))
        .child(
            div()
                .id("opal-manual-summary")
                .role(Role::Label)
                .aria_label(label)
                .p_3(),
        );
    if !simulation.balanced() {
        card = card.child(
            div()
                .id("opal-manual-warning")
                .role(Role::Label)
                .aria_label(format!(
                    "Total opals: {} (the save holds {})",
                    format::value(spent),
                    format::value(saved)
                ))
                .px_3()
                .child(
                    Callout::new(Severity::Warning)
                        .title(format!(
                            "Total opals: {} · the save holds {}",
                            format::value(spent),
                            format::value(saved)
                        ))
                        .child(
                            Caption::new(
                                "Only a split of the save's own total compares fairly; every rate \
                                 below still reads the counts as typed.",
                            )
                            .into_any_element(),
                        ),
                ),
        );
    }
    card.child(
        div()
            .id("opal-manual-figures")
            .flex()
            .flex_wrap()
            .gap_3()
            .px_3()
            .child(crate::ui::layout::figure(
                "opal-manual-total",
                "Total EXP/hr",
                format::rate(total),
                "Every unlocked villager at the typed counts",
                cx,
            ))
            .child(crate::ui::layout::figure(
                "opal-manual-change",
                "vs Saved",
                signed(change),
                "The typed total over the saved split",
                cx,
            ))
            .child(crate::ui::layout::figure(
                "opal-manual-lowest",
                "Lowest Villager",
                format::value(lowest),
                "What Equal Spread scales with",
                cx,
            ))
            .child(crate::ui::layout::figure(
                "opal-manual-spread",
                "Equal Spread Bonus",
                format::percent(spread / 100.0),
                "At the typed lowest count",
                cx,
            )),
    )
    .child(div().id("opal-manual-table").min_w_0().p_3().child(table))
    .child(
        div().px_3().pb_3().child(
            Caption::new(format!(
                "The save earns {} EXP/hr across the unlocked villagers. Opals are whole, never \
             negative, and a villager earns nothing without any.",
                format::rate(opal.total_exp_rate(&opal.opals()))
            ))
            .text_color(muted),
        ),
    )
    .into_any_element()
}

/// A relative change as a signed percentage; a villager that earned nothing has
/// no share to report and reads as a dash.
fn signed(change: Option<f64>) -> String {
    match change {
        Some(change) if change > 0.0 => format!("+{}", format::percent(change)),
        Some(change) => format::percent(change),
        None => "—".to_owned(),
    }
}
