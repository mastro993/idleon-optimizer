//! The Ladle Optimizer section: the ladle summary, its characters and the Cooking
//! EXP balloons.
//!
//! LD-01 builds the Ladles and Cooking EXP tabs; LD-02 adds the Kitchens &
//! Jellyfish reset, with each reset's layout and its copyable instructions.
//! LD-03 adds the forecast comparison and the meal outcomes, and LD-04 the Blue
//! Jellyfish donation planner and the No Meal Left Behind steering panel, both
//! on the same tab.
//!
//! The four values a ladle rate needs and a save cannot answer (Cooking
//! efficiency, AFK gain rate, EXP multiplier and skill prowess) are the panel
//! inputs in [`Panel`]; the shared account stats engine is not built yet, so the
//! section asks the player for them and says so. Everything else comes from the
//! save.
//!
//! The section draws with GPUI Kit: its controls are built once in
//! [`controls`], and what the tabs show that costs a calculation is worked out
//! in [`derived`] when its inputs change, never per frame.

pub(crate) mod breakdowns;
mod controls;
mod derived;
pub(crate) mod exp;
pub(crate) mod forecast;
pub(crate) mod kitchens;
pub(crate) mod outcomes;
pub(crate) mod planner;
pub(crate) mod summary;
mod table;
#[cfg(test)]
mod tests;

use crate::state::AppState;
use crate::ui::layout::tab_bar;
use crate::ui::number_field;
use controls::Controls;
use derived::Derived;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use idleon_ladle::kitchen::{self, Scenario};
use idleon_ladle::{EXP_BALLOONS, Panel};
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Caption, Choice, show_number};
use outcomes::OutcomeSort;

/// The ladle page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// The summary, the character rates and the breakdowns.
    #[default]
    Ladles,
    /// The daily kitchen layout, the reset investment and its instructions.
    Kitchens,
    /// Cooking EXP balloons per character.
    Exp,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Ladles, Tab::Kitchens, Tab::Exp];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Ladles => "ladles",
            Tab::Kitchens => "kitchens",
            Tab::Exp => "exp",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Ladles => "Ladles",
            Tab::Kitchens => "Kitchens & Jellyfish",
            Tab::Exp => "Cooking EXP",
        }
    }
}

