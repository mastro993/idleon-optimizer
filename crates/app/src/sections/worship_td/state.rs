//! The live run's state, its replay and the tick pumps.
//!
//! The section keeps one [`Run`] per trial and seed. Playback advances it on a
//! 16 ms foreground loop at ten ticks per real second times the chosen speed;
//! every edit (placement, upgrade, sale, skip, restart) refreshes the cached
//! view so the HUD and the map always describe the same tick. A finished run's
//! recorder loads as a replay, which the second pump scrubs and plays.

use crate::state::{AppState, discard_run};
use gpui::Context;
use idleon_worship::{
    self as worship, Inputs, Plan, PlanIssue, PlanStage, Replay, ReplayConfig, Run, RunView,
    guide_presets_for_trial, heuristic_plan, plan_issues, plan_stages,
};
use std::time::{Duration, Instant};

/// Where a build plan comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PlanSource {
    /// The community guide preset for this trial, when one ships.
    #[default]
    Guide,
    /// The coverage heuristic over the save's unlocked tower types.
    Heuristic,
}

impl PlanSource {
    pub(crate) fn id(self) -> &'static str {
        match self {
            PlanSource::Guide => "guide",
            PlanSource::Heuristic => "heuristic",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            PlanSource::Guide => "Guide preset",
            PlanSource::Heuristic => "Coverage heuristic",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        [PlanSource::Guide, PlanSource::Heuristic]
            .into_iter()
            .find(|source| source.id() == id)
    }

    /// Whether this source has a plan for `trial`.
    pub(crate) fn available(self, trial: usize) -> bool {
        match self {
            PlanSource::Guide => guide_presets_for_trial(trial).next().is_some(),
            PlanSource::Heuristic => true,
        }
    }
}

/// The inputs a stored plan result was produced from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlanRunInputs {
    pub(crate) trial: usize,
    pub(crate) source: PlanSource,
    pub(crate) seed: u64,
    pub(crate) inputs: Inputs,
    pub(crate) plan: Plan,
}

/// What the optimizer search runs with: the mode's budget, how many runs each
/// candidate is screened on, the seed the whole search derives from, and the
/// wave a simulated run stops at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct OptimizeConfig {
    pub(crate) mode: worship::Mode,
    /// Screening runs per candidate.
    pub(crate) seeds: usize,
    /// The seed the whole search derives from; two runs with it agree.
    pub(crate) base_seed: u64,
    /// Stop a simulated run once it starts this wave.
    pub(crate) max_wave: u32,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            mode: worship::Mode::Standard,
            seeds: 2,
            base_seed: 12_345,
            max_wave: 240,
        }
    }
}

impl OptimizeConfig {
    /// The options a run started now would search with.
    pub(crate) fn options(&self, trial: usize, seed_plan: Option<Plan>) -> worship::Options {
        worship::Options {
            trial,
            mode: self.mode,
            seeds: self.seeds,
            base_seed: self.base_seed,
            max_wave: self.max_wave,
            seed_plan,
            ..worship::Options::default()
        }
    }
}

/// The inputs a stored optimizer result was produced from: the search settings
/// and the account values, not the plan the search compares against.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimizeRunInputs {
    pub(crate) trial: usize,
    pub(crate) config: OptimizeConfig,
    pub(crate) inputs: Inputs,
}

/// The default plan source for a trial: the guide when one ships, else the
/// coverage heuristic.
fn default_plan_source(trial: usize) -> PlanSource {
    if PlanSource::Guide.available(trial) {
        PlanSource::Guide
    } else {
        PlanSource::Heuristic
    }
}

/// How often the play loop wakes; the run's own tick is 100 ms.
const FRAME: Duration = Duration::from_millis(16);

