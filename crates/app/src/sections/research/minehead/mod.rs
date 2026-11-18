//! The Minehead tab: the Depth Charge dashboard, strategy search, upgrade
//! ranking, affordable path, currency breakdown and the playable floor.
//!
//! The formulas and the simulation live in `idleon-minehead`; this module owns
//! the tab state the window keeps and renders the subtabs.

mod currency;
mod dashboard;
mod optimize;
mod path;
mod play;
mod rank;

use play::Play;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Styled, Subscription, Window, div,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use idleon_ui::widgets::{number_state, number_value};
use idleon_minehead::{Grid, Inputs, Params, Recorded, infer, inputs};
use idleon_research::model::Simulation;
use idleon_save::Save;

/// The inputs one strategy search reads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimizeInputs {
    pub(crate) options: idleon_minehead::Options,
    pub(crate) grid: Grid,
    pub(crate) screen_trials: u32,
    pub(crate) final_trials: u32,
    pub(crate) top_n: usize,
    pub(crate) seed: u32,
}

/// The inputs one ranking or upgrade-path run reads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) options: idleon_minehead::Options,
    pub(crate) params: Params,
    pub(crate) path: idleon_minehead::PathInputs,
    pub(crate) trials: u32,
    pub(crate) seed: u32,
}

/// The tab's own subtabs, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    #[default]
    Dashboard,
    Optimize,
    Rank,
    Path,
    Currency,
    Play,
}

impl Tab {
    pub(crate) const ALL: [Tab; 6] = [
        Tab::Dashboard,
        Tab::Optimize,
        Tab::Rank,
        Tab::Path,
        Tab::Currency,
        Tab::Play,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Dashboard => "mh-dashboard",
            Tab::Optimize => "mh-optimize",
            Tab::Rank => "mh-rank",
            Tab::Path => "mh-path",
            Tab::Currency => "mh-currency",
            Tab::Play => "mh-play",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Optimize => "Optimize",
            Tab::Rank => "Rank Upgrades",
            Tab::Path => "Upgrade Path",
            Tab::Currency => "Currency",
            Tab::Play => "Play",
        }
    }
}

/// One optimizer knob's search range, as the dual sliders set it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KnobRange {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    /// The values the slider may reach.
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) step: f64,
    /// The ends of the current search range.
    pub(crate) lo: f64,
    pub(crate) hi: f64,
}

impl KnobRange {
    /// How many decimals the knob's field shows.
    fn decimals(&self) -> usize {
        if self.step < 0.1 {
            2
        } else if self.step < 1.0 {
            1
        } else {
            0
        }
    }

    /// How many decimals the slider's values carry.
    fn precision(&self) -> f64 {
        if self.step < 0.1 {
            100.0
        } else if self.step < 1.0 {
            10.0
        } else {
            1.0
        }
    }

    /// The values this knob contributes to the search.
    fn values(&self) -> Vec<f64> {
        let precision = self.precision();
        let mut values = Vec::new();
        let mut value = self.lo;
        while value <= self.hi + self.step * 0.01 {
            let rounded = (value * precision).round() / precision;
            if rounded >= self.min && rounded <= self.max {
                values.push(rounded);
            }
            value += self.step;
        }
        if values.is_empty() {
            values.push(self.lo);
        }
        values
    }
}

