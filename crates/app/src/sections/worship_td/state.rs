//! The live run's state, its replay and the tick pumps.
//!
//! The section keeps one [`Run`] per trial and seed. Playback advances it on a
//! 16 ms foreground loop at ten ticks per real second times the chosen speed;
//! every edit (placement, upgrade, sale, skip, restart) refreshes the cached
//! view so the HUD and the map always describe the same tick. A finished run's
//! recorder loads as a replay, which the second pump scrubs and plays.

use crate::state::AppState;
use gpui::Context;
use idleon_core::worship::{self, Inputs, Replay, ReplayConfig, Run, RunView};
use std::time::{Duration, Instant};

/// How often the play loop wakes; the run's own tick is 100 ms.
const FRAME: Duration = Duration::from_millis(16);

/// The section's config and its live run.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub trial: usize,
    /// The palette's selected tower type.
    pub tower: usize,
    pub ranges: bool,
    pub speed: f64,
    #[serde(skip)]
    pub run: Option<Run>,
    #[serde(skip)]
    pub view: Option<RunView>,
    #[serde(skip)]
    pub replay: Option<Replay>,
    /// Whether the map shows the recorded replay instead of the live run.
    #[serde(skip)]
    pub replay_mode: bool,
    /// The replay's position in frames; fractional while playing.
    #[serde(skip)]
    pub replay_pos: f64,
    #[serde(skip)]
    pub replay_playing: bool,
    #[serde(skip)]
    pub replay_speed: f64,
    #[serde(skip)]
    replay_generation: u64,
    #[serde(skip)]
    replay_pumping: bool,
    #[serde(skip)]
    replay_frame_at: Option<Instant>,
    #[serde(skip)]
    pub playing: bool,
    #[serde(skip)]
    pub generation: u64,
    #[serde(skip)]
    pub seed: u64,
    #[serde(skip)]
    pub selected: Option<u64>,
    #[serde(skip)]
    pub message: Option<String>,
    #[serde(skip)]
    pub pumping: bool,
    #[serde(skip)]
    frame_at: Option<Instant>,
    #[serde(skip)]
    accumulator: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            trial: 0,
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
    pub fn worship_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
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
    pub fn worship_restart(&mut self, cx: &mut Context<Self>) {
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

    /// Adopt a save's inputs and restart so its tower levels apply.
    pub(crate) fn worship_adopt(&mut self, inputs: Inputs) {
        self.worship_inputs = inputs;
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
    pub fn worship_set_trial(&mut self, trial: usize, cx: &mut Context<Self>) {
        let last = worship::data().trials.len().saturating_sub(1);
        self.worship.trial = trial.min(last);
        self.worship_restart(cx);
    }

    /// Select a tower type from the palette.
    pub fn worship_set_tower(&mut self, tower: usize, cx: &mut Context<Self>) {
        self.worship.tower = tower.min(worship::TOWER_COUNT - 1);
        self.worship.message = None;
        cx.notify();
    }

    /// Set the playback speed.
    pub fn worship_set_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        self.worship.speed = speed;
        cx.notify();
    }

    /// Place the selected tower on a platform slot.
    pub fn worship_place(&mut self, node: usize, x: f64, cx: &mut Context<Self>) {
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
    pub fn worship_select(&mut self, id: Option<u64>, cx: &mut Context<Self>) {
        self.worship.selected = id;
        self.worship.message = None;
        cx.notify();
    }

    /// Upgrade the selected tower.
    pub fn worship_upgrade(&mut self, cx: &mut Context<Self>) {
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
    pub fn worship_sell(&mut self, cx: &mut Context<Self>) {
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
    pub fn worship_skip(&mut self, cx: &mut Context<Self>) {
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
    pub fn worship_step(&mut self, cx: &mut Context<Self>) {
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
    pub fn worship_replay_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
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
    pub fn worship_replay_seek(&mut self, frame: f64, cx: &mut Context<Self>) {
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
    pub fn worship_replay_set_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        if speed > 0.0 {
            self.worship.replay_speed = speed;
        }
        cx.notify();
    }

    /// Open the replay without playing it.
    pub fn worship_replay_open(&mut self, cx: &mut Context<Self>) {
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
    pub fn worship_replay_close(&mut self, cx: &mut Context<Self>) {
        self.worship.replay_playing = false;
        self.worship.replay_pumping = false;
        self.worship.replay_generation += 1;
        self.worship.replay_frame_at = None;
        self.worship.replay_mode = false;
        cx.notify();
    }
}
