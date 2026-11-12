//! The All Stats Dashboard: the five calculation-context selectors upstream's
//! `stats-all.html` carries, the map access summary they resolve against, and
//! the category breakdowns of SA-02.
//!
//! A context change re-resolves every descriptor, so the page never shows a
//! value from the previous character or map. The breakdown rows read the
//! descriptor model in `idleon_stats::breakdowns`; the formula tree is
//! the shared [`FormulaTree`] component, not a second renderer.

mod actions;

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Role, SharedString, Styled, Window,
    div, px,
};
use idleon_research::model::{SimContext, Simulation};
use idleon_save::Save;
use idleon_stats::{
    self as stats, BreakdownCategory, ConstructionMode, Context as StatsContext, MapVisibility,
    Resolution, Skill,
};

/// The section's own state: the context, the save slice and the breakdowns.
pub(crate) struct State {
    pub(crate) context: StatsContext,
    pub(crate) snapshot: Option<stats::Snapshot>,
    /// Every upstream descriptor resolved against the save and the context.
    pub(crate) sheet: Option<stats::Sheet>,
    /// The category tab on screen.
    pub(crate) category: BreakdownCategory,
    /// The descriptor whose formula tree the detail card shows.
    pub(crate) selected: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            context: StatsContext::default(),
            snapshot: None,
            sheet: None,
            category: BreakdownCategory::Combat,
            selected: None,
        }
    }
}

impl State {
    /// Take a freshly imported save, opening on character one and its current
    /// map exactly like upstream `onLoad`. The other selectors keep the user's
    /// choice, which is what upstream reads back from the DOM.
    pub(crate) fn load(&mut self, save: &Save, simulation: &Simulation, sim_context: &SimContext) {
        let snapshot = stats::Snapshot::from_save(save);
        self.context.character = 0;
        self.context.map = snapshot.current_map(0).unwrap_or(0);
        self.snapshot = Some(snapshot);
        self.clamp(&mut Vec::new());
        self.rebuild(save, simulation, sim_context);
    }

    /// Re-resolve every breakdown; a selection the new sheet still carries is
    /// kept, otherwise the first row of the current tab is selected.
    pub(crate) fn rebuild(
        &mut self,
        save: &Save,
        simulation: &Simulation,
        sim_context: &SimContext,
    ) {
        let sheet = stats::Sheet::build(&stats::BreakdownInputs {
            save,
            simulation,
            sim_context,
            character: self.context.character,
            map: self.context.map,
        });
        let selection = self.selected.take().filter(|id| sheet.get(id).is_some());
        self.selected = selection;
        self.sheet = Some(sheet);
        if self.selected.is_none() {
            self.select_first();
        }
    }

    /// Select the first descriptor of the current category.
    pub(crate) fn select_first(&mut self) {
        let Some(sheet) = self.sheet.as_ref() else {
            return;
        };
        self.selected = sheet
            .in_category(self.category)
            .next()
            .map(|row| row.id.to_string());
    }

    /// The descriptor the detail card shows.
    pub(crate) fn selected_row(&self) -> Option<&stats::Breakdown> {
        let sheet = self.sheet.as_ref()?;
        sheet.get(self.selected.as_deref()?)
    }

    /// The map selector's rows for the current character and list level.
    pub(crate) fn options(&self) -> Vec<stats::MapOption> {
        self.snapshot
            .as_ref()
            .map(|snapshot| self.context.map_options(snapshot))
            .unwrap_or_default()
    }

    /// Keep the context inside the save and the list level; names the dropped
    /// map in `warnings`, for a project file restoring an unavailable one.
    pub(crate) fn clamp(&mut self, warnings: &mut Vec<String>) {
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
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
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

    let page = div()
        .flex()
        .flex_col()
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
        .child(breakdowns(state, cx));
    div()
        .id("stats-all-dashboard")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("stats-all-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(page),
        )
        .into_any_element()
}

/// The category tab ids: stable for the keyboard and the automation harness.
trait CategoryTab {
    fn tab_id(self) -> SharedString;
    fn from_tab_id(id: &str) -> Option<Self>
    where
        Self: Sized;
}

impl CategoryTab for BreakdownCategory {
    fn tab_id(self) -> SharedString {
        SharedString::from(format!("stats-{}-tab", self.id()))
    }

