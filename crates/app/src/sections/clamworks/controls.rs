//! What the section draws from but never saves: the kit controls and the
//! figures the tabs read.
//!
//! The persisted values stay on `AppState` (`clamworks_multikill`,
//! `clamworks_budget`, `clamworks_scurge`) and the claim's [`super::State`]
//! (`docs/section-state.md`). The controls only show them: edits flow control
//! → value through the subscriptions made here, and values flow back in render
//! through `show`/`show_number` (`docs/ui-kit/forms.md`). The levels the
//! section shows, the pearl value tree and the Scurge comparison are rebuilt
//! only when what they read moves, so a frame formats what is already there.

use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_clamworks::Clamworks;
use idleon_clamworks::afk::Phase;
use idleon_clamworks::scurge::{self, Inputs, MAX_AFFLICTION, MAX_OTHER_PLAYERS, Mode, Plan};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::rc::Rc;

/// The largest budget the field offers, so the number stays finite.
pub(crate) const MAX_BUDGET: f64 = 1e18;

/// The save revision and the applied plan's levels the shown levels read.
type LevelsKey = (u64, Option<[u32; 9]>);

/// The Scurge comparison, or why it cannot be built.
type ScurgePlan = Rc<Result<Plan, scurge::Error>>;

/// The section's kit controls and render caches.
pub(crate) struct Controls {
    pub(crate) multikill: Entity<InputState>,
    pub(crate) budget: Entity<InputState>,
    pub(crate) character: ChoiceSelect<usize>,
    pub(crate) phase: ChoiceSelect<Phase>,
    /// The selected character's kills per hour, on the claim and Scurge tabs.
    pub(crate) claim_kills: Entity<InputState>,
    pub(crate) scurge_kills: Entity<InputState>,
    pub(crate) drop_rate: Entity<InputState>,
    pub(crate) scurge_mode: ChoiceSelect<Mode>,
    pub(crate) max_affliction: Entity<InputState>,
    pub(crate) other_players: Entity<InputState>,
    levels: Option<(LevelsKey, Rc<Clamworks>)>,
    /// What the pearl value tree was last rooted at.
    tree_key: Option<(LevelsKey, u64)>,
    scurge: Option<((LevelsKey, Inputs), ScurgePlan)>,
    _subscriptions: Vec<Subscription>,
}

/// The pearl tables the claim can start from.
pub(crate) fn phase_choices() -> Vec<Choice<Phase>> {
    Phase::ALL
        .into_iter()
        .map(|phase| Choice::new(phase, phase.label()))
        .collect()
}

/// The two Scurge panels.
pub(crate) fn mode_choices() -> Vec<Choice<Mode>> {
    Mode::ALL
        .into_iter()
        .map(|mode| Choice::new(mode, mode.label()))
        .collect()
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let multikill = number_state(0.0, 1_000_000_000.0, 10.0, window, cx);
        let budget = number_state(0.0, MAX_BUDGET, 1_000_000.0, window, cx);
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        let phase = ChoiceSelect::new(phase_choices(), Some(&Phase::default()), window, cx);
        let claim_kills = number_state(0.0, 1_000_000.0, 100.0, window, cx);
        let scurge_kills = number_state(0.0, 1_000_000.0, 100.0, window, cx);
        let drop_rate = number_state(0.0, 1000.0, 0.1, window, cx);
        let scurge_mode = ChoiceSelect::new(mode_choices(), Some(&Mode::default()), window, cx);
        let max_affliction = number_state(0.0, f64::from(MAX_AFFLICTION), 1.0, window, cx);
        let other_players = number_state(0.0, f64::from(MAX_OTHER_PLAYERS), 1.0, window, cx);

        // Edits flow control -> value, through the section's setters.
        let subscriptions = vec![
            on_number(cx, &multikill, AppState::set_clamworks_multikill),
            on_number(cx, &budget, AppState::set_clamworks_budget),
            on_number(cx, &claim_kills, AppState::set_clamworks_kills_per_hour),
            on_number(cx, &scurge_kills, AppState::set_clamworks_kills_per_hour),
            on_number(cx, &drop_rate, AppState::set_clamworks_drop_rate),
            on_number(cx, &max_affliction, |state, level, cx| {
                state.set_clamworks_scurge_max_affliction(level as u32, cx);
            }),
            on_number(cx, &other_players, |state, players, cx| {
                state.set_clamworks_scurge_other_players(players as u32, cx);
            }),
            on_pick(cx, &character, AppState::set_clamworks_character),
            on_pick(cx, &phase, AppState::set_clamworks_phase),
            on_pick(cx, &scurge_mode, AppState::set_clamworks_scurge_mode),
        ];
        Self {
            multikill,
            budget,
            character,
            phase,
            claim_kills,
            scurge_kills,
            drop_rate,
            scurge_mode,
            max_affliction,
            other_players,
            levels: None,
            tree_key: None,
            scurge: None,
            _subscriptions: subscriptions,
        }
    }
}

/// A number field whose every edit calls `set`.
fn on_number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    set: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            set(state, value, cx);
        }
    })
}

/// A select whose pick calls `set`.
fn on_pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    set: impl Fn(&mut AppState, V, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                set(state, value.clone(), cx);
            }
        },
    )
}

impl AppState {
    fn clamworks_levels_key(&self) -> LevelsKey {
        (self.save_revision, self.clamworks_projection)
    }

    /// The levels the section shows (the applied plan's, or the save's), kept
    /// until the save or the projection changes.
    pub(crate) fn clamworks_shown(&mut self) -> Rc<Clamworks> {
        let key = self.clamworks_levels_key();
        if let Some((at, levels)) = &self.clamworks_controls.levels
            && *at == key
        {
            return levels.clone();
        }
        let levels = Rc::new(self.clamworks_view());
        self.clamworks_controls.levels = Some((key, levels.clone()));
        levels
    }

    /// Root the pearl value tree at the shown levels and multikill, once per
    /// change of either.
    pub(crate) fn sync_clamworks_tree(&mut self, cx: &mut Context<Self>) {
        let key = (
            self.clamworks_levels_key(),
            self.clamworks_multikill.to_bits(),
        );
        if self.clamworks_controls.tree_key == Some(key) {
            return;
        }
        self.clamworks_controls.tree_key = Some(key);
        let root = self
            .clamworks_shown()
            .pearl_value_tree(self.clamworks_multikill);
        self.clamworks_breakdown
            .update(cx, |tree, cx| tree.set_root(root, cx));
    }

    /// The Scurge comparison the section's inputs ask for, built once per
    /// change of the levels or the inputs.
    pub(crate) fn clamworks_scurge_plan(&mut self) -> ScurgePlan {
        let key = (self.clamworks_levels_key(), self.clamworks_scurge);
        if let Some((at, plan)) = &self.clamworks_controls.scurge
            && *at == key
        {
            return plan.clone();
        }
        // The plan previews the levels the section shows, so an applied pearl
        // plan's projection reads through here too.
        let plan = Rc::new(Plan::build(&self.clamworks_shown(), self.clamworks_scurge));
        self.clamworks_controls.scurge = Some((key, plan.clone()));
        plan
    }
}
