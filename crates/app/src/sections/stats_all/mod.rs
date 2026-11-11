//! The All Stats Dashboard's calculation context: the five selectors upstream's
//! `stats-all.html` carries, their save defaults and the map access summary
//! they resolve against.
//!
//! The selectors are the whole of SA-01. The breakdowns that read this context
//! (SA-02), the formula explorer (SA-03) and the input ledger (SA-04) hang off
//! the same state, so a context change recomputes everything the page shows.

use crate::state::AppState;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use idleon_core::save::Save;
use idleon_core::stats::{self, ConstructionMode, Context as StatsContext, MapVisibility, Skill};

/// The section's own state: the context and the save slice it reads.
#[derive(Default)]
pub struct State {
    pub context: StatsContext,
    pub snapshot: Option<stats::Snapshot>,
}

impl State {
    /// Take a freshly imported save, opening on character one and its current
    /// map exactly like upstream `onLoad`. The other selectors keep the user's
    /// choice, which is what upstream reads back from the DOM.
    pub fn load(&mut self, save: &Save) {
        let snapshot = stats::Snapshot::from_save(save);
        self.context.character = 0;
        self.context.map = snapshot.current_map(0).unwrap_or(0);
        self.snapshot = Some(snapshot);
        self.clamp(&mut Vec::new());
    }

    /// The map selector's rows for the current character and list level.
    pub fn options(&self) -> Vec<stats::MapOption> {
        self.snapshot
            .as_ref()
            .map(|snapshot| self.context.map_options(snapshot))
            .unwrap_or_default()
    }

    /// Keep the context inside the save and the list level; names the dropped
    /// map in `warnings`, for a project file restoring an unavailable one.
    pub fn clamp(&mut self, warnings: &mut Vec<String>) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if self.context.character >= snapshot.characters() {
            warnings.push(format!(
                "stats-all: character {} is outside the {} the save carries; character 1 is used",
                self.context.character + 1,
                snapshot.characters()
            ));
            self.context.character = 0;
        }
        if let Some(dropped) = self.context.clamp_map(snapshot) {
            warnings.push(format!(
                "stats-all: map {dropped} is not in the {} map list; map {} is used",
                self.context.map_visibility.label(),
                self.context.map
            ));
        }
    }
}

/// The All Stats Dashboard page: the context card and the map access summary.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let stats = &state.stats_all;
    let context = stats.context;
    let snapshot = stats.snapshot.as_ref();
    let note = match snapshot {
        None => "Load a save to calculate all descriptors.".to_string(),
        Some(snapshot) => {
            let class = stats::classify(context.map);
            let name = snapshot.name(context.character);
            let warning = if class.warning.is_empty() {
                String::new()
            } else {
                format!(" · {}", class.warning)
            };
            format!(
                "{name} · {} · {} · Construction {}{warning}",
                class.name,
                context.skill.label(),
                context.construction.label()
            )
        }
    };

    let snapshot_choices: Vec<Choice> = match snapshot {
        None => Vec::new(),
        Some(snapshot) => (0..snapshot.characters())
            .map(|character| {
                let class = stats::class_name(snapshot.class_id(character)).replace('_', " ");
                Choice::new(
                    character.to_string(),
                    format!("{} ({class})", snapshot.name(character)),
                )
            })
            .collect(),
    };
    let options = stats.options();
    let map_choices: Vec<Choice> = snapshot
        .map(|snapshot| {
            options
                .iter()
                .map(|option| {
                    let map = option.class.map;
                    Choice::new(map.to_string(), option.label(snapshot.map_bonus(map)))
                        .note(format!("World {}", option.class.world))
                })
                .collect()
        })
        .unwrap_or_default();

    let character = entity.clone();
    let map = entity.clone();
    let list = entity.clone();
    let skill = entity.clone();
    let construction = entity.clone();
    let has_save = snapshot.is_some();

    let controls = div()
        .id("stats-context-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(
            field("Character", muted).child(
                Select::new("stats-character", snapshot_choices)
                    .label("Character")
                    .placeholder("Load a save first")
                    .selected(context.character.to_string())
                    .disabled(!has_save)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            character.update(cx, |state, cx| state.set_stats_character(index, cx));
                        }
                    }),
            ),
        )
        .child(
            field("Map", muted).child(
                Select::new("stats-map", map_choices)
                    .label("Map")
                    .placeholder("Load a save first")
                    .selected(context.map.to_string())
                    .disabled(!has_save)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            map.update(cx, |state, cx| state.set_stats_map(index, cx));
                        }
                    }),
            ),
        )
        .child(
            field("Map List", muted).child(
                Select::new(
                    "stats-map-visibility",
                    MapVisibility::ALL.map(|level| Choice::new(level.id(), level.label())),
                )
                .label("Map list")
                .selected(context.map_visibility.id())
                .on_change(move |value, _window, cx| {
                    if let Some(level) = MapVisibility::from_id(value.as_ref()) {
                        list.update(cx, |state, cx| state.set_stats_map_visibility(level, cx));
                    }
                }),
            ),
        )
        .child(
            field("Skill", muted).child(
                Select::new(
                    "stats-skill",
                    Skill::ALL.map(|skill| Choice::new(skill.id(), skill.label())),
                )
                .label("Skill")
                .selected(context.skill.id())
                .on_change(move |value, _window, cx| {
                    if let Some(next) = Skill::from_id(value.as_ref()) {
                        skill.update(cx, |state, cx| state.set_stats_skill(next, cx));
                    }
                }),
            ),
        )
        .child(
            field("Construction Mode", muted).child(
                Select::new(
                    "stats-construction",
                    ConstructionMode::ALL.map(|mode| Choice::new(mode.id(), mode.label())),
                )
                .label("Construction mode")
                .selected(context.construction.id())
                .on_change(move |value, _window, cx| {
                    if let Some(mode) = ConstructionMode::from_id(value.as_ref()) {
                        construction.update(cx, |state, cx| state.set_stats_construction(mode, cx));
                    }
                }),
            ),
        );

    div()
        .id("stats-all-dashboard")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(
            Card::new()
                .header(CardHeader::new("Calculation Context").description(
                    "Every breakdown below resolves against this character, map, skill and construction mode.",
                ))
                .child(controls)
                .child(
                    div()
                        .id("stats-context-note")
                        .role(Role::Label)
                        .aria_label(note.clone())
                        .px_3()
                        .pb_3()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note),
                ),
        )
        .when_some(snapshot, |page, snapshot| {
            page.child(map_card(context, snapshot, &options, muted))
        })
        .into_any_element()
}

