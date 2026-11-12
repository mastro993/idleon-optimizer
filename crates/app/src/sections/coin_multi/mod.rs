//! The Coin Multiplier Calculator section: the character selector, the current
//! calculation cards and the partial state.
//!
//! Ported from `coin-multi-calc.html`: the character select, the six cards and
//! the partial badge. The cards read the save through
//! [`idleon_coin_multi`], which resolves the groups this build ports and
//! names the ones it does not, so the badge always explains itself. The damage
//! card is the one editable value: upstream resolves it through the damage
//! descriptor, which this build does not carry yet, so the field opens on the
//! save's highest recorded hit and the overkill tier follows what it holds.

mod actions;

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Hsla, IntoElement, ParentElement, Role, Styled, Window, div,
    px,
};
use idleon_coin_multi::{self as coin_multi, Character, Evaluation};
use idleon_import::Imported;

/// The section's state: the save slice, the selected character and the damage
/// field.
pub(crate) struct State {
    /// Every character's cards, from the last applied save.
    pub(crate) snapshot: Option<coin_multi::Snapshot>,
    pub(crate) character: usize,
    /// The editable damage value; the save's highest recorded hit prefills it.
    pub(crate) damage: Entity<TextInput>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let damage = crate::ui::text_input("Maximum damage", "Computed damage", window, cx);
        Self {
            snapshot: None,
            character: 0,
            damage,
        }
    }

    /// Take a freshly imported save: character one, its saved target and the
    /// save's highest recorded damage, exactly like upstream's `onLoad`.
    pub(crate) fn load(&mut self, imported: &Imported, cx: &mut Context<AppState>) {
        self.character = 0;
        let prefill = imported
            .coin
            .character(0)
            .map(|character| character.highest_damage)
            .unwrap_or(0.0);
        let text = format!("{prefill}");
        self.damage.update(cx, |input, cx| input.set_text(text, cx));
        self.snapshot = Some(imported.coin.clone());
    }

    /// The selected character's cards, when the save carries it.
    pub(crate) fn character(&self) -> Option<&Character> {
        self.snapshot.as_ref()?.character(self.character)
    }

    /// One character's choices for the selector.
    pub(crate) fn choices(&self) -> Vec<Choice> {
        match self.snapshot.as_ref() {
            None => Vec::new(),
            Some(snapshot) => snapshot
                .characters
                .iter()
                .enumerate()
                .map(|(index, character)| {
                    let mut choice = Choice::new(index.to_string(), character.name.clone());
                    if !character.class_name.is_empty() && character.class_name != "0" {
                        choice = choice.note(character.class_name.replace('_', " "));
                    }
                    choice
                })
                .collect(),
        }
    }

    /// The damage field's value, or why it cannot be read.
    pub(crate) fn damage_value(&self, cx: &App) -> Result<f64, String> {
        let text = self.damage.read(cx).text().trim().to_string();
        if text.is_empty() {
            return Err("Damage is required; the overkill tier and the total read it.".into());
        }
        match text.parse::<f64>() {
            Ok(value) if value.is_finite() && value >= 0.0 => Ok(value),
            Ok(_) => Err("Damage must be a non-negative number.".into()),
            Err(_) => Err(format!("{text} is not a number.")),
        }
    }

    /// The cards' calculation for the selected character and damage.
    pub(crate) fn evaluation(&self, cx: &App) -> Result<Evaluation, String> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Err("Load a save to calculate the coin multiplier.".into());
        };
        let mut inputs = snapshot
            .inputs(self.character)
            .ok_or_else(|| format!("Character {} is not in the save.", self.character + 1))?;
        inputs.damage = self.damage_value(cx)?;
        coin_multi::evaluate(snapshot, &inputs)
    }
}

