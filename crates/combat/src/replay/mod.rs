//! The combat replay codec, recorder and Killroy handoff (CS-07).
//!
//! Ported from `js/stats/systems/common/combat-sim-replay.js`: each recorded
//! frame holds only what changed since the previous frame, every
//! `keyframe_every` frames holds the complete state, positions are quantized to
//! 0.1 px and changed values are stored as tenth deltas. Frames are packed into
//! transferable chunks and read back through a [`Store`] that decodes a frame
//! from the nearest keyframe at or before it.
//!
//! A replay is a list of JSON snapshots: a number for the simulated time under
//! `t`, plus whatever the recorder captured (the run's monsters, for the
//! encounter replay). Object keys are sorted by `serde_json`, and the upstream
//! encoder visits fields in insertion order, so the oracle script sorts the
//! snapshots it hands to the JS encoder; both sides then visit fields in the
//! same order and the bytes stay comparable.
//!
//! `crates/combat/data/replay.json` and `fixtures/combat-replay.json` are
//! written by `script/combat-replay-oracle.mjs` from one upstream checkout, so
//! the encoder's bytes and the store's decoded frames cannot drift from their
//! source. The same fixture carries the Killroy handoff's own capture and its
//! accept/reject cases.
//!
//! The module is split the way the source is: [`codec`] writes a frame,
//! [`store`] reads one back, and [`killroy`] is the practice's handoff document.

mod codec;
mod killroy;
mod ordered;
mod store;

pub use codec::{Encoder, Keys, pack_frames};
pub use killroy::{KillroyReplay, read_killroy_replay};
pub use store::{Batch, Store};

use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

const REPLAY_JSON: &str = include_str!("../../data/replay.json");

/// Keys whose numbers are quantized to 0.1 and stored as tenth deltas.
const QUANTIZED: [&str; 5] = ["x", "y", "vx", "vy", "respawn"];
/// Values below this tag are tags; the rest are the small integers 0..=239.
const SMALL: u8 = 16;
const SMALL_MAX: f64 = (255 - SMALL) as f64;
const KIND_O: u8 = 0;
const KIND_L: u8 = 1;
const KIND_V: u8 = 2;
/// `Number.MAX_SAFE_INTEGER`.
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
/// The message upstream raises for a payload it will not replay.
pub const INVALID_KILLROY_REPLAY: &str = "Invalid captured Killroy replay";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplayData {
    keyframe_every: usize,
    snapshot_ms: u64,
    killroy_maps: Vec<i64>,
}

fn data() -> &'static ReplayData {
    static DATA: OnceLock<ReplayData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(REPLAY_JSON).expect("replay.json is valid"))
}

/// The codec's default keyframe interval (`createReplayEncoder`).
pub fn keyframe_every() -> usize {
    data().keyframe_every
}

/// The recorder's default frame interval in milliseconds (`SNAPSHOT_MS`).
pub fn snapshot_ms() -> u64 {
    data().snapshot_ms
}

/// The maps a captured Killroy replay may name (`killroyMapIds`).
pub fn killroy_maps() -> &'static [i64] {
    &data().killroy_maps
}

// ---------------------------------------------------------------------------
// Shared value helpers
// ---------------------------------------------------------------------------
/// JavaScript's `Math.round`: halves go towards positive infinity, unlike Rust's
/// `f64::round`.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn number(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// A quantized tenths count as JSON writes it: `100` rather than `100.0`.
fn tenths(value: f64) -> Value {
    if is_small_integer(value) {
        Value::from(value as i64)
    } else {
        number(value)
    }
}

/// A uid value as the string a list entry is keyed by.
pub(super) fn uid_key_value(value: &Value) -> String {
    match value {
        Value::String(uid) => uid.clone(),
        uid => serde_json::to_string(uid).unwrap_or_default(),
    }
}

/// The only fields the codec quantizes, and their 0.1 resolution.
fn quantize(field: &str, value: &Value) -> Value {
    match value {
        Value::Number(entry) if QUANTIZED.contains(&field) => match entry.as_f64() {
            Some(raw) if raw.is_finite() => number(js_round(raw * 10.0) / 10.0),
            _ => value.clone(),
        },
        _ => value.clone(),
    }
}