/// The section's own state: the selectors, the panel inputs, the checked
/// characters, the three formula trees, the kit controls and the figures
/// derived from them.
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The character whose ladle hours the summary and later tabs use.
    pub(crate) ladle_character: usize,
    /// The character whose breakdowns are drawn.
    pub(crate) breakdown_character: usize,
    /// `AFK claim every (h)`: how often the player claims.
    pub(crate) claim_hours: f64,
    pub(crate) panel: Panel,
    /// The characters whose ladles the totals count.
    pub(crate) assigned: Vec<usize>,
    /// The EXP balloon the Cooking EXP tab spends, as an index into
    /// [`EXP_BALLOONS`].
    pub(crate) balloon: usize,
    /// The target Cooking level, or 0 for the next level.
    pub(crate) target_level: u32,
    /// Share the owned balloons to keep levels even.
    pub(crate) split: bool,
    /// The Kitchens & Jellyfish inputs: the horizon, the donation policy, the
    /// meal cost terms and the layout pins.
    pub(crate) kitchen: kitchen::Inputs,
    /// Whether the reset plan spends the ladles the save already carries.
    pub(crate) use_owned: bool,
    /// The reset whose layout the forecast draws, 1-based.
    pub(crate) day: usize,
    /// The scenario the comparison and the forecast draw.
    pub(crate) scenario: Scenario,
    /// How the meal outcomes table orders its rows.
    pub(crate) outcome_sort: OutcomeSort,
    /// The resets whose instructions are expanded, by day number.
    pub(crate) open_days: Vec<u32>,
    /// Whether the kitchen pins and meal exclusions are shown.
    pub(crate) show_layout: bool,
    pub(crate) ladles_tree: Entity<FormulaTree>,
    pub(crate) claims_tree: Entity<FormulaTree>,
    pub(crate) balloons_tree: Entity<FormulaTree>,
    controls: Controls,
    derived: Derived,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let ladles_tree = crate::ui::formula_tree("ladle-ladles-formula", window, cx);
        let claims_tree = crate::ui::formula_tree("ladle-claims-formula", window, cx);
        let balloons_tree = crate::ui::formula_tree("ladle-balloons-formula", window, cx);
        Self {
            tab: Tab::Ladles,
            ladle_character: 0,
            breakdown_character: 0,
            claim_hours: 24.0,
            panel: Panel::default(),
            assigned: Vec::new(),
            balloon: 0,
            target_level: 0,
            split: false,
            kitchen: kitchen::Inputs::default(),
            use_owned: true,
            day: 1,
            scenario: Scenario::Recommended,
            outcome_sort: OutcomeSort::Gain,
            open_days: vec![1],
            show_layout: false,
            ladles_tree,
            claims_tree,
            balloons_tree,
            controls: Controls::new(window, cx),
            derived: Derived::default(),
        }
    }

    /// The balloon the Cooking EXP tab spends.
    pub(crate) fn balloon_key(&self) -> idleon_ladle::Balloon {
        EXP_BALLOONS[self.balloon.min(EXP_BALLOONS.len() - 1)]
    }

    /// Count or stop counting a character.
    pub(crate) fn toggle_assigned(&mut self, index: usize) {
        toggle(&mut self.assigned, index);
    }

    /// Adopt a new roster: keep the selections the save still carries and, when
    /// nothing is checked yet, count the characters AFK-ing Cooking.
    /// A schedule applied from the previous save no longer describes this one.
    pub(crate) fn adopt(&mut self, ladle: &idleon_ladle::Ladle) {
        let characters = ladle.characters.len();
        adopt(&mut self.assigned, ladle);
        self.kitchen.donations = None;
        self.ladle_character = self.ladle_character.min(characters.saturating_sub(1));
        self.breakdown_character = self.breakdown_character.min(characters.saturating_sub(1));
    }

    /// Point one meal at a target level, or clear it with 0.
    pub(crate) fn set_target(&mut self, meal: usize, level: f64) {
        set_target(&mut self.kitchen.targets, meal, level);
    }

    /// Pin one kitchen to a meal, or let the optimizer choose with `None`.
    pub(crate) fn set_pin(&mut self, index: usize, meal: Option<i32>) {
        match meal {
            Some(meal) => self.kitchen.layout.pins.insert(index, meal),
            None => self.kitchen.layout.pins.remove(&index),
        };
    }

    /// Exclude or re-include one meal in the reset layout.
    pub(crate) fn toggle_excluded(&mut self, meal: usize) {
        match self
            .kitchen
            .layout
            .excluded
            .iter()
            .position(|entry| *entry == meal)
        {
            Some(position) => {
                self.kitchen.layout.excluded.remove(position);
            }
            None => self.kitchen.layout.excluded.push(meal),
        }
    }

    /// Change one input; the next render rebuilds the plan from it.
    pub(crate) fn set_kitchen(&mut self, edit: impl FnOnce(&mut kitchen::Inputs)) {
        edit(&mut self.kitchen);
    }
}

/// Set one meal's target, growing the list to reach it.
fn set_target(targets: &mut Vec<f64>, meal: usize, level: f64) {
    if targets.len() <= meal {
        targets.resize(meal + 1, 0.0);
    }
    targets[meal] = level.max(0.0);
}

/// Count or stop counting one character, keeping the list in save order.
fn toggle(assigned: &mut Vec<usize>, index: usize) {
    match assigned.iter().position(|entry| *entry == index) {
        Some(position) => {
            assigned.remove(position);
        }
        None => {
            assigned.push(index);
            assigned.sort_unstable();
        }
    }
}

/// Adopt a new roster: keep the checked characters it still carries, and count
/// the ones AFK-ing Cooking when nothing is checked yet.
fn adopt(assigned: &mut Vec<usize>, ladle: &idleon_ladle::Ladle) {
    let characters = ladle.characters.len();
    if assigned.is_empty() {
        *assigned = ladle.cooking_characters();
    }
    assigned.retain(|index| *index < characters);
}

