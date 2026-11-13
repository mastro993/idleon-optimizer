//! The Jelly Operator multipliers and the steady-state board metrics.
//!
//! Ports `jellyCellDamageMultiplier`, `jellyCellSpeedMultiplier`,
//! `jellyCellExpMultiplier`, `jellyFeverBonus`, `jellyAttackCadence`,
//! `jellyCurrencyMultiplier` and `jellyLayoutMetrics` from
//! `js/stats/systems/w7/jelly-operator.js`. The page's `jellyLayoutBreakdown`
//! tree over the same numbers is JO-03.

use super::board::{Layout, footprint};
use super::data::data;
use super::{Jelly, PROXIMITY_ANCHORS};

/// The upstream's own frame progress constant (`0.65` a frame at 1× speed).
pub(crate) const PROGRESS_PER_FRAME: f64 = 0.65;

/// The options a layout metric run reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MetricOptions {
    /// The selected fever, defaulting to the save's own.
    pub fever: Option<u32>,
    /// The COLD ramp percentage; zero outside the deterministic surrogate.
    pub fever_ramp_pct: f64,
    /// The total cell level the damage multiplier reads.
    pub total_cell_level: Option<f64>,
    /// The Best DPS the Bloodcell breakdown prices from; the save's own if unset.
    pub best_dps: Option<f64>,
    /// Whether Stronkroid is active for this metric.
    pub roid_active: bool,
    /// The Amoeba weakening stacks the damage formula reads.
    pub amoeba_stacks: f64,
}

/// `jellyAttackCadence`: the frame breakpoint between two attacks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cadence {
    pub cooldown_progress: f64,
    pub progress_per_frame: f64,
    pub frames_per_attack: f64,
    pub seconds_per_attack: f64,
    pub attacks_per_second: f64,
}

/// One placed cell's steady-state figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellMetrics {
    pub anchor: usize,
    pub cell_type: usize,
    pub name: String,
    pub slots: Vec<usize>,
    /// The Organelle and Proximity multipliers this cell receives.
    pub organelle: f64,
    pub proximity: f64,
    pub damage: f64,
    pub cooldown_progress: f64,
    pub progress_per_frame: f64,
    pub frames_per_attack: f64,
    pub attacks_per_second: f64,
    pub dps: f64,
}

/// `jellyLayoutMetrics`: what the whole board does before the runtime starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub valid: bool,
    pub reason: String,
    /// The cell counts the passives read, after the count bonus.
    pub counts: [f64; 9],
    /// The counts before the count bonus.
    pub raw_counts: [f64; 9],
    pub infected_slots: usize,
    pub damage_passive: f64,
    pub speed_passive: f64,
    pub virus_multiplier: f64,
    pub cells: Vec<CellMetrics>,
    pub dps: f64,
    pub attacks_per_second: f64,
}

/// `jellyAttackCadence`.
pub fn attack_cadence(
    base_cooldown: f64,
    cooldown_progress: Option<f64>,
    progress_per_frame: Option<f64>,
) -> Cadence {
    let cooldown_progress = cooldown_progress.unwrap_or_else(|| base_cooldown.max(0.0));
    let progress_per_frame = progress_per_frame.unwrap_or(PROGRESS_PER_FRAME).max(1e-12);
    let frames_per_attack = (cooldown_progress / progress_per_frame).ceil().max(1.0);
    Cadence {
        cooldown_progress,
        progress_per_frame,
        frames_per_attack,
        seconds_per_attack: frames_per_attack / 60.0,
        attacks_per_second: 60.0 / frames_per_attack,
    }
}