    fn from_tab_id(id: &str) -> Option<Self> {
        let key = id.strip_prefix("stats-")?.strip_suffix("-tab")?;
        BreakdownCategory::from_id(key)
    }
}

/// The category tabs, the descriptor rows and the selected formula tree.
fn breakdowns(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let tree = state.stats_breakdown.clone();
    let muted = cx.theme().colors.fg_muted;
    let accents = (cx.theme().colors.accent, cx.theme().colors.fg_muted);
    let category = state.stats_all.category;
    let selected = state.stats_all.selected.clone();
    let rows: Vec<stats::Breakdown> = state
        .stats_all
        .sheet
        .as_ref()
        .map(|sheet| sheet.in_category(category).cloned().collect())
        .unwrap_or_default();
    let counts: Vec<(&'static str, usize)> = BreakdownCategory::ALL
        .iter()
        .map(|entry| {
            (
                entry.label(),
                state
                    .stats_all
                    .sheet
                    .as_ref()
                    .map_or(0, |sheet| sheet.count(*entry)),
            )
        })
        .collect();
    let summary = counts
        .iter()
        .map(|(label, count)| format!("{label} {count}"))
        .collect::<Vec<_>>()
        .join(" · ");

    let mut tabs = TabBar::new("stats-categories").selected(category.tab_id());
    for (entry, (label, count)) in BreakdownCategory::ALL.iter().zip(&counts) {
        tabs = tabs.tab(WindowTab::new(entry.tab_id(), format!("{label} ({count})")));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        if let Some(entry) = BreakdownCategory::from_tab_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_stats_category(entry, cx));
        }
    });

    let list = rows.iter().fold(
        div()
            .id("stats-breakdown-rows")
            .flex()
            .flex_col()
            .gap_1()
            .p_3(),
        |list, row| {
            let id = row.id.to_string();
            let label = match row.display_value() {
                Some(value) => format!("{}: {value}", row.name),
                None => format!("{}: not modelled", row.name),
            };
            let aria = format!("{label} · {}", row.scope.label());
            let is_selected = selected.as_deref() == Some(row.id);
            let click = cx.entity();
            let row_id = id.clone();
            let partial = matches!(
                &row.resolution,
                Resolution::Value {
                    partial: Some(_),
                    ..
                }
            );
            list.child(
                div()
                    .id(format!("stats-row-{id}"))
                    .role(Role::Button)
                    .aria_label(aria)
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |row| row.bg(accents.0.opacity(0.12)))
                    .on_click(move |_, _window, cx| {
                        let id = row_id.clone();
                        click.update(cx, |state, cx| state.set_stats_descriptor(id, cx));
                    })
                    .child(div().flex_1().child(row.name))
                    .child(
                        Caption::new(row.scope.label())
                            .text_color(muted)
                            .into_any_element(),
                    )
                    .when_some(row.display_value(), |line, value| {
                        line.child(div().child(value))
                    })
                    .when(partial, |line| {
                        line.child(Badge::new("Partial").tone(Tone::Warning))
                    })
                    .when(row.display_value().is_none(), |line| {
                        line.child(Badge::new("Not modelled").tone(Tone::Warning))
                    }),
            )
        },
    );

    let detail = detail_card(state, tree, cx);

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Card::new()
                .header(CardHeader::new("Category Breakdowns").description(
                    "Every descriptor the upstream registry lists, grouped by its category. A row \
                     whose formula this build does not port says so instead of showing a guess.",
                ))
                .child(
                    div()
                        .id("stats-category-summary")
                        .role(Role::Label)
                        .aria_label(summary.clone())
                        .px_3()
                        .pb_2()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(summary),
                )
                .child(div().px_3().pb_2().child(tabs))
                .child(
                    div()
                        .id("stats-category-panel")
                        .role(Role::Label)
                        .aria_label(format!(
                            "{} breakdowns: {} rows",
                            category.label(),
                            rows.len()
                        ))
                        .child(list),
                ),
        )
        .child(detail)
        .into_any_element()
}