/// The section's config and its live run.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) trial: usize,
    /// The simulator, Plan or Model pane the section shows.
    pub(crate) tab: super::Tab,
    /// Where the build plan comes from.
    pub(crate) plan_source: PlanSource,
    /// Whether the plan on screen is the optimizer's recommendation.
    pub(crate) plan_from_optimizer: bool,
    /// What the strategy optimizer searches with.
    pub(crate) optimize: OptimizeConfig,
    /// The palette's selected tower type.
    pub(crate) tower: usize,
    pub(crate) ranges: bool,
    pub(crate) speed: f64,
    /// The current plan, regenerated when the trial or source changes.
    #[serde(skip)]
    pub(crate) plan: Option<Plan>,
    /// The plan's staged layout, cached with the plan.
    #[serde(skip)]
    pub(crate) plan_stages: Vec<PlanStage>,
    /// The plan's missing inputs and refused actions, cached with the plan.
    #[serde(skip)]
    pub(crate) plan_issues: Vec<PlanIssue>,
    #[serde(skip)]
    pub(crate) run: Option<Run>,
    #[serde(skip)]
    pub(crate) view: Option<RunView>,
    #[serde(skip)]
    pub(crate) replay: Option<Replay>,
    /// Whether the map shows the recorded replay instead of the live run.
    #[serde(skip)]
    pub(crate) replay_mode: bool,
    /// The replay's position in frames; fractional while playing.
    #[serde(skip)]
    pub(crate) replay_pos: f64,
    #[serde(skip)]
    pub(crate) replay_playing: bool,
    #[serde(skip)]
    pub(crate) replay_speed: f64,
    #[serde(skip)]
    replay_generation: u64,
    #[serde(skip)]
    replay_pumping: bool,
    #[serde(skip)]
    replay_frame_at: Option<Instant>,
    #[serde(skip)]
    pub(crate) playing: bool,
    #[serde(skip)]
    pub(crate) generation: u64,
    #[serde(skip)]
    pub(crate) seed: u64,
    #[serde(skip)]
    pub(crate) selected: Option<u64>,
    #[serde(skip)]
    pub(crate) message: Option<String>,
    #[serde(skip)]
    pub(crate) pumping: bool,
    #[serde(skip)]
    frame_at: Option<Instant>,
    #[serde(skip)]
    accumulator: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            trial: 0,
            tab: super::Tab::Simulator,
            plan_source: PlanSource::Guide,
            plan_from_optimizer: false,
            optimize: OptimizeConfig::default(),
            plan: None,
            plan_stages: Vec::new(),
            plan_issues: Vec::new(),
            tower: 0,
            ranges: true,
            speed: 1.0,
            run: None,
            view: None,
            replay: None,
            replay_mode: false,
            replay_pos: 0.0,
            replay_playing: false,
            replay_speed: 1.0,
            replay_generation: 0,
            replay_pumping: false,
            replay_frame_at: None,
            playing: false,
            generation: 0,
            seed: 1,
            selected: None,
            message: None,
            pumping: false,
            frame_at: None,
            accumulator: 0.0,
        }
    }
}

impl State {
    /// The replay frame the scrubber shows.
    pub(crate) fn replay_index(&self) -> usize {
        let Some(replay) = &self.replay else {
            return 0;
        };
        let last = replay.len().saturating_sub(1) as f64;
        self.replay_pos.clamp(0.0, last) as usize
    }

    /// Whether the map currently shows recorded frames.
    pub(crate) fn replaying(&self) -> bool {
        self.replay_mode && self.replay.is_some()
    }

    /// The view the map and HUD render, live or replayed.
    pub(crate) fn display_view(&self) -> RunView {
        if self.replaying()
            && let Some(view) = self
                .replay
                .as_ref()
                .and_then(|replay| replay.view(self.replay_index()))
        {
            return view;
        }
        self.view.clone().unwrap_or_else(RunView::empty)
    }
}

impl AppState {
    /// Build the run the section renders, if it has not started one yet.
    pub(crate) fn worship_ensure(&mut self) {
        if self.worship.run.is_some() {
            return;
        }
        let trial = self
            .worship
            .trial
            .min(worship::data().trials.len().saturating_sub(1));
        let run = Run::new_with_replay(
            self.worship_inputs.clone(),
            trial,
            self.worship.seed,
            ReplayConfig::default(),
        );
        let view = run.view();
        self.worship.view = Some(view);
        self.worship.run = Some(run);
    }

