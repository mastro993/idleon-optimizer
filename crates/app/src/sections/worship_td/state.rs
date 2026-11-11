//! The live run's state and the tick pump.
//!
//! The section keeps one [`Run`] per trial and seed. Playback advances it on a
//! 16 ms foreground loop at ten ticks per real second times the chosen speed;
//! every edit (placement, upgrade, sale, skip, restart) refreshes the cached
//! view so the HUD and the map always describe the same tick.

use crate::state::AppState;
use gpui::Context;
use idleon_core::worship::{self, Inputs, Run, RunView};
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
        let run = Run::new(self.worship_inputs.clone(), trial, self.worship.seed);
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
        if let Some(run) = self.worship.run.as_mut() {
            run.step(ticks);
            self.worship.view = Some(run.view());
            if run.done() {
                self.worship.playing = false;
                self.worship.pumping = false;
                self.worship.generation += 1;
            }
        }
        cx.notify();
    }

    /// Play or pause the run.
    pub fn worship_set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
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
}
