//! The seeded generator and the projectile geometry the runtime reads.
//!
//! Ports `seededRandom`, `randomInt`, `backIn`, `_jellyProjectileGeometry`,
//! `_jellyProjectileHitFrame` and `jellyProjectileHitDelay` from
//! `js/stats/systems/w7/jelly-operator.js`. The generator is the upstream's
//! Mulberry32: a 32-bit state wrapped on every draw, so a seed is the same
//! attempt on both sides.

use super::super::data::data;

/// Mulberry32, the upstream's own seeded generator (`seededRandom`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rng {
    state: u32,
}

impl Rng {
    pub(super) fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    pub(super) fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut value = self.state;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        f64::from(value ^ (value >> 14)) / 4_294_967_296.0
    }

    /// The upstream skips one generated value without using it.
    pub(super) fn advance(&mut self, count: u32) {
        self.state = self.state.wrapping_add(0x6D2B_79F5u32.wrapping_mul(count));
    }
}

pub(super) fn random_int(rng: &mut Rng, min: f64, max: f64) -> usize {
    let min = min.ceil();
    let max = max.floor();
    (min + (rng.next() * (max - min + 1.0).max(1.0)).floor()) as usize
}

fn back_in(value: f64) -> f64 {
    if value <= 0.0 {
        0.0
    } else if value >= 1.0 {
        1.0
    } else {
        value * value * (2.70158 * value - 1.70158)
    }
}

/// The projectile's launch geometry, fixed per cell (`_jellyProjectileGeometry`).
#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    anchor_x: f64,
    anchor_y: f64,
    duration: f64,
    offset_x: f64,
    offset_y: f64,
    spread: f64,
    frames: u32,
}

/// `_jellyProjectileGeometry`.
pub(super) fn geometry(anchor: usize, cell_type: usize) -> Geometry {
    let cols = data().cols;
    let projectile = data().projectile(cell_type);
    let anchor_x = 171.0 + 37.0 * (anchor % cols) as f64;
    let anchor_y = 61.0 + 37.0 * (anchor / cols) as f64;
    let duration =
        (1000.0 * (0.6 + (501.0 - anchor_x).hypot(245.0 - anchor_y) / 400.0)).trunc() / 1000.0;
    let frames = ((duration * 60.0).ceil().max(1.0)) as u32;
    Geometry {
        anchor_x,
        anchor_y,
        duration,
        offset_x: projectile.offset_x,
        offset_y: projectile.offset_y,
        spread: projectile.spread,
        frames,
    }
}

/// `_jellyProjectileHitFrame`: how many frames until the shot reaches the boss.
pub(super) fn hit_frame(geometry: Geometry, rng: &mut Rng) -> u32 {
    let roll = rng.next();
    let roll2 = rng.next();
    hit_frame_with(geometry, roll, roll2)
}

/// The same hit frame from two launch rolls, so the surrogate can pass its own
/// constant `() => 0.5` random.
fn hit_frame_with(geometry: Geometry, roll_x: f64, roll_y: f64) -> u32 {
    let start_x =
        (geometry.anchor_x + 18.0 * geometry.offset_x + geometry.spread * (2.0 * roll_x - 1.0))
            .round();
    let start_y =
        (geometry.anchor_y + 18.0 * geometry.offset_y + geometry.spread * (2.0 * roll_y - 1.0))
            .round();
    if (504.0 - start_x).abs() < 25.0 && (243.0 - start_y).abs() < 25.0 {
        return 0;
    }
    let mut first = 1_u32;
    let mut last = geometry.frames;
    while first < last {
        let frame = (first + last) / 2;
        let eased = back_in((f64::from(frame) / 60.0 / geometry.duration).min(1.0));
        let x = start_x + (484.0 - start_x) * eased;
        let y = start_y + (228.0 - start_y) * eased;
        if (504.0 - x).abs() < 25.0 && (243.0 - y).abs() < 25.0 {
            last = frame;
        } else {
            first = frame + 1;
        }
    }
    last
}

/// `jellyProjectileHitDelay`: the travel time with the upstream's fixed 0.5
/// launch roll, which the deterministic surrogate reads.
pub fn projectile_hit_delay(anchor: usize, cell_type: usize) -> f64 {
    f64::from(hit_frame_with(geometry(anchor, cell_type), 0.5, 0.5)) / 60.0
}
