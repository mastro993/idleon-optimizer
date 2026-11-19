//! The Drop Rate Calculator: the character and map context, the live dungeon
//! inputs, the total card and the formula tree behind it.
//!
//! The context card that picks the character and map, the runtime panel that
//! only a dungeon map shows, the total card and the breakdown. The maths and
//! the tree live in [`idleon_drop_rate`]; the breakdown renders through the
//! shared formula tree.

mod actions;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Hsla, ParentElement, Role, Styled, Subscription,
    Window, div, px,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_drop_rate::{self as drop_rate, Runtime, Selection, Total};
use idleon_save::Save;
use idleon_stats::{self as stats, Snapshot};
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, PANEL_GAP, TEXT_CAPTION,
};

/// The page's own state: the context, the save slice it reads, the two live
/// fields a dungeon run shows, what they resolve to and the breakdown tree.
///
/// The selectors' rows and the total are read when the save, the selection or
/// a live field changes ([`State::refresh`]), so a frame only formats them.
pub(crate) struct State {
    pub(crate) selection: Selection,
    pub(crate) snapshot: Option<Snapshot>,
    /// Dungeon Total LUK; a blank field means "not entered".
    luk: Entity<InputState>,
    /// Dungeon Drop Rate stat.
    rate: Entity<InputState>,
    /// The `#tree` breakdown.
    tree: Entity<FormulaTree>,
    characters: Vec<Choice<usize>>,
    maps: Vec<Choice<usize>>,
    character_select: ChoiceSelect<usize>,
    map_select: ChoiceSelect<usize>,
    /// The live fields as they read, or why they cannot be read.
    runtime: Result<Runtime, String>,
    total: Total,
    /// Whether the tree holds the current total's expansion.
    expanded: bool,
    _subscriptions: [Subscription; 4],
}

impl State {
    /// The page with empty fields and no save.
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let luk = live_field("e.g. 1200", window, cx);
        let rate = live_field("e.g. 35", window, cx);
        let maps = map_choices(None, 0);
        let character_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let map_select = ChoiceSelect::new(maps.clone(), None, window, cx);
        let pick = |pick: fn(&mut AppState, usize, &mut Context<AppState>)| {
            move |state: &mut AppState,
                  _: Entity<_>,
                  event: &SelectEvent<Vec<Choice<usize>>>,
                  cx: &mut Context<AppState>| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    pick(state, *value, cx);
                }
            }
        };
        let edited = |state: &mut AppState,
                      _: Entity<InputState>,
                      event: &InputEvent,
                      cx: &mut Context<AppState>| {
            if matches!(event, InputEvent::Change) {
                state.drop_rate.refresh(cx);
                cx.notify();
            }
        };
        let subscriptions = [
            cx.subscribe(
                character_select.state(),
                pick(AppState::set_drop_rate_character),
            ),
            cx.subscribe(map_select.state(), pick(AppState::set_drop_rate_map)),
            cx.subscribe(&luk, edited),
            cx.subscribe(&rate, edited),
        ];
        let mut state = Self {
            selection: Selection::default(),
            snapshot: None,
            luk,
            rate,
            tree: crate::ui::formula_tree("drop-rate-formula", window, cx),
            characters: Vec::new(),
            maps,
            character_select,
            map_select,
            runtime: Ok(Runtime::default()),
            total: Total::Unsupported,
            expanded: false,
            _subscriptions: subscriptions,
        };
        state.refresh(cx);
        state
    }

    /// Take a freshly imported save: character one on its current map.
    pub(crate) fn load(&mut self, save: &Save, cx: &mut App) {
        let snapshot = Snapshot::from_save(save);
        self.selection = Selection::from_snapshot(&snapshot);
        self.characters = character_choices(&snapshot);
        self.snapshot = Some(snapshot);
        self.refresh_maps();
        self.refresh(cx);
    }

    /// Rebuild the map selector's rows, which follow the selected character.
    pub(crate) fn refresh_maps(&mut self) {
        self.maps = map_choices(self.snapshot.as_ref(), self.selection.character);
    }

    /// Read the live fields and the selection again: the total, and the tree
    /// behind it when the map's pool chain has one.
    pub(crate) fn refresh(&mut self, cx: &mut App) {
        self.runtime = Runtime::read(&self.luk.read(cx).value(), &self.rate.read(cx).value());
        self.total = match self.runtime {
            Ok(runtime) => drop_rate::total(self.selection.map, runtime),
            Err(ref reason) => Total::Invalid(reason.clone()),
        };
        let root = self
            .runtime
            .as_ref()
            .ok()
            .copied()
            .and_then(|runtime| drop_rate::tree(self.selection.map, runtime));
        self.expanded = root.is_some();
        if let Some(root) = root {
            self.tree.update(cx, |tree, cx| tree.set_root(root, cx));
        }
    }

    /// Whether the selected map runs the live dungeon formula.
    pub(crate) fn dungeon(&self) -> bool {
        drop_rate::is_dungeon_map(self.selection.map)
    }
}