/// The selected descriptor: its provenance, its value and the shared tree.
fn detail_card(
    state: &mut AppState,
    tree: Entity<idleon_ui::formula::FormulaTree>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(row) = state.stats_all.selected_row() else {
        return Card::new()
            .header(
                CardHeader::new("Descriptor Breakdown")
                    .description("Select a row above to see its formula tree."),
            )
            .child(
                div()
                    .id("stats-descriptor-empty")
                    .role(Role::Label)
                    .aria_label("No descriptor selected")
                    .px_3()
                    .pb_3()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child("Select a row above to see its formula tree."),
            )
            .into_any_element();
    };
    let row = row.clone();
    let value = row
        .display_value()
        .unwrap_or_else(|| "not modelled".to_string());
    let warning = row.warning().map(str::to_string);
    let source = row.source.to_string();
    let scope = row.scope.label();
    let header = CardHeader::new(format!("{} · {scope}", row.name)).description(format!(
        "{source} · the formula tree is the shared breakdown component"
    ));
    let mut card = Card::new().header(header).child(
        div()
            .id("stats-descriptor-summary")
            .role(Role::Label)
            .aria_label(format!("{}: {value}", row.name))
            .px_3()
            .pb_2()
            .text_size(px(12.0))
            .text_color(muted)
            .child(format!("{value} · {source}")),
    );
    if let Some(tree_root) = row.resolution.tree() {
        let root = tree_root.clone();
        tree.update(cx, |tree, cx| tree.set_root(root, cx));
        card = card.child(
            div()
                .id("stats-descriptor-formula")
                .h(px(320.0))
                .min_h_0()
                .child(tree),
        );
    } else if let Some(reason) = warning {
        card = card.child(
            div()
                .id("stats-descriptor-warning")
                .role(Role::Label)
                .aria_label(reason.clone())
                .px_3()
                .pb_3()
                .text_size(px(12.0))
                .text_color(muted)
                .child(reason),
        );
    }
    card.into_any_element()
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
pub(crate) fn restore(state: &mut AppState, context: StatsContext, warnings: &mut Vec<String>) {
    state.stats_all.context = context;
    state.stats_all.clamp(warnings);
    // The restored character and map must re-resolve every breakdown, or the
    // page would keep the values the previous context produced.
    state.rebuild_stats_breakdowns();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn simulation() -> Simulation {
        Simulation::default()
    }

    fn context() -> SimContext {
        SimContext::default()
    }

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
        state.load(&save(), &simulation(), &context());
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
        state.load(&save(), &simulation(), &context());
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

    #[test]
    fn every_upstream_category_is_a_tab_and_an_unported_row_warns() {
        let mut state = State::default();
        state.load(&save(), &simulation(), &context());
        let sheet = state.sheet.as_ref().expect("a save builds the sheet");
        assert_eq!(BreakdownCategory::ALL.len(), 9);
        for category in BreakdownCategory::ALL {
            assert!(sheet.count(category) > 0, "{} is empty", category.label());
        }
        // A save without MaxCarryCap rows reports the capacity rows as missing
        // rather than inventing a value.
        let mining = sheet.get("carry-capacity-mining").expect("the row");
        assert!(mining.display_value().is_none());
        assert!(
            mining
                .warning()
                .unwrap_or_default()
                .contains("MaxCarryCap_0")
        );
        // Combat rows are not modelled yet and name their upstream source.
        let damage = sheet.get("damage").expect("the row");
        assert!(damage.warning().unwrap_or_default().contains("damage.js"));
    }

    #[test]
    fn every_category_tab_id_round_trips() {
        for category in BreakdownCategory::ALL {
            let id = category.tab_id();
            assert_eq!(
                BreakdownCategory::from_tab_id(id.as_ref()),
                Some(category),
                "{id}"
            );
        }
        assert_eq!(BreakdownCategory::from_tab_id("stats-frogs-tab"), None);
    }

    #[test]
    fn a_real_capacity_row_selects_its_formula_tree() {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "MaxCarryCap_0": "{\"Mining\":1000}",
        }));
        let mut state = State::default();
        state.load(&save, &simulation(), &context());
        state.category = BreakdownCategory::Capacity;
        state.select_first();
        let row = state.selected_row().expect("the first capacity row");
        assert_eq!(row.id, "carry-capacity-mining");
        assert!(row.resolution.tree().is_some());
        assert!(row.display_value().is_some());
    }
}
