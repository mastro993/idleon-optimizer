//! The section's kit controls, built once: the context and explorer selects,
//! one number field per sandbox input, and which descriptor each formula tree
//! holds.
//!
//! The values stay on [`State`](super::State) (`docs/section-state.md`). Edits
//! flow control → value through the subscriptions made here, and values flow
//! back in render through `show`/`show_number` (`docs/ui-kit/forms.md`).

use super::State;
use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_stats::{
    self as stats, BreakdownCategory, ConstructionMode, ExplorerStatus, MapVisibility, Skill,
};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value, show_number};

pub(crate) struct Controls {
    pub(super) character: ChoiceSelect<usize>,
    pub(super) map: ChoiceSelect<usize>,
    pub(super) visibility: ChoiceSelect<MapVisibility>,
    pub(super) skill: ChoiceSelect<Skill>,
    pub(super) construction: ChoiceSelect<ConstructionMode>,
    /// `None` lists every category.
    pub(super) explorer_category: ChoiceSelect<Option<BreakdownCategory>>,
    pub(super) explorer_status: ChoiceSelect<ExplorerStatus>,
    /// One field per sandbox input, in ledger order. The inputs follow the
    /// save and the character, so the fields follow them ([`Self::sync`]).
    pub(super) inputs: Vec<InputField>,
    /// The sheet revision and descriptor each formula tree was last handed,
    /// so a frame replaces a root only when it changed.
    pub(super) breakdown_root: Option<(u64, String)>,
    pub(super) explorer_root: Option<(u64, String)>,
    _subscriptions: Vec<Subscription>,
}

/// One sandbox input's number field.
pub(super) struct InputField {
    pub(super) id: String,
    pub(super) field: Entity<InputState>,
    _subscription: Subscription,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        let map = ChoiceSelect::new(Vec::new(), None, window, cx);
        let visibility = ChoiceSelect::new(visibility_choices(), None, window, cx);
        let skill = ChoiceSelect::new(skill_choices(), None, window, cx);
        let construction = ChoiceSelect::new(construction_choices(), None, window, cx);
        let explorer_category = ChoiceSelect::new(category_choices(), Some(&None), window, cx);
        let explorer_status = ChoiceSelect::new(status_choices(), None, window, cx);
        let subscriptions = vec![
            on_pick(&character, cx, AppState::set_stats_character),
            on_pick(&map, cx, AppState::set_stats_map),
            on_pick(&visibility, cx, AppState::set_stats_map_visibility),
            on_pick(&skill, cx, AppState::set_stats_skill),
            on_pick(&construction, cx, AppState::set_stats_construction),
            on_pick(
                &explorer_category,
                cx,
                AppState::set_stats_explorer_category,
            ),
            on_pick(&explorer_status, cx, AppState::set_stats_explorer_status),
        ];
        Self {
            character,
            map,
            visibility,
            skill,
            construction,
            explorer_category,
            explorer_status,
            inputs: Vec::new(),
            breakdown_root: None,
            explorer_root: None,
            _subscriptions: subscriptions,
        }
    }

    /// Bring every control in line with the section's values, building a field
    /// for each sandbox input the save now carries.
    pub(super) fn sync(&mut self, stats: &State, window: &mut Window, cx: &mut Context<AppState>) {
        let context = stats.context;
        self.character.show(
            stats.characters.clone(),
            Some(&context.character),
            window,
            cx,
        );
        self.map
            .show(stats.map_choices.clone(), Some(&context.map), window, cx);
        self.visibility.show(
            visibility_choices(),
            Some(&context.map_visibility),
            window,
            cx,
        );
        self.skill
            .show(skill_choices(), Some(&context.skill), window, cx);
        self.construction.show(
            construction_choices(),
            Some(&context.construction),
            window,
            cx,
        );
        self.explorer_category.show(
            category_choices(),
            Some(&stats.explorer.category),
            window,
            cx,
        );
        self.explorer_status
            .show(status_choices(), Some(&stats.explorer.status), window, cx);
        let Some(sandbox) = stats.sandbox.as_ref() else {
            self.inputs.clear();
            return;
        };
        self.follow_inputs(sandbox.inputs(), window, cx);
        for input in &self.inputs {
            if let Some(value) = sandbox.value(&input.id) {
                show_number(&input.field, value, 0, window, cx);
            }
        }
    }

    /// Keep one field per input, in the sandbox's order: a field whose input
    /// stays keeps its focus and caret, and only a new input builds one.
    fn follow_inputs(
        &mut self,
        inputs: &[stats::GameInput],
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        let same = self.inputs.len() == inputs.len()
            && self
                .inputs
                .iter()
                .zip(inputs)
                .all(|(field, input)| field.id == input.id);
        if same {
            return;
        }
        let mut previous = std::mem::take(&mut self.inputs);
        for input in inputs {
            let kept = previous
                .iter()
                .position(|field| field.id == input.id)
                .map(|at| previous.swap_remove(at));
            let field = kept.unwrap_or_else(|| input_field(input, window, cx));
            self.inputs.push(field);
        }
    }
}