/// `jellyBestDpsMultiplierFromValue`: the Bloodcell gain a Best DPS buys.
///
/// The upstream divides by the literal `2.30259` rather than `ln(10)`, so the
/// port keeps that constant; the parity fixture pins the difference.
#[allow(clippy::approx_constant)]
pub fn best_dps_multiplier_from_value(value: f64) -> f64 {
    let dps = value.max(0.0);
    let log10 = dps.max(1.0).ln() / 2.30259;
    let log2_term = (2.0_f64).min((dps / 100.0).max(1.0).ln() / std::f64::consts::LN_2 / 20.0);
    1.0 + (log2_term + log10 / 50.0 * 15.0 / ((dps / 50.0).max(1.0).ln() / 2.30259 + 20.0))
}

impl Jelly {
    /// `jellyFeverBonus`: the percentage a fever mode adds for one effect.
    ///
    /// Effects are the page's own numbering: 0 COLD's ramp, 1 RASH's damage,
    /// 2 SEPSIS's Bloodcells, 3 NAUSEA's EXP, 4 RABIES' damage (`part` 0) and
    /// speed (`part` 1), 5 PLAGUE's speed.
    pub fn fever_bonus(
        &self,
        effect: usize,
        part: usize,
        ramp_pct: f64,
        selected: Option<u32>,
    ) -> f64 {
        let selected = selected.unwrap_or(self.progress.fever);
        if self.upgrade_quantity(16) <= effect as f64 {
            return 0.0;
        }
        if selected != effect as u32 {
            return 0.0;
        }
        match effect {
            0 => ramp_pct,
            1 | 3 => 100.0,
            2 => 300.0,
            4 => {
                if part == 1 {
                    25.0
                } else {
                    50.0
                }
            }
            5 => 40.0,
            _ => 0.0,
        }
    }

    /// `jellyCellDamageMultiplier`.
    pub fn cell_damage_multiplier(&self, options: MetricOptions) -> f64 {
        let fever = options.fever;
        let additive = self.upgrade_quantity(18)
            + (self.upgrade_quantity(19) + (self.upgrade_quantity(20) + self.account.palette_1));
        let total_cell_level = options.total_cell_level.unwrap_or(self.total_cell_level());
        let fever_factor = self.fever_bonus(0, 0, options.fever_ramp_pct, fever)
            + (self.fever_bonus(1, 0, 0.0, fever) + self.fever_bonus(4, 0, 0.0, fever));
        (1.0 + additive / 100.0)
            * (1.0 + self.account.legend_41 / 100.0)
            * (1.0 + 10.0 * self.account.bloodclot_tier / 100.0)
            * (1.0 + self.upgrade_quantity(21) / 100.0)
            * (1.0 + self.upgrade_quantity(22) / 100.0)
            * (1.0 + self.account.grid_185 / 100.0)
            * (1.0 + self.upgrade_quantity(32) * (total_cell_level / 100.0).floor() / 100.0)
            * (1.0 + fever_factor / 100.0)
    }

    /// `jellyCellSpeedMultiplier`.
    pub fn cell_speed_multiplier(&self, fever: Option<u32>) -> f64 {
        1.0 + (self.fever_bonus(4, 1, 0.0, fever) + self.fever_bonus(5, 0, 0.0, fever)) / 100.0
    }

    /// `jellyCellExpMultiplier`.
    pub fn cell_exp_multiplier(&self, fever: Option<u32>) -> f64 {
        (1.0 + self.fever_bonus(3, 0, 0.0, fever) / 100.0)
            * (1.0
                + (self.upgrade_quantity(30)
                    + (self.upgrade_quantity(31) + self.upgrade_quantity(10)))
                    / 100.0)
            * (1.0 + self.upgrade_quantity(11) / 100.0)
    }

