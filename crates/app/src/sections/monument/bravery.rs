//! The Bravery tab: saved stats, the run-state inputs, sword odds, milestones
//! and the permanent reward levels.

use super::{
    Metric, MilestoneFirst, bonuses_card, empty_card, error_line, metrics, milestones_card,
    notes_card, scroll_pane, unavailable_card,
};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, IntoElement, ParentElement, Styled, Window,
    px,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_monument::{Odds, Snapshot};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, show_number};

/// The Bravery pane.
pub(crate) fn pane(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let body = match &state.monument_controls.derived.snapshot {
        Some(snapshot) if snapshot.available => loaded(state, snapshot, window, cx),
        Some(snapshot) => unavailable_card("Bravery", &snapshot.notes, cx),
        None => empty_card(
            "Bravery",
            "Import a decoded save to read the Bravery training, milestones and permanent rewards.",
            cx,
        ),
    };
    scroll_pane(
        "monument-pane-bravery",
        "monument-bravery-scroll",
        v_flex().id("monument-bravery-body").gap_3().child(body),
    )
}

fn loaded(
    state: &AppState,
    snapshot: &Snapshot,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let stats = &snapshot.stats;
    let cards = metrics(
        "monument-bravery-stats",
        vec![
            Metric::new(
                "monument-hours",
                "Saved training",
                format!("{} hours", format::value(stats.hours)),
                "Paying Respect; not reward levels",
            ),
            Metric::new(
                "monument-milestones",
                "Unlocked milestones",
                format!("{} / 8", stats.milestones),
                "Milestones are saved; the next row shows its ETA",
            ),
            Metric::new(
                "monument-opal",
                "Next opal offer chance",
                format::percent(stats.opal_chance),
                "Uses saved opals; each additional opal lowers later offers",
            ),
            Metric::new(
                "monument-swords",
                "Swords",
                format::value(f64::from(stats.swords)),
                "Swords this story can throw",
            ),
            Metric::new(
                "monument-damage",
                "Initial sword damage",
                format!(
                    "{} - {}",
                    format::value(stats.min_dmg),
                    format::value(stats.max_dmg)
                ),
                "Fresh story: zero boosts, full rethrows",
            ),
            Metric::new(
                "monument-reroll-summary",
                "Rethrows / retellings",
                format!("{} / {}", stats.max_rerolls, stats.retellings),
                "Per story",
            ),
        ],
    );

    let mut column = v_flex().gap_3().child(cards);
    if !snapshot.notes.is_empty() {
        column = column.child(notes_card("monument-bravery-notes", &snapshot.notes, cx));
    }
    column = column.child(controls_card(state, snapshot, window, cx));
    match &state.monument_controls.derived.odds {
        Some(Ok(odds)) => {
            column = column.child(odds_cards(odds));
        }
        Some(Err(error)) => {
            column = column.child(error_line("monument-bravery-error", error.to_string(), cx));
        }
        None => {}
    }
    column = column.child(
        Card::new()
            .header(CardHeader::new("Right-most sword damage boost").description(
                "Run state is not saved. Enter the current enemy and boost count. For the finishing blow, let every other sword land before the right-most sword; an earlier kill ends the fight before a later sword can grant a boost.",
            ))
            .child(
                Caption::new(
                    "Fresh-roll odds use continuous uniform damage and no rethrows. A rethrow replaces the last sword's old damage; it does not add another hit. The optional HP should exclude that sword's old hit.",
                ),
            ),
    );
    column = column.child(milestones_card(
        &snapshot.milestones,
        MilestoneFirst::Damage {
            min: snapshot.stats.min_dmg,
            max: snapshot.stats.max_dmg,
        },
        cx,
    ));
    column = column.child(bonuses_card(&snapshot.bonuses, cx));
    column.into_any_element()
}