/// A number field for one sandbox input; its edits go through the sandbox's
/// own checks ([`AppState::set_stats_input`]).
fn input_field(
    input: &stats::GameInput,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> InputField {
    let field = number_state(0.0, input.max.unwrap_or(1e15), 1.0, window, cx);
    let id = input.id.clone();
    let subscription = cx.subscribe(&field, move |state, field, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            state.set_stats_input(id.clone(), value, cx);
        }
    });
    InputField {
        id: input.id.clone(),
        field,
        _subscription: subscription,
    }
}

/// A subscription that hands a confirmed pick to one of the section's setters.
fn on_pick<V: Clone + PartialEq + 'static>(
    select: &ChoiceSelect<V>,
    cx: &mut Context<AppState>,
    pick: fn(&mut AppState, V, &mut Context<AppState>),
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                pick(state, value.clone(), cx);
            }
        },
    )
}

/// The character selector's rows: every character with its class.
pub(super) fn character_choices(snapshot: &stats::Snapshot) -> Vec<Choice<usize>> {
    (0..snapshot.characters())
        .map(|character| {
            let class = stats::class_label(snapshot.class_id(character));
            Choice::new(character, format!("{} ({class})", snapshot.name(character)))
        })
        .collect()
}

/// The map selector's rows, each with its world.
pub(super) fn map_choices(
    snapshot: &stats::Snapshot,
    options: &[stats::MapOption],
) -> Vec<Choice<usize>> {
    options
        .iter()
        .map(|option| {
            let map = option.class.map;
            Choice::new(
                map,
                format!(
                    "{} · World {}",
                    option.label(snapshot.map_bonus(map)),
                    option.class.world
                ),
            )
        })
        .collect()
}

fn visibility_choices() -> Vec<Choice<MapVisibility>> {
    MapVisibility::ALL
        .map(|level| Choice::new(level, level.label()))
        .into()
}

fn skill_choices() -> Vec<Choice<Skill>> {
    Skill::ALL
        .map(|skill| Choice::new(skill, skill.label()))
        .into()
}

fn construction_choices() -> Vec<Choice<ConstructionMode>> {
    ConstructionMode::ALL
        .map(|mode| Choice::new(mode, mode.label()))
        .into()
}

fn category_choices() -> Vec<Choice<Option<BreakdownCategory>>> {
    std::iter::once(Choice::new(None, "All categories"))
        .chain(BreakdownCategory::ALL.map(|entry| Choice::new(Some(entry), entry.label())))
        .collect()
}

fn status_choices() -> Vec<Choice<ExplorerStatus>> {
    ExplorerStatus::ALL
        .map(|entry| Choice::new(entry, entry.label()))
        .into()
}