/// One live dungeon field: digits and a decimal point, blank until entered.
fn live_field(placeholder: &'static str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .validate(|text, _| text.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
    })
}

/// The character selector's rows: every character with its class.
fn character_choices(snapshot: &Snapshot) -> Vec<Choice<usize>> {
    (0..snapshot.characters())
        .map(|character| {
            let class = stats::class_label(snapshot.class_id(character));
            Choice::new(character, format!("{} ({class})", snapshot.name(character)))
        })
        .collect()
}

/// The map selector's rows, marked when they are dungeons.
fn map_choices(snapshot: Option<&Snapshot>, character: usize) -> Vec<Choice<usize>> {
    drop_rate::maps(snapshot, character)
        .into_iter()
        .map(|row| Choice::new(row.map, row.label))
        .collect()
}

/// The Drop Rate page: the context, the live dungeon fields, the total and the
/// formula tree.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let page = &state.drop_rate;
    let selection = page.selection;
    page.character_select.show(
        page.characters.clone(),
        Some(&selection.character),
        window,
        cx,
    );
    page.map_select
        .show(page.maps.clone(), Some(&selection.map), window, cx);
    let theme = cx.theme();
    let (muted, gold, danger) = (theme.muted_foreground, theme.warning, theme.danger);
    let character = page
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.name(selection.character));
    let loaded = character.is_some();
    let runtime = page.runtime.as_ref().ok();
    let out_of_range =
        |value: Option<f64>| value.is_some_and(|value| !value.is_finite() || value < 0.0);
    let luk_invalid = out_of_range(runtime.and_then(|runtime| runtime.luk));
    let rate_invalid = out_of_range(runtime.and_then(|runtime| runtime.drop_rate));
    let gap = page.total.reason().map(str::to_string);

    let controls = div()
        .id("drop-rate-context-controls")
        .flex()
        .items_end()
        .gap_3()
        .child(field("Character", muted).child(select(
            &page.character_select,
            "drop-rate-character",
            "Character",
            loaded,
        )))
        .child(field("Map", muted).child(select(&page.map_select, "drop-rate-map", "Map", loaded)));

    let note = context_note(character.as_deref(), selection);
    let total = total_card(
        &page.total,
        character.as_deref(),
        selection,
        loaded,
        muted,
        gold,
        danger,
    );
    let formula = if page.expanded {
        formula_card(page.tree.clone())
    } else {
        formula_gap(gap, loaded, muted)
    };
    div()
        .id("drop-rate-calculator")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(PANEL_GAP)
                        .child(
                            Card::new()
                                .header(CardHeader::new("Calculation Context").description(
                                    "The drop-rate descriptor resolves against this character and map.",
                                ))
                                .child(controls)
                                .when(page.dungeon(), |card| {
                                    card.child(dungeon_fields(
                                        &page.luk,
                                        &page.rate,
                                        luk_invalid,
                                        rate_invalid,
                                        cx,
                                    ))
                                })
                                .child(
                                    div()
                                        .id("drop-rate-context-note")
                                        .role(Role::Label)
                                        .aria_label(note.clone())
                                        .text_size(TEXT_CAPTION)
                                        .text_color(muted)
                                        .child(note),
                                ),
                        )
                        .child(total)
                        .child(formula),
                )
                .overflow_y_scrollbar()
                .id("drop-rate-scroll"),
        )
        .into_any_element()
}

