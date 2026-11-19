//! The manual simulator tab: an opal count per unlocked villager and the rate
//! the split earns against the save.
//!
//! One number input per unlocked villager, a warning while the typed total
//! leaves the save's, the lowest count and the Equal Spread it buys, and each
//! villager's rate against what it earns today. The section adds the reset the
//! contract asks for. Every figure is [`idleon_opal::Simulation`]; this module
//! only draws it.

use super::table::{Col, figure, note, table, text};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_opal::{NAMES, Opal, Simulation, VILLAGERS};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Tone, show_number,
};

/// The pane: the typed counts over the rates they earn.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(opal) = state.opal.save.clone() else {
        return div().into_any_element();
    };
    let simulation = opal.simulation(&state.opal.manual);
    super::pane(
        "opal-manual-scroll",
        v_flex()
            .id("opal-manual")
            .gap(PANEL_GAP)
            .child(inputs(state, &opal, window, cx))
            .child(result(&opal, &simulation, cx)),
    )
}

/// One whole opal count per unlocked villager, and the reset to the save.
fn inputs(
    state: &AppState,
    opal: &Opal,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let saved = opal.opals();
    let mut counts = h_flex().id("opal-manual-inputs").gap_3();
    for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
        // Values flow section value -> field; edits come back through the
        // subscriptions in `State::new`.
        let field = &state.opal.manual_fields[villager];
        show_number(field, state.opal.manual[villager], 0, window, cx);
        counts = counts.child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    Caption::new(format!(
                        "{} (★{})",
                        NAMES[villager], opal.villagers[villager].stars as i64
                    ))
                    .text_color(muted),
                )
                .child(
                    ui::number_field(
                        format!("opal-manual-{villager}"),
                        field,
                        format!("{} opals", NAMES[villager]),
                    )
                    .w_full(),
                ),
        );
    }
    let entity = cx.entity();
    let reset = Button::new("opal-manual-reset")
        .label("Use saved opals")
        .icon(IconName::RotateCcw)
        .small()
        .disabled(state.opal.manual == saved)
        .on_click(move |_event, window, cx| {
            // The reset replaces what was typed, so it ends the edit: a focused
            // field would keep its typed number (`show_number`).
            window.blur(cx);
            entity.update(cx, |state, cx| state.opal.reset_manual(cx));
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
        .child(counts)
        .into_any_element()
}

/// The typed split's total, the warning while it leaves the save's total and
/// each villager's rate against what it earns today.
fn result(opal: &Opal, simulation: &Simulation, cx: &App) -> AnyElement {
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

    let rows = (0..VILLAGERS)
        .filter(|villager| opal.unlocked(*villager))
        .map(|villager| {
            let count = simulation.opals[villager];
            let rate = simulation.rate(opal, villager);
            let reading = signed(simulation.change(opal, villager));
            [
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
                text(format::value(count)),
                text(format::rate(rate)),
                text(reading),
            ]
        });
    let table = table(
        "opal-manual",
        &[
            Col::Text("Villager"),
            Col::Number("Opals"),
            Col::Number("EXP/hr"),
            Col::Number("vs Saved"),
        ],
        rows,
        cx,
    );

    Card::new()
        .header(CardHeader::new("Simulated Split").description(
            "One whole opal count per unlocked villager, read against the save's own split",
        ))
        .child(
            div()
                .id("opal-manual-summary")
                .role(Role::Label)
                .aria_label(label),
        )
        .when(!simulation.balanced(), |card| {
            card.child(
                div()
                    .id("opal-manual-warning")
                    .role(Role::Label)
                    .aria_label(format!(
                        "Total opals: {} (the save holds {})",
                        format::value(spent),
                        format::value(saved)
                    ))
                    .child(
                        Callout::new(
                            Tone::Warning,
                            format!(
                                "Total opals: {} · the save holds {}",
                                format::value(spent),
                                format::value(saved)
                            ),
                        )
                        .child(Caption::new(
                            "Only a split of the save's own total compares fairly; every rate \
                             below still reads the counts as typed.",
                        )),
                    ),
            )
        })
        .child(
            div().id("opal-manual-figures").child(
                KpiStrip::new()
                    .child(figure(
                        "opal-manual-total",
                        "Total EXP/hr",
                        format::rate(total),
                        "Every unlocked villager at the typed counts",
                    ))
                    .child(figure(
                        "opal-manual-change",
                        "vs Saved",
                        signed(change),
                        "The typed total over the saved split",
                    ))
                    .child(figure(
                        "opal-manual-lowest",
                        "Lowest Villager",
                        format::value(lowest),
                        "What Equal Spread scales with",
                    ))
                    .child(figure(
                        "opal-manual-spread",
                        "Equal Spread Bonus",
                        format::percent(spread / 100.0),
                        "At the typed lowest count",
                    )),
            ),
        )
        .child(table)
        .child(note(
            format!(
                "The save earns {} EXP/hr across the unlocked villagers. Opals are whole, never \
                 negative, and a villager earns nothing without any.",
                format::rate(opal.total_exp_rate(&opal.opals()))
            ),
            cx,
        ))
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
