//! The expedition controls above the party, belt and depth estimate.
//!
//! Every control the upstream toolbar carries: the logged-in character, the cave
//! (with its Normal/Manic records), Normal/Manic mode, the policy, the Auto
//! full-clear flag and the Auto speed. The account card carries the four values
//! upstream derives from its whole stat context and this port does not decode
//! yet.

use super::state::records;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, div, px};
use idleon_core::spelunking::{Inputs, Policy, StaminaMode};

/// The toolbar card: character, cave, mode, policy, full-clear and speed.
pub fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let character_entity = entity.clone();
    let cave_entity = entity.clone();
    let mode_entity = entity.clone();
    let policy_entity = entity.clone();
    let clear_entity = entity.clone();
    let speed_entity = entity.clone();

    let character_choices: Vec<Choice> = inputs
        .characters
        .iter()
        .map(|character| {
            Choice::new(character.id.to_string(), character.name.clone()).note(format!(
                "{} / {} stamina",
                held(character.stamina),
                held(character.max_stamina)
            ))
        })
        .collect();
    let character_select = Select::new("spelunk-character", character_choices)
        .label("Logged-in character")
        .selected(state.spelunking.character.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(character) = value.parse::<usize>() {
                character_entity.update(cx, |state, cx| state.set_spelunk_character(character, cx));
            }
        });

    let records = records(inputs, state.spelunking.manic);
    let cave_choices: Vec<Choice> = inputs
        .caves
        .iter()
        .map(|cave| {
            let record = records.get(cave.id).copied().unwrap_or(0.0);
            Choice::new(cave.id.to_string(), cave.name.clone()).note(format!(
                "record {} · {} members",
                held(record),
                cave.capacity
            ))
        })
        .collect();
    let cave_select = Select::new("spelunk-cave", cave_choices)
        .label("Cave")
        .selected(state.spelunking.cave.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(cave) = value.parse::<usize>() {
                cave_entity.update(cx, |state, cx| state.set_spelunk_cave(cave, cx));
            }
        });

    let manic_choice = Choice::new("true", "Manic");
    let mode_select = Select::new(
        "spelunk-mode",
        [
            Choice::new("false", "Normal"),
            if inputs.manic {
                manic_choice
            } else {
                manic_choice.disabled()
            },
        ],
    )
    .label("Expedition")
    .selected(state.spelunking.manic.to_string())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        mode_entity.update(cx, |state, cx| {
            state.set_spelunk_manic(value.as_ref() == "true", cx)
        });
    });

    let policy_choices: Vec<Choice> = Policy::ALL
        .into_iter()
        .map(|policy| {
            let choice = Choice::new(policy.id(), policy.label());
            if policy == Policy::Auto && !inputs.auto {
                choice.disabled()
            } else {
                choice
            }
        })
        .collect();
    let policy_select = Select::new("spelunk-policy", policy_choices)
        .label("Policy")
        .selected(state.spelunking.policy.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(policy) = Policy::from_id(value.as_ref()) {
                policy_entity.update(cx, |state, cx| state.set_spelunk_policy(policy, cx));
            }
        });

    let auto = state.spelunking.policy == Policy::Auto;
    let full_clear = Checkbox::new("spelunk-full-clear", state.spelunking.full_clear)
        .label(SharedString::from("Auto full clear"))
        .disabled(!auto)
        .on_change(move |checked, _window, cx| {
            clear_entity.update(cx, |state, cx| state.set_spelunk_full_clear(checked, cx));
        });

    let speeds: Vec<Choice> = (0..=inputs.speed_max)
        .map(|speed| {
            Choice::new(
                speed.to_string(),
                if speed == 0 {
                    "Base".to_string()
                } else {
                    format!("Speed {speed}")
                },
            )
        })
        .collect();
    let speed_select = Select::new("spelunk-speed", speeds)
        .label("Auto speed")
        .selected(state.spelunking.speed.to_string())
        .size(ControlSize::Sm)
        .disabled(!auto)
        .on_change(move |value, _window, cx| {
            if let Ok(speed) = value.parse::<u32>() {
                speed_entity.update(cx, |state, cx| state.set_spelunk_speed(speed, cx));
            }
        });

    Card::new()
        .header(CardHeader::new("Expedition controls").description(
            "The save supplies each character's stamina, the caves and their records, and the \
                 available modes, speeds and elixirs.",
        ))
        .child(
            div()
                .id("spelunk-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .child(crate::sections::spelunking::field(
                    "Character",
                    character_select.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Cave",
                    cave_select.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Mode",
                    mode_select.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Policy",
                    policy_select.into_any_element(),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w(px(150.0))
                        .child(Caption::new("Descent").text_color(muted))
                        .child(full_clear),
                )
                .child(crate::sections::spelunking::field(
                    "Auto speed",
                    speed_select.into_any_element(),
                ))
                .when(state.spelunking.policy == Policy::AmberFarm, |row| {
                    row.child(
                        Badge::new("Amber farming always clears the floor").tone(Tone::Warning),
                    )
                }),
        )
        .into_any_element()
}

