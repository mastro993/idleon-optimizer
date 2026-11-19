//! What the section draws from but never saves: the kit controls and the
//! open-slot cache.
//!
//! The persisted values stay in [`super::State`]. The controls are built once
//! here and edit it only through `AppState`'s setters (`docs/ui-kit/forms.md`).

use crate::state::AppState;
use gpui::{App, AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use idleon_worship as worship;
use std::rc::Rc;

/// The open platform slots, with the trial and the standing towers (platform
/// and x bits) they were found for.
type Slots = (usize, Vec<(usize, u64)>, Rc<[(usize, f64)]>);

/// The section's live controls and render caches.
pub(crate) struct Controls {
    pub(crate) trial: ChoiceSelect<usize>,
    /// The optimizer's screening runs, base seed and wave cap.
    pub(crate) seeds: Entity<InputState>,
    pub(crate) seed: Entity<InputState>,
    pub(crate) wave: Entity<InputState>,
    /// The replay scrubber, rebuilt for a replay with another frame count.
    scrubber: Option<(usize, Entity<SliderState>, Subscription)>,
    slots: Option<Slots>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let trial = ChoiceSelect::new(trial_choices(), Some(&0), window, cx);
        let seeds = number_state(1.0, 6.0, 1.0, window, cx);
        let seed = number_state(0.0, 999_999.0, 1.0, window, cx);
        let wave = number_state(10.0, 1000.0, 10.0, window, cx);
        let subscriptions = vec![
            cx.subscribe(
                trial.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    let SelectEvent::Confirm(trial) = event;
                    if let Some(trial) = *trial {
                        state.worship_set_trial(trial, cx);
                    }
                },
            ),
            number(cx, &seeds, AppState::worship_set_optimize_seeds),
            number(cx, &seed, AppState::worship_set_optimize_seed),
            number(cx, &wave, AppState::worship_set_optimize_wave),
        ];
        Self {
            trial,
            seeds,
            seed,
            wave,
            scrubber: None,
            slots: None,
            _subscriptions: subscriptions,
        }
    }
}

/// Every trial, labelled by its mob and map.
pub(crate) fn trial_choices() -> Vec<Choice<usize>> {
    worship::data()
        .trials
        .iter()
        .map(|trial| {
            Choice::new(
                trial.trial,
                format!("{} · map {}", trial.mob_def.name, trial.map),
            )
        })
        .collect()
}

/// A small segmented single choice under `id`, one segment per
/// `(value, label)` with `current` checked; segment `n` is `<id>-<n>`. A click
/// on the checked segment keeps it: the kit clears every flag then, and
/// `on_change` is not called.
pub(super) fn segmented<V: Copy + PartialEq + 'static>(
    id: &'static str,
    segments: &[(V, &'static str)],
    current: V,
    on_change: impl Fn(V, &mut Window, &mut App) + 'static,
) -> ToggleGroup {
    let values: Vec<V> = segments.iter().map(|(value, _)| *value).collect();
    ToggleGroup::new(id)
        .segmented()
        .single()
        .outline()
        .small()
        .children(segments.iter().enumerate().map(|(ix, (value, label))| {
            Toggle::new(SharedString::from(format!("{id}-{ix}")))
                .label(*label)
                .checked(*value == current)
        }))
        .on_click(move |checked, window, cx| {
            if let Some(ix) = checked.iter().position(|checked| *checked) {
                on_change(values[ix], window, cx);
            }
        })
}

/// A number field whose edits call `set`.
fn number(
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

impl AppState {
    /// The slots a tower can be placed on now: the trial's legal slots that
    /// the run snaps to themselves. Found again only when the trial or the
    /// standing towers change, not on every frame of a playing run.
    pub(crate) fn worship_slots(&mut self) -> Rc<[(usize, f64)]> {
        let Some(run) = self.worship.run.as_ref() else {
            return Rc::from([]);
        };
        let trial = run.trial();
        let towers: Vec<(usize, u64)> = self
            .worship
            .view
            .as_ref()
            .map(|view| {
                view.towers
                    .iter()
                    .map(|tower| (tower.node, tower.x.to_bits()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some((cached_trial, cached_towers, slots)) = &self.worship_controls.slots
            && *cached_trial == self.worship.trial
            && *cached_towers == towers
        {
            return slots.clone();
        }
        let mut slots: Vec<(usize, f64)> = worship::legal_slots(trial, 62.0)
            .into_iter()
            .filter(|spot| run.snapped(spot.node, spot.x) == Some(spot.x))
            .map(|spot| (spot.node, spot.x))
            .collect();
        slots.truncate(120);
        let slots: Rc<[(usize, f64)]> = slots.into();
        self.worship_controls.slots = Some((self.worship.trial, towers, slots.clone()));
        slots
    }

    /// The scrubber over the replay's frames, rebuilt only for a replay with
    /// another frame count. A drag seeks the replay; render keeps the thumb
    /// on it while the replay plays.
    pub(crate) fn worship_scrubber(
        &mut self,
        last: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SliderState> {
        let controls = &mut self.worship_controls;
        if controls.scrubber.as_ref().map(|(built, ..)| *built) != Some(last) {
            let slider = cx.new(|_| SliderState::new().min(0.0).max(last as f32).step(1.0));
            let seeks = cx.subscribe(&slider, |state, _, event: &SliderEvent, cx| {
                let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                state.worship_replay_seek(f64::from(value.end()), cx);
            });
            controls.scrubber = Some((last, slider, seeks));
        }
        let slider = controls
            .scrubber
            .as_ref()
            .map(|(_, slider, _)| slider.clone())
            .expect("the scrubber was just built");
        if slider.read(cx).value().end() as usize != index {
            slider.update(cx, |slider, cx| slider.set_value(index as f32, window, cx));
        }
        slider
    }
}
