//! The Drop Rate Calculator: the character and map context, the live dungeon
//! inputs and the total card.
//!
//! Ported from `drop-rate-calc.html`: the context card that
//! `populateMapSel`/`switchChar`/`switchMap` drive, the runtime panel that only
//! a dungeon map shows, and the `#result-card` total. The formula tree below
//! them is DR-02; the maths the total reads lives in [`idleon_core::drop_rate`].

use crate::state::AppState;
use ely_gpui_component::forms::{Choice, Input, InputEvent, Select, TextInput};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Hsla, ParentElement, Role, Styled, Subscription,
    Window, div, px,
};
use idleon_core::drop_rate::{self, Runtime, Selection, Total};
use idleon_core::save::Save;
use idleon_core::stats::{self, Snapshot};

/// The page's own state: the context, the save slice it reads and the two live
/// fields a dungeon run shows.
pub struct State {
    pub selection: Selection,
    pub snapshot: Option<Snapshot>,
    /// Dungeon Total LUK; a blank field is upstream's "not entered".
    pub luk: Entity<TextInput>,
    /// Dungeon Drop Rate stat.
    pub rate: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

impl State {
    /// The page with empty fields and no save.
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let (luk, luk_subscription) = live_field("Dungeon Total LUK", "e.g. 1200", window, cx);
        let (rate, rate_subscription) = live_field("Dungeon Drop Rate Stat", "e.g. 35", window, cx);
        Self {
            selection: Selection::default(),
            snapshot: None,
            luk,
            rate,
            _subscriptions: vec![luk_subscription, rate_subscription],
        }
    }

    /// Take a freshly imported save: character one on its current map.
    pub fn load(&mut self, save: &Save) {
        let snapshot = Snapshot::from_save(save);
        self.selection = Selection::from_snapshot(&snapshot);
        self.snapshot = Some(snapshot);
    }

    /// The live dungeon values as the fields hold them.
    pub fn runtime(&self, cx: &App) -> Result<Runtime, String> {
        Runtime::read(self.luk.read(cx).text(), self.rate.read(cx).text())
    }

    /// The total card's answer for the current context.
    pub fn total(&self, cx: &App) -> Total {
        match self.runtime(cx) {
            Ok(runtime) => drop_rate::total(self.selection.map, runtime),
            Err(reason) => Total::Invalid(reason),
        }
    }

    /// The character selector's rows: every character with its class.
    pub fn characters(&self) -> Vec<Choice> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Vec::new();
        };
        (0..snapshot.characters())
            .map(|character| {
                let class = stats::class_name(snapshot.class_id(character)).replace('_', " ");
                Choice::new(
                    character.to_string(),
                    format!("{} ({class})", snapshot.name(character)),
                )
            })
            .collect()
    }

    /// The map selector's rows, marked when they are dungeons.
    pub fn maps(&self) -> Vec<Choice> {
        drop_rate::maps(self.snapshot.as_ref(), self.selection.character)
            .into_iter()
            .map(|row| Choice::new(row.map.to_string(), row.label))
            .collect()
    }

    /// Whether the selected map runs the live dungeon formula.
    pub fn dungeon(&self) -> bool {
        drop_rate::is_dungeon_map(self.selection.map)
    }
}

/// One live dungeon field: digits and a decimal point, blank until entered.
fn live_field(
    label: &'static str,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> (Entity<TextInput>, Subscription) {
    let input = cx.new(|cx| {
        TextInput::new(window, cx)
            .filter(|ch| ch.is_ascii_digit() || ch == '.')
            .placeholder(placeholder)
            .label(label)
    });
    let subscription = cx.subscribe(&input, |_state, _input, event: &InputEvent, cx| {
        if *event == InputEvent::Changed {
            cx.notify();
        }
    });
    (input, subscription)
}

