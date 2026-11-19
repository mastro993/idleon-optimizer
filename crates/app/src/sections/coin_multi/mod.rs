//! The Coin Multiplier Calculator section: the character selector, the current
//! calculation strips, the partial state and the formula tree behind the total.
//!
//! The cards read the save through [`idleon_coin_multi`], which resolves all 23
//! groups for a full save and names any group it cannot resolve, so a partial
//! badge always explains itself and the breakdown lists every term. The damage
//! card is the one editable value: the full model resolves it through the
//! damage descriptor, which this build does not carry yet, so the field opens
//! on the save's highest recorded hit and the overkill tier follows what it
//! holds.
//!
//! The evaluation is read once per change of the save, the character or the
//! damage text ([`State::refresh`]), so a frame only formats it.

mod actions;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, Hsla, IntoElement, ParentElement, Role,
    SharedString, Styled, Subscription, Window, div, px,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_coin_multi::{self as coin_multi, Character, Evaluation};
use idleon_import::Imported;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, KpiStrip, PANEL_GAP, Statistic, TEXT_CAPTION,
    Tone,
};

/// The section's state: the save slice, the selected character, the damage
/// text, what they evaluate to, and the kit controls that show them.
pub(crate) struct State {
    /// Every character's cards, from the last applied save.
    pub(crate) snapshot: Option<coin_multi::Snapshot>,
    pub(crate) character: usize,
    /// The damage field's text; the save's highest recorded hit prefills it.
    pub(crate) damage: String,
    /// The cards' calculation for `character` and `damage`.
    evaluation: Result<Evaluation, String>,
    /// The character selector's rows, built once per save.
    choices: Vec<Choice<usize>>,
    character_select: ChoiceSelect<usize>,
    damage_field: Entity<InputState>,
    /// The `#tree` breakdown behind the total card.
    pub(crate) tree: Entity<FormulaTree>,
    _subscriptions: [Subscription; 2],
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let character_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let damage_field = cx.new(|cx| InputState::new(window, cx).placeholder("Maximum damage"));
        let subscriptions = [
            cx.subscribe(
                character_select.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    let SelectEvent::Confirm(Some(character)) = event else {
                        return;
                    };
                    state.set_coin_character(*character, cx);
                },
            ),
            cx.subscribe(&damage_field, |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = field.read(cx).value().to_string();
                    state.set_coin_damage(text, cx);
                }
            }),
        ];
        Self {
            snapshot: None,
            character: 0,
            damage: String::new(),
            evaluation: Err(NO_SAVE.into()),
            choices: Vec::new(),
            character_select,
            damage_field,
            tree: crate::ui::formula_tree("coin-formula", window, cx),
            _subscriptions: subscriptions,
        }
    }

    /// Take a freshly imported save: character one, its saved target and the
    /// save's highest recorded damage.
    pub(crate) fn load(&mut self, imported: &Imported, cx: &mut App) {
        self.character = 0;
        let prefill = imported
            .coin
            .character(0)
            .map(|character| character.highest_damage)
            .unwrap_or(0.0);
        self.damage = format!("{prefill}");
        self.choices = choices(&imported.coin);
        self.snapshot = Some(imported.coin.clone());
        self.refresh(cx);
    }

    /// Read the cards' calculation again and hand its tree to the breakdown.
    pub(crate) fn refresh(&mut self, cx: &mut App) {
        self.evaluation = self.evaluate();
        if let Ok(evaluation) = &self.evaluation {
            let root = coin_multi::tree(evaluation);
            self.tree.update(cx, |tree, cx| tree.set_root(root, cx));
        }
    }

    /// The selected character's cards, when the save carries it.
    pub(crate) fn character(&self) -> Option<&Character> {
        self.snapshot.as_ref()?.character(self.character)
    }

    /// The damage field's value, or why it cannot be read.
    fn damage_value(&self) -> Result<f64, String> {
        let text = self.damage.trim();
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
    fn evaluate(&self) -> Result<Evaluation, String> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Err(NO_SAVE.into());
        };
        let mut inputs = snapshot
            .inputs(self.character)
            .ok_or_else(|| format!("Character {} is not in the save.", self.character + 1))?;
        inputs.damage = self.damage_value()?;
        coin_multi::evaluate(snapshot, &inputs)
    }

    /// Bring the selector and the damage field in line with the section's
    /// values (`docs/ui-kit/forms.md`); a field the user is typing in is left
    /// alone.
    fn show(&self, window: &mut Window, cx: &mut App) {
        self.character_select
            .show(self.choices.clone(), Some(&self.character), window, cx);
        let field = &self.damage_field;
        if !field.focus_handle(cx).is_focused(window) && field.read(cx).value() != self.damage {
            let text = self.damage.clone();
            field.update(cx, |field, cx| field.set_value(text, window, cx));
        }
    }
}

