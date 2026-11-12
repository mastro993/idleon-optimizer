//! The Combat Simulator section: the loadout editor (CS-01).
//!
//! Ported from the Setup editor in `combat-sim-calc.html` and
//! `js/ui/combat-sim-setup-ui.js`: the character picker, the equipment slots
//! with their saved upgrade deltas, the talent table and the named loadouts.
//! The save-derived defaults and the item catalog come from
//! [`idleon_combat`]; every edit recomputes the gear totals the header
//! shows, so the summary never lags behind the editor.
//!
//! The mechanics overrides (CS-02) sit beside it: the curated mechanics the
//! simulator can disable, set or scale, with the active rules shown as chips and
//! a reset back to the save. The encounter run (CS-03) and the result tabs land
//! on top of both; the editor owns the loadout and overrides they read.

mod actions;

pub(crate) mod equipment;
pub(crate) mod overrides;
pub(crate) mod presets;
pub(crate) mod talents;

use crate::state::AppState;
use crate::ui::layout;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, Select, TextInput};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use idleon_combat::Loadout;
use idleon_save::Save;

/// The editor's tabs, matching the upstream setup editor's character tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    #[default]
    Gear,
    Talents,
    Mechanics,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Gear, Tab::Talents, Tab::Mechanics];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Gear => "gear",
            Tab::Talents => "talents",
            Tab::Mechanics => "mechanics",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Gear => "Equipment",
            Tab::Talents => "Talents",
            Tab::Mechanics => "Mechanics",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
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
    pub(crate) name: Entity<TextInput>,
    /// The talent table's filter.
    pub(crate) talent_search: Entity<TextInput>,
    /// The mechanic overrides the working loadout carries.
    pub(crate) overrides: idleon_combat::State,
    /// The mechanics pane's search field.
    pub(crate) override_search: Entity<TextInput>,
    /// The mechanics pane's group filter; empty means every group.
    pub(crate) override_group: String,
    /// Whether the mechanics pane lists only the overridden rows.
    pub(crate) override_only_active: bool,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let name = crate::ui::text_input("Loadout name", "Loadout name", window, cx);
        let talent_search = crate::ui::text_input("Find a talent", "Find a talent", window, cx);
        let override_search =
            crate::ui::text_input("crystal, drop rate, orb…", "Find a mechanic", window, cx);
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
        }
    }

    /// Replace the working loadout with the save's defaults for `character`,
    /// keeping the selected slot when it still exists. Named presets stay: they
    /// live in the project, not in the save.
    pub(crate) fn load(&mut self, save: &Save, character: usize) {
        self.character = character;
        self.loadout = Loadout::from_save(save, character);
        if self.selected >= self.loadout.slots.len() {
            self.selected = 0;
        }
    }

    /// The preset name typed into the bar, trimmed.
    pub(crate) fn preset_name(&self, cx: &App) -> String {
        self.name.read(cx).text().trim().to_string()
    }

    /// The talent table's query.
    pub(crate) fn talent_query(&self, cx: &App) -> String {
        self.talent_search.read(cx).text().to_string()
    }

    /// The mechanics pane's query.
    pub(crate) fn override_query(&self, cx: &App) -> String {
        self.override_search.read(cx).text().to_string()
    }
}

/// The loadout editor page.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let breakpoint = layout::Breakpoint::of(window);
    let totals = state.combat.loadout.totals();
    let summary = totals.line();
    let issues = state.combat.loadout.issues();
    let changed = state.combat.loadout.changed();
    let missing = state.combat.loadout.missing.clone();
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let tab = state.combat.tab;
    let entity = cx.entity();
    let select_tab = entity.clone();
    let reset = entity.clone();
    let reset_enabled = changed > 0;

    let tabs = TabBar::new("combat-tabs")
        .selected(SharedString::from(tab.id()))
        .tab(WindowTab::new(Tab::Gear.id(), Tab::Gear.label()))
        .tab(WindowTab::new(Tab::Talents.id(), Tab::Talents.label()))
        .tab(WindowTab::new(Tab::Mechanics.id(), Tab::Mechanics.label()))
        .on_select(move |id, _window, cx| {
            if let Some(tab) = Tab::from_id(id.as_ref()) {
                select_tab.update(cx, |state, cx| state.set_combat_tab(tab, cx));
            }
        });
    let header = div()
        .id("combat-header")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(character_select(state, cx))
        .child(div().flex_1())
        .child(
            div()
                .id("combat-summary")
                .role(Role::Label)
                .aria_label(summary.clone())
                .text_size(px(12.0))
                .text_color(accent)
                .child(summary),
        )
        .child(
            div()
                .id("combat-changed")
                .role(Role::Label)
                .aria_label(format!("{changed} changes vs the save"))
                .text_size(px(12.0))
                .text_color(muted)
                .child(format!("{changed} change(s)")),
        )
        .when(!issues.is_empty(), |header| {
            header.child(
                Badge::new(format!("{} issue(s)", issues.len()))
                    .tone(ely_gpui_component::data_display::Tone::Danger),
            )
        })
        .child(
            Button::new("combat-reset", "Reset")
                .size(ControlSize::Sm)
                .disabled(!reset_enabled)
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
                Banner::new(
                    "combat-missing",
                    Severity::Warning,
                    format!(
                        "This character carries no {}; those slots stay empty.",
                        missing.join(", ")
                    ),
                )
                .into_any_element(),
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
                    .child(Banner::new(
                        "combat-issues-banner",
                        Severity::Warning,
                        format!(
                            "{} item(s) need a look before the simulator reads them.",
                            issues.len()
                        ),
                    ))
                    .children(
                        details.into_iter().take(6).map(|detail| {
                            div().text_size(px(11.0)).text_color(muted).child(detail)
                        }),
                    ),
            )
        })
        .child(presets::bar(state, cx))
        .when_some(overrides::chips(state, cx), |page, chips| page.child(chips))
        .child(match tab {
            Tab::Gear => equipment::pane(state, breakpoint, cx),
            Tab::Talents => talents::pane(state, window, cx),
            Tab::Mechanics => overrides::pane(state, window, cx),
        })
        .into_any_element()
}

/// The character picker: one option per save character.
fn character_select(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let characters = character_choices(state);
    let has_characters = !characters.is_empty();
    let entity = cx.entity();
    div()
        .id("combat-character-row")
        .flex()
        .items_end()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Character").text_color(muted))
                .child(
                    Select::new("combat-character", characters)
                        .label("Character")
                        .placeholder("No characters")
                        .selected(state.combat.character.to_string())
                        .disabled(!has_characters)
                        .on_change(move |value, _window, cx| {
                            if let Ok(index) = value.parse::<usize>() {
                                entity.update(cx, |state, cx| {
                                    state.set_combat_character(index, cx);
                                });
                            }
                        }),
                ),
        )
        .when(has_characters, |row| {
            row.child(
                div()
                    .id("combat-class")
                    .role(Role::Label)
                    .aria_label(state.combat.loadout.character.class.clone())
                    .text_size(px(12.0))
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
fn character_choices(state: &AppState) -> Vec<Choice> {
    match &state.save {
        Some(save) => (0..save.character_count())
            .map(|index| Choice::new(index.to_string(), save.character_name(index)))
            .collect(),
        None => Vec::new(),
    }
}
