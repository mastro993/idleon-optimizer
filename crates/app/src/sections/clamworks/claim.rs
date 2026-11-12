//! The Clamworks AFK claim: one trip's pearls and the `+1` comparison.
//!
//! Ports the AFK claim tab of `clamworks-calc.html` and
//! `js/ui/clamworks-afk.js`: the character and pearl-table pickers, the hours
//! away, the expected pearl currency and the two drop rows. The `+1` comparison
//! from `js/stats/systems/w7/clamworks-combat.js` is beside it in [`super::compare`].
//!
//! The upstream page runs the claim in a combat worker and compares a reroll
//! policy on top of it. Neither is here: the combat simulator supplies the kills
//! and the drop rate as entries until `CS-02`/`CS-03` land, and the reroll
//! policy needs the talent and guild bonuses the stats context resolves. What is
//! left is the claim itself, which is the upstream summary's first-claim payout.

use super::compare::compare_card;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, Input, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px,
};
use idleon_clamworks::afk::{Claim, ClaimError, Compare, Phase};
use idleon_ui::format;

pub(crate) fn claim_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    // The claim reads the levels the section shows, so an applied plan's
    // projection previews the claim it would buy.
    let clamworks = state.clamworks_view();
    let multikill = state.clamworks_multikill;
    let outcome = state.clamworks_claim.compare(&clamworks, multikill);
    let body = div()
        .id("clamworks-claim")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(claim_card(state, outcome.as_ref(), cx))
        .child(compare_card(
            outcome.as_ref(),
            state.clamworks_claim.phase,
            cx.theme().colors.fg_muted,
        ));
    ScrollArea::new("clamworks-claim-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The claim itself: the controls, the refused entry, the figures and the rows.
fn claim_card(
    state: &mut AppState,
    outcome: Result<&Compare, &ClaimError>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let character = state.clamworks_claim.character;
    let characters = state
        .save
        .as_ref()
        .map(|save| {
            (0..save.character_count())
                .map(|index| Choice::new(index.to_string(), save.character_name(index)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let character_name = state.save.as_ref().map_or_else(
        || "Character".to_string(),
        |save| save.character_name(character),
    );
    let promotion = state.clamworks.promotion_level;
    let entity = cx.entity();
    let select_character = entity.clone();
    let select_phase = entity.clone();
    let set_kills = entity.clone();
    let set_drop_rate = entity.clone();
    let calculate = entity.clone();
    let stand_in = state.clamworks_claim.stand_in(character);
    let phase = state.clamworks_claim.phase;
    let hours_input = state.clamworks_claim.hours_field.clone();
    let hours_focus = hours_input.read(cx).focus().clone();
    let has_characters = !characters.is_empty();

    let controls = div()
        .id("clamworks-claim-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Character").text_color(muted))
                .child(
                    Select::new("clamworks-claim-character", characters)
                        .label("Character")
                        .placeholder("No characters")
                        .selected(character.to_string())
                        .disabled(!has_characters)
                        .on_change(move |value, _window, cx| {
                            if let Ok(index) = value.parse::<usize>() {
                                select_character.update(cx, |state, cx| {
                                    state.set_clamworks_character(index, cx);
                                });
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Pearl table").text_color(muted))
                .child(
                    Select::new(
                        "clamworks-claim-phase",
                        Phase::ALL
                            .into_iter()
                            .map(|phase| Choice::new(phase.id(), phase.label()))
                            .collect::<Vec<_>>(),
                    )
                    .label("Pearl table")
                    .selected(phase.id())
                    .on_change(move |value, _window, cx| {
                        if let Some(phase) = Phase::from_id(value.as_ref()) {
                            select_phase.update(cx, |state, cx| {
                                state.set_clamworks_phase(phase, cx);
                            });
                        }
                    }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Kills / hour").text_color(muted))
                .child(
                    NumberInput::new("clamworks-claim-kills", stand_in.kills_per_hour)
                        .label("Kills per hour")
                        .range(0.0, 1_000_000.0)
                        .step(100.0)
                        .precision(0)
                        .size(ControlSize::Sm)
                        .on_change(move |value, _window, cx| {
                            set_kills.update(cx, |state, cx| {
                                state.set_clamworks_kills_per_hour(value, cx);
                            });
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Drop rate ×").text_color(muted))
                .child(
                    NumberInput::new("clamworks-claim-drop-rate", stand_in.drop_rate)
                        .label("Drop rate multiplier")
                        .range(0.0, 1000.0)
                        .step(0.1)
                        .precision(2)
                        .size(ControlSize::Sm)
                        .on_change(move |value, _window, cx| {
                            set_drop_rate.update(cx, |state, cx| {
                                state.set_clamworks_drop_rate(value, cx);
                            });
                        }),
                ),
        )
        .child(
            div()
                .id("clamworks-claim-hours")
                .role(Role::Label)
                .aria_label("Hours away")
                .flex()
                .flex_col()
                .gap_1()
                .w(px(140.0))
                .child(Caption::new("Hours away").text_color(muted))
                .on_click(move |_event, window, cx| window.focus(&hours_focus, cx))
                .child(Input::new(&hours_input).size(ControlSize::Sm)),
        )
        .child(
            Button::new("clamworks-claim-calculate", "Calculate claim")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    calculate.update(cx, |state, cx| state.calculate_clamworks_claim(cx));
                }),
        );

    let mut card = Card::new().header(CardHeader::new("AFK claim").description(
        "One ordinary trip on Clamworks: the pearls its credited kills roll, Lucky Day included.",
    ));
    card = card.child(div().p_3().child(controls));
    match outcome {
        Ok(compare) => {
            let claim = &compare.rows[0].claim;
            card = card.child(claim_figures(claim, &character_name, promotion, cx));
            card = card.child(div().px_3().child(drops_table(claim)));
            card = card.child(
                div()
                    .id("clamworks-claim-notes")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .pb_3()
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(
                        "Kills per hour and the drop rate are entries until the combat simulator derives them; \
                         the pearl table is the one the game starts the claim from.",
                    )
                    .child(
                        "One claim, before rerolls, claim doubling or inventory losses. Time Candy cannot be \
                         used on this map.",
                    )
                    .children(claim.warnings.iter().enumerate().map(|(index, warning)| {
                        div()
                            .id(("clamworks-claim-warning", index))
                            .role(Role::Label)
                            .aria_label(warning.clone())
                            .child(warning.clone())
                    })),
            );
        }
        Err(error) => {
            card = card.child(
                div().px_3().pb_3().child(
                    div()
                        .id("clamworks-claim-error")
                        .role(Role::Label)
                        .aria_label(error.message())
                        .child(Banner::new(
                            "clamworks-claim-error-banner",
                            Severity::Danger,
                            error.message(),
                        )),
                ),
            );
        }
    }
    card.into_any_element()
}

/// The claim's four figures and the line that says where they came from.
fn claim_figures(
    claim: &Claim,
    character_name: &str,
    promotion: u32,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let (pearls, pearl_suffix) = format::scale(claim.expected_pearls);
    let (rolls, roll_suffix) = format::scale(claim.rolls as f64);
    div()
        .id("clamworks-claim-figures")
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(
            Caption::new(format!(
                "{character_name} · promotion {promotion} · {} × {} kills/hour = {} kills, {} drop rolls",
                format::hours(claim.inputs.hours),
                quantity(claim.inputs.kills_per_hour),
                quantity(claim.kills),
                format::count(claim.rolls),
            ))
            .text_color(muted),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(kpi(
                    "clamworks-claim-pearls",
                    "Expected pearls",
                    format!("{} pearls", quantity(claim.expected_pearls)),
                    Statistic::new("clamworks-claim-pearls-value", "Expected pearls", pearls)
                        .decimals(2)
                        .suffix(pearl_suffix)
                        .size(TextSize::Lg),
                    "Both pearl types, Lucky Day included",
                ))
                .child(kpi(
                    "clamworks-claim-pure",
                    "Pure pearl chance",
                    format::chance(claim.pure.chance),
                    Statistic::new(
                        "clamworks-claim-pure-value",
                        "Pure pearl chance",
                        claim.pure.chance * 100.0,
                    )
                    .decimals(2)
                    .suffix("%")
                    .size(TextSize::Lg),
                    if claim.pure.probability > 0.0 {
                        "At least one Pure Pearl this claim"
                    } else {
                        "PURE PEARLS is not purchased"
                    },
                ))
                .child(kpi(
                    "clamworks-claim-rolls",
                    "Drop rolls",
                    format!("{} kills", format::count(claim.rolls)),
                    Statistic::new("clamworks-claim-rolls-value", "Drop rolls", rolls)
                        .decimals(0)
                        .suffix(roll_suffix)
                        .size(TextSize::Lg),
                    "One pearl roll per kill",
                ))
                .child(kpi(
                    "clamworks-claim-value",
                    "Pearl value",
                    format!("×{}", quantity(claim.value)),
                    Statistic::new("clamworks-claim-value-value", "Pearl value", claim.value)
                        .decimals(2)
                        .prefix("×")
                        .size(TextSize::Lg),
                    &format!("Lucky Day: {} for 10x", format::chance(claim.lucky_chance)),
                )),
        )
        .into_any_element()
}

/// The upstream `dropRow` table: both pearl types over the claim.
fn drops_table(claim: &Claim) -> AnyElement {
    let mut table = Table::new([
        "Drop",
        "Expected quantity",
        "Chance of at least one",
        "Guaranteed quantity",
        "Expected pearl currency",
    ]);
    for (id, name, drop) in [
        ("clamworks-claim-drops-regular", "Pearls", &claim.regular),
        ("clamworks-claim-drops-pure", "Pure Pearls", &claim.pure),
    ] {
        table = table.row([
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{name}: {}", quantity(drop.expected)))
                .child(SharedString::from(name))
                .into_any_element(),
            div().child(quantity(drop.expected)).into_any_element(),
            div()
                .child(format!(
                    "{}{}",
                    format::chance(drop.chance),
                    if drop.guaranteed { " (Guaranteed)" } else { "" }
                ))
                .into_any_element(),
            div().child(quantity(drop.minimum)).into_any_element(),
            div().child(quantity(drop.pearls)).into_any_element(),
        ]);
    }
    div()
        .id("clamworks-claim-drops")
        .child(table)
        .into_any_element()
}

/// A figure on a card, labelled so its value reads as one figure.
fn kpi(
    id: &'static str,
    title: &'static str,
    display: String,
    statistic: Statistic,
    caption: &str,
) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{title}: {display}"))
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(180.0))
        .child(
            KpiCard::new(statistic)
                .caption(caption.to_string())
                .into_any_element(),
        )
        .into_any_element()
}

/// A quantity as the upstream page prints it (`fmt`, with `fmtVal`'s tiers): a
/// fraction below one percent in scientific notation, everything else scaled.
pub(super) fn quantity(value: f64) -> String {
    if value > 0.0 && value < 0.01 {
        format!("{value:.2e}")
    } else {
        format::fmt_val(value)
    }
}
