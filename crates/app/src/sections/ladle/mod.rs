//! The Ladle Optimizer section: the ladle summary, its characters and the Cooking
//! EXP balloons.
//!
//! Ported from `ladle-calc.html` and `js/stats/systems/w4/cooking-ladle.js`. LD-01
//! builds the Ladles and Cooking EXP tabs; the kitchen reset, forecast and NMLB
//! planner tabs arrive with LD-02 to LD-04 and are added to [`Tab::ALL`] then, so
//! the strip never offers a tab that has nothing behind it.
//!
//! The four values a ladle rate needs and a save cannot answer (Cooking
//! efficiency, AFK gain rate, EXP multiplier and skill prowess) are the panel
//! inputs in [`Panel`]; the shared account stats engine is not built yet, so the
//! section asks the player for them and says so. Everything else comes from the
//! save.

pub mod breakdowns;
pub mod exp;
pub mod summary;
#[cfg(test)]
mod tests;

use crate::state::AppState;
use crate::ui::formula::FormulaTree;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div,
};
use idleon_core::ladle::{EXP_BALLOONS, Panel};

/// The ladle page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// The summary, the character rates and the breakdowns.
    #[default]
    Ladles,
    /// Cooking EXP balloons per character.
    Exp,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Ladles, Tab::Exp];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Ladles => "ladles",
            Tab::Exp => "exp",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Ladles => "Ladles",
            Tab::Exp => "Cooking EXP",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section's own state: the selectors, the panel inputs, the checked
/// characters and the three formula trees.
pub struct State {
    pub tab: Tab,
    /// The character whose ladle hours the summary and later tabs use.
    pub ladle_character: usize,
    /// The character whose breakdowns are drawn.
    pub breakdown_character: usize,
    /// `AFK claim every (h)`: how often the player claims.
    pub claim_hours: f64,
    pub panel: Panel,
    /// The characters whose ladles the totals count.
    pub assigned: Vec<usize>,
    /// The EXP balloon the Cooking EXP tab spends, as an index into
    /// [`EXP_BALLOONS`].
    pub balloon: usize,
    /// The target Cooking level, or 0 for the next level.
    pub target_level: u32,
    /// Share the owned balloons to keep levels even.
    pub split: bool,
    pub ladles_tree: Entity<FormulaTree>,
    pub claims_tree: Entity<FormulaTree>,
    pub balloons_tree: Entity<FormulaTree>,
}

impl State {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let ladles_tree = cx.new(|cx| FormulaTree::new("ladle-ladles-formula", window, cx));
        let claims_tree = cx.new(|cx| FormulaTree::new("ladle-claims-formula", window, cx));
        let balloons_tree = cx.new(|cx| FormulaTree::new("ladle-balloons-formula", window, cx));
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
            ladles_tree,
            claims_tree,
            balloons_tree,
        }
    }

    /// The balloon the Cooking EXP tab spends.
    pub fn balloon_key(&self) -> idleon_core::ladle::Balloon {
        EXP_BALLOONS[self.balloon.min(EXP_BALLOONS.len() - 1)]
    }

    /// Whether a character is counted in the totals.
    pub fn is_assigned(&self, index: usize) -> bool {
        self.assigned.contains(&index)
    }

    /// Count or stop counting a character.
    pub fn toggle_assigned(&mut self, index: usize) {
        toggle(&mut self.assigned, index);
    }

    /// Adopt a new roster: keep the selections the save still carries and, when
    /// nothing is checked yet, count the characters AFK-ing Cooking like upstream.
    pub fn adopt(&mut self, ladle: &idleon_core::ladle::Ladle) {
        let characters = ladle.characters.len();
        adopt(&mut self.assigned, ladle);
        self.ladle_character = self.ladle_character.min(characters.saturating_sub(1));
        self.breakdown_character = self.breakdown_character.min(characters.saturating_sub(1));
    }
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
/// the ones AFK-ing Cooking when nothing is checked yet, as upstream does.
fn adopt(assigned: &mut Vec<usize>, ladle: &idleon_core::ladle::Ladle) {
    let characters = ladle.characters.len();
    if assigned.is_empty() {
        *assigned = ladle.cooking_characters();
    }
    assigned.retain(|index| *index < characters);
}