    /// The Bloodcell factors that stay fixed for the whole operation.
    pub(crate) fn currency_factors(
        &self,
        fever: Option<u32>,
        total_cell_level: f64,
    ) -> CurrencyFactors {
        let before_dps = (1.0
            + (self.upgrade_quantity(23)
                + (self.upgrade_quantity(24)
                    + (self.upgrade_quantity(25)
                        + (self.upgrade_quantity(33) * total_cell_level
                            + super::BASE_BLOODCELL_BONUS_PCT))))
                / 100.0)
            * (1.0 + self.account.arcade_72 / 100.0)
            * (1.0 + self.account.grid_187 / 100.0)
            * (1.0 + self.fever_bonus(2, 0, 0.0, fever) / 100.0)
            * (1.0 + if self.account.bundle_ban_j { 1.0 } else { 0.0 })
            * (1.0 + self.completion_bonus(24) / 100.0);
        CurrencyFactors {
            before_dps,
            upgrade_26: 1.0 + self.upgrade_quantity(26) / 100.0,
            upgrade_27: 1.0 + self.upgrade_quantity(27) / 100.0,
            event_56: 1.0 + self.account.event_shop_56,
            atom_15: 1.0 + self.account.atom_15 / 100.0,
        }
    }

    /// `jellyCurrencyMultiplier`: Bloodcells per point of damage.
    pub fn currency_multiplier(
        &self,
        fever: Option<u32>,
        total_cell_level: f64,
        best_dps: f64,
    ) -> f64 {
        self.currency_factors(fever, total_cell_level)
            .multiplier(best_dps)
    }