/// The section page.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let snapshot_missing = state.coin_multi.snapshot.is_none();
    let choices = state.coin_multi.choices();
    let selected = state.coin_multi.character;
    let character = entity.clone();
    let damage_focus = state.coin_multi.damage.read(cx).focus().clone();
    let evaluation = state.coin_multi.evaluation(cx);
    let character_row = state.coin_multi.character().cloned();

    let selector = div()
        .id("coin-context-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(
            field("Character", muted).child(
                Select::new("coin-character", choices)
                    .label("Character")
                    .placeholder("Load a save first")
                    .selected(selected.to_string())
                    .disabled(snapshot_missing)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            character.update(cx, |state, cx| state.set_coin_character(index, cx));
                        }
                    }),
            ),
        )
        .child(
            div()
                .id("coin-damage-input")
                .role(Role::Label)
                .aria_label("Computed damage")
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(220.0))
                .child(Caption::new("Computed damage").text_color(muted))
                .on_click(move |_event, window, cx| window.focus(&damage_focus, cx))
                .child(state.coin_multi.damage.clone()),
        );

    let character_note = match (&character_row, &evaluation) {
        (None, _) => "Load a save to calculate the coin multiplier.".to_string(),
        (Some(character), Ok(evaluation)) => format!(
            "{} · {} · overkill tier {} ({}× thresholds)",
            character.name,
            character.class_name,
            evaluation.overkill.tier,
            evaluation.overkill.exponent
        ),
        (Some(character), Err(error)) => format!("{} · {error}", character.name),
    };

    let mut page = div()
        .id("coin-multi-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(
            Card::new()
                .header(CardHeader::new("Calculation Context").description(
                    "The saved character's map, AFK target and combat values, which every card and the total read.",
                ))
                .child(selector)
                .child(
                    div()
                        .id("coin-context-note")
                        .role(Role::Label)
                        .aria_label(character_note.clone())
                        .px_3()
                        .pb_3()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(character_note),
                ),
        );

    let missing = character_row.is_none();
    let cards = cards(
        character_row.as_ref(),
        evaluation.as_ref().ok(),
        snapshot_missing,
        muted,
    );
    page = page.child(
        Card::new()
            .header(CardHeader::new("Current Calculation").description(
                "The saved target's HP, the damage the overkill tier compares, the tier itself and the total multiplier.",
            ))
            .child(cards),
    );

    match evaluation {
        Ok(evaluation) => {
            page = page.child(partial_card(&evaluation, muted));
        }
        Err(error) => {
            page = page.child(
                div()
                    .id("coin-total-state")
                    .role(Role::Label)
                    .aria_label(error.clone())
                    .px_3()
                    .pb_3()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(error),
            );
        }
    }
    if missing && !snapshot_missing {
        page = page.child(
            div()
                .id("coin-missing-character")
                .role(Role::Label)
                .px_3()
                .pb_3()
                .text_size(px(12.0))
                .text_color(muted)
                .child("The save has no rows for this character."),
        );
    }
    page.into_any_element()
}

/// The six current-calculation cards, in upstream's order.
fn cards(
    character: Option<&Character>,
    evaluation: Option<&Evaluation>,
    snapshot_missing: bool,
    muted: Hsla,
) -> AnyElement {
    let overkill = evaluation.map(|evaluation| evaluation.overkill.tier);
    let partial = evaluation
        .map(|evaluation| evaluation.overkill.partial)
        .unwrap_or(false);
    let damage = evaluation.map(|evaluation| evaluation.overkill.damage);

    let mut grid = div().id("coin-cards").flex().flex_wrap().gap_3().p_3();
    grid = grid.child(text_card(
        "coin-map",
        "Current Map",
        &character
            .map(|character| character.map_name.clone())
            .unwrap_or_default(),
        &character
            .map(|character| format!("Saved location for {}", character.name))
            .unwrap_or_default(),
        muted,
    ));
    grid = grid.child(text_card(
        "coin-target",
        "Saved AFK Target",
        &character
            .map(|character| character.target_name.clone())
            .unwrap_or_default(),
        &character
            .map(|character| {
                if character.target.is_empty() {
                    "No saved AFK target".to_string()
                } else {
                    "Saved AFK target".to_string()
                }
            })
            .unwrap_or_default(),
        muted,
    ));
    let hp = character
        .map(|character| character.monster_hp)
        .filter(|hp| *hp > 0.0);
    grid = grid.child(figure_card(
        "coin-hp",
        "Monster HP",
        hp,
        &match (hp, character) {
            (Some(_), Some(character)) => format!("Target: {}", character.target_name),
            _ => "Monster data unavailable".to_string(),
        },
        muted,
    ));
    grid = grid.child(figure_card(
        "coin-damage",
        "Computed Damage",
        damage,
        "From the damage field above",
        muted,
    ));
    let tier_text = match (overkill, partial) {
        (Some(tier), false) => Some(format!("{tier}")),
        _ => None,
    };
    grid = grid.child(text_card(
        "coin-overkill",
        "Overkill Tier",
        &tier_text.unwrap_or_else(|| "Unavailable".into()),
        if snapshot_missing {
            ""
        } else if partial {
            "Saved target data unavailable"
        } else {
            "Tier progression"
        },
        muted,
    ));
    let total = evaluation
        .map(|evaluation| evaluation.total)
        .filter(|_| !snapshot_missing);
    let caption = evaluation
        .map(|evaluation| {
            format!(
                "Lower bound: {} of {} sources resolved",
                evaluation.groups.len() - evaluation.excluded.len(),
                evaluation.groups.len()
            )
        })
        .unwrap_or_default();
    let total_text = total
        .map(|total| format!("{}x", idleon_ui::format::monster_cash(total)))
        .unwrap_or_else(|| "Unavailable".into());
    let partial_state = evaluation
        .map(|evaluation| !evaluation.complete)
        .unwrap_or(false);
    grid.child(
        div()
            .flex_1()
            .min_w(px(190.0))
            .child(
                Card::new()
                    .header(CardHeader::new("Total Coin Multiplier"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .px_3()
                            .pb_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("coin-total")
                                            .role(Role::Label)
                                            .aria_label(total_text.clone())
                                            .text_size(px(22.0))
                                            .child(total_text),
                                    )
                                    .when(partial_state, |row| {
                                        row.child(Badge::new("Partial").tone(Tone::Warning))
                                    }),
                            )
                            .child(Caption::new(caption).text_color(muted)),
                    ),
            )
            .into_any_element(),
    )
    .into_any_element()
}