/// The map access summary: classification, evidence and the list size.
fn map_card(
    context: StatsContext,
    snapshot: &stats::Snapshot,
    options: &[stats::MapOption],
    muted: gpui::Hsla,
) -> AnyElement {
    let class = stats::classify(context.map);
    let selected = options
        .iter()
        .find(|option| option.class.map == context.map)
        .cloned();
    let evidence = selected
        .as_ref()
        .map(|option| option.evidence.clone())
        .filter(|evidence| !evidence.is_empty())
        .unwrap_or_else(|| "No access evidence for this map.".into());
    let state_label = match &selected {
        Some(option) if option.current => "Current map",
        Some(option) if option.accessible => "Accessible",
        Some(option) if option.visited => "Visited",
        Some(_) => "Not proven accessible",
        None => "Not in this map list",
    };
    let summary = format!(
        "{state_label} · {} maps in the {} list",
        options.len(),
        context.map_visibility.label()
    );
    let target = if class.target_name.is_empty() {
        "no AFK target".to_string()
    } else {
        class.target_name.replace('_', " ")
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Card::new()
                .header(CardHeader::new("Map Access").description(format!(
                    "{} · World {} · {} · {target}",
                    class.name,
                    class.world,
                    class.category.label()
                )))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_3()
                        .pb_3()
                        .child(
                            div()
                                .id("stats-map-class")
                                .role(Role::Label)
                                .aria_label(format!("Map class: {}", class.category.label()))
                                .text_size(px(12.0))
                                .child(format!(
                                    "{} · World {} · {}",
                                    class.name,
                                    class.world,
                                    class.category.label()
                                )),
                        )
                        .child(
                            div()
                                .id("stats-map-evidence")
                                .role(Role::Label)
                                .aria_label(format!("Map evidence: {evidence}"))
                                .text_size(px(12.0))
                                .text_color(muted)
                                .child(evidence),
                        )
                        .child(
                            div()
                                .id("stats-map-count")
                                .role(Role::Label)
                                .aria_label(summary.clone())
                                .text_size(px(12.0))
                                .text_color(muted)
                                .child(summary),
                        ),
                )
                .child(
                    div().px_3().pb_3().child(
                        Caption::new(format!(
                            "{} characters in this save · {} maps in the game data",
                            snapshot.characters(),
                            stats::map_count()
                        ))
                        .text_color(muted),
                    ),
                ),
        )
        .into_any_element()
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(190.0))
        .child(Caption::new(label).text_color(muted))
}

/// Restore a project file's context, warning about values the save cannot show.
pub fn restore(state: &mut AppState, context: StatsContext, warnings: &mut Vec<String>) {
    state.stats_all.context = context;
    state.stats_all.clamp(warnings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[1]",
            "Lv0_1": "[1]",
            "CharacterClass_0": 1,
            "CharacterClass_1": 7,
            "CurrentMap_0": 0,
            "CurrentMap_1": 50,
            "KLA_0": "[[0]]",
            "KLA_1": "[[0]]",
        }))
    }

    #[test]
    fn an_import_opens_on_character_one_and_its_current_map() {
        let mut state = State::default();
        state.load(&save());
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert!(state.snapshot.is_some());
        assert_eq!(
            state.options().first().map(|option| option.class.map),
            Some(0)
        );
    }

    #[test]
    fn a_restored_context_outside_the_save_is_clamped_and_named() {
        let mut state = State::default();
        state.load(&save());
        let mut warnings = Vec::new();
        state.context = StatsContext {
            character: 12,
            map: 29,
            map_visibility: MapVisibility::Accessible,
            skill: Skill::Spelunking,
            construction: ConstructionMode::Inactive,
        };
        state.clamp(&mut warnings);
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert_eq!(state.context.skill, Skill::Spelunking);
        assert_eq!(state.context.construction, ConstructionMode::Inactive);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("character 13"), "{warnings:?}");
        assert!(warnings[1].contains("map 29"), "{warnings:?}");
    }
}
