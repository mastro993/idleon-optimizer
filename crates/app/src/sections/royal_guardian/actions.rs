//! What the window does when the Royal Guardian section is edited or run.

use super::forecast;
use super::upgrades::View as UpgradeView;
use crate::sections::royal_guardian::Tab as RoyalTab;
use crate::state::AppState;
use gpui::Context;
use idleon_royal_guardian::{
    Category, HORIZON, Objective, Request, ShelfPlan, ShelfRequest, Target, UnitExpPlan,
    UnitExpRequest,
};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The pane the Royal Guardian strip shows.
    pub(crate) fn set_royal_tab(&mut self, tab: RoyalTab, cx: &mut Context<Self>) {
        self.royal_state.tab = tab;
        if tab == RoyalTab::Forecast {
            self.ensure_royal_forecast();
        }
        cx.notify();
    }

    /// The world the Royal Guardian tables list.
    pub(crate) fn set_royal_world(&mut self, world: usize, cx: &mut Context<Self>) {
        self.royal_state.world = world.min(idleon_royal_guardian::WORLDS - 1);
        self.royal_state.outpost = None;
        self.royal_state.resource = None;
        cx.notify();
    }

    /// The outpost whose next range target the panel explains.
    pub(crate) fn set_royal_outpost(&mut self, map: usize, cx: &mut Context<Self>) {
        self.royal_state.outpost = Some(map);
        cx.notify();
    }

    /// The resource node the resources pane explains.
    pub(crate) fn set_royal_resource(&mut self, resource: usize, cx: &mut Context<Self>) {
        self.royal_state.resource = Some(resource);
        cx.notify();
    }

    /// The currency slot the currencies pane explains.
    pub(crate) fn set_royal_currency(&mut self, slot: i64, cx: &mut Context<Self>) {
        self.royal_state.currency = Some(slot);
        cx.notify();
    }

    /// The upgrade list the Upgrades pane shows.
    pub(crate) fn set_royal_upgrade_view(&mut self, view: UpgradeView, cx: &mut Context<Self>) {
        self.royal_state.upgrade_view = view;
        cx.notify();
    }

    /// The shelf whose cost plan the Upgrades pane prices.
    pub(crate) fn set_royal_armory_plan(&mut self, order: Option<usize>, cx: &mut Context<Self>) {
        self.royal_state.armory_plan = order;
        cx.notify();
    }

    /// The global rule the Breakdowns pane expands.
    pub(crate) fn set_royal_breakdown(&mut self, category: Category, cx: &mut Context<Self>) {
        self.royal_state.breakdown = category;
        cx.notify();
    }

    /// The forecast horizon, in hours.
    pub(crate) fn set_royal_forecast_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        let hours = hours.clamp(HORIZON.0, HORIZON.1);
        self.royal_state.forecast_hours = hours;
        // A shorter horizon drops the collects and windows past its end, so the
        // agenda never lists an event the engine has filtered out.
        self.royal_state
            .collects
            .retain(|collect| *collect <= hours);
        self.royal_state
            .windows
            .retain(|(start, _)| *start <= hours);
        self.ensure_royal_forecast();
        cx.notify();
    }

    /// Adds the collect hour the field holds.
    pub(crate) fn add_royal_collect(&mut self, cx: &mut Context<Self>) {
        let text = forecast::text(&self.royal_forecast_inputs.collect, cx);
        if let Some(hours) = forecast::hours(&text)
            && hours <= self.royal_state.forecast_hours
        {
            self.royal_state.collects.push(hours);
            self.royal_state.collects.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.ensure_royal_forecast();
            cx.notify();
        }
    }

    /// Removes the collect at one position.
    pub(crate) fn remove_royal_collect(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.royal_state.collects.len() {
            self.royal_state.collects.remove(index);
            self.ensure_royal_forecast();
            cx.notify();
        }
    }

    /// Adds the menu-open window the two fields hold.
    pub(crate) fn add_royal_window(&mut self, cx: &mut Context<Self>) {
        let start = forecast::hours(&forecast::text(
            &self.royal_forecast_inputs.window_start,
            cx,
        ));
        let end = forecast::hours(&forecast::text(&self.royal_forecast_inputs.window_end, cx));
        let (Some(start), Some(end)) = (start, end) else {
            return;
        };
        if start >= end || start > self.royal_state.forecast_hours {
            return;
        }
        self.royal_state.windows.push((start, end));
        self.royal_state.windows.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.ensure_royal_forecast();
        cx.notify();
    }

    /// Removes the menu-open window at one position.
    pub(crate) fn remove_royal_window(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.royal_state.windows.len() {
            self.royal_state.windows.remove(index);
            self.ensure_royal_forecast();
            cx.notify();
        }
    }

    /// Recomputes the forecast when its inputs moved, and keeps the last one
    /// otherwise.
    pub(crate) fn ensure_royal_forecast(&mut self) {
        let Some(royal) = self.royal.as_ref() else {
            self.royal_forecast = None;
            self.royal_forecast_key = None;
            return;
        };
        let options = self.royal_state.forecast_options();
        if self.royal_forecast_key.as_ref() == Some(&options) && self.royal_forecast.is_some() {
            return;
        }
        self.royal_forecast = Some(royal.forecast(&options));
        self.royal_forecast_key = Some(options);
    }
}

