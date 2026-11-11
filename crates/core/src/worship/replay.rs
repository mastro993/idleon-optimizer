//! Recorded frames from a Worship TD run.
//!
//! A port of the upstream `opts.replay` recorder: frames every `interval`
//! seconds of game time, a tower layout per change, and bounded storage that
//! halves the rate once it fills. Scrub and speed controls read these frames,
//! so a replay cannot disagree with the run it came from.

use super::{CloudView, EndReason, KrakenView, MobView, RunView, TowerView};

/// How often a run records replay frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReplayConfig {
    /// Game seconds between frames; upstream defaults to 1.
    pub interval_sec: f64,
    /// Skip frames recorded before this wave.
    pub from_wave: u32,
    /// Frames kept before the recorder halves the rate; at least 16.
    pub max_frames: usize,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            interval_sec: 1.0,
            from_wave: 0,
            max_frames: 4_000,
        }
    }
}

/// One recorded moment of a run, as the map and HUD read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayFrame {
    pub tick: u64,
    pub t: f64,
    pub wave: u32,
    pub countdown: f64,
    pub pts: f64,
    pub totem: f64,
    /// Index into the replay's tower layouts.
    pub set: usize,
    pub mobs: Vec<MobView>,
    pub clouds: Vec<CloudView>,
    pub krakens: Vec<KrakenView>,
}

/// A run's replay timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    interval_sec: f64,
    frames: Vec<ReplayFrame>,
    tower_sets: Vec<Vec<TowerView>>,
    done: bool,
    end_reason: Option<EndReason>,
}

impl Replay {
    pub(crate) fn new(
        interval_sec: f64,
        frames: Vec<ReplayFrame>,
        tower_sets: Vec<Vec<TowerView>>,
        done: bool,
        end_reason: Option<EndReason>,
    ) -> Self {
        Self {
            interval_sec,
            frames,
            tower_sets,
            done,
            end_reason,
        }
    }

    /// Game seconds between frames; doubles when the frame limit is hit.
    pub fn interval_sec(&self) -> f64 {
        self.interval_sec
    }

    pub fn frames(&self) -> &[ReplayFrame] {
        &self.frames
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn frame(&self, index: usize) -> Option<&ReplayFrame> {
        self.frames.get(index)
    }

    /// The tower layout a frame belongs to.
    pub fn tower_set(&self, frame: &ReplayFrame) -> &[TowerView] {
        self.tower_sets
            .get(frame.set)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// A frame as the same view the live simulator renders.
    pub fn view(&self, index: usize) -> Option<RunView> {
        let frame = self.frames.get(index)?;
        let done = self.done && index + 1 == self.frames.len();
        Some(RunView {
            tick: frame.tick,
            t: frame.t,
            wave: frame.wave,
            countdown: frame.countdown,
            pts: frame.pts,
            totem: frame.totem,
            done,
            end_reason: if done { self.end_reason } else { None },
            mobs: frame.mobs.clone(),
            towers: self.tower_set(frame).to_vec(),
            boulders: Vec::new(),
            clouds: frame.clouds.clone(),
            krakens: frame.krakens.clone(),
            effects: Vec::new(),
        })
    }
}