/// The Drop Rate page: the context, the live dungeon fields and the total.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let (muted, gold, danger) = (colors.fg_muted, colors.warning, colors.danger);
    let selection = state.drop_rate.selection;
    let character = state
        .drop_rate
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.name(selection.character));
    let loaded = character.is_some();
    let total = state.drop_rate.total(cx);
    let runtime = state.drop_rate.runtime(cx).ok();
    let out_of_range =
        |value: Option<f64>| value.is_some_and(|value| !value.is_finite() || value < 0.0);
    let luk_invalid = out_of_range(runtime.and_then(|runtime| runtime.luk));
    let rate_invalid = out_of_range(runtime.and_then(|runtime| runtime.drop_rate));
    let characters = state.drop_rate.characters();
    let maps = state.drop_rate.maps();
    let dungeon = state.drop_rate.dungeon();
    let luk = state.drop_rate.luk.clone();
    let rate = state.drop_rate.rate.clone();
    let entity = cx.entity();
    let character_pick = entity.clone();
    let map_pick = entity;

    let controls = div()
        .id("drop-rate-context-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(
            field("Character", muted).child(
                Select::new("drop-rate-character", characters)
                    .label("Character")
                    .placeholder("Load a save first")
                    .selected(selection.character.to_string())
                    .disabled(!loaded)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            character_pick
                                .update(cx, |state, cx| state.set_drop_rate_character(index, cx));
                        }
                    }),
            ),
        )
        .child(
            field("Map", muted).child(
                Select::new("drop-rate-map", maps)
                    .label("Map")
                    .placeholder("Load a save first")
                    .selected(selection.map.to_string())
                    .disabled(!loaded)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            map_pick.update(cx, |state, cx| state.set_drop_rate_map(index, cx));
                        }
                    }),
            ),
        );

    let note = context_note(character.as_deref(), selection);
    div()
        .id("drop-rate-calculator")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(
            Card::new()
                .header(CardHeader::new("Calculation Context").description(
                    "The drop-rate descriptor resolves against this character and map.",
                ))
                .child(controls)
                .when(dungeon, |card| {
                    card.child(dungeon_fields(
                        &luk,
                        &rate,
                        luk_invalid,
                        rate_invalid,
                        muted,
                        colors.sunken,
                    ))
                })
                .child(
                    div()
                        .id("drop-rate-context-note")
                        .role(Role::Label)
                        .aria_label(note.clone())
                        .px_3()
                        .pb_3()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note),
                ),
        )
        .child(total_card(
            &total,
            character.as_deref(),
            selection,
            loaded,
            muted,
            gold,
            danger,
        ))
        .into_any_element()
}

/// The two live dungeon inputs, shown only while a dungeon map is selected.
fn dungeon_fields(
    luk: &Entity<TextInput>,
    rate: &Entity<TextInput>,
    luk_invalid: bool,
    rate_invalid: bool,
    muted: Hsla,
    sunken: Hsla,
) -> AnyElement {
    div()
        .id("drop-rate-dungeon-runtime")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .mx_3()
        .mb_3()
        .p_3()
        .rounded_md()
        .bg(sunken)
        .child(field("Dungeon Total LUK", muted).child(clickable_field(
            "drop-rate-luk",
            "Dungeon Total LUK",
            luk,
            luk_invalid,
        )))
        .child(
            field("Dungeon Drop Rate Stat", muted).child(clickable_field(
                "drop-rate-drop",
                "Dungeon Drop Rate Stat",
                rate,
                rate_invalid,
            )),
        )
        .child(
            Caption::new("These current-run values are not stored in save JSON.").text_color(muted),
        )
        .into_any_element()
}

/// One live field, clickable across its whole row so the harness and a pointer
/// can focus it by id.
fn clickable_field(
    id: &'static str,
    label: &'static str,
    input: &Entity<TextInput>,
    invalid: bool,
) -> AnyElement {
    let field = input.clone();
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(label)
        .on_click(move |_event, window, cx| {
            let focus = field.read(cx).focus().clone();
            window.focus(&focus, cx);
        })
        .child(Input::new(input).size(ControlSize::Sm).invalid(invalid))
        .into_any_element()
}

/// The line under the controls: who and where, or why there is nothing to say.
fn context_note(character: Option<&str>, selection: Selection) -> String {
    let Some(character) = character else {
        return "Load a save to pick a character and map.".into();
    };
    let class = stats::classify(selection.map);
    let run = if drop_rate::is_dungeon_map(selection.map) {
        " · live dungeon inputs apply"
    } else {
        ""
    };
    format!(
        "{character} · {} · {}{run}",
        class.name,
        class.category.label()
    )
}