impl AppState {
    /// The outpost optimizer's stored result, against the inputs it now shows.
    pub(crate) fn royal_optimizer_task(
        &self,
        cx: &gpui::App,
    ) -> TaskState<idleon_royal_guardian::Outcome> {
        let input = self.royal_optimizer_request();
        self.royal_optimizer.read(cx).state(&input)
    }

    /// The objective the outpost optimizer maximizes.
    pub(crate) fn set_royal_optimizer_objective(
        &mut self,
        objective: Objective,
        cx: &mut Context<Self>,
    ) {
        self.royal_state.optimizer = objective;
        cx.notify();
    }

    /// The request the optimizer runs with: the loaded save and its objective.
    fn royal_optimizer_request(&self) -> Request {
        let royal = self
            .royal
            .as_ref()
            .expect("the optimizer pane renders only with a save");
        Request::new(royal, self.royal_state.optimizer)
    }

    /// Search the outpost layouts off the UI thread through the shared runner.
    pub(crate) fn run_royal_optimizer(&mut self, cx: &mut Context<Self>) {
        if self.royal.is_none() {
            return;
        }
        let request = self.royal_optimizer_request();
        self.start_run(
            self.royal_optimizer.clone(),
            "Optimizing the Royal Guardian kingdom…",
            request,
            |request, report| request.royal.optimize(&request, report),
            cx,
        );
    }