const NO_SAVE: &str = "Load a save to calculate the coin multiplier.";

/// One choice per character, its class beside its name.
fn choices(snapshot: &coin_multi::Snapshot) -> Vec<Choice<usize>> {
    snapshot
        .characters
        .iter()
        .enumerate()
        .map(|(index, character)| {
            let label = if character.class_name.is_empty() || character.class_name == "0" {
                character.name.clone()
            } else {
                format!(
                    "{} · {}",
                    character.name,
                    character.class_name.replace('_', " ")
                )
            };
            Choice::new(index, label)
        })
        .collect()
}

/// The section page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let coin = &state.coin_multi;
    coin.show(window, cx);
    let muted = cx.theme().muted_foreground;
    let snapshot_missing = coin.snapshot.is_none();
    let damage_focus = coin.damage_field.focus_handle(cx);
    let evaluation = coin.evaluation.as_ref();
    let character_row = coin.character();

    let selector = div()
        .id("coin-context-controls")
        .flex()
        .items_end()
        .gap_3()
        .child(
            field("Character", muted).child(
                div().flex_none().w(px(220.0)).child(
                    Select::new(coin.character_select.state())
                        .id("coin-character")
                        .accessibility_label("Character")
                        .placeholder("Load a save first")
                        .disabled(snapshot_missing)
                        .small(),
                ),
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
                .w(px(220.0))
                .child(Caption::new("Computed damage").text_color(muted))
                .on_click(move |_event, window, cx| window.focus(&damage_focus, cx))
                .child(
                    Input::new(&coin.damage_field)
                        .aria_label("Computed damage")
                        .small(),
                ),
        );

    let character_note = match (character_row, evaluation) {
        (None, _) => NO_SAVE.to_string(),
        (Some(character), Ok(evaluation)) => format!(
            "{} · {} · overkill tier {} ({}× thresholds)",
            character.name,
            character.class_name,
            evaluation.overkill.tier,
            evaluation.overkill.exponent
        ),
        (Some(character), Err(error)) => format!("{} · {error}", character.name),
    };

    let mut page = div().flex().flex_col().gap(PANEL_GAP).child(
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
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(character_note),
            ),
    );

    let missing = character_row.is_none();
    page = page.child(
        Card::new()
            .header(CardHeader::new("Current Calculation").description(
                "The saved target's HP, the damage the overkill tier compares, the tier itself and the total multiplier.",
            ))
            .child(cards(character_row, evaluation.ok(), snapshot_missing)),
    );

    match evaluation {
        Ok(evaluation) => {
            page = page
                .child(partial_card(evaluation, muted))
                .child(formula_card(coin.tree.clone()));
        }
        Err(error) => {
            page = page
                .child(
                    div()
                        .id("coin-total-state")
                        .role(Role::Label)
                        .aria_label(error.clone())
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(error.clone()),
                )
                .child(formula_gap(error, muted));
        }
    }
    if missing && !snapshot_missing {
        page = page.child(
            div()
                .id("coin-missing-character")
                .role(Role::Label)
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child("The save has no rows for this character."),
        );
    }
    div()
        .id("coin-multi-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(page)
                .overflow_y_scrollbar()
                .id("coin-multi-scroll"),
        )
        .into_any_element()
}

/// The shared formula tree in its card, sized like the other sections'
/// breakdown cards.
fn formula_card(tree: Entity<FormulaTree>) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Formula Breakdown")
                .description("The 23 groups behind the total, each with the source that owns it."),
        )
        .child(
            div()
                .id("coin-formula-card-body")
                .h(px(320.0))
                .min_h_0()
                .child(tree),
        )
        .into_any_element()
}

/// The tree card's empty state: there is no total to expand yet.
fn formula_gap(message: &str, muted: Hsla) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Formula Breakdown")
                .description("Nothing to expand until the total resolves."),
        )
        .child(
            div()
                .id("coin-formula-gap")
                .role(Role::Label)
                .aria_label(message.to_string())
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(message.to_string()),
        )
        .into_any_element()
}

