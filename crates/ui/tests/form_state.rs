//! The form-state pattern of `docs/ui-kit/forms.md`, compiled and driven: the
//! Opal Optimizer's target `Select` and one villager's opal `NumberInput`,
//! converted to the kit. The doc quotes the regions between the `region`
//! markers verbatim, and `the_doc_quotes_this_example` keeps them in step.

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, IntoElement, Render, Subscription, TestAppContext, VisualTestContext,
    Window, div,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::select::{Select, SelectEvent};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value, show_number};

const VILLAGERS: usize = 3;
const NAMES: [&str; VILLAGERS] = ["Theo", "Bolai", "Kaiju"];
const MAX_OPALS: f64 = 1_000.0;

/// Stands in for the app's window state: one plain field per section.
struct AppState {
    opal: State,
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(target_select(self, window, cx))
            .child(manual_input(self, 0, window, cx))
    }
}

// region: state
/// The section's state: the values it persists, and the kit controls that
/// show them. The controls are built once, here, never in render.
pub(crate) struct State {
    /// Persisted. `None` targets every villager's total EXP.
    pub(crate) target: Option<usize>,
    /// Persisted. The opals typed in for each villager.
    pub(crate) manual: [f64; VILLAGERS],
    /// How many villagers the save unlocked; set by `on_import`.
    pub(crate) unlocked: usize,
    target_select: ChoiceSelect<Option<usize>>,
    manual_fields: [Entity<InputState>; VILLAGERS],
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let target_select = ChoiceSelect::new(target_choices(0), Some(&None), window, cx);
        let manual_fields = std::array::from_fn(|_| number_state(0.0, MAX_OPALS, 1.0, window, cx));
        // Edits flow control -> section value, through the section's setters.
        let mut subscriptions = vec![cx.subscribe(
            target_select.state(),
            |state, _, event: &SelectEvent<Vec<Choice<Option<usize>>>>, cx| {
                let SelectEvent::Confirm(target) = event;
                state.opal.set_target(target.flatten(), cx);
            },
        )];
        for (villager, field) in manual_fields.iter().enumerate() {
            subscriptions.push(cx.subscribe(field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(opals) = number_value(field.read(cx))
                {
                    state.opal.set_manual(villager, opals, cx);
                }
            }));
        }
        Self {
            target: None,
            manual: [0.0; VILLAGERS],
            unlocked: 0,
            target_select,
            manual_fields,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn set_target(&mut self, target: Option<usize>, cx: &mut Context<AppState>) {
        self.target = target;
        cx.notify();
    }

    pub(crate) fn set_manual(&mut self, villager: usize, opals: f64, cx: &mut Context<AppState>) {
        self.manual[villager] = opals.clamp(0.0, MAX_OPALS);
        cx.notify();
    }

    /// `apply_config` writes the persisted values only: it has no `Window`,
    /// and the controls catch up on the next render.
    pub(crate) fn apply_config(&mut self, config: Config, cx: &mut Context<AppState>) {
        self.target = config.target;
        self.manual = config.manual;
        cx.notify();
    }
}

/// The target list: everyone, then each unlocked villager.
fn target_choices(unlocked: usize) -> Vec<Choice<Option<usize>>> {
    std::iter::once(Choice::new(None, "All villagers (total EXP)"))
        .chain((0..unlocked).map(|villager| Choice::new(Some(villager), NAMES[villager])))
        .collect()
}
// endregion: state

// region: render
/// Values flow section value -> control here: `show`/`show_number` write
/// only when the control shows something else, and never emit an edit.
fn target_select(state: &AppState, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let opal = &state.opal;
    let choices = target_choices(opal.unlocked);
    opal.target_select
        .show(choices, Some(&opal.target), window, cx);
    Select::new(opal.target_select.state())
        .id("opal-target")
        .accessibility_label("Target")
        .small()
}

fn manual_input(
    state: &AppState,
    villager: usize,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let field = &state.opal.manual_fields[villager];
    show_number(field, state.opal.manual[villager], 0, window, cx);
    // `NumberInput` has no `id`: a wrapper keeps the Ely control's id.
    div().id(format!("opal-manual-{villager}")).child(
        NumberInput::new(field)
            .accessibility_label(format!("{} opals", NAMES[villager]))
            .small(),
    )
}
// endregion: render

/// The section's persisted config, as `config()` writes it.
pub(crate) struct Config {
    target: Option<usize>,
    manual: [f64; VILLAGERS],
}

fn window(cx: &mut TestAppContext) -> (Entity<AppState>, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    cx.add_window_view(|window, cx| AppState {
        opal: State::new(window, cx),
    })
}

#[gpui::test]
fn a_pick_and_typing_reach_the_section_state(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |state, cx| {
        state.opal.unlocked = 2;
        cx.notify();
    });
    cx.run_until_parked();

    let select = view.read_with(cx, |state, _| state.opal.target_select.state().clone());
    select.update(cx, |_, cx| cx.emit(SelectEvent::Confirm(Some(Some(1)))));
    assert_eq!(view.read_with(cx, |state, _| state.opal.target), Some(1));

    let field = view.read_with(cx, |state, _| state.opal.manual_fields[0].clone());
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.set_value("", window, cx);
            field.focus(window, cx);
        })
    });
    cx.simulate_input("42");
    assert_eq!(view.read_with(cx, |state, _| state.opal.manual[0]), 42.0);
}

#[gpui::test]
fn an_applied_config_reaches_the_controls_on_the_next_render(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let (select, field) = view.read_with(cx, |state, _| {
        (
            state.opal.target_select.state().clone(),
            state.opal.manual_fields[0].clone(),
        )
    });
    view.update(cx, |state, cx| {
        state.opal.unlocked = VILLAGERS;
        state.opal.apply_config(
            Config {
                target: Some(2),
                manual: [7.0, 0.0, 0.0],
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(
        select.read_with(cx, |select, _| select.selected_value().cloned()),
        Some(Some(2))
    );
    assert_eq!(field.read_with(cx, |field, _| field.value()), "7");
    // The controls are the same entities: render never rebuilt them.
    view.read_with(cx, |state, _| {
        assert_eq!(state.opal.target_select.state(), &select);
        assert_eq!(state.opal.manual_fields[0], field);
    });
}

#[test]
fn the_doc_quotes_this_example() {
    let doc = include_str!("../../../docs/ui-kit/forms.md");
    let source = include_str!("form_state.rs");
    for name in ["state", "render"] {
        let start = format!("// region: {name}\n");
        let end = format!("// endregion: {name}\n");
        let from = source.find(&start).expect("region start") + start.len();
        let to = source[from..].find(&end).expect("region end") + from;
        assert!(
            doc.contains(&source[from..to]),
            "docs/ui-kit/forms.md no longer quotes the `{name}` region of crates/ui/tests/form_state.rs"
        );
    }
}