/// The explanation the partial badge carries: which groups are missing and why.
fn partial_card(evaluation: &Evaluation, muted: Hsla) -> AnyElement {
    if evaluation.complete && evaluation.warnings.is_empty() {
        return div().into_any_element();
    }
    let mut card = Card::new().header(CardHeader::new("Partial State").description(
        "Where the total is incomplete, and why. The badge marks a total that counts only the resolved sources.",
    ));
    if !evaluation.complete {
        card = card.child(
            div()
                .id("coin-partial")
                .role(Role::Label)
                .aria_label(format!(
                    "Partial total: {} of 23 sources are not modelled yet, so the figure is a lower bound.",
                    evaluation.excluded.len()
                ))
                .px_3()
                .child(
                    Caption::new(format!(
                        "Partial total: {} of 23 sources are not modelled yet, so the figure is a lower bound.",
                        evaluation.excluded.len()
                    ))
                    .text_color(muted),
                ),
        );
        for group in evaluation
            .groups
            .iter()
            .filter(|group| !group.is_modelled())
        {
            card = card.child(
                div()
                    .px_3()
                    .pb_1()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(format!("{}: {}.", group.name, group.reason)),
            );
        }
    }
    for warning in &evaluation.warnings {
        card = card.child(
            div()
                .id("coin-warning")
                .role(Role::Label)
                .aria_label(warning.clone())
                .px_3()
                .pb_1()
                .text_size(px(12.0))
                .text_color(muted)
                .child(warning.clone()),
        );
    }
    div().child(card).into_any_element()
}

/// A labelled field around a control.
fn field(label: &'static str, muted: Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(190.0))
        .child(Caption::new(label).text_color(muted))
}

/// A card whose value is text, e.g. the map and target names.
fn text_card(
    id: &'static str,
    title: &'static str,
    value: &str,
    caption: &str,
    muted: Hsla,
) -> AnyElement {
    let value = if value.is_empty() {
        "Unavailable"
    } else {
        value
    };
    div()
        .flex_1()
        .min_w(px(190.0))
        .child(
            Card::new().header(CardHeader::new(title)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .pb_3()
                    .child(
                        div()
                            .id(id)
                            .role(Role::Label)
                            .aria_label(format!("{title}: {value}"))
                            .child(value.to_string()),
                    )
                    .child(Caption::new(caption.to_string()).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// A card whose value is a figure, shown with the shared number formatter.
fn figure_card(
    id: &'static str,
    title: &'static str,
    value: Option<f64>,
    caption: &str,
    muted: Hsla,
) -> AnyElement {
    let text = match value {
        Some(value) => idleon_ui::format::fmt_val(value),
        None => "Unavailable".to_string(),
    };
    text_card(id, title, &text, caption, muted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn imported(save: serde_json::Value) -> Imported {
        idleon_import::import(&json!({ "data": save }).to_string()).expect("the save imports")
    }

    fn save() -> serde_json::Value {
        json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[1]",
            "Lv0_1": "[1]",
            "CharacterClass_0": 1,
            "CharacterClass_1": 7,
            "CurrentMap_0": 2,
            "CurrentMap_1": 50,
            "AFKtarget_0": "frogG",
            "AFKtarget_1": "Nothing",
            "Research": "[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]",
            "Holes": "[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]",
            "TaskZZ0": "[[],[123456789]]",
        })
    }

    #[test]
    fn a_load_opens_on_character_one_and_the_saved_target() {
        let imported = imported(save());
        let coin = imported.coin;
        assert_eq!(coin.characters.len(), 2);
        assert_eq!(coin.characters[0].name, "Ana");
        assert_eq!(coin.characters[0].target, "frogG");
        assert_eq!(coin.characters[0].monster_hp, 120.0);
        assert_eq!(coin.characters[0].highest_damage, 123_456_789.0);
        assert_eq!(coin.characters[1].class_name, "WARRIOR");
        assert_eq!(coin.characters[1].map, 50);
    }

    #[test]
    fn the_damage_prefill_is_the_save_highest_hit() {
        let imported = imported(save());
        assert_eq!(
            imported.coin.inputs(0).map(|inputs| inputs.damage),
            Some(123_456_789.0)
        );
    }

    #[test]
    fn an_unknown_target_reads_as_unavailable() {
        let imported = imported(save());
        assert_eq!(imported.coin.characters[1].target_name, "Unavailable");
    }
}