    /// Ask a running optimizer to stop at its next layout.
    pub(crate) fn cancel_royal_optimizer(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.royal_optimizer.clone(),
            "Cancelling the Royal Guardian optimizer…",
            cx,
        );
    }

    /// The outpost the unit EXP planner plans for: the selected one, or the
    /// first built outpost of the selected world.
    pub(crate) fn royal_unit_exp_map(&self) -> Option<usize> {
        let royal = self.royal.as_ref()?;
        let world = self.royal_state.world;
        self.royal_state
            .unit_exp_map
            .filter(|map| royal.outpost(*map).built)
            .or_else(|| {
                royal
                    .world_outposts(world)
                    .into_iter()
                    .find(|outpost| outpost.built)
                    .map(|outpost| outpost.map)
            })
    }

    /// The outpost the unit EXP planner plans for.
    pub(crate) fn set_royal_unit_exp_map(&mut self, map: usize, cx: &mut Context<Self>) {
        self.royal_state.unit_exp_map = Some(map);
        cx.notify();
    }

    /// Sets one rank target; zero clears it.
    pub(crate) fn set_royal_unit_exp_target(
        &mut self,
        target: Target,
        rank: u32,
        cx: &mut Context<Self>,
    ) {
        let slot = match target {
            Target::Trading => &mut self.royal_state.unit_exp_targets.trading,
            Target::Command => &mut self.royal_state.unit_exp_targets.command,
            Target::Military => &mut self.royal_state.unit_exp_targets.military,
        };
        *slot = (rank > 0).then_some(rank);
        cx.notify();
    }

    /// Whether the unit EXP plan moves same-world assignments.
    pub(crate) fn set_royal_unit_exp_move_transients(
        &mut self,
        move_transients: bool,
        cx: &mut Context<Self>,
    ) {
        self.royal_state.unit_exp_move_transients = move_transients;
        cx.notify();
    }

    /// The request the unit EXP planner runs with.
    fn royal_unit_exp_request(&self) -> Option<UnitExpRequest> {
        Some(UnitExpRequest {
            royal: self.royal.clone()?,
            map: self.royal_unit_exp_map()?,
            targets: self.royal_state.unit_exp_targets,
            move_transients: self.royal_state.unit_exp_move_transients,
        })
    }

    /// The unit EXP plan's stored result, against the inputs it now shows.
    pub(crate) fn royal_unit_exp_task(&self, cx: &gpui::App) -> Option<TaskState<UnitExpPlan>> {
        let input = self.royal_unit_exp_request()?;
        Some(self.royal_unit_exp.read(cx).state(&input))
    }

    /// Plan the unit EXP split off the UI thread through the shared runner.
    pub(crate) fn run_royal_unit_exp(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.royal_unit_exp_request() else {
            self.set_status("Unit EXP: select a built outpost first.", cx);
            return;
        };
        if !request.targets.any() {
            self.set_status("Unit EXP: set at least one target rank.", cx);
            return;
        }
        let unlocked = request.royal.unlocked_bars();
        if let Some(target) = Target::ALL
            .into_iter()
            .find(|target| !unlocked[target.bar()] && request.targets.get(*target).is_some())
        {
            self.set_status(
                format!("Unit EXP: {} is locked by the Armory.", target.label()),
                cx,
            );
            return;
        }
        self.start_run(
            self.royal_unit_exp.clone(),
            "Planning the Royal Guardian unit EXP…",
            request,
            |request, report| request.royal.plan_unit_exp(&request, report).ok(),
            cx,
        );
    }

    /// Ask a running unit EXP plan to stop at its next candidate.
    pub(crate) fn cancel_royal_unit_exp(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.royal_unit_exp.clone(),
            "Cancelling the Royal Guardian unit EXP plan…",
            cx,
        );
    }

    /// The request the shelf planner runs with.
    fn royal_shelf_request(&self) -> Option<ShelfRequest> {
        Some(ShelfRequest::new(
            self.royal.as_ref()?,
            self.royal_state.shelf_purchases,
        ))
    }

    /// The shelf plan's stored result, against the inputs it now shows.
    pub(crate) fn royal_shelf_task(&self, cx: &gpui::App) -> Option<TaskState<ShelfPlan>> {
        let input = self.royal_shelf_request()?;
        Some(self.royal_shelf.read(cx).state(&input))
    }

    /// How many levels the shelf plan may buy.
    pub(crate) fn set_royal_shelf_purchases(&mut self, purchases: usize, cx: &mut Context<Self>) {
        self.royal_state.shelf_purchases = purchases.clamp(1, 50);
        cx.notify();
    }

    /// Plan the steps toward the next Armory shelf off the UI thread.
    pub(crate) fn run_royal_shelf(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.royal_shelf_request() else {
            return;
        };
        self.start_run(
            self.royal_shelf.clone(),
            "Planning the next Royal Guardian shelf…",
            request,
            |request, report| request.royal.plan_shelf(&request, report),
            cx,
        );
    }

    /// Ask a running shelf plan to stop at its next step.
    pub(crate) fn cancel_royal_shelf(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.royal_shelf.clone(),
            "Cancelling the Royal Guardian shelf plan…",
            cx,
        );
    }
}
