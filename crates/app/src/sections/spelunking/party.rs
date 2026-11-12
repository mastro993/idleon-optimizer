//! The party panel: one slot per cave capacity and the team's stamina.
//!
//! The upstream page picks members from portrait popovers; here each slot is a
//! select with the same roster. A character another slot already holds, or one
//! below half stamina while the saved stamina is in use, is shown but cannot be
//! picked, and the model rejects the same party anyway.

use super::controls;
use crate::state::AppState;
use crate::ui::assets::asset_image;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div, px};
use idleon_core::spelunking::{Inputs, StaminaMode};

/// The party card: the slots, the team stamina and every maximum.
pub fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let capacity = inputs
        .caves
        .get(state.spelunking.cave)
        .map_or(0, |cave| cave.capacity);
    let full_stamina = state.spelunking.starting == StaminaMode::Full;
    let total: f64 = state
        .spelunking
        .party_ids()
        .iter()
        .filter_map(|id| inputs.characters.get(*id))
        .map(|character| {
            if full_stamina {
                character.max_stamina
            } else {
                character.stamina
            }
        })
        .sum();
    let entity = cx.entity();
    let starting_entity = cx.entity();
    let mut slots = div().id("spelunk-party").flex().flex_col().gap_2();
    for slot in 0..capacity {
        let current = state.spelunking.party.get(slot).copied().unwrap_or(-1);
        let current_character = if current >= 0 {
            inputs.characters.get(current as usize)
        } else {
            None
        };
        let mut choices = vec![Choice::new("-1", "Empty")];
        for character in &inputs.characters {
            let eligible = full_stamina || character.has_half_stamina();
            let taken = state.spelunking.in_party(slot, character.id);
            let choice =
                Choice::new(character.id.to_string(), character.name.clone()).note(format!(
                    "{} / {}",
                    crate::ui::format::value(character.stamina),
                    crate::ui::format::value(character.max_stamina)
                ));
            choices.push(if taken || !eligible {
                choice.disabled()
            } else {
                choice
            });
        }
        let entity = entity.clone();
        let select = Select::new(format!("spelunk-party-{slot}"), choices)
            .label(format!("Party slot {}", slot + 1))
            .selected(current.to_string())
            .size(ControlSize::Sm)
            .on_change(move |value, _window, cx| {
                if let Ok(id) = value.parse::<i32>() {
                    entity.update(cx, |state, cx| state.set_spelunk_party(slot, id, cx));
                }
            });
        slots = slots.child(
            div()
                .id(("spelunk-party-slot", slot))
                .flex()
                .items_end()
                .gap_2()
                .child(if let Some(character) = current_character {
                    asset_image(
                        ("spelunk-portrait", slot),
                        &state.assets,
                        &format!("spelunking/ClassIcons{}.png", character.class_id),
                        px(24.0),
                        cx,
                    )
                } else {
                    div().w(px(24.0)).into_any_element()
                })
                .child(select)
                .child(
                    div().flex().flex_col().gap_1().min_w(px(110.0)).child(
                        Caption::new(format!(
                            "max {}",
                            current_character.map_or("—".to_string(), |character| {
                                crate::ui::format::value(character.max_stamina)
                            })
                        ))
                        .text_color(muted),
                    ),
                ),
        );
    }

    let max_rows: Vec<[AnyElement; 3]> = inputs
        .characters
        .iter()
        .map(|character| {
            let entity = cx.entity();
            let id = character.id;
            let input =
                NumberInput::new(format!("spelunk-max-stamina-{id}"), character.max_stamina)
                    .range(0.0, 1e12)
                    .step(10.0)
                    .precision(0)
                    .size(ControlSize::Sm)
                    .label(format!("{} maximum stamina", character.name))
                    .on_change(move |value, _window, cx| {
                        entity.update(cx, |state, cx| state.set_spelunk_max_stamina(id, value, cx));
                    });
            [
                div()
                    .w(px(150.0))
                    .truncate()
                    .child(character.name.clone())
                    .into_any_element(),
                div()
                    .child(crate::ui::format::value(character.stamina))
                    .into_any_element(),
                input.into_any_element(),
            ]
        })
        .collect();
    let mut table = Table::new(["Character", "Saved stamina", "Maximum stamina"]);
    for row in max_rows {
        table = table.row(row);
    }

    Card::new()
        .header(CardHeader::new("Party").description(format!(
            "{capacity} slot(s) in this cave. Members need half their maximum stamina unless \
                 the run starts fully recovered."
        )))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(Badge::new(format!(
                    "Team stamina {}",
                    crate::ui::format::value(total)
                )))
                .child(controls::starting_select(state, starting_entity)),
        )
        .child(slots)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Caption::new(
                        "The save stores current stamina, not the maximum: enter each character's \
                         maximum in game for the half-stamina rule and full-stamina runs.",
                    )
                    .text_color(muted),
                )
                .child(table),
        )
        .into_any_element()
}
