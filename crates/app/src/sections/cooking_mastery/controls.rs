//! The section's kit form controls and what its panes draw, built once.
//!
//! The persisted goals, budgets and character stay on [`State`]
//! (`docs/section-state.md`). The controls only show them: edits flow control →
//! value through the subscriptions made here, and values flow back in render
//! through `show`/`show_number` (`docs/ui-kit/forms.md`). The ranking, the
//! daily distribution and the yellow allocation are planned by
//! [`AppState::sync_mastery`] when the save or an input changes, never on a
//! frame that only redraws.

use super::State;
use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_cooking_mastery::{Allocation, Goal, Plan, Upgrade};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};

/// The section's kit controls and its planned figures.
pub(crate) struct Controls {
    pub(crate) goal: ChoiceSelect<Goal>,
    pub(crate) budget: Entity<InputState>,
    pub(crate) yellow_goal: ChoiceSelect<&'static str>,
    pub(crate) yellow_budget: Entity<InputState>,
    pub(crate) character: ChoiceSelect<usize>,
    /// What the panes draw; `None` until a save carries mastery ranks.
    pub(crate) derived: Option<Derived>,
    /// The save revision and inputs `derived` was planned from.
    key: Option<(u64, State)>,
    _subscriptions: Vec<Subscription>,
}

/// The figures planned from the save and the section's inputs.
pub(crate) struct Derived {
    /// Every EXP upgrade, best next point first.
    pub(crate) ranking: Vec<Upgrade>,
    /// The daily distribution for the goal and budget.
    pub(crate) plan: Plan,
    /// The yellow allocation; `None` when no goal has two leveled meals.
    pub(crate) yellow: Option<Allocation>,
    /// The goals the save offers the allocator.
    pub(crate) yellow_goals: Vec<Choice<&'static str>>,
    /// The characters the stat goals can scale with.
    pub(crate) characters: Vec<Choice<usize>>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let goal_choices = Goal::ALL
            .into_iter()
            .map(|goal| Choice::new(goal, goal.label()))
            .collect();
        let goal = ChoiceSelect::new(goal_choices, Some(&Goal::default()), window, cx);
        let budget = number_state(0.0, f64::MAX, 1.0, window, cx);
        let yellow_goal = ChoiceSelect::new(Vec::new(), None, window, cx);
        let yellow_budget = number_state(0.0, f64::MAX, 1.0, window, cx);
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        // Edits flow control -> section value. A budget is held to the points
        // the save grants, as the Ely stepper's range held it.
        let subscriptions = vec![
            cx.subscribe(
                goal.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Goal>>>, cx| {
                    let SelectEvent::Confirm(Some(goal)) = event else {
                        return;
                    };
                    state.mastery_plan.set_goal(goal.id());
                    cx.notify();
                },
            ),
            cx.subscribe(&budget, |state, field, event: &InputEvent, cx| {
                if let InputEvent::Change = event
                    && let Some(points) = number_value(field.read(cx))
                    && let Some(mastery) = &state.mastery
                {
                    let budget = points.min(mastery.points_earned());
                    state.mastery_plan.set_budget(budget);
                    cx.notify();
                }
            }),
            cx.subscribe(
                yellow_goal.state(),
                |state, _, event: &SelectEvent<Vec<Choice<&'static str>>>, cx| {
                    let SelectEvent::Confirm(Some(id)) = event else {
                        return;
                    };
                    state.mastery_plan.set_yellow_goal(id);
                    cx.notify();
                },
            ),
            cx.subscribe(&yellow_budget, |state, field, event: &InputEvent, cx| {
                if let InputEvent::Change = event
                    && let Some(points) = number_value(field.read(cx))
                    && let Some(yellow) = &state.yellow
                {
                    let budget = points.clamp(0.0, yellow.budget);
                    state.mastery_plan.set_yellow_budget(budget);
                    cx.notify();
                }
            }),
            cx.subscribe(
                character.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    let SelectEvent::Confirm(Some(character)) = event else {
                        return;
                    };
                    state.mastery_plan.set_character(*character);
                    cx.notify();
                },
            ),
        ];
        Self {
            goal,
            budget,
            yellow_goal,
            yellow_budget,
            character,
            derived: None,
            key: None,
            _subscriptions: subscriptions,
        }
    }
}

impl AppState {
    /// Plan the ranking, the distribution and the allocation again when the
    /// save or an input changed; otherwise a comparison and nothing more.
    pub(crate) fn sync_mastery(&mut self, cx: &mut Context<Self>) {
        let revision = self.save_revision;
        let current = self
            .mastery_controls
            .key
            .as_ref()
            .is_some_and(|(at, inputs)| *at == revision && *inputs == self.mastery_plan);
        if current {
            return;
        }
        let save_changed = self
            .mastery_controls
            .key
            .as_ref()
            .is_none_or(|(at, _)| *at != revision);
        self.mastery_controls.derived = self.mastery.as_ref().map(|mastery| {
            let yellow = self.yellow.as_ref();
            Derived {
                ranking: mastery.ranking(),
                plan: self.mastery_plan.plan(mastery),
                yellow: yellow.and_then(|yellow| self.mastery_plan.yellow_plan(yellow)),
                yellow_goals: yellow
                    .map(|yellow| {
                        yellow
                            .goals()
                            .into_iter()
                            .map(|goal| Choice::new(goal.id, goal.name))
                            .collect()
                    })
                    .unwrap_or_default(),
                characters: yellow
                    .map(|yellow| {
                        yellow
                            .characters
                            .iter()
                            .enumerate()
                            .map(|(index, character)| Choice::new(index, character.name.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        });
        if save_changed && let Some(mastery) = &self.mastery {
            let root = mastery.breakdown();
            self.mastery_breakdown
                .update(cx, |tree, cx| tree.set_root(root, cx));
        }
        self.mastery_controls.key = Some((revision, self.mastery_plan.clone()));
    }
}
