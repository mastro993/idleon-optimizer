//! The party panel: one slot per cave capacity and the team's stamina.
//!
//! Each slot is a select with the roster. A character another slot already
//! holds, or one below half stamina while the saved stamina is in use, is shown
//! but cannot be picked, and the model rejects the same party anyway.

use super::controls::{Derived, starting_choices};
use super::{boxed, number, select, tag};
use crate::state::AppState;
use crate::ui::assets::asset_image;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use idleon_spelunking::StaminaMode;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The party card: the slots, the team stamina and every maximum.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let inputs = &derived.inputs;
    let spelunking = &state.spelunking;
    let controls = &state.spelunking_controls;
    let capacity = derived.party.len();
    let full_stamina = spelunking.starting == StaminaMode::Full;
    let total: f64 = spelunking
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
    let mut slots = div().id("spelunk-party").flex().flex_col().gap_2();
    for (slot, choices) in derived.party.iter().enumerate() {
        let current = spelunking.party.get(slot).copied().unwrap_or(-1);
        let current_character = usize::try_from(current)
            .ok()
            .and_then(|id| inputs.characters.get(id));
        let picker = select(
            &controls.party[slot],
            format!("spelunk-party-{slot}"),
            format!("Party slot {}", slot + 1),
            choices,
            &current,
            window,
            cx,
        );
        slots = slots.child(
            div()
                .id(("spelunk-party-slot", slot))
                .flex()
                .items_center()
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
                .child(boxed(&controls.party[slot], picker, window, cx))
                .child(Caption::new(format!(
                    "max {}",
                    current_character.map_or("—".to_string(), |character| {
                        format::value(character.max_stamina)
                    })
                ))),
        );
    }

    let rows: Vec<TableRow> = inputs
        .characters
        .iter()
        .enumerate()
        .map(|(index, character)| {
            let id = character.id;
            TableRow::new()
                .id(("spelunk-max-row", index))
                .child(TableCell::new().child(div().truncate().child(character.name.clone())))
                .child(
                    TableCell::new()
                        .text_right()
                        .child(format::value(character.stamina)),
                )
                .child(TableCell::new().child(number(
                    format!("spelunk-max-stamina-{id}"),
                    &controls.max_stamina[index],
                    format!("{} maximum stamina", character.name),
                    character.max_stamina,
                    0,
                    window,
                    cx,
                )))
        })
        .collect();
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("Character"))
                    .child(TableHead::new().text_right().child("Saved stamina"))
                    .child(TableHead::new().child("Maximum stamina")),
            ),
        )
        .child(TableBody::new().children(rows));

    let starting = select(
        &controls.starting,
        "spelunk-stamina",
        "Starting stamina",
        &starting_choices(),
        &spelunking.starting,
        window,
        cx,
    );
    Card::new()
        .header(CardHeader::new("Party").description(format!(
            "{capacity} slot(s) in this cave. Members need half their maximum stamina unless the \
             run starts fully recovered."
        )))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(tag(
                    format!("Team stamina {}", format::value(total)),
                    Tone::Neutral,
                ))
                .child(boxed(&controls.starting, starting, window, cx)),
        )
        .child(slots)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(
                    "The save stores current stamina, not the maximum: enter each character's \
                     maximum in game for the half-stamina rule and full-stamina runs.",
                ))
                .child(div().id("spelunk-max-stamina").child(table)),
        )
        .into_any_element()
}