fn controls_card(
    state: &AppState,
    snapshot: &Snapshot,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let monument = &state.monument;
    let controls = &state.monument_controls;
    show_number(&controls.wave, f64::from(monument.wave), 0, window, cx);
    show_number(&controls.boosts, f64::from(monument.boosts), 0, window, cx);
    let rerolls = monument.rerolls.unwrap_or(snapshot.stats.max_rerolls);
    show_number(&controls.rerolls, f64::from(rerolls), 0, window, cx);
    show_optional(&controls.hp, monument.remaining_hp, window, cx);
    let reset_rerolls = cx.entity();
    let clear_hp = cx.entity();

    Card::new()
        .header(
            CardHeader::new("Run state")
                .description("Enemy number, boost stacks and the rethrows left in this story."),
        )
        .child(
            h_flex()
                .id("monument-controls")
                .items_end()
                .gap_3()
                .child(labelled(
                    "Enemy number",
                    number_field("monument-wave", &controls.wave, "Enemy number"),
                ))
                .child(labelled(
                    "Current boost stacks",
                    number_field("monument-boosts", &controls.boosts, "Current boost stacks"),
                ))
                .child(labelled(
                    "Rethrows remaining",
                    h_flex()
                        .gap_2()
                        .child(number_field(
                            "monument-rerolls",
                            &controls.rerolls,
                            "Rethrows remaining",
                        ))
                        .child(
                            Button::new("monument-rerolls-max")
                                .label("Max")
                                .small()
                                .disabled(monument.rerolls.is_none())
                                .on_click(move |_event, _window, cx| {
                                    reset_rerolls.update(cx, |state, cx| {
                                        state.reset_monument_rerolls(cx);
                                    });
                                }),
                        ),
                ))
                .child(labelled(
                    "HP before last sword (optional)",
                    h_flex()
                        .gap_2()
                        .child(
                            number_field("monument-hp", &controls.hp, "HP before last sword")
                                .w(px(150.0)),
                        )
                        .child(
                            Button::new("monument-hp-clear")
                                .label("Clear")
                                .small()
                                .disabled(monument.remaining_hp.is_none())
                                .on_click(move |_event, _window, cx| {
                                    clear_hp.update(cx, |state, cx| {
                                        state.clear_monument_hp(cx);
                                    });
                                }),
                        ),
                )),
        )
        .into_any_element()
}

/// `value` in `field`, or a blank field showing its placeholder for `None`.
fn show_optional(
    field: &Entity<InputState>,
    value: Option<f64>,
    window: &mut Window,
    cx: &mut App,
) {
    match value {
        Some(value) => show_number(field, value, 0, window, cx),
        None => {
            if !field.focus_handle(cx).is_focused(window) && !field.read(cx).value().is_empty() {
                field.update(cx, |field, cx| field.set_value("", window, cx));
            }
        }
    }
}

/// A control under its caption.
pub(super) fn labelled(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    v_flex().gap_1().child(Caption::new(label)).child(control)
}

fn odds_cards(odds: &Odds) -> AnyElement {
    let boost_sub = if odds.automatic {
        "Study guarantees the skip and grants a stack"
    } else if odds.stats.rightmost {
        "Others do not kill; the right-most finishes"
    } else {
        "The right-most boost schematic is not owned"
    };
    let mut figures = vec![
        Metric::new(
            "monument-odds-hp",
            "Enemy HP",
            format::value(odds.hp),
            format!("Enemy number {}", odds.wave + 1),
        ),
        Metric::new(
            "monument-odds-damage",
            "This roll damage",
            format!(
                "{} - {}",
                format::value(odds.stats.min_dmg),
                format::value(odds.stats.max_dmg)
            ),
            "Per sword; based on the entered run state",
        ),
        Metric::new(
            "monument-odds-kill",
            "Fresh full-volley kill",
            format::percent(odds.kill),
            "Independent rolls; before any rethrows",
        ),
        Metric::new(
            "monument-odds-boost",
            "Right-most finishing / boost",
            format::percent(odds.boost_chance),
            boost_sub,
        ),
    ];
    if let Some(chance) = odds.last_sword_chance {
        figures.push(Metric::new(
            "monument-odds-last",
            "One last-sword kill",
            format::percent(chance),
            "Conditional on the HP entered",
        ));
    }
    figures.push(Metric::new(
        "monument-odds-next",
        "Next stack damage gain",
        format!("+{:.2}%", (odds.next_damage_factor - 1.0) * 100.0),
        "10 percentage points added to the boost factor",
    ));
    v_flex()
        .gap_3()
        .child(metrics("monument-bravery-odds", figures))
        .child(Caption::new(
            "Boost odds exclude early kills. They assume the right-most sword lands strictly later, not simultaneous animation completions. Existing stacks multiply maximum damage by 1 + 0.1 × stacks.",
        ))
        .into_any_element()
}