    /// The board's steady-state metrics, or the reason the layout is invalid.
    pub fn layout_metrics(&self, layout: &Layout, options: MetricOptions) -> Metrics {
        if let Err(reason) = super::board::validate(self, layout) {
            return Metrics {
                valid: false,
                reason,
                ..Metrics::default()
            };
        }
        let data = data();
        let mut raw_counts = [0.0; 9];
        for cell_type in layout.values() {
            raw_counts[*cell_type] += 1.0;
        }
        let mut counts: [f64; 9] = raw_counts;
        if self.upgrade_quantity(14) >= 1.0 {
            for (cell_type, count) in counts.iter_mut().enumerate() {
                if cell_type != 5 {
                    *count += (*count / 3.0).floor();
                }
            }
        }
        let damage_passive = (1.0 + 200.0 * counts[7] / 100.0)
            * (1.0 + (50.0 * counts[2] + 10.0 * counts[0]) / 100.0);
        let mitochondria_speed = 1.0 + 50.0 * counts[6] / 100.0;
        let organelle_plasmid_speed = 1.0 + (25.0 * counts[3] + 15.0 * counts[1]) / 100.0;
        let speed_passive = mitochondria_speed * organelle_plasmid_speed;
        let unit_cooldown_multiplier = 1.0 / mitochondria_speed * (1.0 / organelle_plasmid_speed);

        let mut organelle_reach = vec![false; data.size()];
        for (anchor, cell_type) in layout {
            if *cell_type != 3 {
                continue;
            }
            for slot in organelle_reach_slots(*anchor) {
                if let Some(reach) = organelle_reach.get_mut(slot) {
                    *reach = true;
                }
            }
        }
        let mut virus_reach = vec![false; data.size()];
        for (anchor, cell_type) in layout {
            if *cell_type != 5 {
                continue;
            }
            let mut reach = |slot: i64| {
                if let Ok(slot) = usize::try_from(slot)
                    && let Some(reach) = virus_reach.get_mut(slot)
                {
                    *reach = true;
                }
            };
            reach(*anchor as i64 - data.cols as i64);
            reach(*anchor as i64 + data.cols as i64);
            if anchor % data.cols > 0 {
                reach(*anchor as i64 - 1);
            }
            if anchor % data.cols < data.cols - 1 {
                reach(*anchor as i64 + 1);
            }
        }

        let mut infected = vec![false; data.size()];
        for (anchor, cell_type) in layout {
            let slots = footprint(*anchor, *cell_type).unwrap_or_default();
            if slots.iter().any(|slot| virus_reach[*slot]) {
                for slot in slots {
                    infected[slot] = true;
                }
            }
        }
        let infected_slots = infected.iter().filter(|slot| **slot).count();
        let virus_multiplier = 1.0 + infected_slots as f64 / 10.0;

        let cell_damage = self.cell_damage_multiplier(options);
        let cell_speed = self.cell_speed_multiplier(options.fever);
        let proximity_level = self.upgrade_quantity(13);
        let proximity_multiplier = 1.0 + proximity_level / 100.0;
        let organelle_multiplier = 1.5 + (self.account.rog_63 / 100.0).clamp(0.0, 0.25);
        let roid_level = self.upgrade_quantity(29);
        let roid_multiplier = if options.roid_active && roid_level >= 1.0 {
            1.0 + (50.0 + roid_level) / 100.0
        } else {
            1.0
        };
        let cell_level_damage = 1.0 + self.upgrade_quantity(17);

        let mut cells = Vec::with_capacity(layout.len());
        let mut dps = 0.0;
        let mut attacks_per_second = 0.0;
        for (anchor, cell_type) in layout {
            let slots = footprint(*anchor, *cell_type).unwrap_or_default();
            let organelle = if slots.iter().any(|slot| organelle_reach[*slot]) {
                organelle_multiplier
            } else {
                1.0
            };
            let proximity = if proximity_level >= 1.0 && PROXIMITY_ANCHORS.contains(anchor) {
                proximity_multiplier
            } else {
                1.0
            };
            let level = self.cell_level(*cell_type) as f64;
            let damage = data.cells.base_damage[*cell_type]
                * cell_damage
                * damage_passive
                * (1.0 + level * cell_level_damage / 100.0)
                * (1.0 + options.amoeba_stacks / 100.0)
                * virus_multiplier
                * proximity;
            let cooldown_progress = data.cells.base_cooldowns[*cell_type]
                * (1.0 / cell_speed)
                * unit_cooldown_multiplier;
            let progress_per_frame = PROGRESS_PER_FRAME * roid_multiplier * organelle * proximity;
            let cadence = attack_cadence(
                data.cells.base_cooldowns[*cell_type],
                Some(cooldown_progress),
                Some(progress_per_frame),
            );
            let cell_dps = damage * cadence.attacks_per_second;
            dps += cell_dps;
            attacks_per_second += cadence.attacks_per_second;
            cells.push(CellMetrics {
                anchor: *anchor,
                cell_type: *cell_type,
                name: data.cell_name(*cell_type).to_string(),
                slots,
                organelle,
                proximity,
                damage,
                cooldown_progress: cadence.cooldown_progress,
                progress_per_frame: cadence.progress_per_frame,
                frames_per_attack: cadence.frames_per_attack,
                attacks_per_second: cadence.attacks_per_second,
                dps: cell_dps,
            });
        }
        Metrics {
            valid: true,
            reason: String::new(),
            counts,
            raw_counts,
            infected_slots,
            damage_passive,
            speed_passive,
            virus_multiplier,
            cells,
            dps,
            attacks_per_second,
        }
    }
}

/// The Bloodcell factors that stay fixed for one operation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CurrencyFactors {
    pub before_dps: f64,
    pub upgrade_26: f64,
    pub upgrade_27: f64,
    pub event_56: f64,
    pub atom_15: f64,
}

impl CurrencyFactors {
    pub fn multiplier(self, best_dps: f64) -> f64 {
        self.before_dps
            * best_dps_multiplier_from_value(best_dps)
            * self.upgrade_26
            * self.upgrade_27
            * self.event_56
            * self.atom_15
    }
}

/// `_organelleReachSlots`: the slots an Organelle at `anchor` powers.
pub(crate) fn organelle_reach_slots(anchor: usize) -> Vec<usize> {
    let cols = data().cols;
    let mut candidates = Vec::with_capacity(8);
    for offset in [-36_i64, -19, -17, 17, 19, 36] {
        if let Ok(slot) = usize::try_from(anchor as i64 + offset) {
            candidates.push(slot);
        }
    }
    if anchor % cols > 1 {
        candidates.push(anchor - 2);
    }
    if anchor % cols < cols - 2 {
        candidates.push(anchor + 2);
    }
    candidates
}
