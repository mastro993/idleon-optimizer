//! One seeded Jelly operation attempt: its options, its result and its rewards.
//!
//! [`simulate`] is the runtime itself (`runtime`); `rng` owns the upstream
//! Mulberry32 generator and the projectile flight model. The option and result
//! types live here so the estimate, the insights and a future simulator share
//! one vocabulary.

mod rng;
mod runtime;

pub use rng::projectile_hit_delay;
pub use runtime::simulate;

use crate::boss_attack_cooldown;

/// `jellyBossAttackCooldown` as a boss interval in seconds.
pub fn boss_interval_seconds(obstruction: usize) -> f64 {
    boss_attack_cooldown(obstruction) / 60.0
}

/// When Stronkroid is used (`roidActivationTime`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RoidTiming {
    /// At the first frame.
    #[default]
    Immediate,
    /// Five seconds in.
    FiveSeconds,
    /// When the normal phase expires.
    Critical,
    /// Never.
    Never,
    /// At this many seconds.
    At(f64),
}

/// Which dead slot a revive brings back (`reviveTarget`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RevivePolicy {
    #[default]
    ImmunoidFirst,
    AnchorFirst,
    DpsFirst,
    SoakFirst,
    None,
}

/// The knobs one attempt reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OperationOptions {
    /// The obstruction fought, defaulting to the save's.
    pub obstruction: Option<usize>,
    /// The selected fever, defaulting to the save's.
    pub fever: Option<u32>,
    pub seed: u32,
    pub roid_timing: RoidTiming,
    pub revive_policy: RevivePolicy,
    /// The critical phase cap; the upstream default is the three-attack bound.
    pub max_critical_seconds: Option<f64>,
}

/// What one cell type did in one attempt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contribution {
    pub cell_type: usize,
    pub name: String,
    pub shots: f64,
    pub hits: f64,
    pub damage: f64,
    pub bloodcells: f64,
    pub exp: f64,
}

/// One boss attack in the critical phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub time: f64,
    pub slot: usize,
    pub focused: bool,
    pub disabled_anchor: bool,
    pub revived: bool,
}

/// The result of one attempt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trial {
    pub valid: bool,
    pub reason: String,
    pub seed: u32,
    pub obstruction: usize,
    pub fever: u32,
    pub hp: f64,
    pub standard_time: f64,
    pub critical_unlocked: bool,
    pub time: f64,
    pub normal_time: f64,
    pub critical_time: f64,
    pub success: bool,
    /// `clear`, `timer`, `all-slots-dead` or `simulation-cap`.
    pub ended_by: String,
    pub damage: f64,
    pub damage_pct: f64,
    pub shots: f64,
    pub hits: f64,
    pub projectiles_remaining: usize,
    pub bloodcells: f64,
    pub cell_exp: f64,
    pub exp_by_type: [f64; 9],
    pub amoeba_stacks: f64,
    pub amoeba_stacks_at_critical_start: f64,
    pub damage_at_critical_start: f64,
    pub fever_seconds: f64,
    pub displayed_dps: f64,
    pub peak_dps: f64,
    pub best_dps_before: f64,
    pub best_dps_after: f64,
    pub deaths: f64,
    pub boss_attacks: f64,
    pub immunoid_hits: f64,
    pub immunoid_focus_time: f64,
    pub live_slots: f64,
    pub revives_used: f64,
    pub revives_remaining: f64,
    pub roid_used: bool,
    pub roid_start: Option<f64>,
    /// The random opening progress each cell started with.
    pub opening_progress: Vec<(usize, f64)>,
    pub contribution: Vec<Contribution>,
    pub death_log: Vec<Death>,
    pub attempt_delta: f64,
    pub tries_before: f64,
    pub obstruction_after: usize,
}
