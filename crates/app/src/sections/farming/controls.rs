//! What the section draws from but never saves: the header's kit controls and
//! what the tabs read from the field.
//!
//! The objective and the crop target stay on [`super::State`]
//! (`docs/section-state.md`). The controls only show them: edits flow control
//! → state through the subscriptions made here, and the state flows back in
//! render through `show`/`show_number` (`docs/ui-kit/forms.md`). The field's
//! figures, curves and tables are read from the farm once per save
//! ([`AppState::farming_field`]) and the evolution chain once per crop target
//! ([`AppState::farming_evolution`]), so a frame formats what is already there.

use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_farming::{
    self as farming, Assignment, CropScientistBonus, Diagnostics, EvolutionChain, GrowthBreakpoint,
    Objective, OvergrowthCurve, PlotMetrics, Snapshot,
};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::rc::Rc;

/// What every tab reads from one imported field.
pub(crate) struct Field {
    pub(crate) snapshot: Snapshot,
    pub(crate) plots: Vec<PlotMetrics>,
    pub(crate) curves: Vec<OvergrowthCurve>,
    pub(crate) diagnostics: Diagnostics,
    /// The Crop Scientist bonuses, and the account multiplier they scale by.
    pub(crate) crop_scientist: Vec<CropScientistBonus>,
    pub(crate) crop_scientist_multiplier: f64,
    pub(crate) breakpoints: Vec<GrowthBreakpoint>,
}

/// The Evolution & OG tab's chain and assignment for one crop target.
pub(crate) struct Evolution {
    pub(crate) chain: Option<EvolutionChain>,
    pub(crate) assignment: Assignment,
}

/// The header's controls and the field read for the tabs.
pub(crate) struct Controls {
    pub(crate) objective: ChoiceSelect<Objective>,
    pub(crate) target: Entity<InputState>,
    /// The field, and the save revision it was read for.
    field: Option<(u64, Rc<Field>)>,
    /// The chain, and the save revision and crop target it was planned for.
    evolution: Option<((u64, u32), Rc<Evolution>)>,
    _subscriptions: Vec<Subscription>,
}

/// The objectives the header offers.
pub(crate) fn objective_choices() -> Vec<Choice<Objective>> {
    Objective::ALL
        .into_iter()
        .map(|objective| Choice::new(objective, objective.label()))
        .collect()
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let objective =
            ChoiceSelect::new(objective_choices(), Some(&Objective::default()), window, cx);
        let last_crop = farming::data()
            .seeds
            .last()
            .map_or(1.0, |seed| seed.end as f64);
        let target = number_state(1.0, last_crop, 1.0, window, cx);
        // Edits flow control -> section state, through the section's setters.
        let subscriptions = vec![
            cx.subscribe(
                objective.state(),
                |state, _, event: &SelectEvent<Vec<Choice<Objective>>>, cx| {
                    if let SelectEvent::Confirm(Some(objective)) = event {
                        state.set_farming_objective(*objective, cx);
                    }
                },
            ),
            cx.subscribe(&target, |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(crop) = number_value(field.read(cx))
                    && crop.round() as u32 != state.farming.target_crop
                {
                    state.set_target_crop(crop, cx);
                }
            }),
        ];
        Self {
            objective,
            target,
            field: None,
            evolution: None,
            _subscriptions: subscriptions,
        }
    }
}

impl AppState {
    /// The field the tabs read, read from the farm once per save.
    pub(crate) fn farming_field(&mut self) -> Rc<Field> {
        let revision = self.save_revision;
        let farm = &self.farm;
        match &self.farming_controls.field {
            Some((at, field)) if *at == revision => field.clone(),
            _ => {
                let snapshot = farm.snapshot();
                let scientist = farm.crop_scientist();
                let field = Rc::new(Field {
                    plots: (0..farming::PLOTS)
                        .map(|index| farm.plot_metrics_with(index, &snapshot))
                        .collect(),
                    curves: farm.overgrowth_curves(&snapshot),
                    diagnostics: farm.diagnostics(&snapshot),
                    crop_scientist: scientist.rows,
                    crop_scientist_multiplier: scientist.multiplier,
                    breakpoints: farm.growth_breakpoints(&snapshot),
                    snapshot,
                });
                self.farming_controls.field = Some((revision, field.clone()));
                field
            }
        }
    }

    /// The chain to the crop target and its evolution assignment, planned
    /// once per save and target.
    pub(crate) fn farming_evolution(&mut self) -> Rc<Evolution> {
        let key = (self.save_revision, self.farming.target_crop);
        if let Some((at, evolution)) = &self.farming_controls.evolution
            && *at == key
        {
            return evolution.clone();
        }
        let field = self.farming_field();
        let target = self.farming.target_crop;
        let evolution = Rc::new(Evolution {
            chain: self
                .farm
                .evolution_chain(i64::from(target), &field.snapshot),
            assignment: self.farm.assignment(Objective::Evolution, Some(target)),
        });
        self.farming_controls.evolution = Some((key, evolution.clone()));
        evolution
    }
}