/// The ladle section: the tab strip, the selectors and the pane.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(ladle) = state.ladle.clone() else {
        return no_roster(cx);
    };
    let tab = state.ladle_state.tab;
    let colors = {
        let palette = &cx.theme().colors;
        (palette.fg_muted, palette.accent)
    };
    let summary = summary::line(
        &ladle,
        state.ladle_state.panel,
        state.ladle_state.claim_hours,
        &state.ladle_state.assigned,
    );
    let entity = cx.entity();
    let select_tab = entity.clone();
    let tabs = TabBar::new("ladle-tabs")
        .selected(SharedString::from(tab.id()))
        .tab(WindowTab::new(Tab::Ladles.id(), Tab::Ladles.label()))
        .tab(WindowTab::new(Tab::Exp.id(), Tab::Exp.label()))
        .on_select(move |id, _window, cx| {
            if let Some(tab) = Tab::from_id(id.as_ref()) {
                select_tab.update(cx, |state, cx| {
                    state.ladle_state.tab = tab;
                    cx.notify();
                });
            }
        });
    let toolbar = toolbar(state, window, cx);
    let header = div()
        .id("ladle-header")
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(tabs)
        .child(div().flex_1())
        .child(
            div()
                .id("ladle-summary-line")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new(summary).text_color(colors.0)),
        );

    div()
        .id("ladle-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(toolbar)
        .child(match tab {
            Tab::Ladles => summary::pane(state, &ladle, window, cx),
            Tab::Exp => exp::pane(state, &ladle, window, cx),
        })
        .into_any_element()
}

/// No character rows: the section has nothing to rate, so it says what it needs.
fn no_roster(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("ladle-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("ladle-empty", IconName::Map, "No Cooking rows")
                .body(
                    "The ladle optimizer reads each character's Cooking level, EXP and AFK target from a                      save. Import one to see the ladle rates, claim plan and EXP balloons.",
                )
                .action(Button::new("ladle-import", "Import save").icon(IconName::Upload).on_click(
                    move |_event, window, cx| {
                        import.update(cx, |state, cx| state.open_import(window, cx));
                    },
                )),
        )
        .into_any_element()
}

/// The upstream toolbar: who uses the ladles, whose breakdown is drawn, and how
/// often the player claims.
fn toolbar(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(ladle) = state.ladle.clone() else {
        return div().into_any_element();
    };
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let characters = ladle.characters.clone();
    let multiplier = state.ladle_state.panel.ladle_hours();
    let ladle_choices: Vec<Choice> = characters
        .iter()
        .map(|character| {
            Choice::new(
                character.index.to_string(),
                format!("{} · ×{multiplier:.3}", character.name),
            )
        })
        .collect();
    let breakdown_choices: Vec<Choice> = characters
        .iter()
        .map(|character| Choice::new(character.index.to_string(), character.name.clone()))
        .collect();
    let has_characters = !characters.is_empty();
    let ladle_pick = entity.clone();
    let breakdown_pick = entity.clone();
    let hours = entity.clone();

    div()
        .id("ladle-toolbar")
        .flex()
        .flex_none()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            field("Uses ladles", muted).child(
                Select::new("ladle-character", ladle_choices)
                    .label("Uses ladles")
                    .selected(state.ladle_state.ladle_character.to_string())
                    .disabled(!has_characters)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            ladle_pick.update(cx, |state, cx| {
                                state.ladle_state.ladle_character = index;
                                cx.notify();
                            });
                        }
                    }),
            ),
        )
        .child(
            field("Breakdown character", muted).child(
                Select::new("ladle-breakdown-character", breakdown_choices)
                    .label("Breakdown character")
                    .selected(state.ladle_state.breakdown_character.to_string())
                    .disabled(!has_characters)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            breakdown_pick.update(cx, |state, cx| {
                                state.ladle_state.breakdown_character = index;
                                cx.notify();
                            });
                        }
                    }),
            ),
        )
        .child(
            field("AFK claim every (h)", muted).child(
                Stepper::new("ladle-claim-hours", state.ladle_state.claim_hours)
                    .range(0.1, 24.0)
                    .step(0.5)
                    .precision(1)
                    .on_change(move |value, _window, cx| {
                        hours.update(cx, |state, cx| {
                            state.ladle_state.claim_hours = value.clamp(0.1, 24.0);
                            cx.notify();
                        });
                    }),
            ),
        )
        .into_any_element()
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}