/// The knob rows the optimizer searches, with their default ranges.
fn default_ranges() -> Vec<KnobRange> {
    let row = |key, label, min, max, step, lo, hi| KnobRange {
        key,
        label,
        min,
        max,
        step,
        lo,
        hi,
    };
    vec![
        row("ev_multiplier", "EV Multiplier", 0.4, 3.0, 0.1, 0.8, 2.0),
        row("min_reveal", "Min Reveals", 0.0, 8.0, 1.0, 0.0, 3.0),
        row("mine_cap_pct", "Mine Cap %", 0.1, 1.0, 0.1, 0.5, 1.0),
        row("golden_mine_pct", "Golden Trigger", 0.0, 1.0, 0.1, 0.3, 1.0),
        row("hp_threshold", "HP Threshold", 0.0, 1.0, 0.1, 0.8, 1.0),
        row("block_aggro", "Block Aggro", 0.0, 1.0, 1.0, 1.0, 1.0),
        row("commit_min", "Commit Min %", 0.0, 0.1, 0.01, 0.0, 0.02),
        row("life_aggro", "Life Aggro", 0.2, 1.0, 0.1, 0.6, 1.0),
        row("turn1_ev_mul", "Turn 1 EV", 0.0, 2.0, 0.1, 0.0, 0.7),
        row("crown_chase", "Crown Chase", 0.0, 0.8, 0.1, 0.0, 0.4),
        row("insta_mode", "Insta Mode", 0.0, 0.6, 0.1, 0.0, 0.3),
        row("commit_turns", "Commit Turns", 0.0, 10.0, 1.0, 0.0, 0.0),
        row(
            "post_mine_panic",
            "Post-Mine Panic",
            0.0,
            0.8,
            0.1,
            0.0,
            0.0,
        ),
        row(
            "last_turn_aggro",
            "Last Turn Aggro",
            0.0,
            0.5,
            0.1,
            0.0,
            0.0,
        ),
        row("target_reveals", "Target Reveals", 0.0, 10.0, 1.0, 0.0, 0.0),
        row("golden_first", "Golden First", 0.0, 1.0, 1.0, 0.0, 0.0),
        row("crown_or_die", "Crown Or Die", 0.0, 0.6, 0.1, 0.0, 0.0),
        row("hot_streak", "Hot Streak", 0.0, 0.5, 0.1, 0.0, 0.0),
        row("block_hoard", "Block Hoard", 0.0, 0.6, 0.1, 0.0, 0.0),
        row("corner_bias", "Corner Bias", 0.5, 3.0, 0.5, 1.0, 1.0),
        row("edge_bias", "Edge Bias", 0.5, 3.0, 0.5, 1.0, 1.0),
        row("cluster_bias", "Cluster Bias", 0.5, 4.0, 0.5, 1.0, 1.0),
    ]
}

