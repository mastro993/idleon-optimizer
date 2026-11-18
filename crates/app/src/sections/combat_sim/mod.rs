//! The Combat Simulator section: the loadout editor (CS-01).
//!
//! The Setup editor: the character picker, the equipment slots with their saved
//! upgrade deltas, the talent table and the named loadouts. The save-derived
//! defaults and the item catalog come from [`idleon_combat`]; every edit
//! recomputes the gear totals the header shows, so the summary never lags
//! behind the editor.
//!
//! The mechanics overrides (CS-02) sit beside it: the curated mechanics the
//! simulator can disable, set or scale, with the active rules shown as chips and
//! a reset back to the save. The encounter run (CS-03) and the comparison
//! (CS-04) land on top of both; the editor owns the loadout and overrides they
//! read. The run's results (CS-05) are tabs over the one stored run: its
//! summary, Killroy practice, obols, kills by monster, cards and the kills
//! timeline.

mod actions;
mod controls;

pub(crate) mod compare;
pub(crate) mod encounter;
pub(crate) mod equipment;
pub(crate) mod overrides;
pub(crate) mod presets;
pub(crate) mod replay;
pub(crate) mod talents;

pub(crate) use encounter::RunResult;

use crate::state::AppState;
use crate::ui::layout;
use controls::Controls;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div,
};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::Select;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_combat::Loadout;
use idleon_combat::drops::Snapshot;
use idleon_save::Save;
use idleon_ui::widgets::{Caption, Choice, PANEL_GAP, TEXT_CAPTION, TEXT_MONO, Tone};
use std::collections::BTreeMap;

/// The result tabs: one stored run read six ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ResultsTab {
    #[default]
    Summary,
    Killroy,
    Obols,
    Kills,
    Cards,
    Timeline,
    Den,
    Progress,
    Smelting,
    Replay,
}

impl ResultsTab {
    pub(crate) const ALL: [ResultsTab; 10] = [
        ResultsTab::Summary,
        ResultsTab::Killroy,
        ResultsTab::Obols,
        ResultsTab::Kills,
        ResultsTab::Cards,
        ResultsTab::Timeline,
        ResultsTab::Den,
        ResultsTab::Progress,
        ResultsTab::Smelting,
        ResultsTab::Replay,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            ResultsTab::Summary => "result-summary",
            ResultsTab::Killroy => "result-killroy",
            ResultsTab::Obols => "result-obols",
            ResultsTab::Kills => "result-kills",
            ResultsTab::Cards => "result-cards",
            ResultsTab::Timeline => "result-timeline",
            ResultsTab::Den => "result-den",
            ResultsTab::Progress => "result-progress",
            ResultsTab::Smelting => "result-smelting",
            ResultsTab::Replay => "result-replay",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            ResultsTab::Summary => "Summary",
            ResultsTab::Killroy => "Killroy",
            ResultsTab::Obols => "Obols",
            ResultsTab::Kills => "Kills",
            ResultsTab::Cards => "Cards",
            ResultsTab::Timeline => "Timeline",
            ResultsTab::Den => "The Den",
            ResultsTab::Progress => "Progress",
            ResultsTab::Smelting => "Smelting",
            ResultsTab::Replay => "Replay",
        }
    }
}

/// The editor's tabs, matching the setup editor's character tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    #[default]
    Gear,
    Talents,
    Mechanics,
    Encounter,
    Compare,
}

impl Tab {
    pub(crate) const ALL: [Tab; 5] = [
        Tab::Gear,
        Tab::Talents,
        Tab::Mechanics,
        Tab::Encounter,
        Tab::Compare,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Gear => "gear",
            Tab::Talents => "talents",
            Tab::Mechanics => "mechanics",
            Tab::Encounter => "encounter",
            Tab::Compare => "compare",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Gear => "Equipment",
            Tab::Talents => "Talents",
            Tab::Mechanics => "Mechanics",
            Tab::Encounter => "Encounter",
            Tab::Compare => "Compare",
        }
    }
}

