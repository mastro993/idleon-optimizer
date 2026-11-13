//! What the window does when the Royal Guardian section is edited or run.

use super::forecast;
use super::upgrades::View as UpgradeView;
use crate::sections::royal_guardian::Tab as RoyalTab;
use crate::state::AppState;
use gpui::Context;
use idleon_royal_guardian::{Category, HORIZON};

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