/// The six current-calculation figures: the saved target in one strip, the
/// overkill inputs and the total in the next.
fn cards(
    character: Option<&Character>,
    evaluation: Option<&Evaluation>,
    snapshot_missing: bool,
) -> AnyElement {
    let overkill = evaluation.map(|evaluation| evaluation.overkill.tier);
    let partial = evaluation
        .map(|evaluation| evaluation.overkill.partial)
        .unwrap_or(false);
    let damage = evaluation.map(|evaluation| evaluation.overkill.damage);

    let map = text_figure(
        "coin-map",
        "Current Map",
        character.map(|character| character.map_name.clone()),
    )
    .caption(
        character
            .map(|character| format!("Saved location for {}", character.name))
            .unwrap_or_default(),
    );
    let target = text_figure(
        "coin-target",
        "Saved AFK Target",
        character.map(|character| character.target_name.clone()),
    )
    .caption(
        character
            .map(|character| {
                if character.target.is_empty() {
                    "No saved AFK target"
                } else {
                    "Saved AFK target"
                }
            })
            .unwrap_or_default(),
    );
    let hp = character
        .map(|character| character.monster_hp)
        .filter(|hp| *hp > 0.0);
    let monster_hp = number_figure("coin-hp", "Monster HP", hp).caption(match (hp, character) {
        (Some(_), Some(character)) => format!("Target: {}", character.target_name),
        _ => "Monster data unavailable".to_string(),
    });
    let damage = number_figure("coin-damage", "Computed Damage", damage)
        .caption("From the damage field above");
    let tier_text = match (overkill, partial) {
        (Some(tier), false) => Some(tier.to_string()),
        _ => None,
    };
    let tier =
        text_figure("coin-overkill", "Overkill Tier", tier_text).caption(if snapshot_missing {
            ""
        } else if partial {
            "Saved target data unavailable"
        } else {
            "Tier progression"
        });
    let total = evaluation
        .map(|evaluation| evaluation.total)
        .filter(|_| !snapshot_missing);
    let caption = evaluation
        .map(|evaluation| {
            if evaluation.complete {
                format!("All {} sources resolved", evaluation.groups.len())
            } else {
                format!(
                    "Lower bound: {} of {} sources resolved",
                    evaluation.groups.len() - evaluation.excluded.len(),
                    evaluation.groups.len()
                )
            }
        })
        .unwrap_or_default();
    let total_text = total
        .map(|total| format!("{}x", idleon_ui::format::monster_cash(total)))
        .unwrap_or_else(|| "Unavailable".into());
    let partial_state = evaluation
        .map(|evaluation| !evaluation.complete)
        .unwrap_or(false);
    // The badge sits beside the figure, so the total's label stays its value.
    let total = h_flex()
        .items_start()
        .justify_between()
        .gap_2()
        .child(Statistic::new("coin-total", "Total Coin Multiplier", total_text).caption(caption))
        .when(partial_state, |cell| {
            cell.child(tag("Partial", Tone::Warning))
        });

    v_flex()
        .id("coin-cards")
        .gap(PANEL_GAP)
        .child(KpiStrip::new().child(map).child(target).child(monster_hp))
        .child(KpiStrip::new().child(damage).child(tier).child(total))
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
        let line = format!(
            "Partial total: {} of 23 sources are not modelled yet, so the figure is a lower bound.",
            evaluation.excluded.len()
        );
        card = card.child(
            div()
                .id("coin-partial")
                .role(Role::Label)
                .aria_label(line.clone())
                .child(Caption::new(line).text_color(muted)),
        );
        for group in evaluation
            .groups
            .iter()
            .filter(|group| !group.is_modelled())
        {
            card = card.child(
                div()
                    .text_size(TEXT_CAPTION)
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
                .text_size(TEXT_CAPTION)
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
        .child(Caption::new(label).text_color(muted))
}

/// A figure whose value is text, e.g. the map and target names.
fn text_figure(id: &'static str, label: &'static str, value: Option<String>) -> Statistic {
    let value = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unavailable".into());
    Statistic::new(id, label, value)
}

/// A figure whose value is a number, shown with the shared number formatter.
fn number_figure(id: &'static str, label: &'static str, value: Option<f64>) -> Statistic {
    text_figure(id, label, value.map(idleon_ui::format::fmt_val))
}

/// A small status tag.
fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
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
        let labels: Vec<_> = choices(&coin)
            .into_iter()
            .map(|choice| gpui_kit::component::searchable_list::SearchableListItem::title(&choice))
            .collect();
        assert_eq!(labels[1], "Bo · WARRIOR");
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
