//! The character, toolkit and claim-length toolbar, with the saved-default reset.

use crate::state::AppState;
use crate::ui::layout::Breakpoint;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_core::fishing::{self, FishingSave};

/// The character, toolkit, location and claim length, with the account card's
/// values applied to the selected toolkit.
pub fn pane(
    state: &mut AppState,
    save: &FishingSave,
    breakpoint: Breakpoint,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let data = fishing::data();
    let config = &state.fishing;
    let character = save.character(config.character);
    let characters: Vec<Choice> = if save.characters.is_empty() {
        vec![Choice::new("0", "Character 1 · no save")]
    } else {
        save.characters
            .iter()
            .map(|character| {
                Choice::new(
                    character.index.to_string(),
                    format!("{} · Lv {}", character.name, character.level),
                )
                .note(format!(
                    "{}{}",
                    character.rod_name,
                    character
                        .saved_location()
                        .map(|location| format!(" · {}", location.label))
                        .unwrap_or_default()
                ))
            })
            .collect()
    };
    let character_entity = entity.clone();
    let character_select = Select::new("fishing-character", characters)
        .label("Character")
        .selected(config.character.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                character_entity.update(cx, |state, cx| state.set_fishing_character(index, cx));
            }
        });
    let lures: Vec<Choice> = data
        .lures
        .iter()
        .map(|lure| {
            Choice::new(lure.idx.to_string(), lure.name.clone()).note(stats_text(&lure.stats))
        })
        .collect();
    let lure_entity = entity.clone();
    let lure_select = Select::new("fishing-lure", lures)
        .label("Lure")
        .selected(config.lure.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                lure_entity.update(cx, |state, cx| state.set_fishing_lure(index, cx));
            }
        });
    let lines: Vec<Choice> = data
        .lines
        .iter()
        .map(|line| {
            Choice::new(line.idx.to_string(), line.name.clone()).note(stats_text(&line.stats))
        })
        .collect();
    let line_entity = entity.clone();
    let line_select = Select::new("fishing-line", lines)
        .label("Line")
        .selected(config.line.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                line_entity.update(cx, |state, cx| state.set_fishing_line(index, cx));
            }
        });
    let locations: Vec<Choice> = data
        .locations
        .iter()
        .map(|location| {
            Choice::new(location.id.clone(), location.label.clone()).note(format!(
                "{} · {} · depth {}",
                location.pool_name,
                location.map_name,
                depth_text(&location.bonus)
            ))
        })
        .collect();
    let location_entity = entity.clone();
    let location_select = Select::new("fishing-location", locations)
        .label("Fishing location")
        .selected(config.location.clone())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            location_entity.update(cx, |state, cx| {
                state.set_fishing_location(value.to_string(), cx)
            });
        });
    let hours_entity = entity;
    let hours = Stepper::new("fishing-hours", config.hours)
        .range(0.04, 10_000.0)
        .step(0.5)
        .precision(2)
        .on_change(move |value, _window, cx| {
            hours_entity.update(cx, |state, cx| state.set_fishing_hours(value, cx));
        });

    Card::new()
        .header(
            CardHeader::new("Toolbar and character context").description(
                "The saved toolkit, spot and rod define the defaults; the claim length is hours away.",
            ),
        )
        .child(
            div()
                .id("fishing-toolbar")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .p_3()
                .when(breakpoint.is_compact(), |row| row.gap_3())
                .child(field("Character", character_select.into_any_element()))
                .child(field("Lure", lure_select.into_any_element()))
                .child(field("Line", line_select.into_any_element()))
                .child(field("Fishing location", location_select.into_any_element()))
                .child(field("Hours away", hours.into_any_element()))
                .when_some(character, |row, character| {
                    let icon = (character.rod_key != "Blank")
                        .then(|| format!("combat-sim/ui/items/{}.png", character.rod_key));
                    row.child(
                        div()
                            .id("fishing-rod")
                            .role(Role::Label)
                            .aria_label(character.rod_name.clone())
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Caption::new("Rod").text_color(cx.theme().colors.fg_muted))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when_some(icon, |cell, reference| {
                                        cell.child(crate::ui::assets::asset_image(
                                            "fishing-rod-icon",
                                            &state.assets,
                                            &reference,
                                            px(28.0),
                                            cx,
                                        ))
                                    })
                                    .child(
                                        div().text_size(px(12.0)).child(format!(
                                            "{} · speed {}",
                                            character.rod_name, character.rod_speed
                                        )),
                                    ),
                            ),
                    )
                })
        )
        .into_any_element()
}

fn field(label: &'static str, control: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// The shared formula tree under a fixed height, so its own scroll area works.
pub(super) fn formula_card(tree: &gpui::Entity<crate::ui::formula::FormulaTree>) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Breakdown")
                .description("Catch time, split, fish, real EXP and the panel comparison."),
        )
        .child(
            div()
                .id("fishing-breakdown")
                .h(px(420.0))
                .min_h_0()
                .child(tree.clone()),
        )
        .into_any_element()
}

/// The toolkit stats the selectors list, as the upstream `toolkitStatText`.
fn stats_text(stats: &fishing::ToolkitStats) -> String {
    [
        ("D0", stats.d0),
        ("D1", stats.d1),
        ("D2", stats.d2),
        ("D3", stats.d3),
        ("EXP", stats.exp),
        ("SPEED", stats.speed),
        ("POW", stats.pow),
    ]
    .iter()
    .filter(|(_, value)| *value != 0.0)
    .map(|(name, value)| format!("{name} {value}"))
    .collect::<Vec<_>>()
    .join(" · ")
}

/// One location's spot depth bonus as `D1 +25, D2 +5`.
fn depth_text(bonus: &[f64; 4]) -> String {
    let parts: Vec<String> = bonus
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0.0)
        .map(|(tier, value)| format!("D{tier} +{value}"))
        .collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(", ")
    }
}

/// The upstream "Reset to saved" action.
pub fn reset(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Button::new("fishing-reset", "Reset to saved")
        .size(ControlSize::Sm)
        .icon(IconName::RefreshCw)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.fishing_reset(cx));
        })
        .into_any_element()
}