/// Structural equality with JavaScript's number semantics, so `2` and `2.0` are
/// the same value and a frame that did not change emits no delta.
fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| same(value, other)))
        }
        _ => left == right,
    }
}

/// `Number.isSafeInteger` without a negative zero.
fn is_small_integer(value: f64) -> bool {
    value.is_finite()
        && value.fract() == 0.0
        && value.abs() <= SAFE_INTEGER
        && !(value == 0.0 && value.is_sign_negative())
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// The most frames a recorded run keeps. A longer run records at a coarser
/// interval instead, so a long encounter still scrubs without holding a frame
/// per 100 ms.
pub const MAX_FRAMES: u64 = 10_000;

/// The frame interval a run of `duration_ms` records at: 100 ms, coarsened in
/// 100 ms steps until the run fits [`MAX_FRAMES`].
pub fn interval_for(duration_ms: u64) -> u64 {
    let needed = duration_ms.div_ceil(MAX_FRAMES).max(snapshot_ms());
    needed.div_ceil(snapshot_ms()) * snapshot_ms()
}

/// A recorded run's frames, encoded as the run goes.
///
/// The store decodes a frame off the nearest keyframe, so the recorder feeds it
/// the encoder's own batches: the frames stay compact and a replay cannot
/// disagree with the run it came from.
#[derive(Clone, Debug)]
pub struct Recording {
    interval_ms: u64,
    /// Behind an `Arc` so the UI can clone a finished run cheaply and decode
    /// frames as it draws them; a `Mutex` because the run hands it over a thread.
    store: Arc<Mutex<Store>>,
}

impl PartialEq for Recording {
    fn eq(&self, other: &Self) -> bool {
        self.interval_ms == other.interval_ms && *lock(&self.store) == *lock(&other.store)
    }
}

/// A store lock that ignores a poisoned mutex: the store holds no invariants
/// that a panicking reader can break.
fn lock(store: &Arc<Mutex<Store>>) -> std::sync::MutexGuard<'_, Store> {
    store.lock().unwrap_or_else(|poison| poison.into_inner())
}

impl Recording {
    /// An empty recording at `interval_ms`.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms: interval_ms.max(1),
            store: std::sync::Arc::new(std::sync::Mutex::new(Store::new())),
        }
    }

    /// The simulated milliseconds between frames.
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Frames recorded so far.
    pub fn len(&self) -> usize {
        lock(&self.store).len()
    }

    pub fn is_empty(&self) -> bool {
        lock(&self.store).is_empty()
    }

    /// The encoded size of every frame.
    pub fn bytes(&self) -> usize {
        lock(&self.store).bytes()
    }

    /// A frame's simulated time, without decoding it.
    pub fn time_ms(&self, index: usize) -> Option<f64> {
        lock(&self.store).time_ms(index)
    }

    /// The simulated time of the last frame.
    pub fn last_ms(&self) -> Option<f64> {
        let store = lock(&self.store);
        store.time_ms(store.len().checked_sub(1)?)
    }

    /// Decode a frame into its snapshot shape.
    pub fn frame(&self, index: usize) -> Option<Value> {
        lock(&self.store).get(index)
    }

    /// The first frame at or after `t_ms`, for a wave jump.
    pub fn index_at(&self, t_ms: f64) -> usize {
        let store = lock(&self.store);
        let mut low = 0;
        let mut high = store.len().saturating_sub(1);
        while low < high {
            let middle = (low + high) / 2;
            if store.time_ms(middle).unwrap_or(f64::MAX) < t_ms {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    /// Append one frame, with the dictionary entries the encoder added.
    pub(crate) fn push(&self, encoder: &mut Encoder, snapshot: &Value) {
        let frame = encoder.push(snapshot);
        let keys = encoder.new_keys();
        lock(&self.store).append(Batch {
            keys_from: Some(keys.from),
            keys: keys.keys,
            frames: vec![frame],
        });
    }
}
