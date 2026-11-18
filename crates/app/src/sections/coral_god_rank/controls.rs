//! Planner controls: every planner input, its validation and the save's data gaps.
//!
//! The inputs stay on [`super::State`]; the kit fields here only show them.
//! Edits flow field → input through the subscriptions made in
//! [`Controls::new`], and inputs flow back in render through
//! `show`/`show_number` (`docs/ui-kit/forms.md`), so Reset and an import need
//! no window.

use super::Field;
use super::plan::{ActionRow, action_rows, action_table};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Styled, Subscription, Window, div, px,
};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::table::TableState;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_coral::{Goal, Inputs, Plan};
use idleon_ui::format;
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, PANEL_GAP, RowsDelegate, Tone, number_state,
    number_value, show_number,
};
use std::rc::Rc;

/// One numeric planner field: its id suffix, label, range, step and the
/// decimals it shows.
struct Spec {
    field: Field,
    id: &'static str,
    label: &'static str,
    range: (f64, f64),
    step: f64,
    precision: usize,
}

const fn spec(
    field: Field,
    id: &'static str,
    label: &'static str,
    range: (f64, f64),
    step: f64,
    precision: usize,
) -> Spec {
    Spec {
        field,
        id,
        label,
        range,
        step,
        precision,
    }
}

/// The budget, then the grid of the other numeric fields, in page order.
const SPECS: [Spec; 11] = [
    spec(Field::Budget, "budget", "Budget", (0.0, 1e24), 1000.0, 0),
    spec(
        Field::MaxLevels,
        "max-levels",
        "Max levels",
        (0.0, 10_000.0),
        1.0,
        0,
    ),
    spec(
        Field::BeamSize,
        "beam-size",
        "Beam size",
        (1.0, 10_000.0),
        10.0,
        0,
    ),
    spec(Field::GodRank, "god-rank", "God rank", (0.0, 100.0), 1.0, 0),
    spec(Field::Y4, "y4", "Y4 %", (0.0, 100.0), 1.0, 0),
    spec(
        Field::DivCostAfter3,
        "div-cost-after-3",
        "DivCostAfter3 %",
        (0.0, 1_000.0),
        1.0,
        0,
    ),
    spec(
        Field::ReefUpgradeCount,
        "reef-upgrades",
        "Reef upgrades",
        (0.0, 100_000.0),
        1.0,
        0,
    ),
    spec(
        Field::MineFloor,
        "mine-floor",
        "Minehead floor",
        (0.0, 1_000.0),
        1.0,
        0,
    ),
    spec(
        Field::Vault76Bonus,
        "vault-76",
        "Vault 76 bonus %",
        (0.0, 100_000.0),
        1.0,
        0,
    ),
    spec(Field::Tier, "tier", "Offer tier", (0.0, 5.0), 1.0, 0),
    spec(
        Field::PlanningConfidence,
        "confidence",
        "Confidence",
        (0.1, 0.99),
        0.01,
        2,
    ),
];

/// The section's kit controls, the stored plan and its action rows.
pub(crate) struct Controls {
    goal: ChoiceSelect<Goal>,
    /// One field per [`SPECS`] entry, in the same order.
    fields: Vec<Entity<InputState>>,
    /// The last stored plan, kept once for render instead of cloned per frame.
    pub(crate) plan: Option<Rc<Plan>>,
    pub(crate) actions: Entity<TableState<RowsDelegate<ActionRow>>>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(
        run: &Entity<TaskRunner<Plan, Inputs>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) -> Self {
        let choices = Goal::ALL
            .into_iter()
            .map(|goal| Choice::new(goal, goal.label()))
            .collect();
        let goal = ChoiceSelect::new(choices, Some(&Goal::default()), window, cx);
        let fields: Vec<_> = SPECS
            .iter()
            .map(|spec| number_state(spec.range.0, spec.range.1, spec.step, window, cx))
            .collect();
        // Edits flow control -> input, held to the range the Ely field held.
        let mut subscriptions = vec![cx.subscribe(
            goal.state(),
            |state, _, event: &SelectEvent<Vec<Choice<Goal>>>, cx| {
                let SelectEvent::Confirm(Some(goal)) = event else {
                    return;
                };
                state.coral.set_goal(goal.id());
                cx.notify();
            },
        )];
        for (spec, field) in SPECS.iter().zip(&fields) {
            let (target, (min, max)) = (spec.field, spec.range);
            subscriptions.push(cx.subscribe(field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    state.coral.set(target, value.clamp(min, max));
                    cx.notify();
                }
            }));
        }
        // A stored plan is kept once and its actions become table rows here,
        // not on every frame.
        subscriptions.push(cx.subscribe(run, |state, run, event: &TaskEvent, cx| {
            if matches!(event, TaskEvent::Finished(Ending::Stored))
                && let Some(plan) = run.read(cx).result().cloned()
            {
                let rows = action_rows(&plan);
                state.coral_controls.plan = Some(Rc::new(plan));
                state.coral_controls.actions.update(cx, |table, cx| {
                    table.delegate_mut().set_rows(rows);
                    cx.notify();
                });
            }
        }));
        Self {
            goal,
            fields,
            plan: None,
            actions: cx.new(|cx| TableState::new(action_table(), window, cx)),
            _subscriptions: subscriptions,
        }
    }
}