/// The section's own state: the working loadout and the named presets.
pub(crate) struct State {
    /// The working copy of the selected character's loadout.
    pub(crate) loadout: Loadout,
    /// Which character the loadout belongs to.
    pub(crate) character: usize,
    pub(crate) tab: Tab,
    /// The flat equipment slot the detail panel edits.
    pub(crate) selected: usize,
    /// The named loadouts the project JSON keeps.
    pub(crate) presets: presets::Presets,
    /// The name typed into the preset bar.
    pub(crate) name: Entity<InputState>,
    /// The talent table's filter.
    pub(crate) talent_search: Entity<InputState>,
    /// The mechanic overrides the working loadout carries.
    pub(crate) overrides: idleon_combat::State,
    /// The mechanics pane's search field.
    pub(crate) override_search: Entity<InputState>,
    /// The mechanics pane's group filter; empty means every group.
    pub(crate) override_group: String,
    /// Whether the mechanics pane lists only the overridden rows.
    pub(crate) override_only_active: bool,
    /// The encounter run's map index.
    pub(crate) sim_map: usize,
    /// Whether the run uses the fixed kill interval instead of the idle player.
    pub(crate) sim_stand_in: bool,
    /// The stand-in's kill interval in milliseconds.
    pub(crate) sim_kill_every_ms: u64,
    /// The run's simulated duration in seconds.
    pub(crate) sim_duration_s: u64,
    /// The seed every drop roll and the Killroy projection read.
    pub(crate) sim_seed: u64,
    /// The results tab the Encounter pane shows.
    pub(crate) results: ResultsTab,
    /// The characters the comparison runs, ascending.
    pub(crate) compare_characters: Vec<usize>,
    /// Each selected character's loadout keys: `saved`, `edited` or
    /// `preset:<name>`. A missing entry means the character's default.
    pub(crate) compare_loadouts: BTreeMap<usize, Vec<String>>,
    /// The maps the comparison runs, ascending.
    pub(crate) compare_maps: Vec<usize>,
    /// The saved furnace and the account progress a run advances.
    pub(crate) progress: idleon_combat::progress::Config,
    /// The config terms this build cannot decode, named for the panel.
    pub(crate) progress_warnings: Vec<String>,
    /// The Den attempt's inputs, when the save carries its `Holes` rows.
    pub(crate) den: Option<idleon_combat::den::Config>,
    /// The replay playhead, in frames; fractional while playing.
    pub(crate) replay_pos: f64,
    /// Playback speed in frames per second, from the replay speeds.
    pub(crate) replay_speed: f64,
    pub(crate) replay_playing: bool,
    /// Whether a playback loop is already pumping frames.
    pub(crate) replay_pumping: bool,
    /// Bumped to stop a playback loop that outlived its run.
    pub(crate) replay_generation: u64,
    /// When the last playback frame ticked, for the elapsed advance.
    pub(crate) replay_at: Option<std::time::Instant>,
    /// A captured Killroy replay, kept for this session.
    pub(crate) killroy_replay: Option<idleon_combat::replay::KillroyReplay>,
    /// The captured replay the next run answers, stamped onto its result.
    pub(crate) pending_source: Option<idleon_combat::replay::KillroyReplay>,
    /// The save's card and obol state a run's drops start from, read once per
    /// save instead of on every frame that asks for the run's options.
    pub(crate) drops: Snapshot,
    /// Every character's loadout as the save has it, for the comparison.
    pub(crate) saved: Vec<Loadout>,
    pub(crate) controls: Controls,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let name = crate::ui::input_state("Loadout name", window, cx);
        let talent_search = crate::ui::input_state("Find a talent", window, cx);
        let override_search = crate::ui::input_state("crystal, drop rate, orb…", window, cx);
        Self {
            loadout: Loadout::empty(),
            character: 0,
            tab: Tab::default(),
            selected: 0,
            presets: presets::Presets::default(),
            name,
            talent_search,
            overrides: idleon_combat::State::new(),
            override_search,
            override_group: String::new(),
            override_only_active: false,
            sim_map: idleon_combat::encounter::maps()
                .first()
                .map_or(1, |map| map.index),
            sim_stand_in: true,
            sim_kill_every_ms: 1_500,
            sim_duration_s: 300,
            sim_seed: 1,
            results: ResultsTab::default(),
            compare_characters: vec![0],
            compare_loadouts: BTreeMap::new(),
            compare_maps: vec![
                idleon_combat::encounter::maps()
                    .first()
                    .map_or(1, |map| map.index),
            ],
            progress: idleon_combat::progress::Config::default(),
            progress_warnings: Vec::new(),
            den: None,
            replay_pos: 0.0,
            replay_speed: 1.0,
            replay_playing: false,
            replay_pumping: false,
            replay_generation: 0,
            replay_at: None,
            killroy_replay: None,
            pending_source: None,
            drops: Snapshot::default(),
            saved: Vec::new(),
            controls: Controls::new(window, cx),
        }
    }

    /// Replace the working loadout with the save's defaults for `character`,
    /// keeping the selected slot when it still exists. Named presets stay: they
    /// live in the project, not in the save. The comparison's selection keeps
    /// only characters and maps the new save has.
    pub(crate) fn load(&mut self, save: &Save, character: usize) {
        self.character = character;
        self.loadout = Loadout::from_save(save, character);
        self.drops = idleon_combat::drops::snapshot(save);
        self.saved = (0..save.character_count())
            .map(|index| Loadout::from_save(save, index))
            .collect();
        let (progress, warnings) = idleon_stats::combat::progress(save, character);
        self.progress = progress;
        self.progress_warnings = warnings;
        self.den = idleon_stats::combat::den(save, character);
        if self.selected >= self.loadout.slots.len() {
            self.selected = 0;
        }
        let count = save.character_count();
        self.compare_characters.retain(|index| *index < count);
        if self.compare_characters.is_empty() {
            self.compare_characters
                .push(character.min(count.saturating_sub(1)));
        }
        self.compare_characters.sort_unstable();
        self.compare_loadouts.retain(|index, _| *index < count);
        self.compare_maps
            .retain(|map| idleon_combat::encounter::map(*map).is_some());
        if self.compare_maps.is_empty() {
            let fallback = if idleon_combat::encounter::map(self.sim_map).is_some() {
                self.sim_map
            } else {
                idleon_combat::encounter::maps()[0].index
            };
            self.compare_maps.push(fallback);
        }
        self.compare_maps.sort_unstable();
        self.compare_maps.dedup();
    }

    /// The loadout keys a character compares, or its default when the user has
    /// not edited the selection: the working edits when they exist, the save
    /// otherwise.
    pub(crate) fn compare_loadout_keys(&self, character: usize) -> Vec<String> {
        if let Some(keys) = self.compare_loadouts.get(&character) {
            return keys.clone();
        }
        if character == self.character && self.loadout.changed() > 0 {
            vec!["edited".into()]
        } else {
            vec!["saved".into()]
        }
    }

    /// The preset name typed into the bar, trimmed.
    pub(crate) fn preset_name(&self, cx: &App) -> String {
        self.name.read(cx).value().trim().to_string()
    }

    /// The talent table's query.
    pub(crate) fn talent_query(&self, cx: &App) -> String {
        self.talent_search.read(cx).value().to_string()
    }

    /// The mechanics pane's query.
    pub(crate) fn override_query(&self, cx: &App) -> String {
        self.override_search.read(cx).value().to_string()
    }
}