/// The Minehead tab's state, kept while the window is open.
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The save-derived inputs, or `None` before a save loads.
    pub(crate) inputs: Option<Inputs>,
    pub(crate) warnings: Vec<String>,
    pub(crate) ranges: Vec<KnobRange>,
    pub(crate) play: Option<Play>,
    pub(crate) decisions: Vec<Recorded>,
    pub(crate) games: u32,
    pub(crate) inferred: Option<infer::Inferred>,
    /// Whether upgrades unlock at every floor, as the dashboard's spoiler toggle.
    pub(crate) show_spoiler: bool,
    /// Whether the Rank and Path runs use the inferred Play strategy.
    pub(crate) use_inferred: bool,
    /// The dashboard's figures and upgrade rows, built from `inputs` the
    /// first time the dashboard shows and dropped whenever `inputs` change.
    dashboard: Option<dashboard::Dashboard>,
    /// The currency/hr the breakdown tree shows; `None` until the tree is
    /// built from the current `inputs`.
    currency_total: Option<f64>,
    /// The dashboard's A_MineCost field.
    server_cost: Entity<InputState>,
    /// Each knob's low and high fields, in `ranges` order.
    knob_fields: Vec<[Entity<InputState>; 2]>,
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let ranges = default_ranges();
        let server_cost = number_state(1.0, 20.0, 0.01, window, cx);
        let knob_fields: Vec<[Entity<InputState>; 2]> = ranges
            .iter()
            .map(|range| {
                std::array::from_fn(|_| number_state(range.min, range.max, range.step, window, cx))
            })
            .collect();
        // Edits flow field -> value through the setters, which clamp.
        let mut subscriptions = vec![cx.subscribe(
            &server_cost,
            |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(cost) = number_value(field.read(cx))
                {
                    state.research.minehead.set_server_var_cost(cost, cx);
                }
            },
        )];
        for (index, [low, high]) in knob_fields.iter().enumerate() {
            subscriptions.push(cx.subscribe(low, move |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    state.research.minehead.set_knob_low(index, value, cx);
                }
            }));
            subscriptions.push(cx.subscribe(high, move |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    state.research.minehead.set_knob_high(index, value, cx);
                }
            }));
        }
        Self {
            tab: Tab::default(),
            inputs: None,
            warnings: Vec::new(),
            ranges,
            play: None,
            decisions: Vec::new(),
            games: 0,
            inferred: None,
            show_spoiler: false,
            use_inferred: false,
            dashboard: None,
            currency_total: None,
            server_cost,
            knob_fields,
            _subscriptions: subscriptions,
        }
    }

    /// Set the A_MineCost server variable every upgrade price scales by.
    pub(crate) fn set_server_var_cost(&mut self, cost: f64, cx: &mut Context<AppState>) {
        if let Some(inputs) = self.inputs.as_mut() {
            inputs.server_var_cost = cost.clamp(1.0, 20.0);
            self.inputs_changed();
        }
        cx.notify();
    }

    /// Set the low end of knob `index`'s search range, at most its high end.
    pub(crate) fn set_knob_low(&mut self, index: usize, value: f64, cx: &mut Context<AppState>) {
        if let Some(range) = self.ranges.get_mut(index) {
            range.lo = value.clamp(range.min, range.hi);
        }
        cx.notify();
    }

    /// Set the high end of knob `index`'s search range, at least its low end.
    pub(crate) fn set_knob_high(&mut self, index: usize, value: f64, cx: &mut Context<AppState>) {
        if let Some(range) = self.ranges.get_mut(index) {
            range.hi = value.clamp(range.lo, range.max);
        }
        cx.notify();
    }

    /// Drop what the panes derived from the old `inputs`.
    fn inputs_changed(&mut self) {
        self.dashboard = None;
        self.currency_total = None;
    }

    /// Decode the tab's inputs from a newly applied save.
    pub(crate) fn load(&mut self, save: Option<&Save>, simulation: &Simulation) {
        match save {
            Some(save) => {
                let (inputs, warnings) = inputs(save, simulation);
                self.inputs = Some(inputs);
                self.warnings = warnings;
            }
            None => {
                self.inputs = None;
                self.warnings.clear();
            }
        }
        self.inputs_changed();
        // A new save draws its own floors, so the game on screen is stale.
        self.play = None;
    }

    /// The strategy Rank and Path run with.
    pub(crate) fn params(&self) -> Params {
        if self.use_inferred
            && let Some(params) = self.inferred.as_ref().and_then(|inferred| inferred.params)
        {
            return params;
        }
        Params::default()
    }

    /// The optimizer search grid the slider ranges describe.
    pub(crate) fn search_grid(&self) -> Grid {
        search_grid(&self.ranges)
    }

    /// The number of combos the current ranges expand to.
    pub(crate) fn combo_count(&self) -> usize {
        self.search_grid().combos()
    }

    /// Record one Play decision and refresh the inferred strategy.
    pub(crate) fn record(&mut self, decision: Recorded) {
        self.decisions.push(decision);
        if self.games >= 3 {
            self.inferred = Some(infer::infer(&self.decisions));
        }
    }

    /// Forget the recorded decisions and the inferred strategy.
    pub(crate) fn reset_inference(&mut self) {
        self.decisions.clear();
        self.games = 0;
        self.inferred = None;
        self.use_inferred = false;
    }

    /// One more finished Play game.
    pub(crate) fn finish_game(&mut self) {
        self.games += 1;
        if self.games >= 3 {
            self.inferred = Some(infer::infer(&self.decisions));
        }
    }
}

