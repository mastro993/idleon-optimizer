//! The elixir belt: equipped and memorized slots, the built-in presets and the
//! selected effects.
//!
//! Upstream taps a belt slot to open a portrait popover; here each slot is a
//! select with the unlocked elixirs. The duplicate and memory rules are enforced
//! by the normaliser as well, so an overfilled belt is reported instead of
//! silently rewritten.

use crate::state::AppState;
use crate::ui::assets::asset_image;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div, px};
use idleon_core::spelunking::{Inputs, PresetKind, data};

/// The belt card.
pub fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let equipped_count = state
        .spelunking
        .elixirs
        .iter()
        .filter(|id| **id >= 0)
        .count();
    let equipped = slots(
        state,
        inputs,
        PresetKind::Belt,
        inputs.slots,
        &state.spelunking.elixirs,
        &entity,
        cx,
    );
    let memory_count = state
        .spelunking
        .memorized
        .iter()
        .filter(|id| **id >= 0)
        .count();
    let memorized = slots(
        state,
        inputs,
        PresetKind::Memory,
        inputs.memory_slots,
        &state.spelunking.memorized,
        &entity,
        cx,
    );
    let presets: Vec<AnyElement> = data()
        .elixir_presets
        .iter()
        .map(|preset| {
            let issues = state.spelunking.preset_issues(inputs, preset);
            let preset = preset.clone();
            let apply_entity = cx.entity();
            let kind = match preset.kind {
                PresetKind::Belt => "Equipped",
                PresetKind::Memory => "Memorized",
            };
            div()
                .id(format!("spelunk-preset-row-{}", preset.id))
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w(px(190.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Badge::new(kind).tone(Tone::Info))
                        .child(div().text_size(px(12.0)).child(preset.label.clone()))
                        .child(
                            Button::new(format!("spelunk-preset-{}", preset.id), "Apply")
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .disabled(!issues.is_empty())
                                .on_click(move |_event, _window, cx| {
                                    apply_entity.update(cx, |state, cx| {
                                        state.apply_spelunk_preset(preset.clone(), cx)
                                    });
                                }),
                        ),
                )
                .children(if issues.is_empty() {
                    Vec::new()
                } else {
                    vec![Caption::new(issues.join(" ")).text_color(cx.theme().colors.warning)]
                })
                .into_any_element()
        })
        .collect();
    let repeats = (1.0 + inputs.shop.get(58).copied().unwrap_or(0.0) / 100.0).floor();
    let mut selected: Vec<i32> = state
        .spelunking
        .elixirs
        .iter()
        .chain(state.spelunking.memorized.iter())
        .copied()
        .filter(|id| *id >= 0)
        .collect();
    selected.sort_unstable();
    selected.dedup();
    let effects = selected
        .iter()
        .filter_map(|id| inputs.elixirs.get(*id as usize))
        .map(|elixir| {
            let memory = [1, 8, 11].contains(&elixir.id);
            let note = if memory {
                "No starting effect when memorized.".to_string()
            } else {
                format!(
                    "Applied at entry {repeats:.0} time(s) when memorized; {} belt charges.",
                    crate::ui::format::value(elixir.charges)
                )
            };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_size(px(12.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().child(format!("{} ({})", elixir.name, note)))
                        .child(
                            Badge::new(format!(
                                "{} charges",
                                crate::ui::format::value(elixir.charges)
                            ))
                            .tone(Tone::Neutral),
                        ),
                )
                .child(Caption::new(elixir.description.clone()).text_color(muted))
                .into_any_element()
        })
        .collect::<Vec<_>>();

    Card::new()
        .header(
            CardHeader::new("Elixir belt").description(
                "The equipped belt spends charges during a run; the memorized set applies at entry \
                 without spending them.",
            ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(slot_heading("Equipped", equipped_count))
                .child(equipped)
                .child(slot_heading(
                    "Memorized",
                    memory_count,
                ))
                .child(memorized)
                .when_some(
                    (!effects.is_empty()).then_some(effects),
                    |card, effects| {
                        card.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(Caption::new("Selected elixir effects").text_color(muted))
                                .children(effects),
                        )
                    },
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Caption::new("Built-in presets").text_color(muted))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .children(presets),
                )
                .child(
                    Caption::new(
                        "Each preset replaces only its half of the belt. These pictured loadouts are \
                         not optimizer recommendations.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

fn slot_heading(label: &'static str, count: usize) -> AnyElement {
    div()
        .text_size(px(12.0))
        .child(format!("{label} ({count} selected)"))
        .into_any_element()
}

fn slots(
    state: &AppState,
    inputs: &Inputs,
    kind: PresetKind,
    count: usize,
    values: &[i32],
    entity: &gpui::Entity<AppState>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if count == 0 {
        let muted = cx.theme().colors.fg_muted;
        return div()
            .child(Caption::new("Locked").text_color(muted))
            .into_any_element();
    }
    let mut row = div()
        .id(match kind {
            PresetKind::Belt => "spelunk-belt",
            PresetKind::Memory => "spelunk-memory",
        })
        .flex()
        .flex_wrap()
        .gap_2();
    for slot in 0..count {
        let current = values.get(slot).copied().unwrap_or(-1);
        let mut choices = vec![Choice::new("-1", "Empty")];
        for elixir in inputs.elixirs.iter().take(inputs.elixir_owned) {
            let id = elixir.id;
            let used = match kind {
                PresetKind::Belt => {
                    values
                        .iter()
                        .enumerate()
                        .filter(|(index, value)| *index != slot && **value == id as i32)
                        .count()
                        >= inputs.dupes
                }
                PresetKind::Memory => values
                    .iter()
                    .enumerate()
                    .any(|(index, value)| index != slot && *value == id as i32),
            };
            let choice = Choice::new(id.to_string(), elixir.name.clone()).note(format!(
                "{} charges",
                crate::ui::format::value(elixir.charges)
            ));
            choices.push(if used { choice.disabled() } else { choice });
        }
        let entity = entity.clone();
        let select = Select::new(
            format!(
                "spelunk-{}-{slot}",
                match kind {
                    PresetKind::Belt => "elixir",
                    PresetKind::Memory => "memorized",
                }
            ),
            choices,
        )
        .label(format!(
            "{} slot {}",
            match kind {
                PresetKind::Belt => "Equipped",
                PresetKind::Memory => "Memorized",
            },
            slot + 1
        ))
        .selected(current.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(id) = value.parse::<i32>() {
                entity.update(cx, |state, cx| match kind {
                    PresetKind::Belt => state.set_spelunk_elixir(slot, id, cx),
                    PresetKind::Memory => state.set_spelunk_memorized(slot, id, cx),
                });
            }
        });
        let icon = inputs
            .elixirs
            .get(current.max(0) as usize)
            .filter(|_| current >= 0);
        let slot_id = slot as u64;
        row = row.child(
            div()
                .id(("spelunk-elixir-slot", slot_id))
                .flex()
                .items_end()
                .gap_1()
                .when_some(icon, |cell, elixir| {
                    cell.child(asset_image(
                        ("spelunk-elixir-icon", slot_id),
                        &state.assets,
                        &format!("spelunking/CaveElix{}.png", elixir.id),
                        px(24.0),
                        cx,
                    ))
                })
                .child(select),
        );
    }
    row.into_any_element()
}