/// The loadout editor page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let breakpoint = layout::Breakpoint::of(window, cx);
    let totals = state.combat.loadout.totals();
    let summary = totals.line();
    let issues = state.combat.loadout.issues();
    let changed = state.combat.loadout.changed();
    let missing = state.combat.loadout.missing.clone();
    let muted = cx.theme().muted_foreground;
    let accent = cx.theme().primary;
    let tab = state.combat.tab;
    let entity = cx.entity();
    let select_tab = entity.clone();
    let reset = entity.clone();

    let selected = Tab::ALL
        .iter()
        .position(|each| *each == tab)
        .unwrap_or_default();
    let tabs = layout::tab_bar(
        "combat-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        selected,
        move |index, _window, cx| {
            select_tab.update(cx, |state, cx| state.set_combat_tab(Tab::ALL[index], cx));
        },
    );
    let header = div()
        .id("combat-header")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(div().flex_none().child(tabs))
        .child(character_select(state, window, cx))
        .child(div().flex_1())
        .child(
            div()
                .id("combat-summary")
                .role(Role::Label)
                .aria_label(summary.clone())
                .text_size(TEXT_MONO)
                .text_color(accent)
                .child(summary),
        )
        .child(
            div()
                .id("combat-changed")
                .role(Role::Label)
                .aria_label(format!("{changed} changes vs the save"))
                .text_size(TEXT_MONO)
                .text_color(muted)
                .child(format!("{changed} change(s)")),
        )
        .when(!issues.is_empty(), |header| {
            header.child(
                Tag::new()
                    .with_variant(Tone::Danger.into())
                    .outline()
                    .small()
                    .child(format!("{} issue(s)", issues.len())),
            )
        })
        .child(
            Button::new("combat-reset")
                .label("Reset")
                .small()
                .disabled(changed == 0)
                .on_click(move |_event, _window, cx| {
                    reset.update(cx, |state, cx| state.reset_combat_loadout(cx));
                }),
        );

    div()
        .id("combat-sim-page")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .when(!state.save_loaded, |page| {
            page.child(div().id("combat-no-save").child(Caption::new(
                "Import a decoded save to edit its equipment and talents; the editor starts empty.",
            )))
        })
        .when(!missing.is_empty(), |page| {
            page.child(
                Alert::new(
                    "combat-missing",
                    format!(
                        "This character carries no {}; those slots stay empty.",
                        missing.join(", ")
                    ),
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            )
        })
        .when(!issues.is_empty(), |page| {
            let details: Vec<String> = issues.iter().map(|issue| issue.detail.clone()).collect();
            page.child(
                div()
                    .id("combat-issues")
                    .role(Role::Label)
                    .aria_label(details.join(" · "))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Alert::new(
                            "combat-issues-banner",
                            format!(
                                "{} item(s) need a look before the simulator reads them.",
                                issues.len()
                            ),
                        )
                        .with_variant(Tone::Warning.into())
                        .banner(),
                    )
                    .children(
                        details
                            .into_iter()
                            .take(6)
                            .map(|detail| Caption::new(detail).text_color(muted)),
                    ),
            )
        })
        .child(presets::bar(state, cx))
        .when_some(overrides::chips(state, cx), |page, chips| page.child(chips))
        .child(match tab {
            Tab::Gear => equipment::pane(state, breakpoint, window, cx),
            Tab::Talents => talents::pane(state, window, cx),
            Tab::Mechanics => overrides::pane(state, window, cx),
            Tab::Encounter => encounter::pane(state, window, cx),
            Tab::Compare => compare::pane(state, window, cx),
        })
        .into_any_element()
}