/// The optimizer search grid `ranges` describe, halving the widest knob
/// until the combos fit the cap.
fn search_grid(ranges: &[KnobRange]) -> Grid {
    const MAX_COMBOS: usize = 50_000;
    let mut values: Vec<Vec<f64>> = ranges.iter().map(KnobRange::values).collect();
    while values.iter().map(Vec::len).product::<usize>() > MAX_COMBOS {
        let Some((index, _)) = values
            .iter()
            .enumerate()
            .max_by_key(|(_, values)| values.len())
        else {
            break;
        };
        let old = values[index].clone();
        if old.len() <= 2 {
            break;
        }
        let new_len = old.len().div_ceil(2).max(2);
        let mut sampled = vec![old[0]];
        for position in 1..new_len - 1 {
            sampled.push(old[position * (old.len() - 1) / (new_len - 1)]);
        }
        sampled.push(old[old.len() - 1]);
        sampled.sort_by(f64::total_cmp);
        sampled.dedup_by(|left, right| (*left - *right).abs() < f64::EPSILON);
        values[index] = sampled;
    }
    Grid {
        knobs: ranges
            .iter()
            .zip(values)
            .map(|(range, values)| (range.key, values))
            .collect(),
    }
}

/// The inputs the Rank and Path runs read, from the loaded save.
pub(super) fn run_inputs(state: &AppState, max_steps: u32, trials: u32) -> Option<RunInputs> {
    let inputs = state.research.minehead.inputs.as_ref()?;
    Some(RunInputs {
        options: idleon_minehead::Options::from_inputs(inputs),
        params: state.research.minehead.params(),
        path: idleon_minehead::PathInputs {
            currency: inputs.currency,
            research_level: inputs.research_level,
            qty26: inputs.qty_levels.get(26).copied().unwrap_or(0.0),
            server_var_cost: inputs.server_var_cost,
            rog_cost_pct: inputs.rog_cost_pct(),
            max_steps,
        },
        trials,
        seed: 42 + inputs.floor as u32,
    })
}

/// The Minehead pane: the subtab strip and the active subtab.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let tab = state.research.minehead.tab;
    let mut strip = div().id("minehead-tabs").flex().items_center().gap_1();
    for candidate in Tab::ALL {
        let entity = cx.entity();
        let mut button = Button::new(candidate.id())
            .label(candidate.label())
            .small();
        if candidate == tab {
            button = button.primary();
        }
        strip = strip.child(button.on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.research.minehead.tab = candidate;
                cx.notify();
            });
        }));
    }

    div()
        .id("minehead-tab")
        .flex()
        .flex_col()
        .gap_3()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id("minehead-header")
                .flex()
                .items_center()
                .gap_3()
                .child(strip),
        )
        .child(match tab {
            Tab::Dashboard => dashboard::pane(state, window, cx),
            Tab::Optimize => optimize::pane(state, window, cx),
            Tab::Rank => rank::pane(state, cx),
            Tab::Path => path::pane(state, cx),
            Tab::Currency => currency::pane(state, cx),
            Tab::Play => play::pane(state, cx),
        })
        .into_any_element()
}

/// The floor's boss name, replacing the data's underscores.
pub(crate) fn boss_name(floor: usize) -> String {
    idleon_minehead::game().boss_name(floor).replace('_', " ")
}

/// The search ranges' combos, for a test that the halving keeps the cap.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_slider_ranges_stay_under_the_combo_cap() {
        let grid = search_grid(&default_ranges());
        assert!(grid.combos() <= 50_000, "{}", grid.combos());
        assert!(grid.combos() > 1_000, "{}", grid.combos());
        assert!(grid.knobs.iter().all(|(_, values)| !values.is_empty()));
    }

    #[test]
    fn a_narrow_range_shrinks_the_search() {
        let mut ranges = default_ranges();
        for range in &mut ranges {
            range.lo = range.hi;
        }
        assert_eq!(search_grid(&ranges).combos(), 1);
    }
}