pub(crate) fn controls_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let entity = cx.entity();
    let inputs = &state.coral.inputs;
    let controls = &state.coral_controls;
    let muted = cx.theme().muted_foreground;

    let choices = Goal::ALL
        .into_iter()
        .map(|goal| Choice::new(goal, goal.label()))
        .collect();
    controls.goal.show(choices, Some(&inputs.goal), window, cx);
    for (spec, field) in SPECS.iter().zip(&controls.fields) {
        show_number(
            field,
            super::value(inputs, spec.field),
            spec.precision,
            window,
            cx,
        );
    }
    let goal = Select::new(controls.goal.state())
        .id("coral-goal")
        .accessibility_label("Goal")
        .small();

    let assume_entity = entity.clone();
    let assume = Checkbox::new("coral-assume-unlocked")
        .label("Assume unlocked")
        .checked(inputs.assume_unlocked)
        .on_click(move |on, _window, cx| {
            let on = *on;
            assume_entity.update(cx, |state, cx| {
                state.coral.set_assume_unlocked(on);
                cx.notify();
            });
        });

    let reset =
        Button::new("coral-reset")
            .label("Reset")
            .small()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.coral.reset();
                    cx.notify();
                });
            });

    let validation = inputs.validation();
    let (tone, message) = if !validation.errors.is_empty() {
        (Tone::Danger, validation.errors.join(" "))
    } else if !validation.warnings.is_empty() {
        (Tone::Warning, validation.warnings.join(" "))
    } else {
        (Tone::Success, "Inputs valid.".into())
    };
    let unlock = unlock_state(inputs);
    let field = |index: usize| {
        let spec = &SPECS[index];
        v_flex()
            .gap_1()
            .w(px(165.0))
            .flex_none()
            .child(Caption::new(spec.label))
            .child(
                ui::number_field(
                    format!("coral-{}", spec.id),
                    &controls.fields[index],
                    spec.label,
                )
                .w_full(),
            )
    };

    let mut card = Card::new()
        .header(
            CardHeader::new("Planner controls")
                .description("The inputs for the Coral Kid and offering plan.")
                .action(reset),
        )
        .child(
            v_flex()
                .gap_3()
                .child(note("Goal", goal))
                .child(note(
                    "Budget (Divinity points)",
                    ui::number_field("coral-budget", &controls.fields[0], "Budget").w_full(),
                ))
                .child(
                    h_flex()
                        .flex_wrap()
                        .w_full()
                        .gap_3()
                        .children((1..SPECS.len()).map(field)),
                )
                .child(
                    v_flex().gap_2().child(assume).child(
                        div()
                            .id("coral-unlock-state")
                            .role(gpui::Role::Label)
                            .aria_label(unlock.clone())
                            .child(Caption::new(unlock).text_color(muted)),
                    ),
                )
                .child(
                    Alert::new("coral-validation", message)
                        .with_variant(tone.into())
                        .banner(),
                ),
        );

    if !state.coral.save.missing.is_empty() {
        card = card.child(
            Alert::new(
                "coral-missing",
                format!(
                    "Missing from the save: {}.",
                    state.coral.save.missing.join(", ")
                ),
            )
            .with_variant(Tone::Info.into())
            .banner(),
        );
    }

    v_flex()
        .id("coral-controls")
        .gap(PANEL_GAP)
        .child(card)
        .into_any_element()
}

/// A label drawn above its control.
fn note(label: &'static str, control: impl IntoElement) -> AnyElement {
    v_flex()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// What the unlock inputs currently say, matching `_upgradeStatus`.
fn unlock_state(inputs: &Inputs) -> String {
    if inputs.assume_unlocked {
        return "Unlock state: all 6 upgrades assumed unlocked.".into();
    }
    match inputs.total_divinity_levels {
        Some(total) => format!(
            "Unlock state: {} of 6 from {} total Divinity levels.",
            inputs.unlocked_count(),
            format::count(u64::from(total))
        ),
        None => "Unlock state: unknown; tick Assume unlocked.".into(),
    }
}