/// The ladle section: the tab strip, the selectors and the pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(ladle) = state.ladle.as_ref() else {
        return no_roster(cx);
    };
    // Bring the derived figures in line with the save and the inputs first:
    // each is rebuilt only when what it reads moved.
    state
        .ladle_state
        .sync(state.save_revision, ladle, &state.ladle_plan, window, cx);
    let state: &AppState = state;
    let ladle = state.ladle.as_ref().expect("checked above");
    let ladle_state = &state.ladle_state;
    let tab = ladle_state.tab;
    let summary = summary::line(ladle, &ladle_state.derived.stats, &ladle_state.assigned);
    let entity = cx.entity();
    let selected = Tab::ALL.iter().position(|each| *each == tab).unwrap_or(0);
    let tabs = tab_bar(
        "ladle-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        selected,
        move |index, _window, cx| {
            entity.update(cx, |state, cx| {
                state.ladle_state.tab = Tab::ALL[index];
                cx.notify();
            });
        },
    )
    .flex_1();
    let header = div()
        .id("ladle-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(
            div()
                .id("ladle-summary-line")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex_none()
                .child(Caption::new(summary)),
        );

    div()
        .id("ladle-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(toolbar(state, window, cx))
        .child(match tab {
            Tab::Ladles => summary::pane(state, ladle, window, cx),
            Tab::Kitchens => kitchens::pane(state, ladle, window, cx),
            Tab::Exp => exp::pane(state, ladle, window, cx),
        })
        .into_any_element()
}

/// No character rows: the section has nothing to rate, so it says what it needs.
fn no_roster(cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("ladle-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(import_prompt(
            "ladle-empty",
            "ladle-import",
            "No Cooking rows",
            "The ladle optimizer reads each character's Cooking level, EXP and AFK target from a \
             save. Import one to see the ladle rates, claim plan and EXP balloons.",
            cx,
        ))
        .into_any_element()
}

/// The empty state both the section and the Kitchens tab show without rows:
/// what the tab needs, and the import that brings it.
pub(super) fn import_prompt(
    id: &'static str,
    import_id: &'static str,
    title: &'static str,
    body: &'static str,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    div()
        .id(id)
        .child(
            Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)))
                        .title(EmptyTitle::new().child(title))
                        .description(EmptyDescription::new().child(body)),
                )
                .content(
                    EmptyContent::new().child(
                        Button::new(import_id)
                            .label("Import save")
                            .icon(IconName::Upload)
                            .on_click(move |_event, window, cx| {
                                import.update(cx, |state, cx| state.open_import(window, cx));
                            }),
                    ),
                ),
        )
        .into_any_element()
}

/// The toolbar: who uses the ladles, whose breakdown is drawn, and how
/// often the player claims.
fn toolbar(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let controls = &ladle_state.controls;
    let characters = state
        .ladle
        .as_ref()
        .map_or(&[][..], |ladle| &ladle.characters[..]);
    let multiplier = ladle_state.panel.ladle_hours();
    let ladle_choices: Vec<Choice<usize>> = characters
        .iter()
        .map(|character| {
            Choice::new(
                character.index,
                format!("{} · ×{multiplier:.3}", character.name),
            )
        })
        .collect();
    let breakdown_choices: Vec<Choice<usize>> = characters
        .iter()
        .map(|character| Choice::new(character.index, character.name.clone()))
        .collect();
    let has_characters = !characters.is_empty();
    controls.ladle_character.show(
        ladle_choices,
        Some(&ladle_state.ladle_character),
        window,
        cx,
    );
    controls.breakdown_character.show(
        breakdown_choices,
        Some(&ladle_state.breakdown_character),
        window,
        cx,
    );
    show_number(
        &controls.claim_hours,
        ladle_state.claim_hours,
        1,
        window,
        cx,
    );

    div()
        .id("ladle-toolbar")
        .flex()
        .flex_none()
        .items_end()
        .gap_3()
        .child(
            field("Uses ladles").child(
                div().flex_none().w(px(220.0)).child(
                    Select::new(controls.ladle_character.state())
                        .id("ladle-character")
                        .accessibility_label("Uses ladles")
                        .disabled(!has_characters)
                        .small(),
                ),
            ),
        )
        .child(
            field("Breakdown character").child(
                div().flex_none().w(px(180.0)).child(
                    Select::new(controls.breakdown_character.state())
                        .id("ladle-breakdown-character")
                        .accessibility_label("Breakdown character")
                        .disabled(!has_characters)
                        .small(),
                ),
            ),
        )
        .child(field("AFK claim every (h)").child(number_field(
            "ladle-claim-hours",
            &controls.claim_hours,
            "AFK claim every (h)",
        )))
        .into_any_element()
}

/// A control under its caption.
pub(super) fn field(label: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label.into()))
}