/// The character picker: one option per save character, its label beside it
/// so it lines up with the tabs.
fn character_select(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let characters = character_choices(state);
    let has_characters = !characters.is_empty();
    let select = &state.combat.controls.character;
    select.show(characters, Some(&state.combat.character), window, cx);
    div()
        .id("combat-character-row")
        .flex()
        .items_center()
        .gap(PANEL_GAP)
        .child(Caption::new("Character").text_color(muted))
        .child(
            Select::new(select.state())
                .id("combat-character")
                .accessibility_label("Character")
                .placeholder("No characters")
                .disabled(!has_characters)
                .small()
                .w(gpui::px(180.0)),
        )
        .when(has_characters, |row| {
            row.child(
                div()
                    .id("combat-class")
                    .role(Role::Label)
                    .aria_label(state.combat.loadout.character.class.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(format!(
                        "{} · level {}",
                        state.combat.loadout.character.class, state.combat.loadout.character.level
                    )),
            )
        })
        .into_any_element()
}

/// One `Choice` per character the save carries.
fn character_choices(state: &AppState) -> Vec<Choice<usize>> {
    match &state.save {
        Some(save) => (0..save.character_count())
            .map(|index| Choice::new(index, save.character_name(index)))
            .collect(),
        None => Vec::new(),
    }
}