    /// Start the play loop when the section renders a playing run.
    pub(crate) fn worship_pump(&mut self, cx: &mut Context<Self>) {
        if !self.worship.playing || self.worship.pumping {
            return;
        }
        self.worship.pumping = true;
        self.worship.frame_at = None;
        let generation = self.worship.generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FRAME).await;
                let keep = this
                    .update(cx, |state, cx| state.worship_frame(generation, cx))
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
        })
        .detach();
    }

    /// One frame of the play loop; false once it should stop.
    fn worship_frame(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        if generation != self.worship.generation || !self.worship.playing {
            self.worship.pumping = false;
            return false;
        }
        let now = Instant::now();
        let elapsed = self
            .worship
            .frame_at
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0)
            .min(0.25);
        self.worship.accumulator += elapsed * worship::TICKS_PER_SECOND * self.worship.speed;
        let ticks = self.worship.accumulator.floor();
        self.worship.accumulator -= ticks;
        if ticks >= 1.0 {
            self.worship_advance(ticks as u32, cx);
        } else {
            cx.notify();
        }
        if self.worship.playing {
            true
        } else {
            self.worship.pumping = false;
            false
        }
    }

    fn worship_advance(&mut self, ticks: u32, cx: &mut Context<Self>) {
        self.worship_ensure();
        let mut recorded = None;
        if let Some(run) = self.worship.run.as_mut() {
            run.step(ticks);
            self.worship.view = Some(run.view());
            if run.done() {
                self.worship.playing = false;
                self.worship.pumping = false;
                self.worship.generation += 1;
                run.result();
                recorded = run.replay();
            }
        }
        if let Some(replay) = recorded {
            self.worship.replay_pos = replay.len().saturating_sub(1) as f64;
            self.worship.replay = Some(replay);
            self.worship.replay_mode = true;
            self.worship.replay_playing = false;
            self.worship.replay_pumping = false;
            self.worship.replay_generation += 1;
        }
        cx.notify();
    }

    /// Play or pause the run.
    pub(crate) fn worship_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        if playing && self.worship.run.as_ref().is_none_or(Run::done) {
            return;
        }
        self.worship.replay_mode = false;
        self.worship.playing = playing;
        self.worship.frame_at = None;
        self.worship.accumulator = 0.0;
        self.worship.generation += 1;
        self.worship.pumping = false;
        if playing {
            self.worship_pump(cx);
        }
        cx.notify();
    }

    /// Restart the trial from tick 0 with the same seed.
    pub(crate) fn worship_restart(&mut self, cx: &mut Context<Self>) {
        self.worship.generation += 1;
        self.worship.playing = false;
        self.worship.pumping = false;
        self.worship.run = None;
        self.worship.view = None;
        self.worship.selected = None;
        self.worship.message = None;
        self.worship.frame_at = None;
        self.worship.accumulator = 0.0;
        self.worship_clear_replay();
        self.worship_ensure();
        cx.notify();
    }

    /// Build the plan the Plan pane renders, if the section has none yet.
    pub(crate) fn worship_ensure_plan(&mut self) {
        if self.worship.plan.is_some() {
            return;
        }
        let trial = self
            .worship
            .trial
            .min(worship::data().trials.len().saturating_sub(1));
        if !self.worship.plan_source.available(trial) {
            self.worship.plan_source = PlanSource::Heuristic;
        }
        let trial_data = worship::trial(trial).expect("the section's trial exists");
        let plan = match self.worship.plan_source {
            PlanSource::Guide => guide_presets_for_trial(trial)
                .next()
                .map(|preset| preset.plan()),
            PlanSource::Heuristic => None,
        }
        .unwrap_or_else(|| heuristic_plan(&self.worship_inputs, trial_data));
        self.worship.plan_stages = plan_stages(trial_data, &plan);
        self.worship.plan_issues = plan_issues(&self.worship_inputs, trial_data, &plan);
        self.worship.plan_from_optimizer = false;
        self.worship.plan = Some(plan);
    }

    /// Switch the build plan's source and drop the previous plan run.
    pub(crate) fn worship_set_plan_source(&mut self, source: PlanSource, cx: &mut Context<Self>) {
        let trial = self.worship.trial;
        if !source.available(trial) {
            return;
        }
        self.worship.plan_source = source;
        self.worship.plan_from_optimizer = false;
        self.worship.plan = None;
        self.worship_ensure_plan();
        discard_run(&self.worship_plan, cx);
        cx.notify();
    }

    /// Adopt a save's inputs and restart so its tower levels apply.
    pub(crate) fn worship_adopt(&mut self, inputs: Inputs) {
        self.worship_inputs = inputs;
        self.worship.plan = None;
        self.worship.plan_from_optimizer = false;
        self.worship.plan_source = default_plan_source(self.worship.trial);
        self.worship.generation += 1;
        self.worship.playing = false;
        self.worship.pumping = false;
        self.worship.run = None;
        self.worship.view = None;
        self.worship.selected = None;
        self.worship.message = None;
        self.worship.frame_at = None;
        self.worship.accumulator = 0.0;
        self.worship_clear_replay();
    }

    /// Drop the recorded replay and leave replay mode.
    fn worship_clear_replay(&mut self) {
        self.worship.replay = None;
        self.worship.replay_mode = false;
        self.worship.replay_pos = 0.0;
        self.worship.replay_playing = false;
        self.worship.replay_pumping = false;
        self.worship.replay_frame_at = None;
        self.worship.replay_generation += 1;
    }

    /// Switch trials, restarting the run.
    pub(crate) fn worship_set_trial(&mut self, trial: usize, cx: &mut Context<Self>) {
        let last = worship::data().trials.len().saturating_sub(1);
        self.worship.trial = trial.min(last);
        self.worship.plan = None;
        self.worship.plan_from_optimizer = false;
        self.worship.plan_source = default_plan_source(self.worship.trial);
        discard_run(&self.worship_plan, cx);
        self.worship_restart(cx);
    }

    /// Switch the section's pane.
    pub(crate) fn worship_set_tab(&mut self, tab: super::Tab, cx: &mut Context<Self>) {
        self.worship.tab = tab;
        if tab != super::Tab::Simulator {
            self.worship.playing = false;
            self.worship.pumping = false;
        }
        cx.notify();
    }

    /// Select a tower type from the palette.
    pub(crate) fn worship_set_tower(&mut self, tower: usize, cx: &mut Context<Self>) {
        self.worship.tower = tower.min(worship::TOWER_COUNT - 1);
        self.worship.message = None;
        cx.notify();
    }

    /// Set the playback speed.
    pub(crate) fn worship_set_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        self.worship.speed = speed;
        cx.notify();
    }

    /// Set the optimizer's search budget.
    pub(crate) fn worship_set_optimize_mode(
        &mut self,
        mode: worship::Mode,
        cx: &mut Context<Self>,
    ) {
        self.worship.optimize.mode = mode;
        cx.notify();
    }

    /// Set how many runs screen each candidate.
    pub(crate) fn worship_set_optimize_seeds(&mut self, seeds: f64, cx: &mut Context<Self>) {
        self.worship.optimize.seeds = (seeds as usize).clamp(1, 6);
        cx.notify();
    }

    /// Set the seed the whole search derives from.
    pub(crate) fn worship_set_optimize_seed(&mut self, seed: f64, cx: &mut Context<Self>) {
        self.worship.optimize.base_seed = (seed.max(0.0) as u64).min(999_999);
        cx.notify();
    }

    /// Set the wave a simulated run stops at.
    pub(crate) fn worship_set_optimize_wave(&mut self, wave: f64, cx: &mut Context<Self>) {
        self.worship.optimize.max_wave = (wave.max(0.0) as u32).clamp(10, 1000);
        cx.notify();
    }

    /// Place the selected tower on a platform slot.
    pub(crate) fn worship_place(&mut self, node: usize, x: f64, cx: &mut Context<Self>) {
        self.worship_ensure();
        let tower = self.worship.tower;
        let outcome = self
            .worship
            .run
            .as_mut()
            .map(|run| run.place(tower, node, x));
        if let Some(run) = self.worship.run.as_ref() {
            self.worship.view = Some(run.view());
        }
        match outcome {
            Some(Ok(placed)) => {
                self.worship.selected = Some(placed.id);
                self.worship.message = None;
            }
            Some(Err(error)) => self.worship.message = Some(error.label()),
            None => {}
        }
        cx.notify();
    }

    /// Select a standing tower, or clear the selection.
    pub(crate) fn worship_select(&mut self, id: Option<u64>, cx: &mut Context<Self>) {
        self.worship.selected = id;
        self.worship.message = None;
        cx.notify();
    }

    /// Upgrade the selected tower.
    pub(crate) fn worship_upgrade(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.worship.selected else {
            return;
        };
        let outcome = self.worship.run.as_mut().map(|run| run.upgrade(id));
        if let Some(run) = self.worship.run.as_ref() {
            self.worship.view = Some(run.view());
        }
        self.worship.message = match outcome {
            Some(Ok(_)) => None,
            Some(Err(error)) => Some(error.label()),
            None => None,
        };
        cx.notify();
    }

    /// Sell the selected tower.
    pub(crate) fn worship_sell(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.worship.selected else {
            return;
        };
        let outcome = self.worship.run.as_mut().map(|run| run.sell(id));
        if let Some(run) = self.worship.run.as_ref() {
            self.worship.view = Some(run.view());
        }
        match outcome {
            Some(Ok(refund)) => {
                self.worship.selected = None;
                self.worship.message = Some(format!("Sold for {refund:.0} points"));
            }
            Some(Err(error)) => self.worship.message = Some(error.label()),
            None => {}
        }
        cx.notify();
    }

    /// Skip the rest of the countdown.
    pub(crate) fn worship_skip(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = self.worship.run.as_mut() {
            let reward = run.skip_wave();
            self.worship.view = Some(run.view());
            if reward > 0.0 {
                self.worship.message = Some(format!("Skipped for {reward:.0} points"));
            }
        }
        cx.notify();
    }

    /// Advance one 100 ms tick while paused.
    pub(crate) fn worship_step(&mut self, cx: &mut Context<Self>) {
        if self.worship.playing {
            return;
        }
        self.worship_advance(1, cx);
    }

    /// Start the replay loop when a replay is playing.
    pub(crate) fn worship_replay_pump(&mut self, cx: &mut Context<Self>) {
        if !self.worship.replay_playing || self.worship.replay_pumping {
            return;
        }
        self.worship.replay_pumping = true;
        self.worship.replay_frame_at = None;
        let generation = self.worship.replay_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FRAME).await;
                let keep = this
                    .update(cx, |state, cx| state.worship_replay_frame(generation, cx))
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
        })
        .detach();
    }

    /// One frame of the replay loop; false once it should stop.
    fn worship_replay_frame(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        if generation != self.worship.replay_generation || !self.worship.replay_playing {
            self.worship.replay_pumping = false;
            return false;
        }
        let Some(replay) = self.worship.replay.as_ref() else {
            self.worship.replay_playing = false;
            self.worship.replay_pumping = false;
            return false;
        };
        let interval = replay.interval_sec().max(0.1);
        let last = replay.len().saturating_sub(1) as f64;
        let now = Instant::now();
        let elapsed = self
            .worship
            .replay_frame_at
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0)
            .min(0.25);
        self.worship.replay_pos += elapsed * self.worship.replay_speed / interval;
        if self.worship.replay_pos >= last {
            self.worship.replay_pos = last;
            self.worship.replay_playing = false;
            self.worship.replay_pumping = false;
        }
        cx.notify();
        self.worship.replay_playing
    }

    /// Play or pause the recorded replay, leaving the live view.
    pub(crate) fn worship_replay_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        if self.worship.replay.is_none() {
            return;
        }
        self.worship.playing = false;
        self.worship.pumping = false;
        self.worship.generation += 1;
        self.worship.replay_mode = true;
        self.worship.replay_playing = playing;
        self.worship.replay_generation += 1;
        self.worship.replay_pumping = false;
        self.worship.replay_frame_at = None;
        if playing {
            let last = self
                .worship
                .replay
                .as_ref()
                .map(|replay| replay.len().saturating_sub(1) as f64)
                .unwrap_or(0.0);
            if self.worship.replay_pos >= last {
                self.worship.replay_pos = 0.0;
            }
            self.worship_replay_pump(cx);
        }
        cx.notify();
    }

    /// Move the replay scrubber to a frame index.
    pub(crate) fn worship_replay_seek(&mut self, frame: f64, cx: &mut Context<Self>) {
        if self.worship.replay.is_none() {
            return;
        }
        let last = self
            .worship
            .replay
            .as_ref()
            .map(|replay| replay.len().saturating_sub(1) as f64)
            .unwrap_or(0.0);
        self.worship.replay_pos = frame.clamp(0.0, last);
        self.worship.replay_mode = true;
        cx.notify();
    }

    /// Set how many recorded frames play per real second.
    pub(crate) fn worship_replay_set_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        if speed > 0.0 {
            self.worship.replay_speed = speed;
        }
        cx.notify();
    }

    /// Open the replay without playing it.
    pub(crate) fn worship_replay_open(&mut self, cx: &mut Context<Self>) {
        if self.worship.replay.is_none() {
            return;
        }
        self.worship.playing = false;
        self.worship.pumping = false;
        self.worship.generation += 1;
        self.worship.replay_mode = true;
        cx.notify();
    }

    /// Return to the live run's field.
    pub(crate) fn worship_replay_close(&mut self, cx: &mut Context<Self>) {
        self.worship.replay_playing = false;
        self.worship.replay_pumping = false;
        self.worship.replay_generation += 1;
        self.worship.replay_frame_at = None;
        self.worship.replay_mode = false;
        cx.notify();
    }
}