/// The account values the upstream stat context would supply.
pub fn account_card(state: &AppState, _inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let power_entity = entity.clone();
    let exit_entity = entity.clone();
    let amber_entity = entity.clone();
    let regen_entity = entity.clone();
    let grand_entity = entity.clone();
    let power = NumberInput::new("spelunk-power", state.spelunking.account.power)
        .range(0.0, 1e30)
        .step(1.0)
        .precision(4)
        .size(ControlSize::Sm)
        .label("Spelunking Power")
        .on_change(move |value, _window, cx| {
            power_entity.update(cx, |state, cx| state.set_spelunk_power(value, cx));
        });
    let exits = NumberInput::new("spelunk-extra-exits", state.spelunking.account.holes_bonus)
        .range(0.0, 100_000.0)
        .step(1.0)
        .precision(2)
        .size(ControlSize::Sm)
        .label("Extra exits")
        .on_change(move |value, _window, cx| {
            exit_entity.update(cx, |state, cx| state.set_spelunk_holes_bonus(value, cx));
        });
    let amber = NumberInput::new("spelunk-amber", state.spelunking.account.amber)
        .range(0.0, 1e30)
        .step(0.1)
        .precision(4)
        .size(ControlSize::Sm)
        .label("Amber multiplier")
        .on_change(move |value, _window, cx| {
            amber_entity.update(cx, |state, cx| state.set_spelunk_amber(value, cx));
        });
    let regen = NumberInput::new("spelunk-regen", state.spelunking.account.regen)
        .range(0.0, 1e9)
        .step(1.0)
        .precision(2)
        .size(ControlSize::Sm)
        .label("Stamina regeneration")
        .on_change(move |value, _window, cx| {
            regen_entity.update(cx, |state, cx| state.set_spelunk_regen(value, cx));
        });
    let grand = NumberInput::new("spelunk-grand", 100.0 * state.spelunking.account.grand)
        .range(0.0, 100.0)
        .step(0.001)
        .precision(4)
        .size(ControlSize::Sm)
        .label("Grand Discovery chance (%)")
        .on_change(move |value, _window, cx| {
            grand_entity.update(cx, |state, cx| state.set_spelunk_grand_pct(value, cx));
        });
    Card::new()
        .header(CardHeader::new("Account inputs").description(
            "Spelunking Power, the extra-exit and Amber multipliers, stamina regeneration and \
                 the Grand Discovery chance are account values the save does not decode yet: read \
                 them in game. The save fixes everything else.",
        ))
        .child(
            div()
                .id("spelunk-account-inputs")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .child(crate::sections::spelunking::field(
                    "Spelunking Power",
                    power,
                ))
                .child(crate::sections::spelunking::field(
                    "Extra exits (%)",
                    exits.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Amber multiplier",
                    amber.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Stamina regen / hr",
                    regen.into_any_element(),
                ))
                .child(crate::sections::spelunking::field(
                    "Grand Discovery (%)",
                    grand.into_any_element(),
                ))
                .child(div().flex().items_center().gap_2().child(
                    Caption::new("The saved belt and memory are the defaults.").text_color(muted),
                )),
        )
        .into_any_element()
}

/// The starting-stamina choice shared by the party card.
pub fn starting_select(state: &AppState, entity: gpui::Entity<AppState>) -> AnyElement {
    let modes = [
        Choice::new(StaminaMode::Saved.id(), "Saved stamina"),
        Choice::new(StaminaMode::Full.id(), "Fully recovered (hypothetical)"),
    ];
    Select::new("spelunk-stamina", modes)
        .label("Starting stamina")
        .selected(state.spelunking.starting.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(mode) = StaminaMode::from_id(value.as_ref()) {
                entity.update(cx, |state, cx| state.set_spelunk_starting(mode, cx));
            }
        })
        .into_any_element()
}

/// A short stamina readout for notes and choices.
fn held(value: f64) -> String {
    crate::ui::format::value(value)
}
