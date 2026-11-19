//! The elixir belt: equipped and memorized slots, the built-in presets and the
//! selected effects.
//!
//! Each slot is a select with the unlocked elixirs. The duplicate and memory
//! rules are enforced by the normaliser as well, so an overfilled belt is
//! reported instead of silently rewritten.

use super::controls::Derived;
use super::{select, tag};
use crate::state::AppState;
use crate::ui::assets::asset_image;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_spelunking::{PresetKind, data};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, ChoiceSelect, TEXT_MONO, Tone};

/// The belt card.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let inputs = &derived.inputs;
    let spelunking = &state.spelunking;
    let controls = &state.spelunking_controls;
    let warning = cx.theme().warning;
    let equipped_count = spelunking.elixirs.iter().filter(|id| **id >= 0).count();
    let equipped = slots(
        state,
        derived,
        PresetKind::Belt,
        &controls.elixirs,
        window,
        cx,
    );
    let memory_count = spelunking.memorized.iter().filter(|id| **id >= 0).count();
    let memorized = slots(
        state,
        derived,
        PresetKind::Memory,
        &controls.memorized,
        window,
        cx,
    );
    let presets: Vec<AnyElement> = data()
        .elixir_presets
        .iter()
        .zip(&derived.preset_issues)
        .map(|(preset, issues)| {
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
                        .child(tag(kind, Tone::Info))
                        .child(div().text_size(TEXT_MONO).child(preset.label.clone()))
                        .child(
                            Button::new(format!("spelunk-preset-{}", preset.id))
                                .label("Apply")
                                .small()
                                .disabled(!issues.is_empty())
                                .on_click(move |_event, _window, cx| {
                                    apply_entity.update(cx, |state, cx| {
                                        state.apply_spelunk_preset(preset.clone(), cx)
                                    });
                                }),
                        ),
                )
                .when(!issues.is_empty(), |row| {
                    row.child(Caption::new(issues.join(" ")).text_color(warning))
                })
                .into_any_element()
        })
        .collect();
    let repeats = (1.0 + inputs.shop.get(58).copied().unwrap_or(0.0) / 100.0).floor();
    let mut selected: Vec<i32> = spelunking
        .elixirs
        .iter()
        .chain(spelunking.memorized.iter())
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
                    format::value(elixir.charges)
                )
            };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_size(TEXT_MONO)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(div().min_w_0().child(format!("{} ({})", elixir.name, note)))
                        .child(tag(
                            format!("{} charges", format::value(elixir.charges)),
                            Tone::Neutral,
                        )),
                )
                .child(Caption::new(elixir.description.clone()))
                .into_any_element()
        })
        .collect::<Vec<_>>();

    Card::new()
        .header(CardHeader::new("Elixir belt").description(
            "The equipped belt spends charges during a run; the memorized set applies at entry \
                 without spending them.",
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(slot_heading("Equipped", equipped_count))
                .child(equipped)
                .child(slot_heading("Memorized", memory_count))
                .child(memorized)
                .when(!effects.is_empty(), |card| {
                    card.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Caption::new("Selected elixir effects"))
                            .children(effects),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Caption::new("Built-in presets"))
                .child(div().flex().flex_wrap().gap_3().children(presets))
                .child(Caption::new(
                    "Each preset replaces only its half of the belt. These pictured loadouts are \
                     not optimizer recommendations.",
                )),
        )
        .into_any_element()
}

fn slot_heading(label: &'static str, count: usize) -> AnyElement {
    div()
        .text_size(TEXT_MONO)
        .child(format!("{label} ({count} selected)"))
        .into_any_element()
}

fn slots(
    state: &AppState,
    derived: &Derived,
    kind: PresetKind,
    controls: &[ChoiceSelect<i32>],
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let (choices, values, name, label, row_id) = match kind {
        PresetKind::Belt => (
            &derived.elixirs,
            &state.spelunking.elixirs,
            "elixir",
            "Equipped",
            "spelunk-belt",
        ),
        PresetKind::Memory => (
            &derived.memorized,
            &state.spelunking.memorized,
            "memorized",
            "Memorized",
            "spelunk-memory",
        ),
    };
    if choices.is_empty() {
        return Caption::new("Locked").into_any_element();
    }
    let mut row = div().id(row_id).flex().flex_wrap().gap_2();
    for (slot, items) in choices.iter().enumerate() {
        let current = values.get(slot).copied().unwrap_or(-1);
        let picker = select(
            &controls[slot],
            format!("spelunk-{name}-{slot}"),
            format!("{label} slot {}", slot + 1),
            items,
            &current,
            window,
            cx,
        );
        let icon = usize::try_from(current)
            .ok()
            .and_then(|id| derived.inputs.elixirs.get(id));
        let slot_id = slot as u64;
        row = row.child(
            div()
                .id(("spelunk-elixir-slot", slot_id))
                .flex()
                .items_center()
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
                .child(div().flex_none().w(px(170.0)).h_6().child(picker)),
        );
    }
    row.into_any_element()
}