/// The `#result-card` total: the multiplier, or the reason there is none.
fn total_card(
    total: &Total,
    character: Option<&str>,
    selection: Selection,
    loaded: bool,
    muted: Hsla,
    gold: Hsla,
    danger: Hsla,
) -> AnyElement {
    let headline = headline(total, loaded);
    let tone = if !loaded {
        muted
    } else if total.value().is_some() {
        gold
    } else {
        danger
    };
    let sub = sub_line(total, character, selection, loaded);

    Card::new()
        .header(CardHeader::new("Total Drop Rate Multiplier"))
        .child(
            div()
                .id("drop-rate-total")
                .role(Role::Label)
                .aria_label(format!("Total drop rate multiplier: {headline}"))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .py(px(24.0))
                .child(
                    div()
                        .id("drop-rate-total-value")
                        .text_size(px(48.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(tone)
                        .child(headline),
                )
                .child(
                    div()
                        .id("drop-rate-total-sub")
                        .role(Role::Label)
                        .aria_label(sub.clone())
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(sub),
                ),
        )
        .into_any_element()
}

/// The number the card shows in place of the multiplier, or the multiplier.
fn headline(total: &Total, loaded: bool) -> String {
    match total {
        _ if !loaded => "Load a save".into(),
        Total::Value(value) => format!("{value:.2}x"),
        _ => "Unavailable".into(),
    }
}

/// The line under the headline: the reason when there is no number, then who
/// and where the answer is for.
fn sub_line(total: &Total, character: Option<&str>, selection: Selection, loaded: bool) -> String {
    if !loaded {
        return "Select a character and map after loading.".into();
    }
    let reason = total
        .reason()
        .map(|reason| format!("{reason} · "))
        .unwrap_or_default();
    let character = character.unwrap_or_default();
    let map = stats::classify(selection.map).name;
    format!("{reason}Character: {character} · Map: {map}")
}

fn field(label: &'static str, muted: Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(190.0))
        .child(Caption::new(label).text_color(muted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::save::Save;
    use serde_json::json;

    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[1]",
            "Lv0_1": "[1]",
            "CharacterClass_0": 1,
            "CharacterClass_1": 7,
            "CurrentMap_0": 39,
            "CurrentMap_1": 0,
        }))
    }

    #[test]
    fn the_note_names_the_character_the_map_and_whether_it_is_a_dungeon() {
        let save = save();
        let snapshot = Snapshot::from_save(&save);
        let selection = Selection::from_snapshot(&snapshot);
        let note = context_note(Some("Ana"), selection);
        assert!(note.starts_with("Ana · "), "{note}");
        assert!(note.contains("live dungeon inputs apply"), "{note}");
        assert_eq!(
            context_note(None, Selection::default()),
            "Load a save to pick a character and map."
        );
        let town = Selection {
            character: 1,
            map: 0,
        };
        assert!(!context_note(Some("Bo"), town).contains("dungeon inputs"));
    }

    #[test]
    fn the_card_shows_the_total_the_reason_or_the_empty_state() {
        let save = save();
        let snapshot = Snapshot::from_save(&save);
        let selection = Selection::from_snapshot(&snapshot);
        assert_eq!(headline(&Total::Value(13.35), true), "13.35x");
        assert_eq!(headline(&Total::NeedsRuntime, true), "Unavailable");
        assert_eq!(
            headline(&Total::Value(13.35), false),
            "Load a save",
            "without a save there is nothing to resolve"
        );

        let value = sub_line(&Total::Value(13.35), Some("Ana"), selection, true);
        assert!(value.starts_with("Character: Ana · Map: "), "{value}");
        assert!(
            value.len() > "Character: Ana · Map: ".len(),
            "the map's name follows: {value}"
        );
        let missing = sub_line(&Total::NeedsRuntime, Some("Ana"), selection, true);
        assert!(missing.starts_with(drop_rate::NEEDS_RUNTIME), "{missing}");
        assert!(missing.contains("Character: Ana · Map: "), "{missing}");
        let unsupported = sub_line(&Total::Unsupported, Some("Ana"), selection, true);
        assert!(
            unsupported.starts_with(drop_rate::UNSUPPORTED),
            "{unsupported}"
        );
        assert_eq!(
            sub_line(&Total::Value(13.35), Some("Ana"), selection, false),
            "Select a character and map after loading."
        );
    }
}