/// One context selector under its Ely id, disabled until a save loads.
fn select(
    select: &ChoiceSelect<usize>,
    id: &'static str,
    label: &'static str,
    loaded: bool,
) -> impl IntoElement {
    div().flex_none().w(px(240.0)).child(
        Select::new(select.state())
            .id(id)
            .accessibility_label(label)
            .placeholder("Load a save first")
            .disabled(!loaded)
            .small(),
    )
}

/// The shared formula tree in its card, sized like the other sections'
/// breakdown cards.
fn formula_card(tree: Entity<FormulaTree>) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Formula Breakdown").description(
                "1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100, term by term",
            ),
        )
        .child(
            div()
                .id("drop-rate-formula-card-body")
                .h(px(240.0))
                .min_h_0()
                .child(tree),
        )
        .into_any_element()
}

/// The tree card's empty state: the reason this map has nothing to expand.
fn formula_gap(reason: Option<String>, loaded: bool, muted: Hsla) -> AnyElement {
    let reason = gap_reason(reason.as_deref(), loaded);
    Card::new()
        .header(
            CardHeader::new("Formula Breakdown")
                .description("Nothing to expand until the missing terms resolve."),
        )
        .child(
            div()
                .id("drop-rate-formula-gap")
                .role(Role::Label)
                .aria_label(reason.clone())
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(reason),
        )
        .into_any_element()
}

/// Why the tree card has no tree: nothing loaded yet, or the map's own gap.
fn gap_reason(reason: Option<&str>, loaded: bool) -> String {
    if loaded {
        reason.unwrap_or(drop_rate::UNSUPPORTED).to_string()
    } else {
        "Load a save to expand the drop-rate formula.".to_string()
    }
}

/// The two live dungeon inputs, shown only while a dungeon map is selected.
fn dungeon_fields(
    luk: &Entity<InputState>,
    rate: &Entity<InputState>,
    luk_invalid: bool,
    rate_invalid: bool,
    cx: &App,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    div()
        .id("drop-rate-dungeon-runtime")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().sidebar)
        .child(
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(field("Dungeon Total LUK", muted).child(clickable_field(
                    "drop-rate-luk",
                    "Dungeon Total LUK",
                    luk,
                    luk_invalid,
                    cx,
                )))
                .child(
                    field("Dungeon Drop Rate Stat", muted).child(clickable_field(
                        "drop-rate-drop",
                        "Dungeon Drop Rate Stat",
                        rate,
                        rate_invalid,
                        cx,
                    )),
                ),
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
    input: &Entity<InputState>,
    invalid: bool,
    cx: &App,
) -> AnyElement {
    let field = input.clone();
    let danger = cx.theme().danger;
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(label)
        .w(px(190.0))
        .on_click(move |_event, window, cx| {
            field.update(cx, |field, cx| field.focus(window, cx));
        })
        .child(
            Input::new(input)
                .aria_label(label)
                .small()
                .when(invalid, |input| input.border_color(danger)),
        )
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
                        .text_size(TEXT_CAPTION)
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
        .child(Caption::new(label).text_color(muted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_save::Save;
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

    #[test]
    fn the_tree_card_names_the_reason_it_has_no_tree() {
        assert_eq!(
            gap_reason(None, false),
            "Load a save to expand the drop-rate formula."
        );
        assert_eq!(gap_reason(None, true), drop_rate::UNSUPPORTED);
        assert_eq!(
            gap_reason(Some("Dungeon Total LUK must be 0 or more."), true),
            "Dungeon Total LUK must be 0 or more."
        );
    }
}
